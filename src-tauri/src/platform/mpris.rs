//! Linux media integration over MPRIS (org.mpris.MediaPlayer2): the desktop's media widgets, `playerctl` and the
//! hardware media keys that the desktop routes to MPRIS players.
//!
//! Same split as the macOS module: `poke()` wakes a publisher task that reads core progress and turns it into a
//! `Snapshot`; commands from the desktop go to `control::run`, the same path the Tauri player commands use. The
//! `mpris-server` player is not `Send`, so it lives on its own thread with a single-threaded runtime and receives
//! snapshots over a channel. Cover art is passed to the desktop as a URL; this app does no network fetch for it, and
//! only https URLs are forwarded.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId};
use tauri::{AppHandle, Manager};
use tokio::sync::{Notify, mpsc};

use super::control::{self, Remote};
use crate::commands::player::{PlaybackStateDto, PlayerProgressDto, PlayerState, progress};

static CHANGED: Notify = Notify::const_new();

/// A seek or stall shows up as the position departing from the extrapolated value by more than this.
const DRIFT_MS: u64 = 1_500;

pub fn poke() {
    CHANGED.notify_one();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub title: String,
    pub artists: Vec<String>,
    pub artwork_url: Option<String>,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub status: Status,
    pub volume: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
}

impl Snapshot {
    /// Buffering still counts as playing: `Remote::Toggle` pauses it, so the widget must offer Pause.
    fn plays(&self) -> bool {
        self.status == Status::Playing
    }

    /// D-Bus object path that identifies this track to clients; stable while the track stays the same.
    fn track_path(&self) -> String {
        let mut hasher = DefaultHasher::new();
        (&self.title, &self.artists, self.duration_ms).hash(&mut hasher);
        format!("/io/lunorx/desktop/track/{:016x}", hasher.finish())
    }
}

/// Pure: core progress to an MPRIS snapshot. `None` means nothing is loaded.
pub fn map(dto: &PlayerProgressDto) -> Option<Snapshot> {
    let track = dto.track.as_ref()?;
    from_parts(
        dto.player.state,
        &track.title,
        &track.artists,
        track.artwork_url.as_deref(),
        dto.position_ms,
        dto.duration_ms,
        dto.volume,
    )
}

fn from_parts(
    state: PlaybackStateDto,
    title: &str,
    artists: &[String],
    artwork_url: Option<&str>,
    position_ms: u64,
    duration_ms: u64,
    volume: u8,
) -> Option<Snapshot> {
    use PlaybackStateDto as S;
    let status = match state {
        S::Playing | S::Buffering => Status::Playing,
        S::Resolving | S::Preparing | S::Ready | S::Paused => Status::Paused,
        S::Idle | S::Blocked | S::Ended | S::Error => return None,
    };
    let position_ms = if duration_ms > 0 {
        position_ms.min(duration_ms)
    } else {
        position_ms
    };
    Some(Snapshot {
        title: title.to_owned(),
        artists: artists.to_vec(),
        artwork_url: artwork_url
            .filter(|url| url.starts_with("https://"))
            .map(str::to_owned),
        duration_ms,
        position_ms,
        status,
        volume,
    })
}

fn metadata(snapshot: &Snapshot) -> Metadata {
    let mut builder = Metadata::builder().title(snapshot.title.clone());
    if let Ok(id) = TrackId::try_from(snapshot.track_path()) {
        builder = builder.trackid(id);
    }
    if !snapshot.artists.is_empty() {
        builder = builder.artist(snapshot.artists.clone());
    }
    if let Some(url) = &snapshot.artwork_url {
        builder = builder.art_url(url.clone());
    }
    if snapshot.duration_ms > 0 {
        builder = builder.length(Time::from_millis(snapshot.duration_ms as i64));
    }
    builder.build()
}

/// Starts the MPRIS service. Failure (no session bus, name taken) is logged once and never retried.
pub fn install(app: &AppHandle) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let publisher = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            CHANGED.notified().await;
            let dto = progress(&publisher.state::<PlayerState>()).await;
            if sender.send(map(&dto)).is_err() {
                return;
            }
        }
    });
    let commands = app.clone();
    let started = std::thread::Builder::new()
        .name("lunor-mpris".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => return eprintln!("mpris runtime unavailable: {error}"),
            };
            let local = tokio::task::LocalSet::new();
            runtime.block_on(local.run_until(serve(commands, receiver)));
        });
    if let Err(error) = started {
        eprintln!("mpris thread not started: {error}");
    }
    // Publish the initial empty state.
    poke();
}

fn connect(player: &Player, app: &AppHandle) {
    let send = |app: &AppHandle, remote: Remote| {
        tauri::async_runtime::spawn(control::run(app.clone(), remote));
    };
    let a = app.clone();
    player.connect_play(move |_| send(&a, Remote::Play));
    let a = app.clone();
    player.connect_pause(move |_| send(&a, Remote::Pause));
    let a = app.clone();
    player.connect_play_pause(move |_| send(&a, Remote::Toggle));
    let a = app.clone();
    player.connect_stop(move |_| send(&a, Remote::Stop));
    let a = app.clone();
    player.connect_next(move |_| send(&a, Remote::Next));
    let a = app.clone();
    player.connect_previous(move |_| send(&a, Remote::Previous));
    let a = app.clone();
    player.connect_seek(move |_, offset| send(&a, Remote::SeekBy(offset.as_millis())));
    let a = app.clone();
    player.connect_set_position(move |_, _, position| {
        send(&a, Remote::SeekTo(position.as_millis().max(0)));
    });
}

async fn serve(app: AppHandle, mut receiver: mpsc::UnboundedReceiver<Option<Snapshot>>) {
    let player = Player::builder("io.lunorx.desktop")
        .identity("LunoRX")
        .desktop_entry("LunoRX")
        .can_quit(false)
        .can_raise(false)
        .can_set_fullscreen(false)
        .can_control(true)
        .build()
        .await;
    let player = match player {
        Ok(player) => Rc::new(player),
        Err(error) => return eprintln!("mpris unavailable: {error}"),
    };
    connect(&player, &app);
    tokio::task::spawn_local(player.run());

    let mut last: Option<Snapshot> = None;
    let mut last_at = Instant::now();
    while let Some(next) = receiver.recv().await {
        let now = Instant::now();
        if let Err(error) = publish(&player, last.as_ref(), last_at, next.as_ref(), now).await {
            eprintln!("mpris update failed: {error}");
        }
        last = next;
        last_at = now;
    }
}

async fn publish(
    player: &Player,
    old: Option<&Snapshot>,
    old_at: Instant,
    new: Option<&Snapshot>,
    now: Instant,
) -> mpris_server::zbus::Result<()> {
    let Some(new) = new else {
        if old.is_some() {
            player.set_metadata(Metadata::new()).await?;
            player.set_playback_status(PlaybackStatus::Stopped).await?;
            player.set_can_play(false).await?;
            player.set_can_pause(false).await?;
            player.set_can_seek(false).await?;
            player.set_can_go_next(false).await?;
            player.set_can_go_previous(false).await?;
            player.set_position(Time::ZERO);
        }
        return Ok(());
    };
    let changed_track = old.is_none_or(|o| {
        (&o.title, &o.artists, &o.artwork_url, o.duration_ms)
            != (&new.title, &new.artists, &new.artwork_url, new.duration_ms)
    });
    if changed_track {
        player.set_metadata(metadata(new)).await?;
    }
    if old.is_none() {
        player.set_can_play(true).await?;
        player.set_can_pause(true).await?;
        player.set_can_go_next(true).await?;
        player.set_can_go_previous(true).await?;
    }
    player.set_can_seek(new.duration_ms > 0).await?;
    if old.is_none_or(|o| o.status != new.status) {
        let status = if new.plays() {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        };
        player.set_playback_status(status).await?;
    }
    if old.is_none_or(|o| o.volume != new.volume) {
        player.set_volume(f64::from(new.volume) / 100.0).await?;
    }
    player.set_position(Time::from_millis(new.position_ms as i64));
    if let Some(old) = old
        && !changed_track
        && drifted(old, old_at, new, now)
    {
        player
            .seeked(Time::from_millis(new.position_ms as i64))
            .await?;
    }
    Ok(())
}

/// True when the position moved by more than normal playback explains, i.e. the user seeked.
fn drifted(old: &Snapshot, old_at: Instant, new: &Snapshot, now: Instant) -> bool {
    let elapsed = if old.plays() {
        now.saturating_duration_since(old_at)
    } else {
        Duration::ZERO
    };
    let expected = old.position_ms.saturating_add(elapsed.as_millis() as u64);
    expected.abs_diff(new.position_ms) > DRIFT_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlaybackStateDto as S;

    fn snap(state: S, pos: u64) -> Option<Snapshot> {
        from_parts(
            state,
            "Song",
            &["A".to_owned(), "B".to_owned()],
            Some("https://img.example/c.jpg"),
            pos,
            200_000,
            80,
        )
    }

    #[test]
    fn nothing_loaded_or_failed_clears() {
        for state in [S::Idle, S::Blocked, S::Ended, S::Error] {
            assert_eq!(snap(state, 0), None);
        }
    }

    #[test]
    fn playing_and_buffering_offer_pause() {
        assert!(snap(S::Playing, 0).unwrap().plays());
        assert!(snap(S::Buffering, 0).unwrap().plays());
        for state in [S::Paused, S::Ready, S::Resolving, S::Preparing] {
            assert!(!snap(state, 0).unwrap().plays());
        }
    }

    #[test]
    fn position_is_clamped_to_duration() {
        assert_eq!(snap(S::Playing, 900_000).unwrap().position_ms, 200_000);
    }

    #[test]
    fn only_https_artwork_is_forwarded() {
        let http = from_parts(S::Playing, "t", &[], Some("http://x/c.jpg"), 0, 1, 0).unwrap();
        assert_eq!(http.artwork_url, None);
        let file = from_parts(S::Playing, "t", &[], Some("file:///etc/passwd"), 0, 1, 0).unwrap();
        assert_eq!(file.artwork_url, None);
        assert!(snap(S::Playing, 0).unwrap().artwork_url.is_some());
    }

    #[test]
    fn track_path_is_valid_and_stable() {
        let a = snap(S::Playing, 0).unwrap();
        let b = snap(S::Paused, 5_000).unwrap();
        assert_eq!(a.track_path(), b.track_path());
        assert!(TrackId::try_from(a.track_path()).is_ok());
    }

    #[test]
    fn steady_playback_is_not_a_seek_but_a_jump_is() {
        let old = snap(S::Playing, 10_000).unwrap();
        let at = Instant::now();
        let later = at + Duration::from_secs(2);
        assert!(!drifted(
            &old,
            at,
            &snap(S::Playing, 12_000).unwrap(),
            later
        ));
        assert!(drifted(&old, at, &snap(S::Playing, 60_000).unwrap(), later));
        // Paused time does not advance, so a small forward move is already a seek.
        let paused = snap(S::Paused, 10_000).unwrap();
        assert!(drifted(
            &paused,
            at,
            &snap(S::Paused, 14_000).unwrap(),
            later
        ));
    }
}

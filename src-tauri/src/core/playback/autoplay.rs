//! One bounded radio session. The host's confirmed EOF is the only advance trigger.
use super::{
    audio::AudioControl,
    cache::{DiskCache, OfflineTrack},
    command::PlaybackCommand,
    engine::PlaybackEngine,
    extract::valid_id,
    state::PlaybackState,
};
use crate::core::domain::{ProviderId, TrackReference};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::{Mutex, watch};

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoplayStatus {
    pub state: &'static str,
    pub next_title: Option<String>,
    pub queue: Vec<OfflineTrack>,
    #[serde(skip)]
    history: VecDeque<String>,
    /// Tracks already left, newest last; backs the Previous command.
    #[serde(skip)]
    back: Vec<TrackReference>,
}
impl AutoplayStatus {
    /// Like `Default`, but keeps the Previous stack across a manual load.
    pub fn reset(&mut self) {
        *self = Self {
            back: std::mem::take(&mut self.back),
            ..Default::default()
        };
    }
}
pub fn push_back(state: &Status, track: TrackReference) {
    if let Ok(mut s) = state.lock() {
        if s.back
            .last()
            .is_some_and(|t| t.provider_track_id == track.provider_track_id)
        {
            return;
        }
        s.back.push(track);
        if s.back.len() > 50 {
            s.back.remove(0);
        }
    }
}
pub fn pop_back(state: &Status) -> Option<TrackReference> {
    state.lock().ok()?.back.pop()
}
/// Head of the up-next list, consumed like a manual queue selection.
pub fn select_first(state: &Status) -> Option<TrackReference> {
    let id = state.lock().ok()?.queue.first()?.id.clone();
    select(state, &id)
}
pub type Status = Arc<std::sync::Mutex<AutoplayStatus>>;
fn status(target: &Status, state: &'static str, title: Option<String>) {
    if let Ok(mut value) = target.lock() {
        value.state = state;
        value.next_title = title;
    }
}

fn remember(history: &mut VecDeque<String>, id: String) {
    history.push_back(id);
    if history.len() > 100 {
        history.pop_front();
    }
}
fn eligible(id: &str, history: &VecDeque<String>) -> bool {
    valid_id(id) && !history.iter().any(|old| old == id)
}
async fn session(
    engine: Arc<Mutex<PlaybackEngine>>,
    pipe: rustypipe::client::RustyPipe,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    mut reports: watch::Receiver<u64>,
    state: Status,
) {
    let mut history = state.lock().map(|s| s.history.clone()).unwrap_or_default();
    loop {
        let current = engine.lock().await.active_track().cloned();
        let Some(current) = current else { return };
        let seed = current.provider_track_id.clone();
        remember(&mut history, seed.clone());
        if let Ok(mut status) = state.lock() {
            status.history = history.clone();
        }
        status(&state, "finding", None);
        let mut queue = state.lock().map(|s| s.queue.clone()).unwrap_or_default();
        if queue.len() < 10 {
            queue = related(&pipe, &seed, &history, queue).await;
        }
        publish_queue(&state, &queue);
        let mut next = None;
        for _ in 0..3 {
            let Some(track) = queue.first().cloned() else {
                break;
            };
            status(&state, "preparing", Some(track.title.clone()));
            // Keep the file alive until Load acquires it, including when cache quota is tight.
            if let Ok(Ok(file)) = tokio::time::timeout(
                Duration::from_secs(90),
                cache.ensure(&pipe, track.clone(), audio.settings().quality),
            )
            .await
            {
                next = Some((track, file));
                break;
            }
            queue.remove(0);
            publish_queue(&state, &queue);
        }
        let Some((next, _file)) = next else { return };
        status(&state, "ready", Some(next.title.clone()));
        // Best effort, after readiness is published; never awaited.
        super::track_info::prefetch_lyrics(&next.id, &next.title, &next.artists, next.duration_ms);
        loop {
            reports.borrow_and_update();
            let mut player = engine.lock().await;
            match advance(&mut player, &seed, &next).await {
                Ok(true) => {
                    push_back(&state, current.clone());
                    queue.remove(0);
                    publish_queue(&state, &queue);
                    break;
                }
                Ok(false) => {}
                Err(()) => return,
            }
            drop(player);
            if reports.changed().await.is_err() {
                return;
            }
        }
    }
}
const QUEUE_SIZE: usize = 50;
fn publish_queue(state: &Status, queue: &[OfflineTrack]) {
    if let Ok(mut status) = state.lock() {
        status.queue = queue.to_vec();
    }
}
fn append(
    queue: &mut Vec<OfflineTrack>,
    tracks: impl IntoIterator<Item = OfflineTrack>,
    history: &VecDeque<String>,
) {
    for track in tracks {
        if queue.len() >= QUEUE_SIZE {
            break;
        }
        if eligible(&track.id, history)
            && !track.title.trim().is_empty()
            && track.title.len() <= 512
            && track.artists.len() <= 30
            && track.artists.iter().all(|a| a.len() <= 512)
            && !queue.iter().any(|old| old.id == track.id)
        {
            queue.push(track);
        }
    }
}
/// Bounded radio pagination; returns partial results when the source is exhausted or unavailable.
pub async fn related(
    pipe: &rustypipe::client::RustyPipe,
    seed: &str,
    history: &VecDeque<String>,
    mut queue: Vec<OfflineTrack>,
) -> Vec<OfflineTrack> {
    queue.truncate(QUEUE_SIZE);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let client = pipe.query();
    let Ok(Ok(mut page)) =
        tokio::time::timeout(Duration::from_secs(14), client.music_radio_track(seed)).await
    else {
        return queue;
    };
    let mut seeds = std::collections::HashSet::from([seed.to_owned()]);
    for page_index in 0..5 {
        let items = std::mem::take(&mut page.items)
            .into_iter()
            .filter(|t| t.track_type != rustypipe::model::TrackType::Episode)
            .map(|t| OfflineTrack {
                provider: "YouTube".into(),
                id: t.id,
                title: t.name,
                artists: t.artists.into_iter().map(|a| a.name).collect(),
                artwork_url: t
                    .cover
                    .into_iter()
                    .max_by_key(|c| c.width)
                    .and_then(|c| super::track_info::artwork_url(&c.url)),
                duration_ms: t.duration.map(|d| i64::from(d) * 1000),
            });
        append(&mut queue, items, history);
        if queue.len() >= QUEUE_SIZE || page_index == 4 {
            break;
        }
        let remaining = deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .min(Duration::from_secs(10));
        if remaining.is_zero() {
            break;
        }
        if page.ctoken.is_some() {
            match tokio::time::timeout(remaining, page.next(&client)).await {
                Ok(Ok(Some(next))) => page = next,
                _ => break,
            }
        } else {
            let Some(seed) = queue
                .iter()
                .rev()
                .find(|t| !seeds.contains(&t.id))
                .map(|t| t.id.clone())
            else {
                break;
            };
            seeds.insert(seed.clone());
            match tokio::time::timeout(remaining, client.music_radio_track(&seed)).await {
                Ok(Ok(next)) => page = next,
                _ => break,
            }
        }
    }
    queue
}
fn reference(next: &OfflineTrack) -> Result<TrackReference, ()> {
    let mut track = TrackReference::new(
        ProviderId::YouTube,
        next.id.clone(),
        format!("https://www.youtube.com/watch?v={}", next.id),
        next.title.clone(),
    )
    .map_err(|_| ())?;
    track.artists = next.artists.clone();
    track.artwork_url = next.artwork_url.clone();
    Ok(track)
}
pub fn select(state: &Status, id: &str) -> Option<TrackReference> {
    let mut state = state.lock().ok()?;
    let index = state.queue.iter().position(|t| t.id == id)?;
    let track = reference(&state.queue[index]).ok()?;
    state.queue.drain(..=index);
    state.next_title = None;
    state.state = "finding";
    Some(track)
}
async fn advance(player: &mut PlaybackEngine, seed: &str, next: &OfflineTrack) -> Result<bool, ()> {
    if player
        .active_track()
        .is_none_or(|t| t.provider_track_id != seed)
        || matches!(
            player.state(),
            PlaybackState::Idle | PlaybackState::Error | PlaybackState::Blocked
        )
    {
        return Err(());
    }
    if player.state() != PlaybackState::Ended {
        return Ok(false);
    }
    let track = reference(next)?;
    player
        .dispatch(PlaybackCommand::Load(track))
        .await
        .map_err(|_| ())?;
    player
        .dispatch(PlaybackCommand::Play)
        .await
        .map_err(|_| ())?;
    Ok(true)
}
pub async fn run(
    engine: Arc<Mutex<PlaybackEngine>>,
    pipe: rustypipe::client::RustyPipe,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    reports: watch::Receiver<u64>,
    state: Status,
) {
    session(engine, pipe, cache, audio, reports, state.clone()).await;
    status(&state, "unavailable", None);
}
#[cfg(test)]
mod tests {
    use super::super::host::{HostFuture, HostOutcome, HostRequest, PlayerEvent, PlayerHost};
    use super::*;
    use crate::core::{
        capabilities::{CapabilityStatus, ProviderCapabilities},
        registry::ProviderRegistry,
    };
    struct Host;
    impl PlayerHost for Host {
        fn provider(&self) -> ProviderId {
            ProviderId::YouTube
        }
        fn prepare<'a>(&'a self, r: HostRequest) -> HostFuture<'a> {
            Box::pin(async move {
                HostOutcome::Confirmed {
                    request_id: r.request_id,
                    revision: r.revision,
                }
            })
        }
    }
    fn engine() -> PlaybackEngine {
        let mut registry = ProviderRegistry::new();
        registry
            .register(
                ProviderId::YouTube,
                ProviderCapabilities {
                    playback: CapabilityStatus::Supported,
                    ..ProviderCapabilities::unverified()
                },
            )
            .unwrap();
        PlaybackEngine::new(
            registry,
            std::collections::HashMap::from([(
                ProviderId::YouTube,
                Box::new(Host) as Box<dyn PlayerHost>,
            )]),
            Duration::from_secs(1),
        )
    }
    fn next() -> OfflineTrack {
        OfflineTrack {
            provider: "YouTube".into(),
            id: "aqz-KE-bpKQ".into(),
            title: "Next".into(),
            artists: vec![],
            artwork_url: None,
            duration_ms: None,
        }
    }
    async fn playing() -> PlaybackEngine {
        let mut player = engine();
        let track = TrackReference::new(
            ProviderId::YouTube,
            "r-G2pKr2_n8".into(),
            "https://www.youtube.com/watch?v=r-G2pKr2_n8".into(),
            "Seed".into(),
        )
        .unwrap();
        player.dispatch(PlaybackCommand::Load(track)).await.unwrap();
        player.dispatch(PlaybackCommand::Play).await.unwrap();
        player
    }
    #[tokio::test]
    async fn advances_only_after_confirmed_eof_and_once() {
        let mut player = playing().await;
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Ok(false)
        );
        player.dispatch(PlaybackCommand::Pause).await.unwrap();
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Ok(false)
        );
        player.dispatch(PlaybackCommand::Resume).await.unwrap();
        player
            .observe(player.revision(), PlayerEvent::Ended)
            .unwrap();
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Ok(true));
        assert_eq!(player.state(), PlaybackState::Playing);
        assert_eq!(
            player.active_track().unwrap().provider_track_id,
            "aqz-KE-bpKQ"
        );
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Err(()));
    }
    #[tokio::test]
    async fn stopped_and_stale_sessions_cannot_restart_playback() {
        let mut player = playing().await;
        assert_eq!(advance(&mut player, "other-seed", &next()).await, Err(()));
        player.dispatch(PlaybackCommand::Stop).await.unwrap();
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Err(()));
    }
    #[test]
    fn queue_is_unique_bounded_and_excludes_history() {
        let history = VecDeque::from(["00000000000".into()]);
        let tracks = (0..100).flat_map(|i| {
            let mut t = next();
            t.id = format!("{i:011}");
            [t.clone(), t]
        });
        let mut queue = vec![];
        append(&mut queue, tracks, &history);
        assert_eq!(queue.len(), 50);
        assert_eq!(queue[0].id, "00000000001");
        assert_eq!(
            queue
                .iter()
                .map(|t| &t.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            50
        );
    }
    #[test]
    fn selecting_a_queue_song_keeps_the_following_order() {
        let state = Status::default();
        let mut queue = vec![];
        for i in 0..5 {
            let mut t = next();
            t.id = format!("{i:011}");
            queue.push(t);
        }
        publish_queue(&state, &queue);
        status(&state, "ready", Some("Next".into()));
        assert_eq!(state.lock().unwrap().queue.len(), 5);
        assert!(select(&state, "not-in-queue").is_none());
        assert_eq!(state.lock().unwrap().queue.len(), 5);
        let selected = select(&state, "00000000002").unwrap();
        assert_eq!(selected.provider_track_id, "00000000002");
        let queue = &state.lock().unwrap().queue;
        assert_eq!(
            queue.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["00000000003", "00000000004"]
        );
    }
    #[test]
    fn excludes_recent_tracks_and_bounds_history() {
        let mut history = VecDeque::new();
        remember(&mut history, "r-G2pKr2_n8".into());
        assert!(!eligible("r-G2pKr2_n8", &history));
        assert!(!eligible("invalid", &history));
        assert!(eligible("aqz-KE-bpKQ", &history));
        for i in 0..120 {
            remember(&mut history, format!("{i:011}"));
        }
        assert_eq!(history.len(), 100);
        assert!(eligible("r-G2pKr2_n8", &history));
    }
}

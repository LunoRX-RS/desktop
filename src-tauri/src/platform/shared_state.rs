//! Now Playing snapshot for the macOS Widget and Screen Saver. Those run in their own processes and cannot use Tauri IPC, so
//! the app writes one small JSON file (and the cover as `artwork.img`) that they read: title, artist, position at a wall-clock
//! time (readers extrapolate while `playing`), and the timed lyric lines. It is written when something changes (track, state,
//! seek, cover, lyrics), never per tick. No network, no secrets: only what the Now Playing widget of the OS already shows, plus lyrics.
//!
//! Up to three places are written, because a sandboxed reader may see only one: the team-prefixed App Group container (the
//! Widget), `Application Support/io.lunorx.desktop/shared` (readers outside a sandbox) and the Screen Saver host's own container.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::macos_media::NowPlaying;
use crate::core::playback::track_info::TrackInfo;

/// Team-prefixed App Group id. The Widget, the Screen Saver and the app must be signed with the same value; set
/// `LUNORX_APP_GROUP` at build time when signing with another team.
const GROUP: &str = match option_env!("LUNORX_APP_GROUP") {
    Some(group) => group,
    None => "VH66R8S4AU.io.lunorx.shared",
};
const FILE: &str = "now-playing.json";
const COVER: &str = "artwork.img";
const MAX_LINES: usize = 600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub start_ms: u32,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot<'a> {
    version: u8,
    title: &'a str,
    artist: &'a str,
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    /// Wall clock (Unix ms) at which `positionMs` was true.
    updated_at_ms: u64,
    /// Name of the cover file next to this one, when a cover is in hand.
    artwork: Option<&'static str>,
    lyrics: &'a [Line],
}

/// Timed lyrics of the last few tracks, keyed by provider track id. A late result for a track that is no longer playing
/// only fills its own entry, so it cannot replace the lyrics of the track now playing.
static LYRICS: Mutex<Vec<(String, Vec<Line>)>> = Mutex::new(Vec::new());
const LYRICS_KEPT: usize = 8;
/// Bumps whenever `LYRICS` changes, so the publisher writes again even when Now Playing itself did not change.
static LYRICS_VERSION: AtomicU64 = AtomicU64::new(0);

pub fn lyrics_version() -> u64 {
    LYRICS_VERSION.load(Ordering::Relaxed)
}

/// Remembers the timed lyrics of a freshly loaded track. Plain (untimed) lyrics are not shared.
pub fn note_lyrics(info: &TrackInfo) {
    let lines: Vec<Line> = match (&info.lyrics_lines, info.lyrics_synced) {
        (Some(lines), true) => lines
            .iter()
            .take(MAX_LINES)
            .map(|l| Line {
                start_ms: l.start_ms,
                text: l.text.clone(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut kept = LYRICS.lock().unwrap_or_else(|e| e.into_inner());
    let id = info.provider_track_id.clone();
    if kept.iter().any(|(k, v)| *k == id && *v == lines) {
        return;
    }
    kept.retain(|(k, _)| *k != id);
    kept.push((id, lines));
    if kept.len() > LYRICS_KEPT {
        kept.remove(0);
    }
    LYRICS_VERSION.fetch_add(1, Ordering::Relaxed);
}

fn lyrics_for(track_id: &str) -> Vec<Line> {
    LYRICS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(k, _)| k == track_id)
        .map(|(_, lines)| lines.clone())
        .unwrap_or_default()
}

/// Serialises the writes and closes them after the final idle snapshot. A publish still running when the app quits
/// cannot then write a playing snapshot over the idle one.
struct Gate(Mutex<bool>);

impl Gate {
    const fn new() -> Self {
        Self(Mutex::new(false))
    }

    /// Runs `write` unless the gate is closed; returns None when it is.
    fn with<T>(&self, write: impl FnOnce() -> T) -> Option<T> {
        let closed = self.0.lock().unwrap_or_else(|e| e.into_inner());
        (!*closed).then(write)
    }

    /// Runs the last write and closes the gate, so nothing runs after it.
    fn close<T>(&self, write: impl FnOnce() -> T) -> T {
        let mut closed = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let out = write();
        *closed = true;
        out
    }
}

static GATE: Gate = Gate::new();

/// Writes the idle snapshot and closes the gate. Called on app exit so readers stop showing a track that is no longer
/// playing, and nothing written after it can bring that track back.
pub fn clear() {
    GATE.close(|| write_snapshot(&None, None));
}

/// Pure: the JSON the readers get. `None` is the idle snapshot (nothing loaded).
pub fn snapshot_json(
    info: &Option<NowPlaying>,
    has_cover: bool,
    lyrics: &[Line],
    now_ms: u64,
) -> String {
    let snapshot = match info {
        Some(n) => Snapshot {
            version: 1,
            title: &n.title,
            artist: &n.artist,
            playing: n.playing,
            position_ms: n.elapsed_ms,
            duration_ms: n.duration_ms,
            updated_at_ms: now_ms,
            artwork: has_cover.then_some(COVER),
            lyrics,
        },
        None => Snapshot {
            version: 1,
            title: "",
            artist: "",
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            updated_at_ms: now_ms,
            artwork: None,
            lyrics: &[],
        },
    };
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_owned())
}

fn targets() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut dirs = vec![
        home.join("Library/Group Containers").join(GROUP),
        home.join("Library/Application Support/io.lunorx.desktop/shared"),
    ];
    // The screen saver host (legacyScreenSaver) is sandboxed and sees this container as its own `~/Library`. The folder is
    // only filled once macOS has made that container, so a missing one is skipped rather than created.
    let host =
        home.join("Library/Containers/com.apple.ScreenSaver.Engine.legacyScreenSaver/Data/Library");
    if host.is_dir() {
        dirs.push(host.join("Application Support/LunoRX"));
    }
    dirs
}

/// Writes through a temp file and a rename, so a reader never sees half a file.
/// Each write has its own temporary name, so two publishes at once cannot share one file, and a failed write leaves no
/// temporary file behind.
fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let tmp = dir.join(format!(".{name}.{:032x}.tmp", rand::random::<u128>()));
    let result = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, dir.join(name)));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_to(dir: &Path, json: &str, cover: Option<&[u8]>) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    if let Some(bytes) = cover {
        write_atomic(dir, COVER, bytes)?;
    }
    write_atomic(dir, FILE, json.as_bytes())
}

/// Publishes the snapshot to every target. A failing target is logged once per call and never stops the app.
pub fn publish(info: &Option<NowPlaying>, cover: Option<&[u8]>) {
    let _ = GATE.with(|| write_snapshot(info, cover));
}

fn write_snapshot(info: &Option<NowPlaying>, cover: Option<&[u8]>) {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let lyrics = info
        .as_ref()
        .map(|n| lyrics_for(&n.track_id))
        .unwrap_or_default();
    let json = snapshot_json(info, cover.is_some(), &lyrics, now_ms);
    for dir in targets() {
        if let Err(error) = write_to(&dir, &json, cover) {
            eprintln!(
                "shared now playing not written to {}: {error}",
                dir.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_closed_gate_runs_nothing_after_the_final_write() {
        let gate = Gate::new();
        let mut writes = Vec::new();
        assert_eq!(gate.with(|| writes.push("playing")), Some(()));
        gate.close(|| writes.push("idle"));
        assert_eq!(gate.with(|| writes.push("late")), None);
        assert_eq!(writes, ["playing", "idle"]);
    }

    use super::*;

    fn playing() -> Option<NowPlaying> {
        Some(NowPlaying {
            track_id: "dQw4w9WgXcQ".into(),
            title: "Midnight Drive".into(),
            artist: "Artist 1".into(),
            artwork_url: None,
            artwork_ready: true,
            duration_ms: 240_000,
            elapsed_ms: 20_000,
            playing: true,
        })
    }

    #[test]
    fn snapshot_has_position_clock_and_cover_name() {
        let lines = [Line {
            start_ms: 4000,
            text: "Neon rivers".into(),
        }];
        let json: serde_json::Value =
            serde_json::from_str(&snapshot_json(&playing(), true, &lines, 1_700_000_000_000))
                .unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["title"], "Midnight Drive");
        assert_eq!(json["playing"], true);
        assert_eq!(json["positionMs"], 20_000);
        assert_eq!(json["updatedAtMs"], 1_700_000_000_000u64);
        assert_eq!(json["artwork"], "artwork.img");
        assert_eq!(json["lyrics"][0]["startMs"], 4000);
        assert_eq!(json["lyrics"][0]["text"], "Neon rivers");
    }

    #[test]
    fn nothing_loaded_is_an_idle_snapshot_without_cover_or_lyrics() {
        let lines = [Line {
            start_ms: 1,
            text: "x".into(),
        }];
        let json: serde_json::Value =
            serde_json::from_str(&snapshot_json(&None, true, &lines, 5)).unwrap();
        assert_eq!(json["title"], "");
        assert_eq!(json["playing"], false);
        assert!(json["artwork"].is_null());
        assert_eq!(json["lyrics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn write_replaces_the_file_whole_and_keeps_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("lunorx-shared-test-{}", std::process::id()));
        write_to(&dir, "{\"a\":1}", Some(&[1, 2, 3])).unwrap();
        write_to(&dir, "{\"a\":2}", None).unwrap();
        assert_eq!(fs::read_to_string(dir.join(FILE)).unwrap(), "{\"a\":2}");
        assert_eq!(fs::read(dir.join(COVER)).unwrap(), vec![1, 2, 3]);
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "unexpected files: {names:?}");
        fs::remove_dir_all(&dir).unwrap();
    }
}

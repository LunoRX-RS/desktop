//! Keeps the device in step with the signed-in YouTube Music account: liked songs, saved playlists, recent history.
//! Read-only toward YouTube (there is no write API behind it). A full pass runs after sign-in, on launch at most every
//! six hours, and on demand. While connected, a quick pass every two minutes picks up new likes, un-likes, new
//! playlists and playlists whose size changed.
use crate::{
    commands::{
        library::{HistoryState, LibraryState},
        sync::SyncEngineState,
        youtube::YouTubeState,
    },
    core::{
        library::{LibraryError, SavedTrack},
        youtube_import::{Import, fetch},
    },
    sync::{api::Provider, local_store::LocalEntry},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

const EVENT: &str = "core://youtube/import";
const AUTO_EVERY_SECS: u64 = 6 * 3600;
const QUICK_EVERY: Duration = Duration::from_secs(120);
const MEMORY_FILE: &str = "youtube-import.json";
static RUNNING: AtomicBool = AtomicBool::new(false);
static LOOPING: AtomicBool = AtomicBool::new(false);
static COUNTS: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub liked_added: usize,
    pub liked_removed: usize,
    pub history_added: usize,
    pub playlists_created: usize,
    pub playlists_updated: usize,
    pub playlist_tracks_added: usize,
    /// Sections that could not be read or stored: "likes", "playlists", "history".
    pub failed: Vec<&'static str>,
}
impl ImportReport {
    fn changed(&self) -> bool {
        self.liked_added
            + self.liked_removed
            + self.history_added
            + self.playlists_created
            + self.playlists_updated
            > 0
            || !self.failed.is_empty()
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "state")]
enum ImportEvent {
    Running,
    Done { report: ImportReport },
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Full,
    Quick,
}

/// What the importer remembers between runs, on disk: which liked songs came from YouTube (only those are removed
/// when they are un-liked there) and which imported playlists the user deleted (never brought back).
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Memory {
    liked_ids: BTreeSet<String>,
    skipped_playlists: BTreeSet<String>,
}
fn memory_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(MEMORY_FILE))
}
fn load_memory(app: &AppHandle) -> Memory {
    memory_path(app)
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| b.len() <= 4 * 1024 * 1024)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save_memory(app: &AppHandle, memory: &Memory) {
    if let Some(path) = memory_path(app)
        && let Ok(bytes) = serde_json::to_vec(memory)
    {
        let _ = std::fs::write(path, bytes);
    }
}

const MARKER_PREFIX: &str = "Imported from YouTube Music [";
fn marker(id: &str) -> String {
    format!("{MARKER_PREFIX}{id}]")
}
/// The YouTube playlist id inside an import marker, if the description is one.
pub fn marker_id(description: &str) -> Option<&str> {
    description
        .strip_prefix(MARKER_PREFIX)
        .and_then(|rest| rest.strip_suffix(']'))
}
/// Called when the user deletes a local playlist: an imported one must not come back on the next pass.
pub fn forget_playlist(app: &AppHandle, description: Option<&str>) {
    if let Some(id) = description.and_then(marker_id) {
        let mut memory = load_memory(app);
        memory.skipped_playlists.insert(id.to_owned());
        save_memory(app, &memory);
    }
}

fn entry(t: &SavedTrack) -> LocalEntry {
    LocalEntry {
        provider: Provider::YouTube,
        provider_track_id: t.provider_track_id.clone(),
        provider_url: t.canonical_url.clone(),
        title: Some(t.title.clone()),
        artists: Some(t.artists.clone()),
        artwork_url: t.artwork_url.clone(),
        duration_ms: t.duration_ms.and_then(|d| i64::try_from(d).ok()),
        remote_entry_id: None,
    }
}

/// Creates each imported playlist once (found again by the marker in its description) and adds only the tracks the
/// local copy does not have yet, so a repeat pass never duplicates and never removes anything the user changed.
async fn apply_playlists(
    app: &AppHandle,
    import: &Import,
    skipped: &BTreeSet<String>,
    report: &mut ImportReport,
) {
    if import.playlists.is_empty() {
        return;
    }
    let state = app.state::<SyncEngineState>();
    let Ok(mut checkout) = state.checkout().await else {
        report.failed.push("playlists");
        return;
    };
    let engine = checkout.engine();
    for playlist in import.playlists.iter().filter(|p| !skipped.contains(&p.id)) {
        let note = marker(&playlist.id);
        let existing = engine
            .local_playlists()
            .iter()
            .find(|p| p.description.as_deref() == Some(note.as_str()))
            .map(|p| p.id.clone());
        let (local_id, created) = match existing {
            Some(id) => (id, false),
            None => match engine.create_local(&playlist.name, Some(&note)) {
                Ok(id) => (id, true),
                Err(_) => {
                    report.failed.push("playlists");
                    continue;
                }
            },
        };
        let have: HashSet<String> = engine
            .local_playlists()
            .iter()
            .find(|p| p.id == local_id)
            .map(|p| {
                p.entries
                    .iter()
                    .map(|e| e.provider_track_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut added = 0;
        for track in playlist
            .tracks
            .iter()
            .filter(|t| !have.contains(&t.provider_track_id))
        {
            if engine.add_local_entry(&local_id, entry(track)).is_ok() {
                added += 1;
            }
        }
        report.playlist_tracks_added += added;
        if created {
            report.playlists_created += 1;
        } else if added > 0 {
            report.playlists_updated += 1;
        }
    }
}

pub async fn run(app: &AppHandle) -> Result<ImportReport, LibraryError> {
    run_mode(app, Mode::Full).await
}

async fn run_mode(app: &AppHandle, mode: Mode) -> Result<ImportReport, LibraryError> {
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Err(LibraryError::InUse);
    }
    let result = run_inner(app, mode).await;
    RUNNING.store(false, Ordering::Release);
    // A quick pass that found nothing stays silent, so the settings line does not flicker every two minutes.
    let event = match &result {
        Ok(report) if mode == Mode::Quick && !report.changed() => None,
        Ok(report) => Some(ImportEvent::Done {
            report: report.clone(),
        }),
        Err(_) if mode == Mode::Quick => None,
        Err(_) => Some(ImportEvent::Failed),
    };
    if let Some(event) = event {
        let _ = app.emit_to("main", EVENT, event);
    }
    result
}

async fn run_inner(app: &AppHandle, mode: Mode) -> Result<ImportReport, LibraryError> {
    let pipe = app
        .state::<YouTubeState>()
        .pipe
        .clone()
        .ok_or(LibraryError::Unavailable)?;
    if mode == Mode::Full {
        let _ = app.emit_to("main", EVENT, ImportEvent::Running);
    }
    let known = if mode == Mode::Quick {
        COUNTS
            .lock()
            .map(|c| c.clone().unwrap_or_default())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let import = fetch(&pipe, &known).await;
    let mut memory = load_memory(app);
    let mut report = ImportReport {
        failed: import.failed.clone(),
        ..Default::default()
    };

    if !import.failed.contains(&"likes") {
        let incoming: BTreeSet<String> = import
            .liked
            .iter()
            .map(|t| t.provider_track_id.clone())
            .collect();
        let library = app.state::<LibraryState>();
        // Only songs that came from YouTube are removed, and only when the whole list was read.
        let removed: HashSet<String> = if import.liked_complete {
            memory
                .liked_ids
                .iter()
                .filter(|id| !incoming.contains(*id))
                .cloned()
                .collect()
        } else {
            HashSet::new()
        };
        let outcome = library.with(|l| {
            let removed_n = if removed.is_empty() {
                0
            } else {
                l.remove_youtube_ids(&removed)?
            };
            let added_n = l.merge_append(import.liked.clone(), 5000)?;
            Ok((added_n, removed_n))
        });
        match outcome {
            Ok((added, removed_n)) => {
                report.liked_added = added;
                report.liked_removed = removed_n;
                memory.liked_ids = if import.liked_complete {
                    incoming
                } else {
                    memory.liked_ids.union(&incoming).cloned().collect()
                };
                if added + removed_n > 0 {
                    let _ = app.emit_to("main", "core://library/changed", ());
                }
            }
            Err(_) => report.failed.push("likes"),
        }
    }

    if !import.history.is_empty() {
        match app.state::<HistoryState>().0.with(|l| {
            l.merge_append(import.history.clone(), 100)
                .and_then(|n| Ok((n, l.list()?)))
        }) {
            Ok((n, tracks)) => {
                report.history_added = n;
                if n > 0 {
                    let _ = app.emit_to("main", "core://history/changed", tracks);
                }
            }
            Err(_) => report.failed.push("history"),
        }
    }

    apply_playlists(app, &import, &memory.skipped_playlists, &mut report).await;
    if !import.failed.contains(&"playlists")
        && let Ok(mut counts) = COUNTS.lock()
    {
        *counts = Some(import.counts.clone());
    }
    save_memory(app, &memory);
    report.failed.sort_unstable();
    report.failed.dedup();
    Ok(report)
}

/// Starts the two-minute quick pass once. It runs only while the account is connected.
pub fn start_sync_loop(app: &AppHandle) {
    if LOOPING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(QUICK_EVERY).await;
            let connected = app.state::<YouTubeState>().connected();
            if connected {
                let _ = run_mode(&app, Mode::Quick).await;
            }
        }
    });
}

fn stamp_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("youtube-import.last"))
}
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Automatic full pass: skipped when the last one finished less than six hours ago, unless `force`. Also starts
/// the quick loop.
pub async fn run_auto(app: &AppHandle, force: bool) {
    start_sync_loop(app);
    let path = stamp_path(app);
    if !force
        && let Some(last) = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
        && now_secs().saturating_sub(last) < AUTO_EVERY_SECS
    {
        return;
    }
    if run(app).await.is_ok()
        && let Some(path) = path
    {
        let _ = std::fs::write(path, now_secs().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_round_trips_and_rejects_other_descriptions() {
        assert_eq!(marker_id(&marker("PL123")), Some("PL123"));
        assert_eq!(marker_id("My own playlist"), None);
        assert_eq!(marker_id("Imported from YouTube Music [PL1"), None);
    }
}

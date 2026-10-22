//! Imports the signed-in YouTube Music collection into the device: liked songs, saved playlists, recent history.
//! Read-only toward YouTube. It runs after a sign-in, on later launches at most every six hours, and on demand.
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
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

const EVENT: &str = "core://youtube/import";
const AUTO_EVERY_SECS: u64 = 6 * 3600;
static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub liked_added: usize,
    pub history_added: usize,
    pub playlists_created: usize,
    pub playlists_updated: usize,
    pub playlist_tracks_added: usize,
    /// Sections that could not be read or stored: "likes", "playlists", "history".
    pub failed: Vec<&'static str>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "state")]
enum ImportEvent {
    Running,
    Done { report: ImportReport },
    Failed,
}

fn marker(id: &str) -> String {
    format!("Imported from YouTube Music [{id}]")
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
/// local copy does not have yet, so a repeat import never duplicates and never removes anything the user changed.
async fn apply_playlists(app: &AppHandle, import: &Import, report: &mut ImportReport) {
    let state = app.state::<SyncEngineState>();
    let Ok(mut checkout) = state.checkout().await else {
        report.failed.push("playlists");
        return;
    };
    let engine = checkout.engine();
    for playlist in &import.playlists {
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
        let have: std::collections::HashSet<String> = engine
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
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Err(LibraryError::InUse);
    }
    let result = run_inner(app).await;
    RUNNING.store(false, Ordering::Release);
    let _ = app.emit_to(
        "main",
        EVENT,
        match &result {
            Ok(report) => ImportEvent::Done {
                report: report.clone(),
            },
            Err(_) => ImportEvent::Failed,
        },
    );
    result
}

async fn run_inner(app: &AppHandle) -> Result<ImportReport, LibraryError> {
    let pipe = app
        .state::<YouTubeState>()
        .pipe
        .clone()
        .ok_or(LibraryError::Unavailable)?;
    let _ = app.emit_to("main", EVENT, ImportEvent::Running);
    let import = fetch(&pipe).await;
    let mut report = ImportReport {
        failed: import.failed.clone(),
        ..Default::default()
    };
    match app
        .state::<LibraryState>()
        .with(|l| l.merge_append(import.liked.clone(), 5000))
    {
        Ok(n) => {
            report.liked_added = n;
            if n > 0 {
                let _ = app.emit_to("main", "core://library/changed", ());
            }
        }
        Err(_) => report.failed.push("likes"),
    }
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
    apply_playlists(app, &import, &mut report).await;
    report.failed.sort_unstable();
    report.failed.dedup();
    Ok(report)
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

/// Automatic run: skipped when the last one finished less than six hours ago.
pub async fn run_auto(app: &AppHandle, force: bool) {
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

//! Reads the signed-in user's YouTube Music collection: liked songs, saved playlists and recent history.
//! Read-only: nothing is written back to YouTube. Each section fails on its own, so a partial import still lands.
use super::library::{LibraryError, SavedTrack, saved_track};
use rustypipe::client::RustyPipe;
use std::time::Duration;

const LIKED_MAX: usize = 1000;
const HISTORY_MAX: usize = 50;
const PLAYLISTS_MAX: usize = 30;
const PLAYLIST_TRACKS_MAX: usize = 300;
const STEP_TIMEOUT: Duration = Duration::from_secs(45);

pub struct ImportedPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<SavedTrack>,
}

#[derive(Default)]
pub struct Import {
    pub liked: Vec<SavedTrack>,
    pub history: Vec<SavedTrack>,
    pub playlists: Vec<ImportedPlaylist>,
    /// Sections that could not be read, by name.
    pub failed: Vec<&'static str>,
}

async fn step<T>(
    fut: impl std::future::Future<Output = Result<T, rustypipe::error::Error>>,
) -> Result<T, LibraryError> {
    tokio::time::timeout(STEP_TIMEOUT, fut)
        .await
        .map_err(|_| LibraryError::Unavailable)?
        .map_err(|_| LibraryError::Unavailable)
}

async fn liked(pipe: &RustyPipe) -> Result<Vec<SavedTrack>, LibraryError> {
    let query = pipe.query();
    let mut page = step(query.music_saved_tracks()).await?;
    step(page.extend_limit(&query, LIKED_MAX)).await?;
    Ok(page
        .items
        .into_iter()
        .take(LIKED_MAX)
        .filter_map(|t| saved_track(t, None))
        .collect())
}

async fn history(pipe: &RustyPipe) -> Result<Vec<SavedTrack>, LibraryError> {
    let query = pipe.query();
    let page = step(query.music_history()).await?;
    Ok(page
        .items
        .into_iter()
        .take(HISTORY_MAX)
        .filter_map(|h| saved_track(h.item, None))
        .collect())
}

async fn playlists(pipe: &RustyPipe) -> Result<Vec<ImportedPlaylist>, LibraryError> {
    let query = pipe.query();
    let mut saved = step(query.music_saved_playlists()).await?;
    step(saved.extend_limit(&query, PLAYLISTS_MAX)).await?;
    let mut out = Vec::new();
    for item in saved
        .items
        .into_iter()
        .filter(|p| !p.is_podcast)
        .take(PLAYLISTS_MAX)
    {
        // One unreadable playlist must not drop the others.
        let Ok(mut playlist) = step(query.music_playlist(&item.id)).await else {
            continue;
        };
        if step(playlist.tracks.extend_limit(&query, PLAYLIST_TRACKS_MAX))
            .await
            .is_err()
        {
            continue;
        }
        let tracks: Vec<SavedTrack> = playlist
            .tracks
            .items
            .into_iter()
            .take(PLAYLIST_TRACKS_MAX)
            .filter_map(|t| saved_track(t, None))
            .collect();
        if !tracks.is_empty() {
            out.push(ImportedPlaylist {
                id: item.id,
                name: item.name,
                tracks,
            });
        }
    }
    Ok(out)
}

pub async fn fetch(pipe: &RustyPipe) -> Import {
    let mut import = Import::default();
    match liked(pipe).await {
        Ok(v) => import.liked = v,
        Err(_) => import.failed.push("likes"),
    }
    match playlists(pipe).await {
        Ok(v) => import.playlists = v,
        Err(_) => import.failed.push("playlists"),
    }
    match history(pipe).await {
        Ok(v) => import.history = v,
        Err(_) => import.failed.push("history"),
    }
    import
}

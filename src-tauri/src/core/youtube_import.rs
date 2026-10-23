//! Reads the signed-in user's YouTube Music and YouTube collections: liked songs and videos, saved playlists,
//! Watch later and recent history.
//! Reading never changes the account. The one write is `rate`, a like or un-like the user did here, sent only when they turned that on.
//! Each section fails on its own, so a partial import still lands.
use super::library::{LibraryError, SavedTrack, saved_track};
use rustypipe::client::RustyPipe;
use std::{collections::HashMap, time::Duration};

const LIKED_MAX: usize = 1000;
const HISTORY_MAX: usize = 50;
const PLAYLISTS_MAX: usize = 30;
const PLAYLIST_TRACKS_MAX: usize = 300;
const STEP_TIMEOUT: Duration = Duration::from_secs(45);

/// Where an imported playlist came from. It decides the marker in the local playlist's description.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Music,
    YouTube,
}

pub struct ImportedPlaylist {
    pub id: String,
    pub name: String,
    pub origin: Origin,
    pub tracks: Vec<SavedTrack>,
}

/// A YouTube video as a saved track. Livestreams and Shorts are not music to play, so they are left out.
fn video_track(v: rustypipe::model::VideoItem) -> Option<SavedTrack> {
    if v.is_live || v.is_short || !super::playback::extract::valid_id(&v.id) {
        return None;
    }
    let (artists, artist_refs) = match v.channel {
        Some(c) if !c.name.trim().is_empty() => (
            vec![c.name.clone()],
            vec![super::search::hit::ArtistRef {
                provider: super::domain::ProviderId::YouTube,
                id: Some(c.id).filter(|id| {
                    super::library::artist_id(&super::domain::ProviderId::YouTube, id)
                }),
                name: c.name,
            }],
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let mut track = SavedTrack {
        source: Some(super::search::hit::SearchSource::YouTube),
        provider: super::domain::ProviderId::YouTube,
        provider_track_id: v.id.clone(),
        canonical_url: format!("https://www.youtube.com/watch?v={}", v.id),
        title: v.name,
        artists,
        artist_refs,
        artwork_url: v
            .thumbnail
            .into_iter()
            .max_by_key(|t| t.width)
            .and_then(|t| super::playback::track_info::artwork_url(&t.url)),
        duration_ms: v.duration.map(|d| u64::from(d) * 1000),
    };
    track.validate().ok().map(|()| track)
}

#[derive(Default)]
pub struct Import {
    pub liked: Vec<SavedTrack>,
    pub history: Vec<SavedTrack>,
    pub playlists: Vec<ImportedPlaylist>,
    /// True when every liked song was read (the list ended before the cap), so a missing one was really unliked.
    pub liked_complete: bool,
    /// Track counts of every saved playlist seen, for the next quick pass.
    pub counts: HashMap<String, u64>,
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

/// Likes or un-likes one video on the signed-in account (`like/like`, `like/removelike`), as the YouTube site does.
/// Needs the session cookie; rustypipe has no call for it, so this is the raw endpoint. `Err` means nothing was changed.
pub async fn rate(pipe: &RustyPipe, video_id: &str, like: bool) -> Result<(), LibraryError> {
    if !super::playback::extract::valid_id(video_id) {
        return Err(LibraryError::Validation);
    }
    let endpoint = if like { "like/like" } else { "like/removelike" };
    let body = serde_json::json!({ "target": { "videoId": video_id } });
    step(pipe.query().authenticated().raw(
        rustypipe::client::ClientType::DesktopMusic,
        endpoint,
        &body,
    ))
    .await
    .map(|_| ())
}

async fn liked(pipe: &RustyPipe) -> Result<(Vec<SavedTrack>, bool), LibraryError> {
    let query = pipe.query();
    let mut page = step(query.music_saved_tracks()).await?;
    step(page.extend_limit(&query, LIKED_MAX)).await?;
    let complete = page.items.len() < LIKED_MAX;
    Ok((
        page.items
            .into_iter()
            .take(LIKED_MAX)
            .filter_map(|t| saved_track(t, None))
            .collect(),
        complete,
    ))
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

async fn playlists(
    pipe: &RustyPipe,
    known: &HashMap<String, u64>,
    counts: &mut HashMap<String, u64>,
) -> Result<Vec<ImportedPlaylist>, LibraryError> {
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
        if let Some(n) = item.track_count {
            counts.insert(item.id.clone(), n);
            // A quick pass reads only playlists whose size changed since the last pass.
            if known.get(&item.id) == Some(&n) {
                continue;
            }
        }
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
                origin: Origin::Music,
                tracks,
            });
        }
    }
    Ok(out)
}

/// `known` holds playlist sizes from the last pass; pass an empty map to read every playlist.
async fn youtube_liked(pipe: &RustyPipe) -> Result<(Vec<SavedTrack>, bool), LibraryError> {
    let query = pipe.query();
    let mut list = step(query.liked_videos()).await?;
    step(list.videos.extend_limit(&query, LIKED_MAX)).await?;
    let complete = list.videos.items.len() < LIKED_MAX;
    Ok((
        list.videos
            .items
            .into_iter()
            .take(LIKED_MAX)
            .filter_map(video_track)
            .collect(),
        complete,
    ))
}

async fn youtube_history(pipe: &RustyPipe) -> Result<Vec<SavedTrack>, LibraryError> {
    let query = pipe.query();
    let page = step(query.history()).await?;
    Ok(page
        .items
        .into_iter()
        .take(HISTORY_MAX)
        .filter_map(|h| video_track(h.item))
        .collect())
}

async fn video_playlist(
    query: &rustypipe::client::RustyPipeQuery,
    id: &str,
    watch_later: bool,
) -> Option<Vec<SavedTrack>> {
    let mut list = if watch_later {
        step(query.watch_later()).await.ok()?
    } else {
        step(query.playlist(id)).await.ok()?
    };
    step(list.videos.extend_limit(query, PLAYLIST_TRACKS_MAX))
        .await
        .ok()?;
    let tracks: Vec<SavedTrack> = list
        .videos
        .items
        .into_iter()
        .take(PLAYLIST_TRACKS_MAX)
        .filter_map(video_track)
        .collect();
    (!tracks.is_empty()).then_some(tracks)
}

/// Saved YouTube playlists and Watch later. Playlists already read through YouTube Music (`skip`) are not read twice.
async fn youtube_playlists(
    pipe: &RustyPipe,
    known: &HashMap<String, u64>,
    counts: &mut HashMap<String, u64>,
    skip: &std::collections::HashSet<String>,
) -> Result<Vec<ImportedPlaylist>, LibraryError> {
    let query = pipe.query();
    let mut saved = step(query.saved_playlists()).await?;
    step(saved.extend_limit(&query, PLAYLISTS_MAX)).await?;
    let mut out = Vec::new();
    for item in saved
        .items
        .into_iter()
        .filter(|p| !skip.contains(&p.id))
        .take(PLAYLISTS_MAX)
    {
        if let Some(n) = item.video_count {
            counts.insert(item.id.clone(), n);
            if known.get(&item.id) == Some(&n) {
                continue;
            }
        }
        if let Some(tracks) = video_playlist(&query, &item.id, false).await {
            out.push(ImportedPlaylist {
                id: item.id,
                name: item.name,
                origin: Origin::YouTube,
                tracks,
            });
        }
    }
    if let Some(tracks) = video_playlist(&query, "WL", true).await {
        out.push(ImportedPlaylist {
            id: "WL".into(),
            name: "Watch later".into(),
            origin: Origin::YouTube,
            tracks,
        });
    }
    Ok(out)
}

/// `known` holds playlist sizes from the last pass; pass an empty map to read every playlist.
pub async fn fetch(pipe: &RustyPipe, known: &HashMap<String, u64>) -> Import {
    let mut import = Import::default();
    match liked(pipe).await {
        Ok((v, complete)) => {
            import.liked = v;
            import.liked_complete = complete;
        }
        Err(_) => import.failed.push("likes"),
    }
    match youtube_liked(pipe).await {
        Ok((videos, complete)) => {
            let seen: std::collections::HashSet<String> = import
                .liked
                .iter()
                .map(|t| t.provider_track_id.clone())
                .collect();
            import.liked.extend(
                videos
                    .into_iter()
                    .filter(|t| !seen.contains(&t.provider_track_id)),
            );
            import.liked_complete &= complete;
        }
        Err(_) => {
            import.failed.push("ytLikes");
            // A missing list cannot prove a song was un-liked.
            import.liked_complete = false;
        }
    }
    let mut counts = HashMap::new();
    let result = playlists(pipe, known, &mut counts).await;
    match result {
        Ok(v) => import.playlists = v,
        Err(_) => import.failed.push("playlists"),
    }
    let music_ids: std::collections::HashSet<String> = counts.keys().cloned().collect();
    match youtube_playlists(pipe, known, &mut counts, &music_ids).await {
        Ok(v) => import.playlists.extend(v),
        Err(_) => import.failed.push("ytPlaylists"),
    }
    import.counts = counts;
    match history(pipe).await {
        Ok(v) => import.history = v,
        Err(_) => import.failed.push("history"),
    }
    match youtube_history(pipe).await {
        Ok(v) => {
            let seen: std::collections::HashSet<String> = import
                .history
                .iter()
                .map(|t| t.provider_track_id.clone())
                .collect();
            import.history.extend(
                v.into_iter()
                    .filter(|t| !seen.contains(&t.provider_track_id)),
            );
        }
        Err(_) => import.failed.push("ytHistory"),
    }
    import
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rate_refuses_a_bad_video_id_before_any_request() {
        let pipe = RustyPipe::new();
        for bad in ["", "short", "has space!!", "../../etc/pa"] {
            assert!(matches!(
                rate(&pipe, bad, true).await,
                Err(LibraryError::Validation)
            ));
        }
    }
}

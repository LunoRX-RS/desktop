//! The music video behind the backgrounds. The search and the ranking live in core::mv; this runs the search on the
//! signed-out YouTube client and returns an id only when YouTube lets it play embedded. No account token is used.
use std::time::Duration;

use crate::commands::youtube::YouTubeState;
use crate::core::mv::{Candidate, best};
use tauri::{State, WebviewWindow};

fn main_only(window: &WebviewWindow) -> bool {
    window.label() == "main"
}

/// Whether YouTube lets this video play in an embed. oEmbed answers 200 for embeddable videos and 401 or 404 for the
/// rest, so a video whose owner turned embedding off is never handed to the frame.
async fn embeddable(http: &reqwest::Client, id: &str) -> bool {
    let url = format!(
        "https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3D{id}"
    );
    matches!(http.get(url).send().await, Ok(r) if r.status().is_success())
}

/// The id of the song's music video if it can play embedded, else the track's own id if that can. None means there is
/// nothing to show and the backdrop falls back to the artwork.
#[tauri::command]
pub async fn mv_find(
    window: WebviewWindow,
    yt: State<'_, YouTubeState>,
    title: String,
    artist: String,
    duration_ms: Option<u64>,
    own_id: Option<String>,
) -> Result<Option<String>, ()> {
    if !main_only(&window)
        || title.trim().is_empty()
        || title.chars().count() > 200
        || artist.chars().count() > 200
    {
        return Ok(None);
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| ())?;
    let own = own_id.filter(|id| id.len() == 11);
    for id in search(&yt, &title, &artist, duration_ms)
        .await
        .into_iter()
        .chain(own)
    {
        if embeddable(&http, &id).await {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

async fn search(
    yt: &State<'_, YouTubeState>,
    title: &str,
    artist: &str,
    duration_ms: Option<u64>,
) -> Option<String> {
    let api = yt.yt.as_ref()?.youtube();
    let queries = [
        format!("{title} {artist} official music video"),
        format!("{title} {artist}"),
    ];
    let mut found: Vec<Candidate> = Vec::new();
    for q in queries {
        let Ok(page) = api.search_videos(&q, None).await else {
            continue;
        };
        found.extend(
            page.items
                .into_iter()
                .filter(|v| !v.is_live && !v.is_short)
                .map(|v| Candidate {
                    id: v.id,
                    title: v.title,
                    channel: v.channel.map(|c| c.name).unwrap_or_default(),
                    duration_ms: v.duration_ms,
                }),
        );
        if let Some(id) = best(title, artist, duration_ms, &found) {
            return Some(id);
        }
    }
    None
}

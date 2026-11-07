//! The music video behind the backgrounds. The search and the ranking live in core::mv; this only runs the search
//! on the signed-out YouTube client and hands the best id back. No account token is used.
use crate::commands::youtube::YouTubeState;
use crate::core::mv::{Candidate, best};
use tauri::{State, WebviewWindow};

fn main_only(window: &WebviewWindow) -> bool {
    window.label() == "main"
}

/// The id of the song's music video, or None when none was found (the caller then falls back).
#[tauri::command]
pub async fn mv_find(
    window: WebviewWindow,
    yt: State<'_, YouTubeState>,
    title: String,
    artist: String,
    duration_ms: Option<u64>,
) -> Result<Option<String>, ()> {
    if !main_only(&window)
        || title.trim().is_empty()
        || title.chars().count() > 200
        || artist.chars().count() > 200
    {
        return Ok(None);
    }
    let Some(client) = yt.yt.as_ref() else {
        return Ok(None);
    };
    let api = client.youtube();
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
        if let Some(id) = best(&title, &artist, duration_ms, &found) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

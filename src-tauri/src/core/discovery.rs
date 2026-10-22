//! Provider-backed discovery. All recommendations and search suggestions stay on the desktop.
use crate::core::{
    domain::ProviderId,
    library::{LibraryError, SavedTrack},
    search::hit::{ArtistRef, SearchSource},
};
use rustypipe::{client::RustyPipe, model::TrackItem};
use serde::Serialize;
use std::time::Duration;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedSection {
    pub title: String,
    pub tracks: Vec<SavedTrack>,
}
fn tracks(items: Vec<TrackItem>) -> Vec<SavedTrack> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|t| t.track_type != rustypipe::model::TrackType::Episode)
        .filter_map(|t| {
            let mut track = SavedTrack {
                source: Some(SearchSource::YouTubeMusic),
                provider: ProviderId::YouTube,
                provider_track_id: t.id.clone(),
                canonical_url: format!("https://www.youtube.com/watch?v={}", t.id),
                title: t.name,
                artists: t.artists.iter().map(|a| a.name.clone()).collect(),
                artist_refs: t
                    .artists
                    .into_iter()
                    .map(|a| ArtistRef {
                        provider: ProviderId::YouTube,
                        id: a.id,
                        name: a.name,
                    })
                    .collect(),
                artwork_url: t
                    .cover
                    .into_iter()
                    .max_by_key(|a| a.width)
                    .and_then(|a| super::playback::track_info::artwork_url(&a.url)),
                duration_ms: t.duration.map(|d| u64::from(d) * 1000),
            };
            (track.validate().is_ok() && seen.insert(track.provider_track_id.clone()))
                .then_some(track)
        })
        .take(20)
        .collect()
}
pub fn topic_query(topic: &str) -> Option<(&'static str, &'static str)> {
    match topic {
        "focus" => Some(("Focus", "instrumental focus music")),
        "chill" => Some(("Chill", "chill music")),
        "pop" => Some(("Pop", "pop music")),
        "hiphop" => Some(("Hip-hop", "hip hop music")),
        "thai" => Some(("Thai music", "เพลงไทย")),
        "acoustic" => Some(("Acoustic", "acoustic music")),
        _ => None,
    }
}
fn personalized_title(kind: &str) -> Option<&'static str> {
    match kind {
        "home" => Some("More from your listening"),
        "explore" => Some("Picked for you"),
        _ => None,
    }
}
pub async fn feed(
    pipe: &RustyPipe,
    kind: &str,
    topic: &str,
    seed: Option<&str>,
) -> Result<Vec<FeedSection>, LibraryError> {
    if !["home", "explore"].contains(&kind) || (topic != "charts" && topic_query(topic).is_none()) {
        return Err(LibraryError::Validation);
    }
    let client = pipe.query();
    if let Some(title) = personalized_title(kind)
        && let Some(seed) = seed.filter(|id| super::playback::extract::valid_id(id))
        && let Ok(Ok(radio)) =
            tokio::time::timeout(Duration::from_secs(12), client.music_radio_track(seed)).await
    {
        let list = tracks(radio.items.into_iter().filter(|t| t.id != seed).collect());
        if !list.is_empty() {
            return Ok(vec![FeedSection {
                title: title.into(),
                tracks: list,
            }]);
        }
    }
    if topic == "charts"
        && let Ok(Ok(charts)) =
            tokio::time::timeout(Duration::from_secs(12), client.music_charts(None)).await
    {
        let sections: Vec<_> = [
            ("Top music videos", tracks(charts.top_tracks)),
            ("Trending on YouTube Music", tracks(charts.trending_tracks)),
        ]
        .into_iter()
        .filter(|(_, t)| !t.is_empty())
        .map(|(title, tracks)| FeedSection {
            title: title.into(),
            tracks,
        })
        .collect();
        if !sections.is_empty() {
            return Ok(sections);
        }
    }
    let (title, query) = if topic == "charts" {
        ("Discover music", "music")
    } else {
        topic_query(topic).ok_or(LibraryError::Validation)?
    };
    let result = tokio::time::timeout(Duration::from_secs(12), client.music_search_tracks(query))
        .await
        .map_err(|_| LibraryError::Unavailable)?
        .map_err(|_| LibraryError::Unavailable)?;
    let list = tracks(result.items.items);
    if list.is_empty() {
        return Err(LibraryError::Unavailable);
    }
    Ok(vec![FeedSection {
        title: title.into(),
        tracks: list,
    }])
}
pub async fn suggestions(pipe: &RustyPipe, query: &str) -> Result<Vec<String>, LibraryError> {
    let query = query.trim();
    if query.chars().count() < 2 || query.len() > 256 {
        return Err(LibraryError::Validation);
    }
    let values = tokio::time::timeout(
        Duration::from_secs(4),
        pipe.query().search_suggestion(query),
    )
    .await
    .map_err(|_| LibraryError::Unavailable)?
    .map_err(|_| LibraryError::Unavailable)?;
    let mut seen = std::collections::HashSet::new();
    Ok(values
        .into_iter()
        .filter(|s| !s.trim().is_empty() && s.len() <= 256 && seen.insert(s.clone()))
        .take(6)
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feed_tracks_preserve_identity_filter_podcasts_and_deduplicate() {
        let item = |id: &str, typ: &str| {
            serde_json::from_value::<TrackItem>(serde_json::json!({"id":id,"name":"Real fixture","duration":180,"cover":[],"artists":[{"id":null,"name":"Artist"}],"track_type":typ,"by_va":false})).unwrap()
        };
        let found = tracks(vec![
            item("r-G2pKr2_n8", "track"),
            item("r-G2pKr2_n8", "track"),
            item("aqz-KE-bpKQ", "episode"),
            item("invalid", "track"),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].provider_track_id, "r-G2pKr2_n8");
        assert_eq!(found[0].duration_ms, Some(180000));
    }
    #[test]
    fn topics_are_bounded_provider_queries() {
        assert!(topic_query("thai").is_some());
        assert!(topic_query("anything untrusted").is_none());
    }
    #[test]
    fn explore_uses_personalized_radio_when_listening_seed_exists() {
        assert_eq!(personalized_title("explore"), Some("Picked for you"));
        assert_eq!(personalized_title("home"), Some("More from your listening"));
        assert_eq!(personalized_title("other"), None);
    }
}

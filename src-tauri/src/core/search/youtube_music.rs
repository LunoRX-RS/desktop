//! Unified YouTube Music and ordinary YouTube search using the desktop session.
use super::{
    hit::{ArtistRef, SearchHit, SearchSource},
    provider::{SearchFuture, SearchProvider},
    rank::{Candidate, Channel, Kind, order},
};
use crate::core::{domain::ProviderId, errors::CoreError, playback::extract::valid_id, yt::Yt};
use innerlight::{
    Error as YtError,
    music::{Track, TrackKind},
    youtube::{Verification, Video},
};

pub struct YouTubeMusicSearch {
    yt: Yt,
}
impl YouTubeMusicSearch {
    pub fn new(yt: Yt) -> Self {
        Self { yt }
    }
}
// After the song search answers, the video search gets this long to finish. Waiting out its full timeout made every search as
// slow as the slower of the two requests.
const VIDEO_GRACE: std::time::Duration = std::time::Duration::from_millis(900);
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(14);

impl SearchProvider for YouTubeMusicSearch {
    fn id(&self) -> ProviderId {
        ProviderId::YouTube
    }
    fn search<'a>(&'a self, query: &'a str, limit: u8) -> SearchFuture<'a> {
        Box::pin(async move {
            let (music_api, youtube_api) = (self.yt.music(), self.yt.youtube());
            let mut music = std::pin::pin!(tokio::time::timeout(
                REQUEST_TIMEOUT,
                music_api.search_tracks(query)
            ));
            let mut videos = std::pin::pin!(tokio::time::timeout(
                REQUEST_TIMEOUT,
                youtube_api.search_videos(query, None)
            ));
            let (music, videos) = tokio::select! {
                music = &mut music => {
                    let videos = tokio::time::timeout(VIDEO_GRACE, &mut videos).await;
                    (music, videos.unwrap_or(Ok(Err(YtError::Network))))
                }
                videos = &mut videos => ((&mut music).await, videos),
            };
            let music = music.ok().and_then(Result::ok);
            let videos = videos.ok().and_then(Result::ok);
            if music.is_none() && videos.is_none() {
                return Err(CoreError::ProviderUnavailable);
            }
            // Each source is ranked on its own, so ordinary YouTube videos are not crowded out by YouTube Music songs.
            // A video that is also a YouTube Music result is shown once, under YouTube Music.
            let limit = usize::from(limit.clamp(1, 10));
            let songs = order(
                query,
                music
                    .into_iter()
                    .flat_map(|r| r.items)
                    .filter_map(music_candidate)
                    .collect(),
                limit,
            );
            let known: std::collections::HashSet<String> = songs
                .iter()
                .map(|h| h.provider_track_id().to_owned())
                .collect();
            let clips = order(
                query,
                videos
                    .into_iter()
                    .flat_map(|r| r.items)
                    .filter_map(video_candidate)
                    .filter(|c| !known.contains(c.hit.provider_track_id()))
                    .collect(),
                limit,
            );
            Ok(songs.into_iter().chain(clips).collect())
        })
    }
}

fn music_candidate(track: Track) -> Option<Candidate> {
    if !valid_id(&track.id) {
        return None;
    }
    let artist_refs = track
        .artists
        .iter()
        .map(|a| ArtistRef {
            provider: ProviderId::YouTube,
            id: a.id.clone(),
            name: a.name.clone(),
        })
        .collect();
    let mut hit = SearchHit::new(
        ProviderId::YouTube,
        track.id.clone(),
        format!("https://www.youtube.com/watch?v={}", track.id),
        track.title,
        track.artists.into_iter().map(|a| a.name).collect(),
    )
    .ok()?;
    hit.source = Some(SearchSource::YouTubeMusic);
    hit.artist_refs = artist_refs;
    hit.duration_ms = track.duration_ms;
    hit.artwork_url = track
        .thumbnails
        .into_iter()
        .max_by_key(|cover| cover.width)
        .and_then(|cover| crate::core::playback::track_info::artwork_url(&cover.url));
    let kind = if track.kind == TrackKind::Audio {
        Kind::Release
    } else {
        Kind::MusicVideo
    };
    Some(Candidate {
        hit,
        kind,
        channel: Channel::Unknown,
    })
}

fn video_candidate(video: Video) -> Option<Candidate> {
    // Live streams, Shorts and premieres that have not started (no length) are not songs to play.
    if !valid_id(&video.id) || video.is_live || video.is_short || video.duration_ms.is_none() {
        return None;
    }
    let channel = match video.channel.as_ref().map(|c| c.verification) {
        Some(Verification::Artist) => Channel::Artist,
        Some(Verification::Verified) => Channel::Verified,
        _ => Channel::Unknown,
    };
    let artists = video
        .channel
        .as_ref()
        .map(|c| vec![c.name.clone()])
        .unwrap_or_default();
    let artist_refs = video
        .channel
        .into_iter()
        .map(|c| ArtistRef {
            provider: ProviderId::YouTube,
            id: c.id,
            name: c.name,
        })
        .collect();
    let mut hit = SearchHit::new(
        ProviderId::YouTube,
        video.id.clone(),
        format!("https://www.youtube.com/watch?v={}", video.id),
        video.title,
        artists,
    )
    .ok()?;
    hit.source = Some(SearchSource::YouTube);
    hit.artist_refs = artist_refs;
    hit.duration_ms = video.duration_ms;
    hit.artwork_url = video
        .thumbnails
        .into_iter()
        .max_by_key(|c| c.width)
        .and_then(|c| crate::core::playback::track_info::artwork_url(&c.url));
    Some(Candidate {
        hit,
        kind: Kind::Video,
        channel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(transport: innerlight::FixtureTransport) -> YouTubeMusicSearch {
        let client = innerlight::Client::builder()
            .transport(std::sync::Arc::new(transport))
            .build()
            .unwrap();
        YouTubeMusicSearch::new(Yt::from_client(client))
    }
    fn video(id: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut renderer = serde_json::json!({
            "videoId": id, "title": {"simpleText": "Video"},
            "ownerText": {"runs": [{"text": "Channel"}]},
            "lengthText": {"simpleText": "3:21"},
            "thumbnail": {"thumbnails": [{"url": "https://i.ytimg.com/vi/x/hq.jpg", "width": 360, "height": 202}]}
        });
        renderer
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::json!({"videoRenderer": renderer})
    }
    #[tokio::test]
    async fn video_search_alone_supplies_hits_and_skips_live_shorts_and_premieres() {
        let page = serde_json::json!({"contents": [
            video("abcdefghijk", serde_json::json!({})),
            video("kJQP7kiw5Fk", serde_json::json!({"badges": [{"metadataBadgeRenderer": {"style": "BADGE_STYLE_TYPE_LIVE_NOW"}}], "lengthText": null})),
            video("aqz-KE-bpKQ", serde_json::json!({"lengthText": null})),
            video("r-G2pKr2_n8", serde_json::json!({"navigationEndpoint": {"reelWatchEndpoint": {"videoId": "r-G2pKr2_n8"}}})),
        ]});
        // Music search answers with nothing readable; the video half still delivers.
        let hits = provider(
            innerlight::FixtureTransport::new()
                .with_match("search", "EgIQAQ", page)
                .with("search", serde_json::json!({"unexpected": true})),
        )
        .search("lofi", 5)
        .await;
        let hits = hits.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source(), Some(SearchSource::YouTube));
        assert_eq!(hits[0].provider_track_id(), "abcdefghijk");
    }
    #[tokio::test]
    async fn unreadable_answers_from_both_sources_are_unavailable_not_a_panic() {
        for body in [
            serde_json::json!({}),
            serde_json::json!(null),
            serde_json::json!({"contents": 7}),
        ] {
            let result = provider(innerlight::FixtureTransport::new().with("search", body))
                .search("lofi", 5)
                .await;
            // Parse failures on the music half, an empty list on the video half: no hits, no panic.
            assert!(result.map_or(true, |hits| hits.is_empty()));
        }
        let down = provider(innerlight::FixtureTransport::new().with_raw("search", 503, b"{}"));
        assert!(matches!(
            down.search("lofi", 5).await,
            Err(CoreError::ProviderUnavailable)
        ));
    }

    #[test]
    fn a_verified_artist_channel_ranks_as_artist_and_a_live_row_is_dropped() {
        let channel = innerlight::youtube::ChannelRef {
            id: Some("UCuAXFkgsw1L7xaCfnd5JJOw".into()),
            name: "Artist".into(),
            verification: Verification::Artist,
        };
        let row = Video {
            id: "abcdefghijk".into(),
            title: "Song".into(),
            channel: Some(channel.clone()),
            duration_ms: Some(200_000),
            thumbnails: vec![],
            is_live: false,
            is_short: false,
            set_video_id: None,
        };
        let candidate = video_candidate(row.clone()).unwrap();
        assert_eq!(candidate.channel, Channel::Artist);
        assert_eq!(candidate.kind, Kind::Video);
        assert!(
            video_candidate(Video {
                is_live: true,
                ..row
            })
            .is_none()
        );
    }
}

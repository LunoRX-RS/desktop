//! Unified YouTube Music and ordinary YouTube search using the desktop session.
use super::{
    hit::{ArtistRef, SearchHit, SearchSource},
    provider::{SearchFuture, SearchProvider},
    rank::{Candidate, Channel, Kind, order},
};
use crate::core::{domain::ProviderId, errors::CoreError, playback::extract::valid_id};
use rustypipe::client::RustyPipe;
use rustypipe::model::{TrackType, Verification};

pub struct YouTubeMusicSearch {
    pipe: RustyPipe,
}
impl YouTubeMusicSearch {
    pub fn new(pipe: RustyPipe) -> Self {
        Self { pipe }
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
            let client = self.pipe.query();
            let mut music = std::pin::pin!(tokio::time::timeout(
                REQUEST_TIMEOUT,
                client.music_search_tracks(query)
            ));
            let mut videos = std::pin::pin!(tokio::time::timeout(
                REQUEST_TIMEOUT,
                client.search::<rustypipe::model::VideoItem, _>(query)
            ));
            let (music, videos) = tokio::select! {
                music = &mut music => {
                    let videos = tokio::time::timeout(VIDEO_GRACE, &mut videos).await;
                    (music, videos.unwrap_or(Ok(Err(rustypipe::error::Error::Other("video search still running".into())))))
                }
                videos = &mut videos => ((&mut music).await, videos),
            };
            let music = music.ok().and_then(Result::ok);
            let videos = videos.ok().and_then(Result::ok);
            if music.is_none() && videos.is_none() {
                return Err(CoreError::ProviderUnavailable);
            }
            let mut candidates = Vec::new();
            for track in music.into_iter().flat_map(|r| r.items.items) {
                candidates.extend(music_candidate(track));
            }
            for video in videos.into_iter().flat_map(|r| r.items.items) {
                candidates.extend(video_candidate(video));
            }
            Ok(order(query, candidates, usize::from(limit.clamp(1, 10))))
        })
    }
}

fn music_candidate(track: rustypipe::model::TrackItem) -> Option<Candidate> {
    if !valid_id(&track.id) || track.track_type == TrackType::Episode {
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
        track.name,
        track.artists.into_iter().map(|a| a.name).collect(),
    )
    .ok()?;
    hit.source = Some(SearchSource::YouTubeMusic);
    hit.artist_refs = artist_refs;
    hit.duration_ms = track.duration.map(|seconds| u64::from(seconds) * 1000);
    hit.artwork_url = track
        .cover
        .into_iter()
        .max_by_key(|cover| cover.width)
        .and_then(|cover| crate::core::playback::track_info::artwork_url(&cover.url));
    let kind = if track.track_type == TrackType::Track {
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

fn video_candidate(video: rustypipe::model::VideoItem) -> Option<Candidate> {
    if !valid_id(&video.id) || video.is_live || video.is_upcoming || video.is_short {
        return None;
    }
    let channel = match video.channel.as_ref().map(|c| c.verification) {
        Some(Verification::Artist) => Channel::Artist,
        Some(Verification::Verified) => Channel::Verified,
        _ => Channel::Unknown,
    };
    let mut hit = SearchHit::new(
        ProviderId::YouTube,
        video.id.clone(),
        format!("https://www.youtube.com/watch?v={}", video.id),
        video.name,
        video
            .channel
            .as_ref()
            .map(|c| vec![c.name.clone()])
            .unwrap_or_default(),
    )
    .ok()?;
    hit.source = Some(SearchSource::YouTube);
    hit.artist_refs = video
        .channel
        .into_iter()
        .map(|c| ArtistRef {
            provider: ProviderId::YouTube,
            id: Some(c.id),
            name: c.name,
        })
        .collect();
    hit.duration_ms = video.duration.map(|d| u64::from(d) * 1000);
    hit.artwork_url = video
        .thumbnail
        .into_iter()
        .max_by_key(|c| c.width)
        .and_then(|c| crate::core::playback::track_info::artwork_url(&c.url));
    Some(Candidate {
        hit,
        kind: Kind::Video,
        channel,
    })
}

//! Unified YouTube Music and ordinary YouTube search using the desktop session.
use super::{
    hit::{SearchHit, SearchSource},
    provider::{SearchFuture, SearchProvider},
};
use crate::core::{domain::ProviderId, errors::CoreError, playback::extract::valid_id};
use rustypipe::client::RustyPipe;

pub struct YouTubeMusicSearch {
    pipe: RustyPipe,
}
impl YouTubeMusicSearch {
    pub fn new(pipe: RustyPipe) -> Self {
        Self { pipe }
    }
}
impl SearchProvider for YouTubeMusicSearch {
    fn id(&self) -> ProviderId {
        ProviderId::YouTube
    }
    fn search<'a>(&'a self, query: &'a str, limit: u8) -> SearchFuture<'a> {
        Box::pin(async move {
            let client = self.pipe.query();
            let (music, videos) = tokio::join!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(14),
                    client.music_search_tracks(query)
                ),
                tokio::time::timeout(
                    std::time::Duration::from_secs(14),
                    client.search::<rustypipe::model::VideoItem, _>(query)
                ),
            );
            let music = music.ok().and_then(Result::ok);
            let videos = videos.ok().and_then(Result::ok);
            if music.is_none() && videos.is_none() {
                return Err(CoreError::ProviderUnavailable);
            }
            let mut music_hits = Vec::new();
            if let Some(result) = music {
                for track in result
                    .items
                    .items
                    .into_iter()
                    .take(usize::from(limit.clamp(1, 10)))
                {
                    if !valid_id(&track.id) {
                        continue;
                    }
                    let artist_refs = track
                        .artists
                        .iter()
                        .map(|a| super::hit::ArtistRef {
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
                        track
                            .artists
                            .into_iter()
                            .map(|artist| artist.name)
                            .collect(),
                    )?;
                    hit.source = Some(SearchSource::YouTubeMusic);
                    hit.artist_refs = artist_refs;
                    hit.duration_ms = track.duration.map(|seconds| u64::from(seconds) * 1000);
                    hit.artwork_url = track
                        .cover
                        .into_iter()
                        .max_by_key(|cover| cover.width)
                        .and_then(|cover| {
                            crate::core::playback::track_info::artwork_url(&cover.url)
                        });
                    music_hits.push(hit);
                }
            }
            let mut video_hits = Vec::new();
            if let Some(result) = videos {
                for video in result
                    .items
                    .items
                    .into_iter()
                    .filter(|v| valid_id(&v.id) && !v.is_live && !v.is_upcoming)
                    .take(usize::from(limit.clamp(1, 10)))
                {
                    let artists = video
                        .channel
                        .as_ref()
                        .map(|c| vec![c.name.clone()])
                        .unwrap_or_default();
                    let mut hit = SearchHit::new(
                        ProviderId::YouTube,
                        video.id.clone(),
                        format!("https://www.youtube.com/watch?v={}", video.id),
                        video.name,
                        artists,
                    )?;
                    hit.source = Some(SearchSource::YouTube);
                    hit.artist_refs = video
                        .channel
                        .into_iter()
                        .map(|c| super::hit::ArtistRef {
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
                    video_hits.push(hit);
                }
            }
            Ok(combine(music_hits, video_hits, limit))
        })
    }
}

fn combine(music: Vec<SearchHit>, videos: Vec<SearchHit>, limit: u8) -> Vec<SearchHit> {
    let mut music = music.into_iter();
    let mut videos = videos.into_iter();
    let mut seen = std::collections::HashSet::new();
    let mut hits = Vec::new();
    loop {
        let pair = [music.next(), videos.next()];
        if pair.iter().all(Option::is_none) {
            break;
        }
        for hit in pair.into_iter().flatten() {
            if seen.insert(hit.provider_track_id().to_owned()) {
                hits.push(hit);
            }
            if hits.len() >= usize::from(limit.clamp(1, 10)) {
                return hits;
            }
        }
    }
    hits
}
#[cfg(test)]
mod tests {
    use super::*;
    fn hit(id: &str, source: SearchSource) -> SearchHit {
        let mut hit = SearchHit::new(
            ProviderId::YouTube,
            id.into(),
            format!("https://www.youtube.com/watch?v={id}"),
            "Same title".into(),
            vec![],
        )
        .unwrap();
        hit.source = Some(source);
        hit
    }
    #[test]
    fn combines_sources_by_rank_and_deduplicates_only_video_identity() {
        let hits = combine(
            vec![
                hit("r-G2pKr2_n8", SearchSource::YouTubeMusic),
                hit("aqz-KE-bpKQ", SearchSource::YouTubeMusic),
            ],
            vec![
                hit("r-G2pKr2_n8", SearchSource::YouTube),
                hit("abcdefghijk", SearchSource::YouTube),
            ],
            10,
        );
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].source(), Some(SearchSource::YouTubeMusic));
        assert_eq!(hits[2].source(), Some(SearchSource::YouTube));
    }
    #[test]
    fn either_source_can_supply_results_alone_and_limit_is_bounded() {
        assert_eq!(
            combine(vec![], vec![hit("abcdefghijk", SearchSource::YouTube)], 5).len(),
            1
        );
        assert_eq!(
            combine(
                vec![hit("r-G2pKr2_n8", SearchSource::YouTubeMusic)],
                vec![],
                5
            )
            .len(),
            1
        );
        assert_eq!(
            combine(
                vec![hit("r-G2pKr2_n8", SearchSource::YouTubeMusic)],
                vec![hit("abcdefghijk", SearchSource::YouTube)],
                1
            )
            .len(),
            1
        );
    }
}

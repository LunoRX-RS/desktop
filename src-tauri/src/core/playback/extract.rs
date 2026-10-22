//! YouTube resolution stays on the desktop; signed URLs never enter IPC or storage.
use super::audio::AudioQuality;
use crate::core::errors::{HostRejectKind, PlaybackErrorKind};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::AudioStream,
};
use std::time::Duration;

pub fn valid_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub(super) fn failed() -> PlaybackErrorKind {
    PlaybackErrorKind::HostRejected(HostRejectKind::Failed)
}
pub fn resolver() -> Result<RustyPipe, PlaybackErrorKind> {
    RustyPipe::builder()
        .no_storage()
        .no_reporter()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| failed())
}
fn best_audio(streams: &[AudioStream], quality: AudioQuality) -> Option<&AudioStream> {
    let eligible = || {
        streams
            .iter()
            .filter(|s| s.drm_track_type.is_none() && s.size > 0)
    };
    let default = eligible().any(|s| s.track.as_ref().is_none_or(|t| t.is_default));
    let candidates =
        || eligible().filter(|s| !default || s.track.as_ref().is_none_or(|t| t.is_default));
    candidates()
        .filter(|s| s.bitrate <= quality.bitrate_limit())
        .max_by_key(|s| s.bitrate)
        .or_else(|| candidates().min_by_key(|s| s.bitrate))
}
pub struct ResolvedAudio {
    pub url: String,
    pub size: u64,
}

pub async fn resolve(pipe: &RustyPipe, id: &str) -> Result<ResolvedAudio, PlaybackErrorKind> {
    resolve_with_quality(pipe, id, AudioQuality::High).await
}

pub async fn resolve_with_quality(
    pipe: &RustyPipe,
    id: &str,
    quality: AudioQuality,
) -> Result<ResolvedAudio, PlaybackErrorKind> {
    if !valid_id(id) {
        return Err(PlaybackErrorKind::Validation("invalid YouTube id"));
    }
    if let Ok(audio) = super::visionos::resolve(pipe, id, quality).await {
        return Ok(audio);
    }
    let query = pipe.query();
    let player = tokio::time::timeout(Duration::from_secs(40), async {
        if query.auth_enabled(ClientType::DesktopMusic) {
            // Stored cookies are sent only for explicitly authenticated queries.
            for client in [ClientType::DesktopMusic, ClientType::Desktop] {
                if let Ok(player) = query
                    .clone()
                    .authenticated()
                    .player_from_client(id, client)
                    .await
                {
                    return Ok(player);
                }
            }
        }
        query
            .player_from_clients(id, query.player_client_order())
            .await
    })
    .await
    .map_err(|_| PlaybackErrorKind::HostRejected(HostRejectKind::Timeout))?
    .map_err(|_| failed())?;
    let stream = best_audio(&player.audio_streams, quality).ok_or_else(failed)?;
    Ok(ResolvedAudio {
        url: checked_url(&stream.url)?,
        size: stream.size,
    })
}
pub(super) fn checked_url(raw: &str) -> Result<String, PlaybackErrorKind> {
    let url = reqwest::Url::parse(raw).map_err(|_| failed())?;
    if raw.len() > 16_384
        || raw.contains(['\n', '\r', '"', '\\'])
        || url.scheme() != "https"
        || !url
            .host_str()
            .is_some_and(|h| h.ends_with(".googlevideo.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(failed());
    }
    Ok(url.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_selects_bitrate_and_falls_back_when_limit_unavailable() {
        let streams: Vec<AudioStream> = [48_000, 96_000, 160_000]
            .into_iter()
            .map(|bitrate| {
                serde_json::from_value(serde_json::json!({
                    "url": "https://rr1.googlevideo.com/x", "itag": 1,
                    "bitrate": bitrate, "average_bitrate": bitrate, "size": 100,
                    "mime": "audio/webm", "format": "webm", "codec": "opus", "drm_systems": []
                }))
                .unwrap()
            })
            .collect();
        assert_eq!(
            best_audio(&streams, AudioQuality::Low).unwrap().bitrate,
            48_000
        );
        assert_eq!(
            best_audio(&streams, AudioQuality::Normal).unwrap().bitrate,
            96_000
        );
        assert_eq!(
            best_audio(&streams, AudioQuality::High).unwrap().bitrate,
            160_000
        );
        assert_eq!(
            best_audio(&streams[1..], AudioQuality::Low)
                .unwrap()
                .bitrate,
            96_000
        );
        assert!(best_audio(&[], AudioQuality::High).is_none());
        let mut unavailable = streams;
        unavailable.iter_mut().for_each(|s| s.size = 0);
        assert!(best_audio(&unavailable, AudioQuality::High).is_none());
    }
    #[test]
    fn ids_and_stream_hosts_are_restricted() {
        assert!(valid_id("a_B-9zZ0-_x"));
        for id in ["", "--exec=evil", "dQw4w9WgXcQ&x", "dQw4w9WgX Q"] {
            assert!(!valid_id(id));
        }
        assert!(checked_url("https://rr1.googlevideo.com/videoplayback?expire=123").is_ok());
        for url in [
            "http://rr1.googlevideo.com/x",
            "https://googlevideo.com.evil.test/x",
            "https://evil.test/x",
            "file:///etc/passwd",
            "https://u@rr1.googlevideo.com/x",
            "https://rr1.googlevideo.com:444/x",
            "https://rr1.googlevideo.com/x\nhttps://rr2.googlevideo.com/x",
        ] {
            assert!(checked_url(url).is_err(), "{url}");
        }
    }
}

//! Public-content direct client; iOS streams can be limited to their first MiB.
use super::{
    audio::AudioQuality,
    extract::{ResolvedAudio, checked_url, failed},
};
use crate::core::errors::PlaybackErrorKind;
use serde_json::{Value, json};
use std::time::Duration;

pub(super) async fn resolve(
    pipe: &rustypipe::client::RustyPipe,
    id: &str,
    quality: AudioQuality,
) -> Result<ResolvedAudio, PlaybackErrorKind> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| failed())?;
    let visitor_data = pipe
        .query()
        .get_visitor_data(false)
        .await
        .map_err(|_| failed())?;
    let mut response = client.post("https://music.youtube.com/youtubei/v1/player?prettyPrint=false")
        .header("X-Goog-Api-Format-Version", "1")
        .header("X-Origin", "https://music.youtube.com")
        .header("Referer", "https://music.youtube.com/")
        .header("X-Goog-Visitor-Id", &visitor_data)
        .header("X-YouTube-Client-Name", "101")
        .header("X-YouTube-Client-Version", "0.1")
        .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15")
        .json(&json!({
            "context": {"client": {
                "clientName": "VISIONOS", "clientVersion": "0.1", "visitorData": visitor_data,
                "osName": "visionOS", "osVersion": "1.3.21O771",
                "deviceMake": "Apple", "deviceModel": "RealityDevice14,1", "hl": "en", "gl": "US"
            }, "request": {"internalExperimentFlags": [], "useSsl": true}, "user": {"lockedSafetyMode": false}},
            "videoId": id, "contentCheckOk": true, "racyCheckOk": true
        }))
        .send().await.map_err(|_| failed())?
        .error_for_status().map_err(|_| failed())?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failed())? {
        if body.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(failed());
        }
        body.extend_from_slice(&chunk);
    }
    select(
        &serde_json::from_slice(&body).map_err(|_| failed())?,
        quality,
    )
}

fn select(player: &Value, quality: AudioQuality) -> Result<ResolvedAudio, PlaybackErrorKind> {
    if player
        .pointer("/playabilityStatus/status")
        .and_then(Value::as_str)
        != Some("OK")
    {
        return Err(failed());
    }
    let formats = player
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .ok_or_else(failed)?;
    let candidates: Vec<_> = formats
        .iter()
        .filter_map(|f| {
            if !f.get("mimeType")?.as_str()?.starts_with("audio/")
                || f.get("drmFamilies")
                    .and_then(Value::as_array)
                    .is_some_and(|v| !v.is_empty())
                || f.get("drmTrackType").is_some_and(|v| !v.is_null())
            {
                return None;
            }
            let size = f.get("contentLength")?.as_str()?.parse::<u64>().ok()?;
            let bitrate = u32::try_from(f.get("bitrate")?.as_u64()?).ok()?;
            let url = f.get("url")?.as_str()?;
            let default = f
                .pointer("/audioTrack/audioIsDefault")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            (size > 0 && bitrate > 0).then_some((bitrate, size, url, default))
        })
        .collect();
    let has_default = candidates.iter().any(|c| c.3);
    let eligible = || candidates.iter().filter(|c| !has_default || c.3);
    let selected = eligible()
        .filter(|c| c.0 <= quality.bitrate_limit())
        .max_by_key(|c| c.0)
        .or_else(|| eligible().min_by_key(|c| c.0))
        .ok_or_else(failed)?;
    Ok(ResolvedAudio {
        url: checked_url(selected.2)?,
        size: selected.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_preserves_quality_default_track_and_url_validation() {
        let mut player = json!({"playabilityStatus":{"status":"OK"},"streamingData":{"adaptiveFormats":[
            {"mimeType":"audio/mp4","url":"https://rr1.googlevideo.com/low","bitrate":48000,"contentLength":"100"},
            {"mimeType":"audio/mp4","url":"https://rr1.googlevideo.com/high","bitrate":160000,"contentLength":"200"},
            {"mimeType":"audio/mp4","url":"https://rr1.googlevideo.com/dub","bitrate":256000,"contentLength":"300","audioTrack":{"audioIsDefault":false}}
        ]}});
        assert!(
            select(&player, AudioQuality::Low)
                .unwrap()
                .url
                .ends_with("/low")
        );
        assert!(
            select(&player, AudioQuality::High)
                .unwrap()
                .url
                .ends_with("/high")
        );
        player["streamingData"]["adaptiveFormats"][1]["url"] = json!("https://evil.test/audio");
        assert!(select(&player, AudioQuality::High).is_err());
        player["playabilityStatus"]["status"] = json!("LOGIN_REQUIRED");
        assert!(select(&player, AudioQuality::Low).is_err());
    }
}

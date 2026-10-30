//! Followed artists on the LunoRX account. The app keeps its own list; these commands push it to the server and read
//! the server's back. The account token never leaves Rust.
use crate::sync::api::FollowedArtist;
use crate::sync::engine::{self, ErrorCategory, Host};
use tauri::{State, WebviewWindow};

use super::sync::SyncEngineState;

fn main(window: &WebviewWindow) -> Result<(), ErrorCategory> {
    (window.label() == "main")
        .then_some(())
        .ok_or(ErrorCategory::Validation)
}

fn valid(a: &FollowedArtist) -> bool {
    let text = |s: &str, max: usize| {
        (1..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
    };
    matches!(a.provider.as_str(), "YouTube" | "Spotify")
        && text(&a.artist_key, 200)
        && a.artist_key.trim() == a.artist_key
        && text(&a.name, 200)
        && a.artist_id
            .as_deref()
            .is_none_or(|id| (1..=128).contains(&id.len()))
        && a.artwork_url.as_deref().is_none_or(|u| u.len() <= 2048)
}

/// Sends this device's unfollows and follows, then returns the account's full list.
#[tauri::command]
pub async fn artists_cloud_sync(
    window: WebviewWindow,
    sync: State<'_, SyncEngineState>,
    local: Vec<FollowedArtist>,
    removed: Vec<(String, String)>,
) -> Result<Vec<FollowedArtist>, ErrorCategory> {
    main(&window)?;
    if local.len() > 5_000 || removed.len() > 5_000 || !local.iter().all(valid) {
        return Err(ErrorCategory::Validation);
    }
    let _net = sync.net.lock().await;
    engine::artists_sync(&mut Host::Shared(&sync), &local, &removed).await
}

/// Follows or unfollows one artist on the account straight away.
#[tauri::command]
pub async fn artists_cloud_follow(
    window: WebviewWindow,
    sync: State<'_, SyncEngineState>,
    artist: FollowedArtist,
    follow: bool,
) -> Result<(), ErrorCategory> {
    main(&window)?;
    if !valid(&artist) {
        return Err(ErrorCategory::Validation);
    }
    let _net = sync.net.lock().await;
    engine::artist_follow(&mut Host::Shared(&sync), &artist, follow).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(key: &str) -> FollowedArtist {
        FollowedArtist {
            provider: "YouTube".into(),
            artist_key: key.into(),
            artist_id: None,
            name: "N".into(),
            artwork_url: None,
            followed_at_ms: 0,
        }
    }

    #[test]
    fn artist_input_is_checked() {
        assert!(valid(&a("UCxyz")));
        assert!(valid(&a("Some Artist")));
        assert!(!valid(&a(" padded")));
        assert!(!valid(&a("")));
        assert!(!valid(&a("line\nbreak")));
        assert!(!valid(&FollowedArtist {
            provider: "Tidal".into(),
            ..a("x")
        }));
    }
}

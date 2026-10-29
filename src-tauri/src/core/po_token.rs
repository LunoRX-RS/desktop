//! Holds the one proof-of-origin token the app's own signed-in browser view handed over. This app never makes a token:
//! it only keeps what the user's real session produced, for one video, for a short time, and gives it to `innerlight`
//! when that video is resolved. Nothing here logs, persists or prints the value.
use innerlight::player::PoTokenSource;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

/// A token is only valid for a short while; holding one longer than this would just send a stale value.
pub const MAX_TTL: Duration = Duration::from_secs(10 * 60);

struct Held {
    video_id: String,
    token: String,
    expires: Instant,
}

#[derive(Default)]
pub struct PoTokenStore {
    held: Mutex<Option<Held>>,
}

impl std::fmt::Debug for PoTokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PoTokenStore(..)")
    }
}

/// Why a token was not kept. Carries no part of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoTokenError {
    InvalidVideoId,
    InvalidToken,
}

impl PoTokenStore {
    /// Keeps `token` for `video_id`, replacing any earlier one. `ttl` is capped at [`MAX_TTL`].
    pub fn set(&self, video_id: &str, token: &str, ttl: Duration) -> Result<(), PoTokenError> {
        let id_ok = video_id.len() == 11
            && video_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !id_ok {
            return Err(PoTokenError::InvalidVideoId);
        }
        let token_ok = (16..=4096).contains(&token.len())
            && token.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'=' | b'+' | b'/')
            });
        if !token_ok {
            return Err(PoTokenError::InvalidToken);
        }
        let held = Held {
            video_id: video_id.to_owned(),
            token: token.to_owned(),
            expires: Instant::now() + ttl.min(MAX_TTL),
        };
        *self.held.lock().unwrap_or_else(|e| e.into_inner()) = Some(held);
        Ok(())
    }

    /// True while a live token for `video_id` is held.
    pub fn token_for_check(&self, video_id: &str) -> bool {
        self.token_for(video_id).is_some()
    }

    pub fn clear(&self) {
        *self.held.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl PoTokenSource for PoTokenStore {
    /// Only the video the token was made for, and only until it expires. A token for another video is never sent.
    fn token_for(&self, video_id: &str) -> Option<String> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        match held.as_ref() {
            Some(h) if h.expires <= Instant::now() => {
                *held = None;
                None
            }
            Some(h) if h.video_id == video_id => Some(h.token.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "dQw4w9WgXcQ";
    const TOKEN: &str = "MnQx_synthetic-token-AAAA-BBBB_cccc=";

    #[test]
    fn a_token_is_given_back_only_for_its_own_video() {
        let s = PoTokenStore::default();
        s.set(ID, TOKEN, Duration::from_secs(60)).unwrap();
        assert_eq!(s.token_for(ID).as_deref(), Some(TOKEN));
        assert_eq!(s.token_for("jNQXAC9IVRw"), None);
    }

    #[test]
    fn an_expired_token_is_dropped_not_sent() {
        let s = PoTokenStore::default();
        s.set(ID, TOKEN, Duration::ZERO).unwrap();
        assert_eq!(s.token_for(ID), None);
        s.set(ID, TOKEN, Duration::from_secs(60)).unwrap();
        s.clear();
        assert_eq!(s.token_for(ID), None);
    }

    #[test]
    fn bad_input_is_refused_without_echoing_it() {
        let s = PoTokenStore::default();
        assert_eq!(
            s.set("short", TOKEN, MAX_TTL),
            Err(PoTokenError::InvalidVideoId)
        );
        assert_eq!(s.set(ID, "tiny", MAX_TTL), Err(PoTokenError::InvalidToken));
        assert_eq!(
            s.set(ID, "has space 0123456789", MAX_TTL),
            Err(PoTokenError::InvalidToken)
        );
        assert_eq!(s.token_for(ID), None);
        s.set(ID, TOKEN, MAX_TTL).unwrap();
        assert!(!format!("{s:?}").contains(TOKEN));
    }

    #[test]
    fn the_ttl_is_capped() {
        let s = PoTokenStore::default();
        s.set(ID, TOKEN, Duration::from_secs(86_400)).unwrap();
        let expires = s.held.lock().unwrap().as_ref().unwrap().expires;
        assert!(expires <= Instant::now() + MAX_TTL);
    }
}

//! Plays YouTube through its own real page in an app-owned window, for when the direct stream is refused. The page
//! is the provider's own player, ads included; the window is not shown. The window has no Tauri IPC: it reports state
//! through its title and takes play/pause/seek/volume as plain script calls. Only the player's own reports settle
//! playback state.
use crate::core::{
    domain::ProviderId,
    errors::{HostRejectKind, PlaybackErrorKind},
    playback::{
        audio::{AudioControl, Effects},
        command::PlaybackCommand,
        host::{HostFuture, HostOutcome, HostRequest, PlayerEvent, PlayerHost},
        native::{NativeHost, NativeReport},
    },
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, webview::NewWindowResponse,
};
use tokio::sync::{Notify, mpsc::UnboundedSender};

const WINDOW: &str = "youtube-player";
const PREFIX: &str = "LUNORX_ST|";
const SCRIPT: &str = include_str!("page_player.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageState {
    Playing,
    Paused,
    Waiting,
    Ended,
    Error,
}

/// `LUNORX_ST|<video id>|<position ms>|<duration ms>|<state>` from the page title.
fn parse_title(title: &str) -> Option<(&str, u64, u64, PageState)> {
    let mut p = title.strip_prefix(PREFIX)?.split('|');
    let id = p.next()?;
    let pos = p.next()?.parse().ok()?;
    let dur = p.next()?.parse().ok()?;
    let state = match p.next()? {
        "playing" => PageState::Playing,
        "paused" => PageState::Paused,
        "waiting" => PageState::Waiting,
        "ended" => PageState::Ended,
        "error" => PageState::Error,
        _ => return None,
    };
    Some((id, pos, dur, state))
}

#[derive(Default)]
struct Shared {
    expected: String,
    /// Between `Load` and its confirmation the page may autoplay; those reports must not reach the engine.
    loading: bool,
    seen: Option<PageState>,
    ready: bool,
}

pub struct PageHost {
    app: AppHandle,
    reports: UnboundedSender<NativeReport>,
    revision: Arc<AtomicU64>,
    shared: Arc<Mutex<Shared>>,
    ready: Arc<Notify>,
    /// Set when a new page loads, so the sound settings are sent to it again.
    resend: Arc<std::sync::atomic::AtomicBool>,
}

impl PageHost {
    pub fn new(
        app: &AppHandle,
        reports: UnboundedSender<NativeReport>,
        audio: AudioControl,
    ) -> Self {
        let resend = Arc::new(std::sync::atomic::AtomicBool::new(true));
        // The page applies the same sound settings the native player does (speed and pitch, equaliser, 8D/4D, reverb...).
        let (watch_app, watch_resend) = (app.clone(), resend.clone());
        tauri::async_runtime::spawn(async move {
            let mut sent: Option<Effects> = None;
            loop {
                tokio::time::sleep(Duration::from_millis(400)).await;
                let now = audio.settings().effects;
                let again = watch_resend.swap(false, Ordering::Relaxed);
                if again || sent != Some(now) {
                    if let Some(w) = watch_app.get_webview_window(WINDOW)
                        && let Ok(json) = serde_json::to_string(&now)
                        && w.eval(format!("window.__lunorx&&window.__lunorx.effects({json})"))
                            .is_ok()
                    {
                        sent = Some(now);
                    } else if again {
                        watch_resend.store(true, Ordering::Relaxed);
                    }
                }
            }
        });
        Self {
            app: app.clone(),
            reports,
            revision: Arc::new(AtomicU64::new(0)),
            shared: Arc::default(),
            ready: Arc::default(),
            resend,
        }
    }

    fn window(&self, first_url: &tauri::Url) -> Result<WebviewWindow, PlaybackErrorKind> {
        if let Some(w) = self.app.get_webview_window(WINDOW) {
            return Ok(w);
        }
        let (reports, revision, shared, ready) = (
            self.reports.clone(),
            self.revision.clone(),
            self.shared.clone(),
            self.ready.clone(),
        );
        WebviewWindowBuilder::new(&self.app, WINDOW, WebviewUrl::External(first_url.clone()))
            .title("LunoRX: YouTube player")
            .inner_size(480.0, 300.0)
            // Kept out of sight at the user's request: the page still runs and plays its own player, ads included.
            .visible(false)
            // A hidden WebView is otherwise suspended by the OS after a few minutes, which would stop the music.
            .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled)
            .initialization_script(SCRIPT)
            .on_navigation(|u| {
                u.scheme() == "https"
                    && matches!(
                        u.host_str(),
                        Some(
                            "www.youtube.com"
                                | "youtube.com"
                                | "consent.youtube.com"
                                | "accounts.google.com"
                                | "www.google.com"
                        )
                    )
            })
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .on_document_title_changed(move |_, title| {
                let Some((id, pos, dur, state)) = parse_title(&title) else {
                    return;
                };
                let mut s = lock(&shared);
                if id != s.expected {
                    return;
                }
                let changed = s.seen != Some(state);
                s.seen = Some(state);
                if s.loading {
                    s.ready = true;
                    drop(s);
                    ready.notify_waiters();
                    return;
                }
                drop(s);
                let event = if changed { event_for(state) } else { None };
                let _ = reports.send(NativeReport {
                    revision: revision.load(Ordering::Relaxed),
                    event,
                    position_ms: pos,
                    duration_ms: dur,
                });
            })
            .build()
            .map_err(|_| failed())
    }

    async fn load(&self, video_id: &str) -> Result<(), PlaybackErrorKind> {
        let url: tauri::Url = format!("https://www.youtube.com/watch?v={video_id}")
            .parse()
            .map_err(|_| failed())?;
        {
            let mut s = lock(&self.shared);
            *s = Shared {
                expected: video_id.to_owned(),
                loading: true,
                seen: None,
                ready: false,
            };
        }
        let existed = self.app.get_webview_window(WINDOW).is_some();
        let window = self.window(&url)?;
        if existed {
            window.navigate(url).map_err(|_| failed())?;
        }
        let waited = tokio::time::timeout(Duration::from_secs(25), async {
            loop {
                let notified = self.ready.notified();
                if lock(&self.shared).ready {
                    return;
                }
                notified.await;
            }
        })
        .await;
        if waited.is_err() {
            return Err(PlaybackErrorKind::HostRejected(HostRejectKind::Timeout));
        }
        // Loaded: hold it paused, as the engine expects, then let reports through.
        let _ = window.eval("window.__lunorx&&window.__lunorx.pause()");
        lock(&self.shared).loading = false;
        self.resend.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn script(&self, js: &str) -> Result<(), PlaybackErrorKind> {
        let w = self.app.get_webview_window(WINDOW).ok_or_else(failed)?;
        w.eval(js).map_err(|_| failed())
    }

    fn stop(&self) {
        if let Some(w) = self.app.get_webview_window(WINDOW) {
            let _ = w.eval("window.__lunorx&&window.__lunorx.pause()");
            // Destroyed at once, not asked to close: a later load reuses the label straight away.
            let _ = w.destroy();
        }
    }
}

/// A poisoned lock still holds consistent data here (plain flags and numbers), so keep going with it.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn failed() -> PlaybackErrorKind {
    PlaybackErrorKind::HostRejected(HostRejectKind::Failed)
}
/// What a change to `state` tells the engine. A pause is the app's own doing, so it reports nothing.
fn event_for(state: PageState) -> Option<PlayerEvent> {
    match state {
        PageState::Playing => Some(PlayerEvent::Playing),
        PageState::Waiting => Some(PlayerEvent::Buffering),
        PageState::Ended => Some(PlayerEvent::Ended),
        PageState::Error => Some(PlayerEvent::Failed),
        PageState::Paused => None,
    }
}

impl PlayerHost for PageHost {
    fn provider(&self) -> ProviderId {
        ProviderId::YouTube
    }
    fn prepare<'a>(&'a self, req: HostRequest) -> HostFuture<'a> {
        Box::pin(async move {
            self.revision.store(req.revision, Ordering::Relaxed);
            let result = match &req.command {
                PlaybackCommand::Load(track) => self.load(&track.provider_track_id).await,
                PlaybackCommand::Play | PlaybackCommand::Resume => {
                    self.script("window.__lunorx&&window.__lunorx.play()")
                }
                PlaybackCommand::Pause => self.script("window.__lunorx&&window.__lunorx.pause()"),
                PlaybackCommand::Seek { position_ms } => self.script(&format!(
                    "window.__lunorx&&window.__lunorx.seek({position_ms})"
                )),
                PlaybackCommand::SetVolume(v) => self.script(&format!(
                    "window.__lunorx&&window.__lunorx.volume({})",
                    v.value()
                )),
                PlaybackCommand::Stop => {
                    self.stop();
                    Ok(())
                }
                _ => Err(failed()),
            };
            match result {
                Ok(()) => HostOutcome::Confirmed {
                    request_id: req.request_id,
                    revision: req.revision,
                },
                Err(kind) => HostOutcome::Rejected {
                    request_id: req.request_id,
                    revision: req.revision,
                    kind,
                },
            }
        })
    }
}

/// Tries the direct native player first. Only when it refuses a load does the YouTube page play the track, and every
/// later command goes to whichever one holds the track.
pub struct RoutingHost {
    native: NativeHost,
    page: PageHost,
    on_page: Mutex<bool>,
}

impl RoutingHost {
    pub fn new(native: NativeHost, page: PageHost) -> Self {
        Self {
            native,
            page,
            on_page: Mutex::new(false),
        }
    }
}

impl PlayerHost for RoutingHost {
    fn provider(&self) -> ProviderId {
        ProviderId::YouTube
    }
    fn prepare<'a>(&'a self, req: HostRequest) -> HostFuture<'a> {
        Box::pin(async move {
            let on_page = *lock(&self.on_page);
            match &req.command {
                PlaybackCommand::Load(_) => {
                    // A new load always starts from the direct player; leave the page paused and closed.
                    if on_page {
                        self.page.stop();
                    }
                    let retry = HostRequest {
                        request_id: req.request_id,
                        revision: req.revision,
                        command: req.command.clone(),
                    };
                    match self.native.prepare(req).await {
                        confirmed @ HostOutcome::Confirmed { .. } => {
                            *lock(&self.on_page) = false;
                            confirmed
                        }
                        HostOutcome::Rejected { .. } => {
                            let outcome = self.page.prepare(retry).await;
                            *lock(&self.on_page) = matches!(outcome, HostOutcome::Confirmed { .. });
                            outcome
                        }
                    }
                }
                PlaybackCommand::Stop => {
                    self.page.stop();
                    *lock(&self.on_page) = false;
                    self.native.prepare(req).await
                }
                _ if on_page => self.page.prepare(req).await,
                _ => self.native.prepare(req).await,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_channel_parses_and_ignores_everything_else() {
        assert_eq!(
            parse_title("LUNORX_ST|dQw4w9WgXcQ|1500|212000|playing"),
            Some(("dQw4w9WgXcQ", 1500, 212_000, PageState::Playing))
        );
        assert_eq!(
            parse_title("LUNORX_ST|dQw4w9WgXcQ|0|0|waiting").map(|t| t.3),
            Some(PageState::Waiting)
        );
        assert_eq!(parse_title("Never Gonna Give You Up - YouTube"), None);
        assert_eq!(parse_title("LUNORX_ST|id|x|0|playing"), None);
        assert_eq!(parse_title("LUNORX_ST|id|0|0|bogus"), None);
    }

    #[test]
    fn only_real_changes_become_engine_events() {
        assert_eq!(event_for(PageState::Playing), Some(PlayerEvent::Playing));
        assert_eq!(event_for(PageState::Ended), Some(PlayerEvent::Ended));
        assert_eq!(event_for(PageState::Paused), None);
    }
}

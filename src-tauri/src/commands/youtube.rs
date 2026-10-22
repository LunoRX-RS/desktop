//! App-owned Google sign-in window. No remote IPC grants, password interception or OAuth flow.
use crate::{commands::providers::KeyringKeyStore, sync::credentials::TokenStore};
use rustypipe::client::RustyPipe;
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
    webview::{Cookie, NewWindowResponse, PageLoadEvent},
};

const LOGIN_WINDOW: &str = "youtube-sign-in";
const EVENT: &str = "core://youtube/connection";

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionStatus {
    Restoring,
    Disconnected,
    Connecting,
    Connected,
    Expired,
    StoreError,
    Unavailable,
}

pub struct YouTubeState {
    pub pipe: Option<RustyPipe>,
    store: KeyringKeyStore,
    status: Mutex<ConnectionStatus>,
    operation: tokio::sync::Mutex<()>,
}
impl YouTubeState {
    pub fn new(pipe: Option<RustyPipe>) -> Self {
        let status = if pipe.is_some() {
            ConnectionStatus::Restoring
        } else {
            ConnectionStatus::Unavailable
        };
        Self {
            pipe,
            store: KeyringKeyStore::youtube_session(),
            status: Mutex::new(status),
            operation: tokio::sync::Mutex::new(()),
        }
    }
    fn status(&self) -> ConnectionStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn idle_status(&self) -> ConnectionStatus {
        if self.pipe.as_ref().is_some_and(|pipe| {
            pipe.query()
                .auth_enabled(rustypipe::client::ClientType::DesktopMusic)
        }) {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        }
    }
    fn publish(&self, app: &AppHandle, status: ConnectionStatus) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
        let _ = app.emit_to("main", EVENT, status);
    }
    pub async fn restore(app: AppHandle) {
        let state = app.state::<Self>();
        let _guard = state.operation.lock().await;
        let Some(pipe) = &state.pipe else {
            return;
        };
        let status = match state.store.get() {
            Ok(Some(cookie)) => match tokio::time::timeout(
                Duration::from_secs(20),
                pipe.user_auth_set_cookie(cookie),
            )
            .await
            {
                Ok(Ok(())) => ConnectionStatus::Connected,
                _ => ConnectionStatus::Expired,
            },
            Ok(None) => ConnectionStatus::Disconnected,
            Err(_) => ConnectionStatus::StoreError,
        };
        state.publish(&app, status);
    }
}

#[derive(Serialize)]
#[serde(tag = "category", rename_all = "camelCase")]
pub enum YouTubeError {
    Unavailable,
    WindowFailed,
    SignInIncomplete,
    SessionRejected,
    StoreAccess,
    Forbidden,
}

fn main_only(window: &WebviewWindow) -> Result<(), YouTubeError> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err(YouTubeError::Forbidden)
    }
}
fn youtube_origin(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("www.youtube.com" | "music.youtube.com" | "youtube.com")
        )
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
}
fn login_navigation(url: &tauri::Url) -> bool {
    youtube_origin(url)
        || (url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(
                url.host_str(),
                Some(
                    "accounts.google.com"
                        | "accounts.youtube.com"
                        | "consent.google.com"
                        | "consent.youtube.com"
                        | "myaccount.google.com"
                )
            ))
}

fn session_cookie(cookies: &[Cookie<'_>]) -> Option<String> {
    let allowed = [
        "SAPISID",
        "APISID",
        "SID",
        "HSID",
        "SSID",
        "__Secure-1PAPISID",
        "__Secure-3PAPISID",
        "__Secure-1PSID",
        "__Secure-3PSID",
        "__Secure-1PSIDTS",
        "__Secure-3PSIDTS",
        "LOGIN_INFO",
    ];
    let mut selected = std::collections::BTreeMap::new();
    for cookie in cookies {
        let domain = cookie.domain().unwrap_or("").trim_start_matches('.');
        if !matches!(
            domain,
            "youtube.com" | "www.youtube.com" | "music.youtube.com"
        ) || !allowed.contains(&cookie.name())
        {
            continue;
        }
        let value = cookie.value();
        if value.is_empty()
            || value.len() > 8192
            || !value
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) && b != b';')
        {
            continue;
        }
        selected.insert(cookie.name(), value);
    }
    if !["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"]
        .iter()
        .any(|key| selected.contains_key(key))
    {
        return None;
    }
    let header = selected
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    (header.len() <= 32_768).then_some(header)
}

async fn capture(app: &AppHandle) -> Result<ConnectionStatus, YouTubeError> {
    let state = app.state::<YouTubeState>();
    let _guard = state.operation.lock().await;
    if state.status() == ConnectionStatus::Connected {
        return Ok(ConnectionStatus::Connected);
    }
    if state.status() != ConnectionStatus::Connecting {
        return Err(YouTubeError::SignInIncomplete);
    }
    let window = app
        .get_webview_window(LOGIN_WINDOW)
        .ok_or(YouTubeError::SignInIncomplete)?;
    if !youtube_origin(&window.url().map_err(|_| YouTubeError::WindowFailed)?) {
        return Err(YouTubeError::SignInIncomplete);
    }
    let cookies = window
        .cookies_for_url("https://music.youtube.com/".parse().expect("fixed URL"))
        .map_err(|_| YouTubeError::WindowFailed)?;
    let cookie = session_cookie(&cookies).ok_or(YouTubeError::SignInIncomplete)?;
    let pipe = state.pipe.as_ref().ok_or(YouTubeError::Unavailable)?;
    if !matches!(
        tokio::time::timeout(
            Duration::from_secs(20),
            pipe.user_auth_set_cookie(cookie.clone())
        )
        .await,
        Ok(Ok(()))
    ) {
        return Err(YouTubeError::SessionRejected);
    }
    if state.store.set(&cookie).is_err() {
        let _ = pipe.user_auth_remove_cookie().await;
        state.publish(app, ConnectionStatus::StoreError);
        return Err(YouTubeError::StoreAccess);
    }
    state.publish(app, ConnectionStatus::Connected);
    let _ = window.close();
    Ok(ConnectionStatus::Connected)
}

#[tauri::command]
pub async fn youtube_connect(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, YouTubeState>,
) -> Result<ConnectionStatus, YouTubeError> {
    main_only(&window)?;
    let _guard = state.operation.lock().await;
    if state.pipe.is_none() {
        return Err(YouTubeError::Unavailable);
    }
    state.publish(&app, ConnectionStatus::Connecting);
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        existing
            .set_focus()
            .map_err(|_| YouTubeError::WindowFailed)?;
        return Ok(ConnectionStatus::Connecting);
    }
    let login = WebviewWindowBuilder::new(&app, LOGIN_WINDOW, WebviewUrl::External(
        "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F".parse().expect("fixed URL")))
        .title("Sign in to YouTube — Google").inner_size(520.0, 720.0).incognito(true)
        .on_navigation(login_navigation)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished && youtube_origin(payload.url()) {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move { let _ = capture(&app).await; });
            }
        }).build().map_err(|_| { state.publish(&app, state.idle_status()); YouTubeError::WindowFailed })?;
    let handle = app.clone();
    login.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let app = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<YouTubeState>();
                let _guard = state.operation.lock().await;
                if state.status() == ConnectionStatus::Connecting {
                    state.publish(&app, state.idle_status());
                }
            });
        }
    });
    Ok(ConnectionStatus::Connecting)
}

#[tauri::command]
pub async fn youtube_connect_finish(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<ConnectionStatus, YouTubeError> {
    main_only(&window)?;
    capture(&app).await
}

#[tauri::command]
pub async fn youtube_connection_status(
    window: WebviewWindow,
    state: State<'_, YouTubeState>,
) -> Result<ConnectionStatus, YouTubeError> {
    main_only(&window)?;
    Ok(state.status())
}

#[tauri::command]
pub async fn youtube_disconnect(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, YouTubeState>,
) -> Result<ConnectionStatus, YouTubeError> {
    main_only(&window)?;
    let _guard = state.operation.lock().await;
    if let Some(login) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = login.close();
    }
    if let Some(pipe) = &state.pipe {
        let _ = pipe.user_auth_remove_cookie().await;
    }
    if state.store.clear().is_err() {
        state.publish(&app, ConnectionStatus::StoreError);
        return Err(YouTubeError::StoreAccess);
    }
    let _ = crate::commands::player::player_stop(
        app.clone(),
        app.state::<crate::commands::player::PlayerState>(),
    )
    .await;
    state.publish(&app, ConnectionStatus::Disconnected);
    Ok(ConnectionStatus::Disconnected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_youtube_session_cookies_are_selected() {
        let cookies = [
            Cookie::build(("SAPISID", "SECRET"))
                .domain(".youtube.com")
                .build(),
            Cookie::build(("SID", "GOOGLE_SECRET"))
                .domain(".google.com")
                .build(),
            Cookie::build(("arbitrary", "OTHER_SECRET"))
                .domain(".youtube.com")
                .build(),
        ];
        assert_eq!(session_cookie(&cookies).as_deref(), Some("SAPISID=SECRET"));
        assert!(session_cookie(&cookies[1..]).is_none());
        let bad = [Cookie::build(("SAPISID", "bad\r\nheader"))
            .domain(".youtube.com")
            .build()];
        assert!(session_cookie(&bad).is_none());
    }
    #[test]
    fn login_navigation_has_exact_https_host_allowlist() {
        for url in [
            "https://accounts.google.com/signin",
            "https://music.youtube.com/",
        ] {
            assert!(login_navigation(&url.parse().unwrap()));
        }
        for url in [
            "http://accounts.google.com/",
            "https://accounts.google.com.evil.test/",
            "https://evil.test/",
            "file:///tmp/anything",
            "https://u@music.youtube.com/",
        ] {
            assert!(!login_navigation(&url.parse().unwrap()));
        }
    }
}

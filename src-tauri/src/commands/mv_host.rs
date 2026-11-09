//! The loopback origin that serves the music-video host page. Set up once at start-up; the frontend asks for it.
use tauri::{State, WebviewWindow};

pub struct MvHostState(pub Option<u16>);

/// `http://localhost:<port>` for the music-video frame, or None when the local server did not start.
#[tauri::command]
pub fn mv_host_origin(window: WebviewWindow, state: State<'_, MvHostState>) -> Option<String> {
    if window.label() != "main" {
        return None;
    }
    state.0.map(|port| format!("http://localhost:{port}"))
}

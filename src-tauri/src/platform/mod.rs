//! OS integration. macOS: system Now Playing, hardware media keys and the Playback menu. Linux: MPRIS (media widgets,
//! `playerctl`, media keys). Every desktop OS also gets a tray icon with playback controls (`tray`).

pub mod discord;
pub mod fonts;
pub mod mcp;

mod control;
#[cfg(target_os = "macos")]
mod dark_icon;
#[cfg(target_os = "macos")]
mod macos_media;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(target_os = "linux")]
mod mpris;
#[cfg(target_os = "macos")]
mod shared_state;
#[cfg(target_os = "macos")]
mod sound_menu;
mod tray;

/// Wires media keys, Now Playing and the Playback menu. Call from `setup`, on the main thread, after
/// `PlayerState` is managed. A failure is logged once and never retried.
#[cfg(target_os = "macos")]
pub fn install(app: &tauri::AppHandle) {
    dark_icon::install();
    let menu = menu::install(app);
    sound_menu::install(app);
    macos_media::install(app, menu);
    tray::install(app);
}

/// A track's lyrics were loaded: shares the timed lines with the macOS Widget and Screen Saver. No-op elsewhere.
#[cfg(target_os = "macos")]
pub fn note_lyrics(info: &crate::core::playback::track_info::TrackInfo) {
    shared_state::note_lyrics(info);
    macos_media::poke();
}

#[cfg(not(target_os = "macos"))]
pub fn note_lyrics(_info: &crate::core::playback::track_info::TrackInfo) {}

/// The sound settings changed (from the settings panel): refresh the Sound menu's checks.
#[cfg(target_os = "macos")]
pub fn sound_changed(app: &tauri::AppHandle) {
    sound_menu::refresh(app);
}

#[cfg(not(target_os = "macos"))]
pub fn sound_changed(_app: &tauri::AppHandle) {}

/// Tells the OS integration that player state, position or volume may have changed. Cheap and
/// non-blocking; safe to call from any thread.
#[cfg(target_os = "macos")]
pub fn poke() {
    macos_media::poke();
    tray::poke();
}

#[cfg(target_os = "linux")]
pub fn install(app: &tauri::AppHandle) {
    mpris::install(app);
    tray::install(app);
}

#[cfg(target_os = "linux")]
pub fn poke() {
    mpris::poke();
    tray::poke();
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install(app: &tauri::AppHandle) {
    tray::install(app);
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn poke() {
    tray::poke();
}

/// WebKitGTK on a Wayland compositor schedules frames from the compositor's frame callbacks. On hybrid-GPU laptops
/// (Intel plus NVIDIA, outputs split across the two cards) those callbacks stop arriving for the app window, so the page
/// stops repainting while its scripts keep running: the lyrics stay still, the time stays put and clicks seem dead.
/// `WEBKIT_FORCE_VBLANK_TIMER` makes WebKit pace frames with its own timer instead, and keeps GPU compositing. The user's own
/// setting wins. Call this first thing in `main`, before any thread exists, because it edits the process environment.
pub fn prepare_webview_environment() {
    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
        if wayland && std::env::var_os("WEBKIT_FORCE_VBLANK_TIMER").is_none() {
            // SAFETY: called from `run` before Tauri or any other thread starts, so nothing reads the environment concurrently.
            unsafe { std::env::set_var("WEBKIT_FORCE_VBLANK_TIMER", "1") };
        }
    }
}

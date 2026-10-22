//! OS integration. macOS only: system Now Playing, hardware media keys and the Playback menu.
//! Every other platform gets two empty functions, so callers need no cfg and behave as before.

pub mod discord;
pub mod fonts;

#[cfg(target_os = "macos")]
mod control;
#[cfg(target_os = "macos")]
mod macos_media;
#[cfg(target_os = "macos")]
mod menu;

/// Wires media keys, Now Playing and the Playback menu. Call from `setup`, on the main thread, after
/// `PlayerState` is managed. A failure is logged once and never retried.
#[cfg(target_os = "macos")]
pub fn install(app: &tauri::AppHandle) {
    macos_media::set_app_icon();
    let menu = menu::install(app);
    macos_media::install(app, menu);
}

/// Tells the OS integration that player state, position or volume may have changed. Cheap and
/// non-blocking; safe to call from any thread.
#[cfg(target_os = "macos")]
pub fn poke() {
    macos_media::poke();
}

#[cfg(not(target_os = "macos"))]
pub fn install(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn poke() {}

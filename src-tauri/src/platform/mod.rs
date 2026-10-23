//! OS integration. macOS: system Now Playing, hardware media keys and the Playback menu. Linux: MPRIS (media widgets,
//! `playerctl`, media keys). Every other platform gets two empty functions, so callers need no cfg and behave as before.

pub mod discord;
pub mod fonts;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod control;
#[cfg(target_os = "macos")]
mod macos_media;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(target_os = "linux")]
mod mpris;

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

#[cfg(target_os = "linux")]
pub fn install(app: &tauri::AppHandle) {
    mpris::install(app);
}

#[cfg(target_os = "linux")]
pub fn poke() {
    mpris::poke();
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install(_app: &tauri::AppHandle) {}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn poke() {}

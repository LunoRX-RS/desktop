//! Launch-at-login, backed by `tauri-plugin-autostart`. The OS login item is the source of truth, so there is no
//! settings file: `get` reads back whether the app is registered, `set` registers or unregisters it. macOS only in the
//! UI, but the plugin compiles on every platform.

use tauri::WebviewWindow;
use tauri_plugin_autostart::ManagerExt;

fn main(window: &WebviewWindow) -> Result<(), &'static str> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("forbidden")
    }
}

/// Whether the app is registered to open at login.
#[tauri::command]
pub fn autostart_get(window: WebviewWindow) -> Result<bool, &'static str> {
    main(&window)?;
    window
        .autolaunch()
        .is_enabled()
        .map_err(|_| "autostart unavailable")
}

/// Registers or unregisters the app as a login item. Fails when the OS refuses (e.g. an unbundled dev run).
#[tauri::command]
pub fn autostart_set(window: WebviewWindow, enabled: bool) -> Result<(), &'static str> {
    main(&window)?;
    let autolaunch = window.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|_| "autostart unavailable")
}

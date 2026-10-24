//! A tray icon on every desktop OS: the notification area on Windows, the menu bar on macOS and the indicator area on
//! Linux. Its menu plays, pauses and skips through the same path as the media keys (`control::run`), shows what is
//! playing, brings the window back and quits. Left click shows the window where the OS reports tray clicks (Windows,
//! macOS); on Linux the menu is the way in. If the OS has no tray, setup logs once and the app runs without it.

use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use super::control::{self, Remote};
use crate::commands::player::{PlaybackStateDto, PlayerState, progress};

static CHANGED: Notify = Notify::const_new();

/// Tells the tray that player state or track may have changed. Cheap; safe from any thread.
pub fn poke() {
    CHANGED.notify_one();
}

/// What the tray shows: the menu line and tooltip text, and whether playback is running (so the button says Pause).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub label: String,
    pub playing: bool,
}

/// Pure: core state and track to what the tray shows.
pub fn view(state: PlaybackStateDto, track: Option<(&str, &[String])>) -> View {
    use PlaybackStateDto as S;
    let playing = matches!(state, S::Playing | S::Buffering);
    let label = match track {
        Some((title, artists)) if !title.trim().is_empty() => {
            let artists = artists.join(", ");
            if artists.is_empty() {
                title.to_owned()
            } else {
                format!("{title} · {artists}")
            }
        }
        _ => "Nothing playing".to_owned(),
    };
    View { label, playing }
}

/// Menu text: Windows and Linux read `&` as the mark of a keyboard shortcut, so a literal one is written twice.
fn menu_text(label: &str) -> String {
    label.replace('&', "&&")
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Builds the tray. Call from `setup` on the main thread.
pub fn install(app: &AppHandle) {
    if let Err(error) = build(app) {
        eprintln!("tray unavailable: {error}");
    }
}

fn build(app: &AppHandle) -> tauri::Result<()> {
    let now = MenuItem::with_id(app, "now", "Nothing playing", false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Play", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "Previous", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show LUNOR-X", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let first = PredefinedMenuItem::separator(app)?;
    let second = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &now, &first, &toggle, &previous, &next, &second, &show, &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("lunorx")
        .menu(&menu)
        .tooltip("LUNOR-X")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let remote = match event.id.as_ref() {
                "toggle" => Remote::Toggle,
                "previous" => Remote::Previous,
                "next" => Remote::Next,
                "show" => return show_window(app),
                "quit" => return app.exit(0),
                _ => return,
            };
            tauri::async_runtime::spawn(control::run(app.clone(), remote));
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut shown: Option<View> = None;
        loop {
            CHANGED.notified().await;
            // Several changes arrive together (state, then position): look once after they settle.
            tokio::time::sleep(Duration::from_millis(250)).await;
            let state = handle.state::<PlayerState>();
            let dto = progress(&state).await;
            let next_view = view(
                dto.player.state,
                dto.track
                    .as_ref()
                    .map(|t| (t.title.as_str(), t.artists.as_slice())),
            );
            if shown.as_ref() == Some(&next_view) {
                continue;
            }
            let _ = now.set_text(menu_text(&next_view.label));
            let _ = toggle.set_text(if next_view.playing { "Pause" } else { "Play" });
            let tooltip = if next_view.label == "Nothing playing" {
                "LUNOR-X".to_owned()
            } else {
                format!("LUNOR-X · {}", next_view.label)
            };
            let _ = tray.set_tooltip(Some(tooltip));
            shown = Some(next_view);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlaybackStateDto as S;

    #[test]
    fn shows_title_and_artists_and_the_right_button() {
        let artists = vec!["A".to_owned(), "B".to_owned()];
        let v = view(S::Playing, Some(("Song", &artists)));
        assert_eq!(v.label, "Song · A, B");
        assert!(v.playing);
        assert!(view(S::Buffering, Some(("Song", &artists))).playing);
        assert!(!view(S::Paused, Some(("Song", &artists))).playing);
    }

    #[test]
    fn an_ampersand_in_a_title_is_shown_literally_in_the_menu() {
        assert_eq!(menu_text("Rock & Roll · AC&DC"), "Rock && Roll · AC&&DC");
        assert_eq!(menu_text("Plain"), "Plain");
    }

    #[test]
    fn nothing_loaded_or_a_blank_title_reads_as_nothing_playing() {
        assert_eq!(view(S::Idle, None).label, "Nothing playing");
        assert_eq!(view(S::Playing, Some(("  ", &[]))).label, "Nothing playing");
        let none: Vec<String> = Vec::new();
        assert_eq!(view(S::Paused, Some(("Solo", &none))).label, "Solo");
    }
}

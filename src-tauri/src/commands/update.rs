//! Self-update with a channel: `stable` (full releases) or `dev` (pre-releases too). Each channel is one feed file on
//! the project's GitHub releases. The updater plugin checks the signature of every download against the public key in
//! tauri.conf.json, so a feed cannot install anything the project did not sign. Nothing installs without `update_install`.
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};

const FEED_BASE: &str = "https://github.com/LunoRX-RS/desktop/releases/download/updates";

#[derive(Default)]
pub struct UpdateState(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDto {
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum UpdateError {
    /// Not the main window, or an unknown channel.
    Validation,
    /// The feed or the download could not be reached or read.
    Unavailable,
    /// There is nothing checked to install.
    NothingToInstall,
    /// The download did not install; nothing was changed.
    InstallFailed,
}

fn feed(channel: &str) -> Result<url::Url, UpdateError> {
    match channel {
        "stable" | "dev" => format!("{FEED_BASE}/{channel}.json")
            .parse()
            .map_err(|_| UpdateError::Validation),
        _ => Err(UpdateError::Validation),
    }
}

fn guard(window: &WebviewWindow) -> Result<(), UpdateError> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err(UpdateError::Validation)
    }
}

/// Asks the channel's feed for a newer signed version. `None`: this version is the newest on that channel.
#[tauri::command]
pub async fn update_check(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, UpdateState>,
    channel: String,
) -> Result<Option<UpdateDto>, UpdateError> {
    guard(&window)?;
    let url = feed(&channel)?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|_| UpdateError::Unavailable)?
        .build()
        .map_err(|_| UpdateError::Unavailable)?;
    let found = updater
        .check()
        .await
        .map_err(|_| UpdateError::Unavailable)?;
    let dto = found.as_ref().map(|u| UpdateDto {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    });
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = found;
    Ok(dto)
}

/// Downloads, verifies and installs the update found by the last `update_check`. Progress goes out as `update-progress`
/// events carrying a percent (or null when the size is unknown).
#[tauri::command]
pub async fn update_install(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<(), UpdateError> {
    guard(&window)?;
    let update = state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or(UpdateError::NothingToInstall)?;
    let mut done: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let percent = total.filter(|t| *t > 0).map(|t| (done * 100 / t).min(100));
                let _ = app.emit("update-progress", percent);
            },
            || {},
        )
        .await
        .map_err(|_| UpdateError::InstallFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_channels_have_a_feed() {
        assert!(
            feed("stable")
                .unwrap()
                .as_str()
                .ends_with("/updates/stable.json")
        );
        assert!(feed("dev").unwrap().as_str().ends_with("/updates/dev.json"));
        for bad in ["", "Stable", "latest", "../dev", "dev.json", "stable?x=1"] {
            assert_eq!(feed(bad), Err(UpdateError::Validation), "{bad}");
        }
    }
}

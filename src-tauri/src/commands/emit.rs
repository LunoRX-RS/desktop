//! Every event the app sends to the main window goes through here, so the activity log sees the same stream the UI does.
//! Only the event name is recorded, never the payload: `core://share/open` carries a share token and `remote://message`
//! carries text chosen by a device on the local network, and neither belongs in a log that may be uploaded. User content
//! reaches the log from the action and IPC sites, where arguments are masked before they are stored.
use crate::commands::reports::ActivityState;
use crate::core::activity::{ActivityKind, NewActivity, valid_name};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Emits to the main window and notes the event with its outcome. A missing or failing log changes neither the emit nor
/// the caller's view of it.
pub fn emit_main<T: Serialize>(
    app: &AppHandle,
    event: &'static str,
    payload: &T,
) -> tauri::Result<()> {
    let result = app.emit_to("main", event, payload);
    if let (Some(name), Some(state)) = (slug_for(event), app.try_state::<ActivityState>()) {
        state.record(&NewActivity {
            kind: "event".into(),
            name,
            detail: String::new(),
            ok: result.is_ok(),
        });
    }
    result
}

/// `core://player/state` becomes `event.player-state` and `remote://message` becomes `event.remote-message`,
/// so the two hubs cannot collide on one slug. A name outside that shape is not logged at all rather than logged
/// under a slug the backend would refuse.
fn slug_for(event: &str) -> Option<String> {
    // Sent several times a second while audio plays or a deck runs. Logging them rewrote the activity file (up to 8 MB,
    // fsync'd) every few seconds, which was most of the app's disk writes. They are not user actions, so they are not logged.
    const HIGH_RATE: &[&str] = &[
        "core://player/progress",
        "core://dj/wave",
        "core://dj/deck",
        "remote://status",
    ];
    if HIGH_RATE.contains(&event) {
        return None;
    }
    let (scheme, tail) = event.split_once("://")?;
    if !matches!(scheme, "core" | "remote") {
        return None;
    }
    let prefix = if scheme == "remote" {
        "event.remote-"
    } else {
        "event."
    };
    let name = format!("{prefix}{}", tail.replace('/', "-"));
    valid_name(ActivityKind::Event, &name).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_the_app_emits_becomes_a_logged_slug() {
        for event in [
            "core://player/state",
            "core://history/changed",
            "core://history/error",
            "core://library/changed",
            "core://offline/changed",
            "core://audio/settings",
            "core://youtube/import",
            "core://youtube/connection",
            "core://share/open",
            "core://sync/auto",
            "remote://message",
        ] {
            let name = slug_for(event).unwrap_or_else(|| panic!("{event} has no slug"));
            assert!(name.starts_with("event."), "{event} became {name}");
        }
        assert_eq!(slug_for("core://player/progress"), None);
        assert_eq!(slug_for("remote://status"), None);
        assert_eq!(slug_for("core://dj/wave"), None);
        assert_eq!(slug_for("core://dj/deck"), None);
    }

    #[test]
    fn a_name_that_is_not_an_app_event_is_not_logged() {
        assert_eq!(slug_for("update-progress"), None);
        assert_eq!(slug_for("plugin:updater|download"), None);
    }
}

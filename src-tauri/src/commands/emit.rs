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

/// `core://player/progress` becomes `event.player-progress` and `remote://status` becomes `event.remote-status`,
/// so the two hubs cannot collide on one slug. A name outside that shape is not logged at all rather than logged
/// under a slug the backend would refuse.
fn slug_for(event: &str) -> Option<String> {
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
            "core://player/progress",
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
            "remote://status",
        ] {
            let name = slug_for(event).unwrap_or_else(|| panic!("{event} has no slug"));
            assert!(name.starts_with("event."), "{event} became {name}");
        }
        assert_eq!(
            slug_for("core://player/progress").as_deref(),
            Some("event.player-progress")
        );
        assert_eq!(
            slug_for("remote://status").as_deref(),
            Some("event.remote-status")
        );
    }

    #[test]
    fn a_name_that_is_not_an_app_event_is_not_logged() {
        assert_eq!(slug_for("update-progress"), None);
        assert_eq!(slug_for("plugin:updater|download"), None);
    }
}

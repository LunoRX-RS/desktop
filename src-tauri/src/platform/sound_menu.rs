//! macOS "Sound" menu, a tab of its own in the menu bar: sound shaping on or off, every preset (by group), moving sound
//! (8D and 4D), speed, keep pitch and reset. It changes the same effects the settings panel does, through the same checks,
//! so the two always agree: a change here goes to the window (which saves it) and a change there refreshes the checks here.

use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItemBuilder, MenuItemKind, Submenu,
    SubmenuBuilder,
};
use tauri::{AppHandle, Manager, Wry};

use crate::commands::player::{change_effects, current_effects};
use crate::core::playback::audio::{Effects, Spatial};
use crate::core::playback::presets::{self, Preset};

const SPEEDS: [u8; 9] = [50, 75, 85, 100, 110, 125, 150, 175, 200];
const ID_ON: &str = "sound_on";
const ID_KEEP: &str = "sound_keep_pitch";
const ID_RESET: &str = "sound_reset";

/// One menu choice, decoded from its id. Pure, so the mapping is tested without AppKit.
#[derive(Debug, PartialEq)]
enum Choice {
    Toggle,
    KeepPitch,
    Reset,
    Preset(Effects),
    Moving(Spatial),
    Speed(u8),
}

fn decode(id: &str, presets: &[Preset]) -> Option<Choice> {
    match id {
        ID_ON => Some(Choice::Toggle),
        ID_KEEP => Some(Choice::KeepPitch),
        ID_RESET => Some(Choice::Reset),
        _ => {
            let (kind, value) = id.split_once(':')?;
            match kind {
                "sound_preset" => presets
                    .iter()
                    .find(|p| p.id == value)
                    .map(|p| Choice::Preset(p.effects)),
                "sound_moving" => match value {
                    "off" => Some(Choice::Moving(Spatial::Off)),
                    "8d" => Some(Choice::Moving(Spatial::Eight)),
                    "4d" => Some(Choice::Moving(Spatial::Four)),
                    _ => None,
                },
                "sound_speed" => value
                    .parse::<u8>()
                    .ok()
                    .filter(|v| SPEEDS.contains(v))
                    .map(Choice::Speed),
                _ => None,
            }
        }
    }
}

fn apply(choice: Choice, e: &mut Effects) {
    match choice {
        Choice::Toggle => e.enabled = !e.enabled,
        Choice::KeepPitch => e.keep_pitch = !e.keep_pitch,
        Choice::Reset => *e = Effects::default(),
        Choice::Preset(p) => *e = p,
        Choice::Moving(s) => {
            e.spatial = s;
            e.enabled = true;
        }
        Choice::Speed(v) => {
            e.speed = v;
            e.enabled = true;
        }
    }
}

struct SoundMenu {
    on: CheckMenuItem<Wry>,
    keep: CheckMenuItem<Wry>,
    presets: Vec<(Effects, CheckMenuItem<Wry>)>,
    moving: Vec<(Spatial, CheckMenuItem<Wry>)>,
    speeds: Vec<(u8, CheckMenuItem<Wry>)>,
}

fn check(app: &AppHandle, id: &str, text: &str) -> tauri::Result<CheckMenuItem<Wry>> {
    CheckMenuItemBuilder::with_id(id, text).build(app)
}

fn build(app: &AppHandle) -> tauri::Result<(Submenu<Wry>, SoundMenu)> {
    let all = presets::all();
    let on = check(app, ID_ON, "Sound Shaping")?;
    let keep = check(app, ID_KEEP, "Keep Pitch When Speed Changes")?;
    let mut preset_items = Vec::new();
    let mut preset_menu = SubmenuBuilder::new(app, "Presets");
    for group in presets::GROUPS {
        let mut sub = SubmenuBuilder::new(app, group);
        for p in all.iter().filter(|p| p.group == group) {
            let item = check(app, &format!("sound_preset:{}", p.id), p.label)?;
            sub = sub.item(&item);
            preset_items.push((p.effects, item));
        }
        preset_menu = preset_menu.item(&sub.build()?);
    }
    let mut moving = Vec::new();
    let mut moving_menu = SubmenuBuilder::new(app, "Moving Sound (Headphones)");
    for (spatial, id, label) in [
        (Spatial::Off, "off", "Off"),
        (Spatial::Eight, "8d", "8D"),
        (Spatial::Four, "4d", "4D"),
    ] {
        let item = check(app, &format!("sound_moving:{id}"), label)?;
        moving_menu = moving_menu.item(&item);
        moving.push((spatial, item));
    }
    let mut speeds = Vec::new();
    let mut speed_menu = SubmenuBuilder::new(app, "Speed");
    for v in SPEEDS {
        let item = check(
            app,
            &format!("sound_speed:{v}"),
            &format!("{:.2}×", f64::from(v) / 100.0),
        )?;
        speed_menu = speed_menu.item(&item);
        speeds.push((v, item));
    }
    let reset = MenuItemBuilder::with_id(ID_RESET, "Reset Sound").build(app)?;
    let sound = SubmenuBuilder::new(app, "Sound")
        .item(&on)
        .separator()
        .item(&preset_menu.build()?)
        .item(&moving_menu.build()?)
        .item(&speed_menu.build()?)
        .item(&keep)
        .separator()
        .item(&reset)
        .build()?;
    Ok((
        sound,
        SoundMenu {
            on,
            keep,
            presets: preset_items,
            moving,
            speeds,
        },
    ))
}

/// Adds the Sound menu after Playback. A failure only means no Sound menu; the app and the panel are unaffected.
pub fn install(app: &AppHandle) {
    let Some(root) = app.menu() else { return };
    let Ok((sound, items)) = build(app) else {
        eprintln!("sound menu unavailable");
        return;
    };
    if insert(&root, &sound).is_err() {
        eprintln!("sound menu unavailable");
        return;
    }
    app.manage(items);
    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if !id.starts_with("sound_") {
            return;
        }
        if let Some(choice) = decode(id, &presets::all()) {
            change_effects(app, |e| apply(choice, e));
            refresh(app);
        }
    });
    refresh(app);
}

fn insert(root: &Menu<Wry>, sound: &Submenu<Wry>) -> tauri::Result<()> {
    let items = root.items()?;
    let at = items
        .iter()
        .position(
            |i| matches!(i, MenuItemKind::Submenu(s) if s.text().is_ok_and(|t| t == "Window")),
        )
        .unwrap_or(items.len());
    root.insert(sound, at)
}

/// Sets every check from the effects in force.
pub fn refresh(app: &AppHandle) {
    let Some(menu) = app.try_state::<SoundMenu>() else {
        return;
    };
    let e = current_effects(app);
    // Failures only mean a check is momentarily stale; the next change retries.
    let _ = menu.on.set_checked(e.enabled);
    let _ = menu.keep.set_checked(e.keep_pitch);
    for (preset, item) in &menu.presets {
        let _ = item.set_checked(*preset == e);
    }
    for (spatial, item) in &menu.moving {
        let _ = item.set_checked(*spatial == e.spatial);
    }
    for (speed, item) in &menu.speeds {
        let _ = item.set_checked(*speed == e.speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_decode_to_the_choice_they_name_and_nothing_else() {
        let list = presets::all();
        assert_eq!(decode("sound_on", &list), Some(Choice::Toggle));
        assert_eq!(
            decode("sound_moving:8d", &list),
            Some(Choice::Moving(Spatial::Eight))
        );
        assert_eq!(decode("sound_speed:125", &list), Some(Choice::Speed(125)));
        assert_eq!(decode("sound_speed:126", &list), None);
        assert_eq!(decode("sound_speed:999", &list), None);
        assert_eq!(decode("sound_moving:9d", &list), None);
        assert_eq!(decode("sound_preset:nope", &list), None);
        assert_eq!(decode("playback_toggle", &list), None);
        let night = decode("sound_preset:night", &list).unwrap();
        assert!(matches!(night, Choice::Preset(e) if e.speed == 125 && !e.keep_pitch));
    }

    #[test]
    fn choices_change_only_what_they_name() {
        let mut e = Effects::default();
        apply(Choice::Speed(125), &mut e);
        assert_eq!((e.speed, e.bass), (125, 0));
        apply(Choice::Moving(Spatial::Four), &mut e);
        assert_eq!((e.spatial, e.speed), (Spatial::Four, 125));
        apply(Choice::KeepPitch, &mut e);
        assert!(!e.keep_pitch);
        apply(Choice::Toggle, &mut e);
        assert!(!e.enabled);
        // Picking a speed or moving sound with shaping off turns it on, so the choice is heard.
        apply(Choice::Speed(75), &mut e);
        assert!(e.enabled);
        apply(Choice::Reset, &mut e);
        assert_eq!(e, Effects::default());
        // A preset replaces everything and turns shaping on.
        e.enabled = false;
        let list = presets::all();
        let Some(Choice::Preset(p)) = decode("sound_preset:8d-bass", &list) else {
            panic!("preset");
        };
        apply(Choice::Preset(p), &mut e);
        assert!(e.enabled && e.spatial == Spatial::Eight && e.bands[0] == 5);
    }
}

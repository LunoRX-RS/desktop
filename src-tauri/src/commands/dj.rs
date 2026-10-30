//! DJ decks: two YouTube pages, each in its own window with no IPC, driven from the DJ screen. Each deck reports its
//! own position and state; the crossfader and master volume are applied as per-deck volume. Only the main window may
//! call these commands.
use crate::{
    commands::{
        emit::emit_main,
        page_host::{PageState, SCRIPT, allowed_page_url, parse_title},
    },
    core::{
        dj::{Deck, DeckControl, Loop, deck_volumes, position_ok},
        playback::audio::Effects,
    },
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    webview::NewWindowResponse,
};

const EVENT: &str = "core://dj/deck";
const WAVE_EVENT: &str = "core://dj/wave";

/// A batch of waveform samples from a deck page: low, mid and high energy (0 to 255), one every 50 ms from `at_ms`.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Wave {
    deck: Deck,
    video_id: String,
    at_ms: u64,
    bands: Vec<[u8; 3]>,
}

/// `LUNORX_WF|<id>|<at ms>|l.m.h,l.m.h,...`. At most 16 samples; anything malformed is dropped whole.
fn parse_wave(deck: Deck, title: &str) -> Option<Wave> {
    let mut p = title.strip_prefix("LUNORX_WF|")?.splitn(3, '|');
    let video_id = p.next()?.to_owned();
    let at_ms = p.next()?.parse().ok()?;
    let bands = p
        .next()?
        .split(',')
        .map(|s| {
            let mut v = s.split('.').map(|x| x.parse::<u8>().ok());
            Some([v.next()??, v.next()??, v.next()??])
        })
        .collect::<Option<Vec<_>>>()?;
    (!bands.is_empty() && bands.len() <= 16 && valid_video_id(&video_id)).then_some(Wave {
        deck,
        video_id,
        at_ms,
        bands,
    })
}

#[derive(Serialize)]
#[serde(tag = "category", rename_all = "camelCase")]
pub enum DjError {
    Forbidden,
    InvalidInput,
    WindowFailed,
    NotLoaded,
}

#[derive(Default)]
struct DeckSlot {
    /// The video this deck was asked to play. Reports from any other video (YouTube moving on by itself) are ignored.
    expected: String,
    /// True until the first report of a freshly loaded page, which is when the deck is paused and set up.
    fresh: bool,
    effects: Option<Effects>,
    looping: Option<Loop>,
    /// The deck's latest report: position, duration and state.
    last: Option<(u64, u64, &'static str)>,
}

pub struct DjState {
    decks: Arc<Mutex<HashMap<Deck, DeckSlot>>>,
    /// Crossfader (0 all A, 100 all B) and master volume.
    mix: Arc<Mutex<(u8, u8)>>,
}

impl Default for DjState {
    fn default() -> Self {
        Self {
            decks: Arc::default(),
            mix: Arc::new(Mutex::new((50, 100))),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DeckReport<'a> {
    deck: Deck,
    video_id: &'a str,
    position_ms: u64,
    duration_ms: u64,
    state: &'static str,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn main_only(window: &WebviewWindow) -> Result<(), DjError> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err(DjError::Forbidden)
    }
}

fn state_name(s: PageState) -> &'static str {
    match s {
        PageState::Playing => "playing",
        PageState::Paused => "paused",
        PageState::Waiting => "waiting",
        PageState::Ended => "ended",
        PageState::Error => "error",
    }
}

fn run(app: &AppHandle, deck: Deck, js: &str) -> Result<(), DjError> {
    app.get_webview_window(deck.label())
        .ok_or(DjError::NotLoaded)?
        .eval(js)
        .map_err(|_| DjError::WindowFailed)
}

/// Sends a deck everything it should be holding: its share of the volume, its effects and its loop.
fn apply(app: &AppHandle, state: &DjState, deck: Deck) {
    let (cross, master) = *lock(&state.mix);
    let (a, b) = deck_volumes(cross, master);
    let volume = if deck == Deck::A { a } else { b };
    let _ = run(
        app,
        deck,
        &format!("window.__lunorx&&window.__lunorx.volume({volume})"),
    );
    let (effects, looping) = {
        let d = lock(&state.decks);
        d.get(&deck)
            .map(|s| (s.effects, s.looping))
            .unwrap_or((None, None))
    };
    if let Some(fx) = effects
        && let Ok(json) = serde_json::to_string(&fx)
    {
        let _ = run(
            app,
            deck,
            &format!("window.__lunorx&&window.__lunorx.effects({json})"),
        );
    }
    let loop_js = match looping {
        Some(l) => format!(
            "window.__lunorx&&window.__lunorx.loop({},{})",
            l.start_ms, l.end_ms
        ),
        None => "window.__lunorx&&window.__lunorx.loop(null,null)".to_owned(),
    };
    let _ = run(app, deck, &loop_js);
}

fn open_window(
    app: &AppHandle,
    state: &DjState,
    deck: Deck,
    url: tauri::Url,
) -> Result<WebviewWindow, DjError> {
    if let Some(w) = app.get_webview_window(deck.label()) {
        w.navigate(url).map_err(|_| DjError::WindowFailed)?;
        return Ok(w);
    }
    let (events, decks, mix) = (app.clone(), state.decks.clone(), state.mix.clone());
    WebviewWindowBuilder::new(app, deck.label(), WebviewUrl::External(url))
        .title("LunoRX: DJ deck")
        .inner_size(480.0, 300.0)
        .visible(false)
        .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled)
        .initialization_script(SCRIPT)
        .on_navigation(allowed_page_url)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_document_title_changed(move |_, title| {
            if let Some(wave) = parse_wave(deck, &title) {
                if lock(&decks)
                    .get(&deck)
                    .is_some_and(|s| s.expected == wave.video_id)
                {
                    let _ = emit_main(&events, WAVE_EVENT, &wave);
                }
                return;
            }
            let Some((id, position_ms, duration_ms, page)) = parse_title(&title) else {
                return;
            };
            let first = {
                let mut d = lock(&decks);
                let Some(slot) = d.get_mut(&deck) else { return };
                if slot.expected != id {
                    return;
                }
                slot.last = Some((position_ms, duration_ms, state_name(page)));
                std::mem::take(&mut slot.fresh)
            };
            if first {
                // A freshly loaded deck waits for the DJ: pause the page's autoplay, then give it its settings.
                let _ = run(&events, deck, "window.__lunorx&&window.__lunorx.pause()");
                let st = DjState {
                    decks: decks.clone(),
                    mix: mix.clone(),
                };
                apply(&events, &st, deck);
            }
            let _ = emit_main(
                &events,
                EVENT,
                &DeckReport {
                    deck,
                    video_id: id,
                    position_ms,
                    duration_ms,
                    state: state_name(page),
                },
            );
        })
        .build()
        .map_err(|_| DjError::WindowFailed)
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[tauri::command]
pub fn dj_load(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DjState>,
    deck: Deck,
    video_id: String,
) -> Result<(), DjError> {
    main_only(&window)?;
    if !valid_video_id(&video_id) {
        return Err(DjError::InvalidInput);
    }
    let url: tauri::Url = format!("https://www.youtube.com/watch?v={video_id}")
        .parse()
        .map_err(|_| DjError::InvalidInput)?;
    {
        let mut d = lock(&state.decks);
        let slot = d.entry(deck).or_default();
        slot.expected = video_id;
        slot.fresh = true;
        slot.looping = None;
        slot.last = None;
    }
    open_window(&app, &state, deck, url).map(|_| ())
}

#[tauri::command]
pub fn dj_play(app: AppHandle, window: WebviewWindow, deck: Deck) -> Result<(), DjError> {
    main_only(&window)?;
    run(&app, deck, "window.__lunorx&&window.__lunorx.play()")
}

#[tauri::command]
pub fn dj_pause(app: AppHandle, window: WebviewWindow, deck: Deck) -> Result<(), DjError> {
    main_only(&window)?;
    run(&app, deck, "window.__lunorx&&window.__lunorx.pause()")
}

#[tauri::command]
pub fn dj_seek(
    app: AppHandle,
    window: WebviewWindow,
    deck: Deck,
    position_ms: u64,
) -> Result<(), DjError> {
    main_only(&window)?;
    if !position_ok(position_ms) {
        return Err(DjError::InvalidInput);
    }
    run(
        &app,
        deck,
        &format!("window.__lunorx&&window.__lunorx.seek({position_ms})"),
    )
}

#[tauri::command]
pub fn dj_set_effects(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DjState>,
    deck: Deck,
    effects: Effects,
) -> Result<(), DjError> {
    main_only(&window)?;
    if !effects.valid() {
        return Err(DjError::InvalidInput);
    }
    lock(&state.decks).entry(deck).or_default().effects = Some(effects);
    apply(&app, &state, deck);
    Ok(())
}

#[tauri::command]
pub fn dj_set_loop(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DjState>,
    deck: Deck,
    looping: Option<Loop>,
) -> Result<(), DjError> {
    main_only(&window)?;
    if looping.is_some_and(|l| !l.valid()) {
        return Err(DjError::InvalidInput);
    }
    lock(&state.decks).entry(deck).or_default().looping = looping;
    apply(&app, &state, deck);
    Ok(())
}

#[tauri::command]
pub fn dj_set_mix(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DjState>,
    crossfader: u8,
    master: u8,
) -> Result<(), DjError> {
    main_only(&window)?;
    if crossfader > 100 || master > 100 {
        return Err(DjError::InvalidInput);
    }
    *lock(&state.mix) = (crossfader, master);
    for deck in [Deck::A, Deck::B] {
        if app.get_webview_window(deck.label()).is_some() {
            apply(&app, &state, deck);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn dj_control(
    app: AppHandle,
    window: WebviewWindow,
    deck: Deck,
    control: DeckControl,
) -> Result<(), DjError> {
    main_only(&window)?;
    if !control.valid() {
        return Err(DjError::InvalidInput);
    }
    let json = serde_json::to_string(&control).map_err(|_| DjError::InvalidInput)?;
    run(
        &app,
        deck,
        &format!("window.__lunorx&&window.__lunorx.control({json})"),
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStatus {
    deck: Deck,
    video_id: String,
    position_ms: u64,
    duration_ms: u64,
    state: &'static str,
}

/// Every loaded deck with its latest report, so the screen can catch up after it was closed.
#[tauri::command]
pub fn dj_status(
    window: WebviewWindow,
    state: State<'_, DjState>,
) -> Result<Vec<DeckStatus>, DjError> {
    main_only(&window)?;
    let d = lock(&state.decks);
    Ok([Deck::A, Deck::B]
        .into_iter()
        .filter_map(|deck| {
            let s = d.get(&deck)?;
            let (position_ms, duration_ms, st) = s.last?;
            Some(DeckStatus {
                deck,
                video_id: s.expected.clone(),
                position_ms,
                duration_ms,
                state: st,
            })
        })
        .collect())
}

#[tauri::command]
pub fn dj_stop(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, DjState>,
    deck: Deck,
) -> Result<(), DjError> {
    main_only(&window)?;
    lock(&state.decks).remove(&deck);
    if let Some(w) = app.get_webview_window(deck.label()) {
        let _ = w.eval("window.__lunorx&&window.__lunorx.pause()");
        let _ = w.destroy();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_ids_are_eleven_url_safe_characters() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(!valid_video_id("short"));
        assert!(!valid_video_id("dQw4w9WgXc!"));
        assert!(!valid_video_id("dQw4w9WgXcQ0"));
    }

    #[test]
    fn wave_batches_parse_and_reject_junk() {
        let w = parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|10.20.30,255.0.7").unwrap();
        assert_eq!(
            (w.at_ms, w.bands.clone()),
            (1500, vec![[10, 20, 30], [255, 0, 7]])
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|10.20").is_none(),
            "missing band"
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|300.0.0").is_none(),
            "out of range"
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|bad|1500|1.2.3").is_none(),
            "bad id"
        );
        assert!(
            parse_wave(
                Deck::A,
                &format!("LUNORX_WF|dQw4w9WgXcQ|0|{}", vec!["1.2.3"; 17].join(","))
            )
            .is_none(),
            "too many"
        );
        assert!(parse_wave(Deck::A, "LUNORX_ST|dQw4w9WgXcQ|0|0|playing").is_none());
    }

    #[test]
    fn state_names_match_what_the_screen_expects() {
        assert_eq!(state_name(PageState::Playing), "playing");
        assert_eq!(state_name(PageState::Ended), "ended");
    }
}

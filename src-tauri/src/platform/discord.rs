//! Discord Rich Presence: shows the playing track on the user's Discord profile, through the Discord app that is
//! already running on this computer. Local IPC only (a Unix socket, or a named pipe on Windows); this code opens no
//! network connection and needs no secret. Off until the user turns it on and gives their own Application ID.
//!
//! What leaves the app: the track title, artists, cover URL, a YouTube link and the playback time, handed to the local
//! Discord app, which then shows them to people who can see the user's profile.
use crate::commands::player::{PlaybackStateDto, PlayerProgressDto};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

/// LunoRX's own Discord application. An Application ID is public (it appears in every Rich Presence client), not a secret.
pub const DEFAULT_CLIENT_ID: &str = "1562868203460493373";
/// The name of the logo in the Discord application's Art Assets (Developer Portal > Rich Presence).
/// Used as the small corner badge, and as the big picture when a track has no cover.
const LOGO_ASSET: &str = "logo";
const REPO_URL: &str = "https://github.com/LunoRX-RS/desktop";
const SETTINGS_FILE: &str = "discord-presence.json";
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const MIN_GAP: Duration = Duration::from_secs(5);
const RETRY: Duration = Duration::from_secs(20);
const DRIFT_MS: i64 = 3_000;

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub enabled: bool,
    /// An Application ID of the user's own, replacing the default. Empty uses `DEFAULT_CLIENT_ID`.
    pub client_id: String,
    /// How the card looks: text templates, images, clock, buttons and the user's own image catalog.
    pub design: Design,
}

/// One image in the user's own catalog: a name to pick it by, and where Discord finds it.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CatalogEntry {
    pub name: String,
    /// An `https://` image URL, or the key of an Art Asset uploaded to the Application ID's Rich Presence page.
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CardButton {
    pub label: String,
    /// An `https://` link, or `{track_url}` for the playing video.
    pub url: String,
}

/// The user's card design. Every text field is a template: `{title}`, `{artist}` (first), `{artists}`, `{status}` and `{app}`.
/// Images are `cover`, `logo`, `none` or `catalog:<name>`. `timestamps` is `bar`, `elapsed` or `off`.
/// `status_display` is what Discord names the card in the member list: `app`, `artist` or `title`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Design {
    pub details: String,
    pub state: String,
    pub paused_state: String,
    pub large_text: String,
    pub small_text: String,
    pub paused_small_text: String,
    pub large_image: String,
    pub small_image: String,
    pub timestamps: String,
    pub status_display: String,
    pub tidy_title: bool,
    pub listen_button: bool,
    pub app_button: bool,
    pub buttons: Vec<CardButton>,
    pub catalog: Vec<CatalogEntry>,
}

impl Default for Design {
    fn default() -> Self {
        Self {
            details: "{title}".into(),
            state: "{artists}".into(),
            paused_state: "Paused · {artists}".into(),
            large_text: "on {app}".into(),
            small_text: "{app}".into(),
            paused_small_text: "{app} - paused".into(),
            large_image: "cover".into(),
            small_image: "logo".into(),
            timestamps: "bar".into(),
            status_display: "artist".into(),
            tidy_title: true,
            listen_button: true,
            app_button: true,
            buttons: Vec::new(),
            catalog: Vec::new(),
        }
    }
}

const MAX_CATALOG: usize = 12;
const MAX_CUSTOM_BUTTONS: usize = 2;
const MAX_TEXT: usize = 128;
const TRACK_URL_TOKEN: &str = "{track_url}";

fn catalog_name(n: &str) -> bool {
    (1..=24).contains(&n.len())
        && n.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// An Art Asset key: lowercase letters, digits and underscores, as the Developer Portal stores them.
fn asset_key(v: &str) -> bool {
    (1..=32).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl Design {
    /// Trims and checks everything the user typed. `Err` names the field.
    pub fn clean(mut self) -> Result<Self, &'static str> {
        for (t, field) in [
            (&mut self.details, "details"),
            (&mut self.state, "state"),
            (&mut self.paused_state, "paused text"),
            (&mut self.large_text, "image text"),
            (&mut self.small_text, "badge text"),
            (&mut self.paused_small_text, "paused badge text"),
        ] {
            *t = t.trim().to_owned();
            if t.chars().count() > MAX_TEXT || t.chars().any(char::is_control) {
                return Err(field);
            }
        }
        if !["bar", "elapsed", "off"].contains(&self.timestamps.as_str()) {
            return Err("clock");
        }
        if !["app", "artist", "title"].contains(&self.status_display.as_str()) {
            return Err("status line");
        }
        if self.catalog.len() > MAX_CATALOG {
            return Err("catalog");
        }
        let mut seen = std::collections::HashSet::new();
        for e in &mut self.catalog {
            e.name = e.name.trim().to_owned();
            e.value = e.value.trim().to_owned();
            if !catalog_name(&e.name) || !seen.insert(e.name.clone()) {
                return Err("catalog name");
            }
            if !(https(&e.value) || asset_key(&e.value)) {
                return Err("catalog image");
            }
        }
        for (img, field) in [
            (&self.large_image, "large image"),
            (&self.small_image, "small image"),
        ] {
            let ok = matches!(img.as_str(), "cover" | "logo" | "none")
                || img
                    .strip_prefix("catalog:")
                    .is_some_and(|n| self.catalog.iter().any(|e| e.name == n));
            // The big picture cannot be empty: Discord shows no card image without one.
            if !ok || (field == "large image" && img == "none") {
                return Err(field);
            }
        }
        if self.buttons.len() > MAX_CUSTOM_BUTTONS {
            return Err("buttons");
        }
        for b in &mut self.buttons {
            b.label = b.label.trim().to_owned();
            b.url = b.url.trim().to_owned();
            let label = b.label.chars().count();
            if !(1..=32).contains(&label)
                || !(b.url == TRACK_URL_TOKEN || (https(&b.url) && b.url.len() <= 512))
            {
                return Err("button");
            }
        }
        Ok(self)
    }

    fn image(&self, which: &str, cover: Option<&str>) -> Option<String> {
        match which {
            "cover" => cover.map(str::to_owned),
            "logo" => Some(LOGO_ASSET.to_owned()),
            other => other
                .strip_prefix("catalog:")
                .and_then(|n| self.catalog.iter().find(|e| e.name == n))
                .map(|e| e.value.clone()),
        }
    }
}

impl Settings {
    /// The Application ID to connect with: the user's own, else the built-in one.
    fn effective_client_id(&self) -> &str {
        if self.client_id.is_empty() {
            DEFAULT_CLIENT_ID
        } else {
            &self.client_id
        }
    }
}

/// 17 to 20 digits, the shape of a Discord snowflake.
pub fn valid_client_id(id: &str) -> bool {
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Connecting,
    Connected,
    /// Enabled, but Discord is not running or refused the Application ID.
    Unavailable,
}

#[derive(Clone, PartialEq, Debug)]
enum Play {
    Playing,
    Paused,
}

#[derive(Clone, Debug)]
struct Snapshot {
    track_id: String,
    title: String,
    artists: Vec<String>,
    artwork: Option<String>,
    play: Play,
    position_ms: u64,
    duration_ms: u64,
}

enum Msg {
    Settings(Box<Settings>),
    Track(Option<Snapshot>),
}

struct Shared {
    tx: mpsc::Sender<Msg>,
    status: Mutex<Status>,
    settings: Mutex<Settings>,
    path: Option<PathBuf>,
    /// The latest track the player offered, so the Settings preview can show the real song.
    last: Mutex<Option<Snapshot>>,
}
static SHARED: OnceLock<Shared> = OnceLock::new();

// ---- wire format -------------------------------------------------------------------------------------------------

fn frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn read_frame(r: &mut impl Read) -> std::io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    r.read_exact(&mut head)?;
    let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    if len > 64 * 1024 {
        return Err(std::io::Error::other("discord frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

/// Discord wants 2 to 128 characters for the two text lines.
fn line(text: &str, fallback: &str) -> String {
    let t: String = text.trim().chars().take(128).collect();
    if t.chars().count() >= 2 {
        t
    } else {
        fallback.to_owned()
    }
}

fn https(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() <= 256
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

/// YouTube titles carry hashtags and trailing emoji ("... #saran #ติดตามเค้าหน่อย 🎵🎵"). Those only make the card long:
/// drop hashtags, and trailing words that have no letter or digit in them. Falls back to the original when nothing is left.
fn tidy_title(title: &str) -> String {
    let mut words: Vec<&str> = title
        .split_whitespace()
        .filter(|w| !w.starts_with('#'))
        .collect();
    while words
        .last()
        .is_some_and(|w| !w.chars().any(char::is_alphanumeric))
    {
        words.pop();
    }
    let t = words.join(" ");
    if t.is_empty() {
        title.trim().to_owned()
    } else {
        t
    }
}

/// Fills the `{...}` tokens of one template. Unknown braces stay as typed.
fn render(template: &str, s: &Snapshot, title: &str) -> String {
    template
        .replace("{title}", title)
        .replace("{artists}", &s.artists.join(", "))
        .replace("{artist}", s.artists.first().map_or("", String::as_str))
        .replace(
            "{status}",
            if s.play == Play::Paused {
                "paused"
            } else {
                "playing"
            },
        )
        .replace("{app}", "LunoRX")
}

/// A text line for Discord, or `None` when the template is empty so the line is left out.
fn text_line(template: &str, s: &Snapshot, title: &str, fallback: &str) -> Option<String> {
    (!template.trim().is_empty()).then(|| line(&render(template, s, title), fallback))
}

/// `now_secs` is injected so the timestamps can be tested.
fn activity(s: &Snapshot, d: &Design, now_secs: i64) -> Value {
    let title = if d.tidy_title {
        tidy_title(&s.title)
    } else {
        s.title.trim().to_owned()
    };
    let paused = s.play == Play::Paused;
    let mut a = json!({
        "type": 2,
        "status_display_type": match d.status_display.as_str() { "app" => 0, "title" => 2, _ => 1 },
    });
    if let Some(t) = text_line(&d.details, s, &title, "Unknown title") {
        a["details"] = json!(t);
    }
    let state = if paused && !d.paused_state.trim().is_empty() {
        &d.paused_state
    } else {
        &d.state
    };
    if let Some(t) = text_line(state, s, &title, "Unknown artist") {
        a["state"] = json!(t);
    }
    let video = crate::core::playback::extract::valid_id(&s.track_id)
        .then(|| format!("https://www.youtube.com/watch?v={}", s.track_id));
    // The title is a link to the video.
    if let Some(url) = &video {
        a["details_url"] = json!(url);
    }
    if !paused && s.duration_ms > 0 && d.timestamps != "off" {
        let start = now_secs - (s.position_ms / 1000) as i64;
        a["timestamps"] = if d.timestamps == "elapsed" {
            json!({ "start": start })
        } else {
            json!({ "start": start, "end": start + (s.duration_ms / 1000) as i64 })
        };
    }
    // The big picture falls back to the logo so a card never goes without one.
    let cover = s.artwork.as_deref().filter(|u| https(u));
    let large = d
        .image(&d.large_image, cover)
        .unwrap_or_else(|| LOGO_ASSET.to_owned());
    let mut assets = json!({ "large_image": large });
    if let Some(t) = text_line(&d.large_text, s, &title, "LunoRX") {
        // On a "Listening" card Discord shows this as the third line, under the artist.
        assets["large_text"] = json!(t);
    }
    // A badge that repeats the big picture adds nothing, so it is left out.
    if let Some(small) = d.image(&d.small_image, cover).filter(|v| *v != large) {
        assets["small_image"] = json!(small);
        let badge = if paused && !d.paused_small_text.trim().is_empty() {
            &d.paused_small_text
        } else {
            &d.small_text
        };
        if let Some(t) = text_line(badge, s, &title, "LunoRX") {
            assets["small_text"] = json!(t);
        }
    }
    a["assets"] = assets;
    let mut buttons = Vec::new();
    if let (true, Some(url)) = (d.listen_button, &video) {
        buttons.push(json!({ "label": "Listen on YouTube", "url": url }));
    }
    if d.app_button {
        buttons.push(json!({ "label": "Get LunoRX", "url": REPO_URL }));
    }
    for b in &d.buttons {
        match (b.url == TRACK_URL_TOKEN, &video) {
            (true, None) => {}
            (true, Some(url)) => buttons.push(json!({ "label": b.label, "url": url })),
            (false, _) => buttons.push(json!({ "label": b.label, "url": b.url })),
        }
    }
    // Discord shows at most two buttons; the user's own come first.
    let custom = d.buttons.len().min(buttons.len());
    let keep: Vec<Value> = buttons
        .split_off(buttons.len() - custom)
        .into_iter()
        .chain(buttons)
        .take(2)
        .collect();
    if !keep.is_empty() {
        a["buttons"] = Value::Array(keep);
    }
    a
}

// ---- transport ---------------------------------------------------------------------------------------------------

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

#[cfg(unix)]
fn open_pipe() -> Option<Box<dyn Pipe>> {
    use std::os::unix::net::UnixStream;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for var in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Some(d) = std::env::var_os(var) {
            dirs.push(PathBuf::from(d));
        }
    }
    dirs.push(PathBuf::from("/tmp"));
    // Flatpak and snap keep the socket in a subfolder of the runtime dir.
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") {
        dirs.push(PathBuf::from(&d).join("app/com.discordapp.Discord"));
        dirs.push(PathBuf::from(&d).join("snap.discord"));
    }
    for dir in dirs {
        for n in 0..10 {
            if let Ok(s) = UnixStream::connect(dir.join(format!("discord-ipc-{n}"))) {
                let _ = s.set_read_timeout(Some(Duration::from_secs(4)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(4)));
                return Some(Box::new(s));
            }
        }
    }
    None
}

#[cfg(windows)]
fn open_pipe() -> Option<Box<dyn Pipe>> {
    (0..10).find_map(|n| {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(r"\\.\pipe\discord-ipc-{n}"))
            .ok()
            .map(|f| Box::new(f) as Box<dyn Pipe>)
    })
}

#[cfg(not(any(unix, windows)))]
fn open_pipe() -> Option<Box<dyn Pipe>> {
    None
}

struct Link {
    pipe: Box<dyn Pipe>,
    nonce: u64,
}

impl Link {
    fn connect(client_id: &str) -> Option<Self> {
        Self::over(open_pipe()?, client_id)
    }
    fn over(mut pipe: Box<dyn Pipe>, client_id: &str) -> Option<Self> {
        pipe.write_all(&frame(
            OP_HANDSHAKE,
            &json!({ "v": 1, "client_id": client_id }),
        ))
        .ok()?;
        let (_, reply) = read_frame(&mut pipe).ok()?;
        // READY on success; an error event when the Application ID is unknown.
        (reply["evt"] == "READY").then_some(Self { pipe, nonce: 0 })
    }
    fn set(&mut self, activity: Option<Value>) -> bool {
        self.nonce += 1;
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": self.nonce.to_string(),
        });
        if self.pipe.write_all(&frame(OP_FRAME, &payload)).is_err() {
            return false;
        }
        // Read the answer so the pipe never fills up.
        read_frame(&mut self.pipe).is_ok()
    }
}

// ---- worker ------------------------------------------------------------------------------------------------------

fn set_status(s: Status) {
    if let Some(sh) = SHARED.get() {
        *sh.status.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }
}

fn worker(rx: mpsc::Receiver<Msg>, initial: Settings) {
    let mut settings = initial;
    let mut link: Option<Link> = None;
    let mut want: Option<Snapshot> = None;
    let mut sent: Option<(Snapshot, i64)> = None; // what Discord shows, and the start time it was given
    let mut last_send = Instant::now() - MIN_GAP;
    let mut last_try = Instant::now() - RETRY;
    loop {
        match rx.recv_timeout(Duration::from_millis(1000)) {
            Ok(Msg::Settings(s)) => {
                let s = *s;
                if s != settings {
                    if let Some(mut l) = link.take() {
                        l.set(None);
                    }
                    sent = None;
                    last_try = Instant::now() - RETRY;
                }
                settings = s;
            }
            Ok(Msg::Track(t)) => want = t,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if !settings.enabled || !valid_client_id(settings.effective_client_id()) {
            if let Some(mut l) = link.take() {
                l.set(None);
            }
            sent = None;
            set_status(Status::Off);
            continue;
        }
        if link.is_none() {
            if last_try.elapsed() < RETRY {
                continue;
            }
            last_try = Instant::now();
            set_status(Status::Connecting);
            match Link::connect(settings.effective_client_id()) {
                Some(l) => {
                    link = Some(l);
                    sent = None;
                    set_status(Status::Connected);
                }
                None => {
                    set_status(Status::Unavailable);
                    continue;
                }
            }
        }
        let Some(l) = link.as_mut() else { continue };
        if last_send.elapsed() < MIN_GAP {
            continue;
        }
        let now = unix_now();
        let due = match (&want, &sent) {
            (None, None) => false,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (Some(w), Some((s, start))) => {
                let expected = now - (w.position_ms / 1000) as i64;
                s.track_id != w.track_id
                    || s.play != w.play
                    || (w.play == Play::Playing && (expected - start).abs() * 1000 > DRIFT_MS)
            }
        };
        if !due {
            continue;
        }
        let ok = match &want {
            Some(w) => l.set(Some(activity(w, &settings.design, now))),
            None => l.set(None),
        };
        last_send = Instant::now();
        if ok {
            sent = want.clone().map(|w| {
                let start = now - (w.position_ms / 1000) as i64;
                (w, start)
            });
        } else {
            // Discord closed: connect again on the next round.
            link = None;
            sent = None;
            set_status(Status::Unavailable);
        }
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ---- public ------------------------------------------------------------------------------------------------------

/// Reads the saved settings and starts the worker. Call once from `setup`.
pub fn start(dir: Option<PathBuf>) {
    let path = dir.map(|d| d.join(SETTINGS_FILE));
    let settings: Settings = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| b.len() < 4096)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let (tx, rx) = mpsc::channel();
    let initial = settings.clone();
    let shared = Shared {
        tx,
        status: Mutex::new(Status::Off),
        settings: Mutex::new(settings),
        path,
        last: Mutex::new(None),
    };
    if SHARED.set(shared).is_ok() {
        let _ = std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || worker(rx, initial));
    }
}

pub fn settings() -> (Settings, Status) {
    match SHARED.get() {
        Some(s) => (
            s.settings.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            *s.status.lock().unwrap_or_else(|e| e.into_inner()),
        ),
        None => (Settings::default(), Status::Off),
    }
}

/// Saves and applies new settings. `Err` for a malformed Application ID.
pub fn apply(new: Settings) -> Result<(), &'static str> {
    let clean = Settings {
        enabled: new.enabled,
        client_id: new.client_id.trim().to_owned(),
        design: new.design.clean()?,
    };
    if !clean.client_id.is_empty() && !valid_client_id(&clean.client_id) {
        return Err("invalid application id");
    }
    let Some(sh) = SHARED.get() else {
        return Err("unavailable");
    };
    *sh.settings.lock().unwrap_or_else(|e| e.into_inner()) = clean.clone();
    if let (Some(path), Ok(bytes)) = (&sh.path, serde_json::to_vec(&clean)) {
        let _ = std::fs::write(path, bytes);
    }
    let _ = sh.tx.send(Msg::Settings(Box::new(clean)));
    Ok(())
}

/// Offers the player's latest state. Cheap: it only hands a snapshot to the worker, which decides whether Discord needs an update.
pub fn offer(dto: &PlayerProgressDto) {
    let Some(sh) = SHARED.get() else { return };
    let play = match dto.player.state {
        PlaybackStateDto::Playing | PlaybackStateDto::Buffering => Some(Play::Playing),
        PlaybackStateDto::Paused => Some(Play::Paused),
        _ => None,
    };
    let snapshot = match (&dto.track, play) {
        (Some(t), Some(play)) => Some(Snapshot {
            track_id: t.provider_track_id.clone(),
            title: t.title.clone(),
            artists: t.artists.clone(),
            artwork: t.artwork_url.clone(),
            play,
            position_ms: dto.position_ms,
            duration_ms: dto.duration_ms,
        }),
        _ => None,
    };
    *sh.last.lock().unwrap_or_else(|e| e.into_inner()) = snapshot.clone();
    if sh
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .enabled
    {
        let _ = sh.tx.send(Msg::Track(snapshot));
    }
}

/// The card `design` would show for the song playing now, or for a sample song when nothing plays.
pub fn preview(design: Design) -> Result<Value, &'static str> {
    let design = design.clean()?;
    let last = SHARED
        .get()
        .and_then(|sh| sh.last.lock().unwrap_or_else(|e| e.into_inner()).clone());
    let sample = Snapshot {
        track_id: "dQw4w9WgXcQ".into(),
        title: "Your song title".into(),
        artists: vec!["Artist One".into(), "Artist Two".into()],
        artwork: None,
        play: Play::Playing,
        position_ms: 45_000,
        duration_ms: 212_000,
    };
    Ok(activity(&last.unwrap_or(sample), &design, unix_now()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(play: Play) -> Snapshot {
        Snapshot {
            track_id: "aqz-KE-bpKQ".into(),
            title: "BU".into(),
            artists: vec!["SURIYA MQT".into(), "P6ICK".into()],
            artwork: Some("https://i.ytimg.com/vi/aqz-KE-bpKQ/hqdefault.jpg".into()),
            play,
            position_ms: 30_000,
            duration_ms: 166_000,
        }
    }

    /// A stand-in for the Discord app on the other end of a socket pair.
    #[cfg(unix)]
    #[test]
    fn handshake_then_set_and_clear_activity_against_a_fake_discord() {
        use std::os::unix::net::UnixStream;
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let fake = std::thread::spawn(move || {
            let (op, hello) = read_frame(&mut theirs).unwrap();
            assert_eq!(
                (op, hello["client_id"].as_str()),
                (OP_HANDSHAKE, Some("123456789012345678"))
            );
            theirs
                .write_all(&frame(
                    OP_FRAME,
                    &json!({ "cmd": "DISPATCH", "evt": "READY" }),
                ))
                .unwrap();
            let mut seen = Vec::new();
            for _ in 0..2 {
                let (_, set) = read_frame(&mut theirs).unwrap();
                seen.push(set);
                theirs
                    .write_all(&frame(OP_FRAME, &json!({ "cmd": "SET_ACTIVITY" })))
                    .unwrap();
            }
            seen
        });
        let mut link = Link::over(Box::new(ours), "123456789012345678").expect("READY");
        assert!(link.set(Some(activity(
            &snap(Play::Playing),
            &Design::default(),
            100
        ))));
        assert!(link.set(None));
        let seen = fake.join().unwrap();
        assert_eq!(seen[0]["cmd"], "SET_ACTIVITY");
        assert_eq!(seen[0]["args"]["activity"]["details"], "BU");
        assert_eq!(seen[0]["args"]["pid"], std::process::id());
        assert!(seen[1]["args"]["activity"].is_null());
    }

    #[cfg(unix)]
    #[test]
    fn an_unknown_application_id_is_not_ready() {
        use std::os::unix::net::UnixStream;
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let fake = std::thread::spawn(move || {
            let _ = read_frame(&mut theirs).unwrap();
            theirs
                .write_all(&frame(
                    OP_FRAME,
                    &json!({ "evt": "ERROR", "data": { "code": 4000 } }),
                ))
                .unwrap();
        });
        assert!(Link::over(Box::new(ours), "123456789012345678").is_none());
        fake.join().unwrap();
    }

    #[test]
    fn the_built_in_id_is_valid_and_a_custom_one_replaces_it() {
        assert!(valid_client_id(DEFAULT_CLIENT_ID));
        let mut s = Settings {
            enabled: true,
            client_id: String::new(),
            design: Design::default(),
        };
        assert_eq!(s.effective_client_id(), DEFAULT_CLIENT_ID);
        s.client_id = "123456789012345678".into();
        assert_eq!(s.effective_client_id(), "123456789012345678");
    }

    #[test]
    fn client_ids_are_snowflake_shaped() {
        assert!(valid_client_id("123456789012345678"));
        assert!(!valid_client_id("12345"));
        assert!(!valid_client_id("12345678901234567x"));
        assert!(!valid_client_id(&"1".repeat(21)));
    }

    #[test]
    fn titles_lose_hashtags_and_trailing_symbols_but_keep_their_text() {
        assert_eq!(
            tidy_title("BIG ASS - เล่นของสูง ft. SARAN (Don 420) #saran #ติดตามเค้าหน่อย 🎵🎵"),
            "BIG ASS - เล่นของสูง ft. SARAN (Don 420)"
        );
        assert_eq!(tidy_title("Song - Live ~ "), "Song - Live");
        assert_eq!(tidy_title("#only #tags"), "#only #tags");
        assert_eq!(tidy_title("Plain title"), "Plain title");
    }

    #[test]
    fn frames_round_trip() {
        let bytes = frame(OP_FRAME, &json!({ "a": 1 }));
        assert_eq!(&bytes[0..4], &1u32.to_le_bytes());
        let (op, v) = read_frame(&mut std::io::Cursor::new(bytes)).unwrap();
        assert_eq!((op, v["a"].as_i64()), (OP_FRAME, Some(1)));
        let mut huge = vec![0u8; 8];
        huge[4..8].copy_from_slice(&(1_000_000u32).to_le_bytes());
        assert!(read_frame(&mut std::io::Cursor::new(huge)).is_err());
    }

    #[test]
    fn playing_activity_has_listening_type_times_and_a_link() {
        let a = activity(&snap(Play::Playing), &Design::default(), 1_000_000);
        assert_eq!(a["type"], 2);
        assert_eq!(a["details"], "BU");
        assert_eq!(a["state"], "SURIYA MQT, P6ICK");
        assert_eq!(a["timestamps"]["start"], 1_000_000 - 30);
        assert_eq!(a["timestamps"]["end"], 1_000_000 - 30 + 166);
        assert_eq!(
            a["assets"]["large_image"],
            "https://i.ytimg.com/vi/aqz-KE-bpKQ/hqdefault.jpg"
        );
        assert_eq!(a["status_display_type"], 1);
        assert_eq!(
            a["details_url"],
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
        );
        assert_eq!(
            a["buttons"][0]["url"],
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
        );
    }

    #[test]
    fn paused_activity_drops_the_clock_and_bad_input_is_made_safe() {
        let mut s = snap(Play::Paused);
        s.title = "x".into();
        s.artwork = Some("http://insecure.example/a.jpg".into());
        s.track_id = "not an id".into();
        let a = activity(&s, &Design::default(), 5);
        assert_eq!(a["state"], "Paused · SURIYA MQT, P6ICK");
        assert!(a.get("timestamps").is_none());
        assert_eq!(a["details"], "Unknown title");
        // No usable cover: the logo is the big picture, and there is no corner badge on top of it.
        assert_eq!(a["assets"]["large_image"], LOGO_ASSET);
        assert!(a["assets"].get("small_image").is_none());
        // A badge text with no badge picture would show nothing, so it is left out too.
        assert!(a["assets"].get("small_text").is_none());
        let with_cover = activity(&snap(Play::Paused), &Design::default(), 5);
        assert_eq!(with_cover["assets"]["small_text"], "LunoRX - paused");
        // No valid video id: only the link to the app remains.
        assert_eq!(a["buttons"].as_array().unwrap().len(), 1);
        assert_eq!(a["buttons"][0]["label"], "Get LunoRX");
    }

    fn design(f: impl FnOnce(&mut Design)) -> Design {
        let mut d = Design::default();
        f(&mut d);
        d
    }

    #[test]
    fn a_custom_design_changes_text_images_clock_and_buttons() {
        let d = design(|d| {
            d.details = "{artist} - {title}".into();
            d.state = "".into();
            d.large_image = "catalog:mine".into();
            d.catalog = vec![CatalogEntry {
                name: "mine".into(),
                value: "my_art".into(),
            }];
            d.small_image = "cover".into();
            d.timestamps = "elapsed".into();
            d.status_display = "title".into();
            d.listen_button = false;
            d.buttons = vec![CardButton {
                label: "My site".into(),
                url: "https://example.com".into(),
            }];
        });
        let a = activity(&snap(Play::Playing), &d.clean().unwrap(), 1_000);
        assert_eq!(a["details"], "SURIYA MQT - BU");
        assert!(
            a.get("state").is_none(),
            "an empty template leaves the line out"
        );
        assert_eq!(a["assets"]["large_image"], "my_art");
        assert_eq!(
            a["assets"]["small_image"],
            "https://i.ytimg.com/vi/aqz-KE-bpKQ/hqdefault.jpg"
        );
        assert_eq!(a["timestamps"], json!({ "start": 970 }));
        assert_eq!(a["status_display_type"], 2);
        let labels: Vec<_> = a["buttons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["label"].as_str().unwrap())
            .collect();
        assert_eq!(labels, ["My site", "Get LunoRX"]);
    }

    #[test]
    fn at_most_two_buttons_and_custom_ones_win() {
        let d = design(|d| {
            d.buttons = vec![
                CardButton {
                    label: "One".into(),
                    url: "{track_url}".into(),
                },
                CardButton {
                    label: "Two".into(),
                    url: "https://example.com".into(),
                },
            ];
        });
        let a = activity(&snap(Play::Playing), &d.clean().unwrap(), 5);
        let labels: Vec<_> = a["buttons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["label"].as_str().unwrap())
            .collect();
        assert_eq!(labels, ["One", "Two"]);
        assert_eq!(
            a["buttons"][0]["url"],
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
        );
    }

    #[test]
    fn a_design_is_checked_before_it_is_saved() {
        assert!(Design::default().clean().is_ok());
        assert_eq!(
            design(|d| d.timestamps = "x".into()).clean().unwrap_err(),
            "clock"
        );
        assert_eq!(
            design(|d| d.large_image = "none".into())
                .clean()
                .unwrap_err(),
            "large image"
        );
        assert_eq!(
            design(|d| d.large_image = "catalog:missing".into())
                .clean()
                .unwrap_err(),
            "large image"
        );
        assert_eq!(
            design(|d| d.details = "a".repeat(129)).clean().unwrap_err(),
            "details"
        );
        assert_eq!(
            design(|d| d.catalog = vec![CatalogEntry {
                name: "Bad Name".into(),
                value: "x".into()
            }])
            .clean()
            .unwrap_err(),
            "catalog name"
        );
        assert_eq!(
            design(|d| d.catalog = vec![CatalogEntry {
                name: "a".into(),
                value: "http://insecure".into()
            }])
            .clean()
            .unwrap_err(),
            "catalog image"
        );
        assert_eq!(
            design(|d| d.buttons = vec![CardButton {
                label: "x".into(),
                url: "javascript:alert(1)".into()
            }])
            .clean()
            .unwrap_err(),
            "button"
        );
        assert_eq!(
            design(|d| d.buttons = vec![
                CardButton {
                    label: "x".into(),
                    url: "{track_url}".into()
                };
                3
            ])
            .clean()
            .unwrap_err(),
            "buttons"
        );
    }

    #[test]
    fn old_settings_files_without_a_design_still_load_with_the_default_look() {
        let s: Settings = serde_json::from_str(r#"{"enabled":true,"clientId":""}"#).unwrap();
        assert_eq!(s.design, Design::default());
    }
}

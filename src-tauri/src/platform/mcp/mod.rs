//! Local MCP server: lets an AI client on this computer (Claude Code, Claude Desktop, ...) control the player.
//! Off by default; the user turns it on in Settings. It listens on 127.0.0.1 only and every request needs the
//! bearer token, which lives in the OS key store like the other credentials and is registered with the masker.
//!
//! Tools reach Core exactly like the media keys do (`control::run`) or like the webview does (the `player_*` and
//! search commands), so validation, the engine lock and state events are the same; the player UI follows along.
pub mod http;
pub mod protocol;

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::control::{self, Remote};
use crate::commands::player::{
    PlayerLoadArgs, PlayerQueueEditArgs, PlayerState, player_load, player_play, player_queue_edit,
    progress,
};
use crate::core::playback::cache::OfflineTrack;
use protocol::{Runner, Tool, ToolError};

const SETTINGS_FILE: &str = "mcp.json";
pub const DEFAULT_PORT: u16 = 47615;
const KEYRING_SERVICE: &str = "io.lunorx.desktop";
const KEYRING_ACCOUNT: &str = "mcp-token";
/// Requests handled at once; more wait. A local client sends one at a time.
const MAX_CONCURRENT: usize = 4;
const PROVIDER: &str = "YouTube";
const BIND_ATTEMPTS: u32 = 5;
const BIND_RETRY: std::time::Duration = std::time::Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
        }
    }
}

/// What the Settings panel shows. `PortInUse` and `KeyStoreUnavailable` keep the switch on but explain why
/// nothing is listening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Off,
    Listening,
    PortInUse,
    KeyStoreUnavailable,
}

struct Server {
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    /// Aborts the task and waits until it has ended, so its listener is closed and the port is free.
    async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}

struct Shared {
    app: AppHandle,
    path: Option<PathBuf>,
    settings: Mutex<Settings>,
    status: Mutex<Status>,
    server: Mutex<Option<Server>>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- token ---------------------------------------------------------------------------------------------------

fn token_entry() -> Option<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).ok()
}

fn new_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The saved token, creating one the first time. `None` when the key store cannot be used.
fn token() -> Option<String> {
    let entry = token_entry()?;
    let token = match entry.get_password() {
        Ok(t) if t.len() == 64 => t,
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let t = new_token();
            entry.set_password(&t).ok()?;
            t
        }
        Err(_) => return None,
    };
    crate::core::secrets::registry().register(&token);
    Some(token)
}

// ---- lifecycle -----------------------------------------------------------------------------------------------

/// Reads the saved settings and starts listening when the user left it on. Call once from `setup`.
pub fn start(app: &AppHandle) {
    let path = app
        .path()
        .app_data_dir()
        .ok()
        .map(|d| d.join(SETTINGS_FILE));
    let settings: Settings = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| b.len() < 1024)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .filter(|s: &Settings| valid_port(s.port))
        .unwrap_or_default();
    let shared = Shared {
        app: app.clone(),
        path,
        settings: Mutex::new(settings),
        status: Mutex::new(Status::Off),
        server: Mutex::new(None),
    };
    if SHARED.set(shared).is_ok() {
        restart();
    }
}

/// Unprivileged ports only, so a typo cannot ask for 80 or 0.
pub fn valid_port(port: u16) -> bool {
    port >= 1024
}

pub fn settings() -> (Settings, Status) {
    match SHARED.get() {
        Some(sh) => (*lock(&sh.settings), *lock(&sh.status)),
        None => (Settings::default(), Status::Off),
    }
}

/// The address a client connects to, with the token. Only for the Settings panel (main window).
pub fn connection() -> Option<(String, String)> {
    let (s, _) = settings();
    Some((
        format!("http://127.0.0.1:{}{}", s.port, http::PATH),
        token()?,
    ))
}

pub fn apply(new: Settings) -> Result<(), &'static str> {
    if !valid_port(new.port) {
        return Err("invalid port");
    }
    let Some(sh) = SHARED.get() else {
        return Err("unavailable");
    };
    // Saved first: a choice that could not be written must not look applied, then revert at the next launch.
    let path = sh.path.as_ref().ok_or("unavailable")?;
    let bytes = serde_json::to_vec(&new).map_err(|_| "unavailable")?;
    std::fs::write(path, bytes).map_err(|_| "unavailable")?;
    *lock(&sh.settings) = new;
    restart();
    Ok(())
}

/// Replaces the token. Clients holding the old one are refused from the next request on.
pub fn rotate_token() -> Result<(), &'static str> {
    let entry = token_entry().ok_or("unavailable")?;
    let t = new_token();
    entry.set_password(&t).map_err(|_| "unavailable")?;
    crate::core::secrets::registry().register(&t);
    restart();
    Ok(())
}

fn restart() {
    let Some(sh) = SHARED.get() else { return };
    let old = lock(&sh.server).take();
    let s = *lock(&sh.settings);
    if !s.enabled {
        *lock(&sh.status) = Status::Off;
        if let Some(old) = old {
            tauri::async_runtime::spawn(old.stop());
        }
        return;
    }
    let Some(token) = token() else {
        *lock(&sh.status) = Status::KeyStoreUnavailable;
        return;
    };
    let app = sh.app.clone();
    let task = tauri::async_runtime::spawn(async move {
        // Abort is asynchronous: wait for the old task so its listener is closed before binding the same port.
        if let Some(old) = old {
            old.stop().await;
        }
        let Some(listener) = bind(s.port).await else {
            if let Some(sh) = SHARED.get() {
                *lock(&sh.status) = Status::PortInUse;
            }
            return;
        };
        if let Some(sh) = SHARED.get() {
            *lock(&sh.status) = Status::Listening;
        }
        serve(listener, app, token, s.port).await;
    });
    *lock(&sh.server) = Some(Server { task });
}

/// Binds the loopback port. A switch turned off and on again quickly can still find the previous listener closing
/// (its stop runs on another task), so a busy port is retried briefly before it is reported as in use.
async fn bind(port: u16) -> Option<TcpListener> {
    for attempt in 0..BIND_ATTEMPTS {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => return Some(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && attempt + 1 < BIND_ATTEMPTS => {
                tokio::time::sleep(BIND_RETRY).await;
            }
            Err(_) => return None,
        }
    }
    None
}

async fn serve(listener: TcpListener, app: AppHandle, token: String, port: u16) {
    let slots = std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT));
    let token = std::sync::Arc::new(token);
    loop {
        let Ok((mut stream, peer)) = listener.accept().await else {
            continue;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        let Ok(permit) = slots.clone().acquire_owned().await else {
            return;
        };
        let app = app.clone();
        let token = token.clone();
        tauri::async_runtime::spawn(async move {
            let _permit = permit;
            let req = match http::read(&mut stream).await {
                Ok(r) => r,
                Err(why) => return http::refuse(&mut stream, why).await,
            };
            if let Err(why) = http::admit(&req, &token, port) {
                return http::refuse(&mut stream, why).await;
            }
            let reply = match serde_json::from_slice::<Value>(&req.body) {
                Ok(msg) => protocol::handle(&msg, &AppRunner { app }).await,
                Err(_) => Some(json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": { "code": protocol::PARSE_ERROR, "message": "parse error" },
                })),
            };
            match reply {
                Some(v) => http::respond_json(&mut stream, v.to_string().as_bytes()).await,
                None => http::respond_accepted(&mut stream).await,
            }
        });
    }
}

// ---- tools ---------------------------------------------------------------------------------------------------

/// A playable result from the latest `search`. `play_track` and `queue_next` only accept these ids, so the model
/// cannot invent a track or its metadata.
#[derive(Clone)]
struct Hit {
    id: String,
    title: String,
    artists: Vec<String>,
    artwork_url: Option<String>,
    duration_ms: Option<u64>,
    canonical_url: String,
}

static LAST_SEARCH: Mutex<Vec<Hit>> = Mutex::new(Vec::new());

fn find_hit(id: &str) -> Result<Hit, ToolError> {
    lock(&LAST_SEARCH)
        .iter()
        .find(|h| h.id == id)
        .cloned()
        .ok_or_else(|| {
            ToolError("That id is not in the latest search results. Run `search` first.".into())
        })
}

struct AppRunner {
    app: AppHandle,
}

impl AppRunner {
    async fn now_playing(&self) -> Value {
        let state = self.app.state::<PlayerState>();
        let p = progress(&state).await;
        json!({
            "state": p.player.state,
            "track": p.track.as_ref().map(|t| json!({
                "id": t.provider_track_id,
                "title": t.title,
                "artists": t.artists,
            })),
            "positionSeconds": p.position_ms / 1000,
            "durationSeconds": p.duration_ms / 1000,
            "volume": p.volume,
        })
    }

    async fn remote(&self, cmd: Remote) -> Value {
        control::run(self.app.clone(), cmd).await;
        self.now_playing().await
    }

    async fn search(&self, query: String, limit: u8) -> Result<Value, ToolError> {
        use crate::commands::dto::SearchQueryArgs;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let state = self.app.state::<crate::commands::state::SearchState>();
        let args = SearchQueryArgs {
            query,
            providers: vec![PROVIDER.to_owned()],
            limit,
            cancellation_id: format!("mcp-{}", SEQ.fetch_add(1, Ordering::Relaxed)),
        };
        let response =
            crate::commands::search::run_search(&state, args, crate::commands::clock::today())
                .await
                .map_err(|_| {
                    ToolError("Search failed. Check that YouTube Music is reachable.".into())
                })?;
        let hits: Vec<Hit> = response
            .hits
            .into_iter()
            .filter(|h| h.provider == PROVIDER)
            .map(|h| Hit {
                id: h.provider_track_id,
                title: h.title,
                artists: h.artists,
                artwork_url: h.artwork_url,
                duration_ms: h.duration_ms,
                canonical_url: h.canonical_url,
            })
            .collect();
        let out = hits
            .iter()
            .map(|h| {
                json!({
                    "id": h.id,
                    "title": h.title,
                    "artists": h.artists,
                    "durationSeconds": h.duration_ms.map(|ms| ms / 1000),
                })
            })
            .collect::<Vec<_>>();
        *lock(&LAST_SEARCH) = hits;
        Ok(json!({ "results": out }))
    }

    async fn play_track(&self, id: String) -> Result<Value, ToolError> {
        let hit = find_hit(&id)?;
        let state = self.app.state::<PlayerState>();
        let args = PlayerLoadArgs {
            queue_selection: false,
            artists: hit.artists,
            artwork_url: hit.artwork_url,
            duration_ms: hit.duration_ms,
            provider: PROVIDER.to_owned(),
            provider_track_id: hit.id,
            canonical_url: hit.canonical_url,
            title: hit.title,
            following: Vec::new(),
        };
        player_load(self.app.clone(), state.clone(), args)
            .await
            .map_err(|e| ToolError(format!("LunoRX could not load the track: {}", e.message)))?;
        player_play(self.app.clone(), state)
            .await
            .map_err(|e| ToolError(format!("The track loaded but did not start: {}", e.message)))?;
        // The state is whatever the engine reports now (often preparing or buffering), never assumed "playing".
        Ok(self.now_playing().await)
    }

    async fn queue_next(&self, id: String) -> Result<Value, ToolError> {
        let hit = find_hit(&id)?;
        let args = PlayerQueueEditArgs {
            op: "next".into(),
            track: Some(OfflineTrack {
                provider: PROVIDER.to_owned(),
                id: hit.id,
                title: hit.title.clone(),
                artists: hit.artists,
                artwork_url: hit.artwork_url,
                duration_ms: hit.duration_ms.and_then(|ms| i64::try_from(ms).ok()),
            }),
            id: None,
            from: None,
            to: None,
        };
        player_queue_edit(self.app.clone(), self.app.state::<PlayerState>(), args)
            .await
            .map_err(|e| ToolError(format!("Could not queue the track: {}", e.message)))?;
        Ok(json!({ "queuedNext": hit.title }))
    }
}

impl Runner for AppRunner {
    async fn run(&self, tool: Tool) -> Result<Value, ToolError> {
        match tool {
            Tool::NowPlaying => Ok(self.now_playing().await),
            Tool::Play => Ok(self.remote(Remote::Play).await),
            Tool::Pause => Ok(self.remote(Remote::Pause).await),
            Tool::Toggle => Ok(self.remote(Remote::Toggle).await),
            Tool::Next => Ok(self.remote(Remote::Next).await),
            Tool::Previous => Ok(self.remote(Remote::Previous).await),
            Tool::Seek { position_ms } => Ok(self.remote(Remote::SeekTo(position_ms)).await),
            Tool::SetVolume { percent } => {
                let state = self.app.state::<PlayerState>();
                let reply =
                    crate::commands::player::run_set_volume(&state, i64::from(percent)).await;
                crate::commands::player::finish(&self.app, reply)
                    .map_err(|e| ToolError(format!("Volume not changed: {}", e.message)))?;
                // `core://player/state` does not fire for volume, so push progress for the slider, as `control` does.
                let dto = progress(&state).await;
                let _ = crate::commands::emit::emit_main(&self.app, "core://player/progress", &dto);
                Ok(self.now_playing().await)
            }
            Tool::Search { query, limit } => self.search(query, limit).await,
            Tool::PlayTrack { id } => self.play_track(id).await,
            Tool::QueueNext { id } => self.queue_next(id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn bind_waits_for_a_listener_that_is_closing() {
        block(async {
            let old = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let port = old.local_addr().unwrap().port();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                drop(old);
            });
            assert!(bind(port).await.is_some());
        });
    }

    #[test]
    fn bind_gives_up_on_a_port_that_stays_busy() {
        block(async {
            let held = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let port = held.local_addr().unwrap().port();
            assert!(bind(port).await.is_none());
        });
    }

    #[test]
    fn privileged_ports_are_refused() {
        assert!(!valid_port(80));
        assert!(!valid_port(0));
        assert!(valid_port(DEFAULT_PORT));
    }
}

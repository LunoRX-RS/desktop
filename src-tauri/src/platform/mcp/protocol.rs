//! MCP (Model Context Protocol) over JSON-RPC 2.0: message handling, the tool list and argument validation.
//! Pure: no Tauri and no sockets, so every rule here is tested without the app. Running a tool is the
//! `Runner`'s job; this file only decides which tool, with which validated arguments.
use serde_json::{Value, json};

/// The revision this server implements. Older clients that ask for a revision we also understand get it back.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
const ACCEPTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const MAX_QUERY_CHARS: usize = 200;
pub const MAX_SEARCH_LIMIT: u8 = 10;
const DEFAULT_SEARCH_LIMIT: u8 = 5;
/// YouTube video ids are 11 characters of [A-Za-z0-9_-]; anything else never reaches Core.
const VIDEO_ID_LEN: usize = 11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tool {
    NowPlaying,
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek { position_ms: i64 },
    SetVolume { percent: u8 },
    Search { query: String, limit: u8 },
    PlayTrack { id: String },
    QueueNext { id: String },
}

/// A tool ran but refused or failed. The text is shown to the model; it is fixed wording, never backend text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(pub String);

pub trait Runner {
    fn run(&self, tool: Tool)
    -> impl std::future::Future<Output = Result<Value, ToolError>> + Send;
}

fn valid_video_id(id: &str) -> bool {
    id.len() == VIDEO_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn id_arg(args: &Value) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .ok_or("`id` is required: a track id from `search`")?;
    if valid_video_id(id) {
        Ok(id.to_owned())
    } else {
        Err("`id` is not a track id from `search`".into())
    }
}

/// Maps a `tools/call` name and arguments to a validated tool. `Err` text goes back as an invalid-params error.
pub fn parse_tool(name: &str, args: &Value) -> Result<Tool, String> {
    let tool = match name {
        "now_playing" => Tool::NowPlaying,
        "play" => Tool::Play,
        "pause" => Tool::Pause,
        "toggle_play_pause" => Tool::Toggle,
        "next_track" => Tool::Next,
        "previous_track" => Tool::Previous,
        "seek" => {
            let seconds = args
                .get("seconds")
                .and_then(Value::as_f64)
                .ok_or("`seconds` is required")?;
            if !(0.0..=86_400.0).contains(&seconds) {
                return Err("`seconds` must be between 0 and 86400".into());
            }
            Tool::Seek {
                position_ms: (seconds * 1000.0).round() as i64,
            }
        }
        "set_volume" => {
            let percent = args
                .get("percent")
                .and_then(Value::as_u64)
                .ok_or("`percent` is required")?;
            if percent > 100 {
                return Err("`percent` must be between 0 and 100".into());
            }
            Tool::SetVolume {
                percent: percent as u8,
            }
        }
        "search" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
                return Err(format!("`query` must be 1 to {MAX_QUERY_CHARS} characters"));
            }
            let limit = match args.get("limit") {
                None | Some(Value::Null) => DEFAULT_SEARCH_LIMIT,
                Some(v) => match v.as_u64() {
                    Some(n) if (1..=u64::from(MAX_SEARCH_LIMIT)).contains(&n) => n as u8,
                    _ => {
                        return Err(format!("`limit` must be between 1 and {MAX_SEARCH_LIMIT}"));
                    }
                },
            };
            Tool::Search {
                query: query.to_owned(),
                limit,
            }
        }
        "play_track" => Tool::PlayTrack { id: id_arg(args)? },
        "queue_next" => Tool::QueueNext { id: id_arg(args)? },
        _ => return Err(format!("unknown tool `{name}`")),
    };
    Ok(tool)
}

fn no_args() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn tool(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false },
    })
}

pub fn tool_list() -> Value {
    let id_schema = json!({
        "type": "object",
        "properties": { "id": { "type": "string", "description": "A track id returned by `search`." } },
        "required": ["id"],
        "additionalProperties": false,
    });
    json!([
        tool(
            "now_playing",
            "The track LunoRX is playing now, the player state, position, duration and volume.",
            no_args(),
            true
        ),
        tool(
            "play",
            "Start or resume playback of the loaded track.",
            no_args(),
            false
        ),
        tool("pause", "Pause playback.", no_args(), false),
        tool(
            "toggle_play_pause",
            "Pause when playing, resume when paused.",
            no_args(),
            false
        ),
        tool(
            "next_track",
            "Skip to the next track in the queue.",
            no_args(),
            false
        ),
        tool(
            "previous_track",
            "Restart the track, or go to the previous one when it has just begun.",
            no_args(),
            false
        ),
        tool(
            "seek",
            "Jump to a position in the current track.",
            json!({
                "type": "object",
                "properties": { "seconds": { "type": "number", "minimum": 0, "description": "Position from the start, in seconds." } },
                "required": ["seconds"],
                "additionalProperties": false,
            }),
            false
        ),
        tool(
            "set_volume",
            "Set the player volume.",
            json!({
                "type": "object",
                "properties": { "percent": { "type": "integer", "minimum": 0, "maximum": 100 } },
                "required": ["percent"],
                "additionalProperties": false,
            }),
            false
        ),
        tool(
            "search",
            "Search YouTube Music for playable tracks. Returns ids for `play_track` and `queue_next`.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "minLength": 1, "maxLength": MAX_QUERY_CHARS },
                    "limit": { "type": "integer", "minimum": 1, "maximum": MAX_SEARCH_LIMIT, "default": DEFAULT_SEARCH_LIMIT },
                },
                "required": ["query"],
                "additionalProperties": false,
            }),
            true
        ),
        tool(
            "play_track",
            "Play a track from the latest `search` results now.",
            id_schema.clone(),
            false
        ),
        tool(
            "queue_next",
            "Play a track from the latest `search` results after the current one.",
            id_schema,
            false
        ),
    ])
}

fn reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Handles one JSON-RPC message. `None` for notifications and responses, which get no reply.
pub async fn handle<R: Runner>(msg: &Value, runner: &R) -> Option<Value> {
    let Some(obj) = msg.as_object() else {
        // Batches were removed from MCP in 2025-06-18; a bare array is not a request.
        return Some(error(&Value::Null, INVALID_REQUEST, "invalid request"));
    };
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(error(&Value::Null, INVALID_REQUEST, "invalid request"));
    }
    let id = match obj.get("id") {
        Some(id @ (Value::String(_) | Value::Number(_))) => id.clone(),
        Some(_) => return Some(error(&Value::Null, INVALID_REQUEST, "invalid request")),
        // A notification (e.g. notifications/initialized) or a response to us: nothing to answer.
        None => return None,
    };
    let method = obj.get("method").and_then(Value::as_str)?;
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|v| ACCEPTED_VERSIONS.contains(v))
                .unwrap_or(PROTOCOL_VERSION);
            reply(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "lunorx", "title": "LunoRX", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Controls the LunoRX music player on this computer. Use `search` to find a track, then `play_track` or `queue_next` with its id. `now_playing` reports what is actually playing; a track is only playing once it says so.",
                }),
            )
        }
        "ping" => reply(&id, json!({})),
        "tools/list" => reply(&id, json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match parse_tool(name, &args) {
                Err(message) => error(&id, INVALID_PARAMS, &message),
                Ok(tool) => match runner.run(tool).await {
                    Ok(value) => reply(
                        &id,
                        json!({
                            "content": [{ "type": "text", "text": value.to_string() }],
                            "structuredContent": value,
                            "isError": false,
                        }),
                    ),
                    // Tool failures are results the model can read and react to, not protocol errors.
                    Err(ToolError(text)) => reply(
                        &id,
                        json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
                    ),
                },
            }
        }
        _ => error(&id, METHOD_NOT_FOUND, "method not found"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        ran: Mutex<Vec<Tool>>,
        fail: bool,
    }

    impl Runner for Recorder {
        async fn run(&self, tool: Tool) -> Result<Value, ToolError> {
            self.ran.lock().unwrap().push(tool);
            if self.fail {
                Err(ToolError("nothing is loaded".into()))
            } else {
                Ok(json!({ "state": "playing" }))
            }
        }
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn call(name: &str, args: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } })
    }

    #[test]
    fn initialize_answers_with_a_known_version() {
        let r = Recorder::default();
        let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } });
        let out = block(handle(&msg, &r)).unwrap();
        assert_eq!(out["result"]["protocolVersion"], "2025-03-26");
        assert!(out["result"]["capabilities"]["tools"].is_object());
        let unknown = json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } });
        assert_eq!(
            block(handle(&unknown, &r)).unwrap()["result"]["protocolVersion"],
            PROTOCOL_VERSION
        );
    }

    #[test]
    fn notifications_and_batches() {
        let r = Recorder::default();
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(block(handle(&note, &r)), None);
        let batch = json!([{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]);
        assert_eq!(
            block(handle(&batch, &r)).unwrap()["error"]["code"],
            INVALID_REQUEST
        );
        let bad_version = json!({ "jsonrpc": "1.0", "id": 1, "method": "ping" });
        assert_eq!(
            block(handle(&bad_version, &r)).unwrap()["error"]["code"],
            INVALID_REQUEST
        );
    }

    #[test]
    fn every_listed_tool_parses_with_valid_arguments() {
        for t in tool_list().as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            let args = match name {
                "seek" => json!({ "seconds": 30 }),
                "set_volume" => json!({ "percent": 40 }),
                "search" => json!({ "query": "lofi" }),
                "play_track" | "queue_next" => json!({ "id": "aqz-KE-bpKQ" }),
                _ => json!({}),
            };
            assert!(parse_tool(name, &args).is_ok(), "{name}");
        }
    }

    #[test]
    fn arguments_are_validated_before_running() {
        let r = Recorder::default();
        for (name, args) in [
            ("set_volume", json!({ "percent": 101 })),
            ("set_volume", json!({})),
            ("seek", json!({ "seconds": -1 })),
            ("search", json!({ "query": "   " })),
            (
                "search",
                json!({ "query": "x".repeat(MAX_QUERY_CHARS + 1) }),
            ),
            ("search", json!({ "query": "ok", "limit": 0 })),
            ("search", json!({ "query": "ok", "limit": 11 })),
            ("play_track", json!({ "id": "../../etc" })),
            ("play_track", json!({ "id": "short" })),
            ("queue_next", json!({})),
            ("format_disk", json!({})),
        ] {
            let out = block(handle(&call(name, args), &r)).unwrap();
            assert_eq!(out["error"]["code"], INVALID_PARAMS, "{name}");
        }
        assert!(r.ran.lock().unwrap().is_empty());
    }

    #[test]
    fn values_are_converted() {
        assert_eq!(
            parse_tool("seek", &json!({ "seconds": 12.5 })),
            Ok(Tool::Seek {
                position_ms: 12_500
            })
        );
        assert_eq!(
            parse_tool("search", &json!({ "query": "  city pop  " })),
            Ok(Tool::Search {
                query: "city pop".into(),
                limit: DEFAULT_SEARCH_LIMIT
            })
        );
    }

    #[test]
    fn tool_results_and_failures() {
        let ok = Recorder::default();
        let out = block(handle(&call("now_playing", json!({})), &ok)).unwrap();
        assert_eq!(out["result"]["isError"], false);
        assert_eq!(out["result"]["structuredContent"]["state"], "playing");
        assert_eq!(*ok.ran.lock().unwrap(), vec![Tool::NowPlaying]);

        let failing = Recorder {
            fail: true,
            ..Default::default()
        };
        let out = block(handle(&call("play", json!({})), &failing)).unwrap();
        assert_eq!(out["result"]["isError"], true);
        assert_eq!(out["result"]["content"][0]["text"], "nothing is loaded");
    }

    #[test]
    fn unknown_method() {
        let r = Recorder::default();
        let msg = json!({ "jsonrpc": "2.0", "id": "a", "method": "resources/list" });
        let out = block(handle(&msg, &r)).unwrap();
        assert_eq!(out["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(out["id"], "a");
    }
}

//! Settings for the local MCP server (AI control of the player). The server itself is `platform::mcp`.
use crate::platform::mcp::{self, Settings, Status};
use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDto {
    pub enabled: bool,
    pub port: u16,
    pub default_port: u16,
    pub status: Status,
    /// Endpoint and token for the client config. `None` when the key store is unavailable.
    pub url: Option<String>,
    pub token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpArgs {
    pub enabled: bool,
    pub port: u16,
}

fn main(window: &WebviewWindow) -> Result<(), &'static str> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("forbidden")
    }
}

#[tauri::command]
pub fn mcp_settings_get(window: WebviewWindow) -> Result<McpDto, &'static str> {
    main(&window)?;
    let (s, status) = mcp::settings();
    let (url, token) = match mcp::connection() {
        Some((u, t)) => (Some(u), Some(t)),
        None => (None, None),
    };
    Ok(McpDto {
        enabled: s.enabled,
        port: s.port,
        default_port: mcp::DEFAULT_PORT,
        status,
        url,
        token,
    })
}

/// Saves the choice and starts or stops the server. Refuses a privileged port.
#[tauri::command]
pub fn mcp_settings_set(window: WebviewWindow, args: McpArgs) -> Result<(), &'static str> {
    main(&window)?;
    mcp::apply(Settings {
        enabled: args.enabled,
        port: args.port,
    })
}

/// Issues a new token; clients using the old one stop working.
#[tauri::command]
pub fn mcp_rotate_token(window: WebviewWindow) -> Result<(), &'static str> {
    main(&window)?;
    mcp::rotate_token()
}

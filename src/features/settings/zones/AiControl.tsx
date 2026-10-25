"use client";

import { useEffect, useState } from "react";
import { Switch } from "../../../components/Lyrics";
import { Button } from "../../../components/ui";
import { copyText } from "../../../lib/clipboard";
import { mcpRotateToken, mcpSettingsGet, mcpSettingsSet, type McpSettings, type McpStatus } from "../../../services/tauri/mcp";
import { FIELD, Inset, NoteRow, SettingRow, type Tone } from "../rows";

const STATUS: Record<McpStatus, { tone: Tone; text: string }> = {
  off: { tone: "off", text: "Off. No AI client can reach the player." },
  listening: { tone: "ok", text: "On. AI clients on this computer that have the token can control playback." },
  portInUse: { tone: "warn", text: "Another program is using this port. Pick a different one." },
  keyStoreUnavailable: { tone: "warn", text: "The system key store is locked or missing, so there is no token to protect the server. Unlock it and try again." },
};

/** Settings > AI control. Off by default: a local MCP server lets AI tools like Claude play, pause, search and queue. */
export function AiControl() {
  const [s, setS] = useState<McpSettings | null>(null);
  const [port, setPort] = useState("");
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [showToken, setShowToken] = useState(false);
  const read = () => mcpSettingsGet().then((v) => { setS(v); return v; }, () => null);
  useEffect(() => {
    let live = true;
    void read().then((v) => { if (live && v) setPort(String(v.port)); });
    return () => { live = false; };
  }, []);
  async function save(enabled: boolean, p: string) {
    setError("");
    const n = Number(p);
    if (!Number.isInteger(n) || n < 1024 || n > 65535) { setError("The port must be a whole number from 1024 to 65535."); return; }
    try { await mcpSettingsSet(enabled, n); } catch { setError("The setting could not be saved."); }
    // The server binds a moment after the switch; read the status once it has.
    await read();
    window.setTimeout(() => void read(), 600);
  }
  async function copy(label: string, text: string) {
    setCopied((await copyText(text)) ? label : "");
    window.setTimeout(() => setCopied(""), 2000);
  }
  if (!s) return null;
  const command = s.url && s.token ? `claude mcp add --transport http lunorx ${s.url} --header "Authorization: Bearer ${s.token}"` : null;
  const status = STATUS[s.status];
  return (
    <Inset id="settings-ai" title="AI control">
      <SettingRow label="Let AI tools control the player" hint="Starts a Model Context Protocol server on this computer only (127.0.0.1). An AI client with the token can see what is playing, play, pause, skip, change volume, search YouTube Music and play or queue a result.">
        <Switch on={s.enabled} onChange={(on) => void save(on, port)} name="Let AI tools control the player">{s.enabled ? "On" : "Off"}</Switch>
      </SettingRow>
      <SettingRow label="Port" htmlFor="mcp-port" hint={`Default ${s.defaultPort}. Change it only if another program uses that port.`}>
        <input id="mcp-port" inputMode="numeric" autoComplete="off" spellCheck={false} value={port}
          onChange={(e) => setPort(e.target.value)} onBlur={() => { if (port !== String(s.port)) void save(s.enabled, port); }} className={`${FIELD} w-[110px] tabular-nums`} />
      </SettingRow>
      <NoteRow tone={status.tone} role="status">{status.text}</NoteRow>
      {error && <NoteRow tone="bad" role="alert">{error}</NoteRow>}
      {s.enabled && command && (
        <>
          <SettingRow label="Add to Claude Code" hint="Run this once in a terminal. It contains the token: do not share it.">
            <Button onClick={() => void copy("command", command)}>{copied === "command" ? "Copied" : "Copy command"}</Button>
          </SettingRow>
          <SettingRow label="Token" hint="For other MCP clients: send it as an Authorization: Bearer header.">
            <div className="flex items-center gap-2">
              <code className="max-w-[260px] truncate text-xs tabular-nums" aria-label="MCP token">{showToken ? s.token : "•".repeat(16)}</code>
              <Button onClick={() => setShowToken((v) => !v)}>{showToken ? "Hide" : "Show"}</Button>
              <Button onClick={() => void copy("token", s.token ?? "")}>{copied === "token" ? "Copied" : "Copy"}</Button>
            </div>
          </SettingRow>
          <SettingRow label="New token" hint="Clients that use the old token stop working until you update them.">
            <Button onClick={() => void mcpRotateToken().then(read, () => setError("A new token could not be made."))}>Make a new token</Button>
          </SettingRow>
        </>
      )}
    </Inset>
  );
}

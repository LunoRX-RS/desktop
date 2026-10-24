"use client";

import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { usePlayback } from "../../../lib/playback";
import { useDevLog } from "../../../lib/devLog";
import { useSettingsStore } from "../../../lib/settingsStore";
import { youtubeConnectionStatus, subscribeYouTubeConnection, type YouTubeConnectionStatus } from "../../../services/tauri/youtube";
import { Button } from "../../../components/ui";
import { Switch } from "../../../components/Lyrics";
import { Inset, NoteRow, SettingRow, StatusRow } from "../rows";

const SESSION_LABELS: Record<YouTubeConnectionStatus, { tone: "ok" | "off" | "warn" | "bad"; text: string }> = {
  connected: { tone: "ok", text: "Connected" },
  connecting: { tone: "warn", text: "Connecting" },
  restoring: { tone: "warn", text: "Restoring" },
  disconnected: { tone: "off", text: "Not connected" },
  expired: { tone: "bad", text: "Session expired" },
  storeError: { tone: "bad", text: "Key store unavailable" },
  unavailable: { tone: "off", text: "Unavailable" },
};

export function DevMode() {
  const devMode = useSettingsStore((s) => s.devMode);
  const toggleDevMode = useSettingsStore((s) => s.toggleDevMode);
  const nativeAvailable = usePlayback((s) => s.available);
  const entries = useDevLog((s) => s.entries);
  const clear = useDevLog((s) => s.clear);
  const [session, setSession] = useState<YouTubeConnectionStatus | null>(null);
  useEffect(() => {
    let active = true;
    let stop: UnlistenFn | undefined;
    subscribeYouTubeConnection((next) => { if (active) setSession(next); })
      .then(async (unlisten) => {
        if (!active) { unlisten(); return; }
        stop = unlisten;
        if (active) setSession(await youtubeConnectionStatus());
      })
      .catch(() => { if (active) setSession("unavailable"); });
    return () => { active = false; stop?.(); };
  }, []);

  return (
    <>
      <Inset id="settings-dev-mode" title="Dev Mode">
        <SettingRow label="Dev Mode" hint="Show the raw failure reason in the Player and keep recent errors here for reporting.">
          <Switch on={devMode} onChange={toggleDevMode} name="Dev Mode">{devMode ? "On" : "Off"}</Switch>
        </SettingRow>
      </Inset>
      {devMode && (
        <>
          <Inset id="settings-dev-status" title="What the app can reach">
            <StatusRow
              label="YouTube session"
              tone={session ? SESSION_LABELS[session].tone : "off"}
              status={session ? SESSION_LABELS[session].text : "Checking…"}
              hint="A signed-in session is what lets YouTube serve a whole stream. Without it the media host refuses the audio bytes."
            />
            <StatusRow
              label="Native player (libmpv)"
              tone={nativeAvailable ? "ok" : "off"}
              status={nativeAvailable ? "Available" : "Not confirmed"}
              hint="Confirmed once a track has loaded in the Player."
            />
          </Inset>
          <Inset id="settings-dev-errors" title="Recent errors">
            {entries.length === 0 ? (
              <NoteRow tone="off">Nothing has failed since the app started.</NoteRow>
            ) : (
              <>
                {entries.map((e) => (
                  <SettingRow
                    key={`${e.atMs}-${e.where}`}
                    label={`${new Date(e.atMs).toLocaleTimeString()} · ${e.where}`}
                    hint={`${e.category}${e.message ? ` — ${e.message}` : ""}`}
                  />
                ))}
                <div className="flex justify-end px-4 py-3">
                  <Button variant="ghost" onClick={clear}>Clear</Button>
                </div>
              </>
            )}
          </Inset>
        </>
      )}
    </>
  );
}

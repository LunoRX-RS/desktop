"use client";

import { useEffect, useState } from "react";
import { usePlayback } from "../../../lib/playback";
import { Switch } from "../../../components/Lyrics";
import { autostartGet, autostartSet } from "../../../services/tauri/autostart";
import { Inset, NoteRow, SettingRow, StatusRow } from "../rows";
import { useSettingsStore } from "../../../lib/settingsStore";
import { useDj } from "../../dj/djStore";
import { djStop } from "../../../services/tauri/dj";

export function Playback({ mac = false }: { mac?: boolean }) {
  const nativeAvailable = usePlayback((s) => s.available);
  const djMode = useSettingsStore((s) => s.djMode);
  const setDjMode = useSettingsStore((s) => s.setDjMode);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [autostartError, setAutostartError] = useState("");
  useEffect(() => {
    if (!mac) return;
    let live = true;
    autostartGet().then((on) => { if (live) setAutostart(on); }, () => { if (live) setAutostart(null); });
    return () => { live = false; };
  }, [mac]);
  async function saveAutostart(on: boolean) {
    setAutostartError("");
    try {
      await autostartSet(on);
      setAutostart(await autostartGet());
    } catch {
      setAutostartError("macOS would not let the app register. Try again from the installed app.");
    }
  }
  return (
    <>
      <Inset id="settings-playback" title="What can play">
        <StatusRow
          label="YouTube audio in the app"
          tone={nativeAvailable ? "ok" : "off"}
          status={nativeAvailable ? "Available" : "Not confirmed"}
          hint="Plays through the native player (libmpv). Confirmed once a track has loaded in the Player."
        />
        <StatusRow
          label="Spotify audio"
          tone="off"
          status="Unsupported"
          hint="Spotify’s third-party playback SDK requires Premium. Spotify Free playback in LunoRX is unavailable."
        />
      </Inset>
      <Inset id="settings-dj" title="DJ mode">
        <SettingRow label="Show DJ mode" hint="Adds the DJ screen to the sidebar and a DJ button to the player bar: two YouTube decks with loops, cues, tempo, effects and lyrics. Turning it off stops both decks.">
          <Switch on={djMode} name="Show DJ mode" onChange={(on) => {
            setDjMode(on);
            if (!on) {
              void djStop("a").catch(() => {});
              void djStop("b").catch(() => {});
              useDj.setState((s) => ({ decks: { a: { ...s.decks.a, videoId: null }, b: { ...s.decks.b, videoId: null } }, reports: { a: null, b: null } }));
            }
          }}>{djMode ? "On" : "Off"}</Switch>
        </SettingRow>
      </Inset>
      <Inset id="settings-startup" title="Startup">
        <SettingRow label="Restore last track" hint="Read-only. A restored track waits for you to press Play.">Resumes paused</SettingRow>
        {mac && (
          <SettingRow label="Open at login" hint="Starts LunoRX automatically when you sign in to macOS. It still opens to the paused last track.">
            {autostart === null ? "Unavailable" : (
              <Switch on={autostart} onChange={(on) => void saveAutostart(on)} name="Open at login">{autostart ? "On" : "Off"}</Switch>
            )}
          </SettingRow>
        )}
        {mac && autostartError && <NoteRow tone="bad" role="alert">{autostartError}</NoteRow>}
      </Inset>
    </>
  );
}

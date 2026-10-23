"use client";

import { usePlayback } from "../../../lib/playback";
import { Inset, SettingRow, StatusRow } from "../rows";

export function Playback() {
  const nativeAvailable = usePlayback((s) => s.available);
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
      <Inset id="settings-startup" title="Startup">
        <SettingRow label="Restore last track" hint="Read-only. A restored track waits for you to press Play.">Resumes paused</SettingRow>
      </Inset>
    </>
  );
}

"use client";

import pkg from "../../../../package.json";
import { usePlayback } from "../../../lib/playback";
import { Inset, SettingRow, StatusRow } from "../rows";

export function About({ platform }: { platform: string }) {
  const nativeAvailable = usePlayback((s) => s.available);
  return (
    <Inset id="settings-about" title="This app">
      <SettingRow label="App version">{pkg.version}</SettingRow>
      <SettingRow label="Platform">{platform || "Reading…"}</SettingRow>
      <StatusRow
        label="Native player (libmpv)"
        tone={nativeAvailable ? "ok" : "off"}
        status={nativeAvailable ? "Available" : "Not confirmed"}
        hint="Confirmed once a track has loaded in the Player."
      />
      <SettingRow label="Cache management" hint="Manage disk space in Storage.">Disk cache + offline playlists</SettingRow>
      <SettingRow label="Core status" hint="Not reported: no diagnostics command yet.">Not reported</SettingRow>
    </Inset>
  );
}

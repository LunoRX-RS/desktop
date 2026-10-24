"use client";

import { useEffect } from "react";
import { APP_VERSION } from "../../../lib/version";
import { usePlayback } from "../../../lib/playback";
import { Button, Segmented } from "../../../components/ui";
import { checkForUpdate, installUpdate, readChannel, restartApp, setChannel, useUpdater, type UpdateChannel } from "../../../lib/updater";
import { Inset, NoteRow, SettingRow, StatusRow } from "../rows";

export function About({ platform }: { platform: string }) {
  const nativeAvailable = usePlayback((s) => s.available);
  return (
    <Inset id="settings-about" title="This app">
      <SettingRow label="App version">{APP_VERSION}</SettingRow>
      <UpdateRow />
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

function UpdateRow() {
  const state = useUpdater((s) => s.state);
  const channel = useUpdater((s) => s.channel);
  // The About page can open before the launch check ran; read the saved choice once.
  useEffect(() => { if (useUpdater.getState().state.kind === "idle") useUpdater.setState({ channel: readChannel() }); }, []);
  const hint =
    state.kind === "checking" ? "Checking…"
    : state.kind === "upToDate" ? "You have the latest version."
    : state.kind === "available" ? `Version ${state.version} is available.`
    : state.kind === "downloading" ? (state.percent === null ? "Downloading…" : `Downloading… ${state.percent}%`)
    : state.kind === "installed" ? `Version ${state.version} is installed. Restart to use it.`
    : "Only updates signed by the LunoRX team are installed. Checked once at launch.";
  return (
    <>
      <SettingRow
        label="Update channel"
        hint={channel === "dev" ? "Dev: pre-release builds too. Newer sooner, less tested." : "Stable: full releases only."}
      >
        <Segmented<UpdateChannel>
          label="Update channel"
          value={channel}
          options={[{ key: "stable", label: "Stable" }, { key: "dev", label: "Dev" }]}
          onChange={setChannel}
        />
      </SettingRow>
      <SettingRow label="Updates" hint={hint}>
        {state.kind === "available" ? (
          <Button variant="primary" onClick={() => void installUpdate()}>Install update</Button>
        ) : state.kind === "installed" ? (
          <Button variant="primary" onClick={() => void restartApp()}>Restart now</Button>
        ) : (
          <Button disabled={state.kind === "checking" || state.kind === "downloading"} onClick={() => void checkForUpdate()}>
            Check for updates
          </Button>
        )}
      </SettingRow>
      {state.kind === "failed" && <NoteRow tone="bad" role="alert">{state.message}</NoteRow>}
    </>
  );
}

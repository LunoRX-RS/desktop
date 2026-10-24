// Self-update. Only releases signed with the project's key are accepted (public key in tauri.conf.json); the plugin
// checks the signature before installing. Nothing installs without the user pressing Install.
import { create } from "zustand";
import { APP_VERSION } from "./version";
import { invoke } from "../services/tauri/invoke";
import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { appEvent } from "./activity";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; version: string; percent: number | null }
  | { kind: "installed"; version: string }
  | { kind: "failed"; message: string };

const CHANNEL_KEY = "lunorx.updateChannel";

/** `stable`: full releases. `dev`: pre-releases too (release candidates), which can be less finished. */
export type UpdateChannel = "stable" | "dev";

/** A pre-release build follows the dev channel until the user chooses; a full release follows stable. */
export function readChannel(): UpdateChannel {
  try {
    const v = localStorage.getItem(CHANNEL_KEY);
    if (v === "stable" || v === "dev") return v;
  } catch {
    /* storage blocked: use the default */
  }
  return APP_VERSION.includes("-") ? "dev" : "stable";
}

export function setChannel(channel: UpdateChannel) {
  try {
    localStorage.setItem(CHANNEL_KEY, channel);
  } catch {
    /* the choice lasts for this session */
  }
  useUpdater.setState({ channel, dismissed: false });
  set({ kind: "idle" });
}

type Reject = "validation" | "unavailable" | "nothingToInstall" | "installFailed";

export const useUpdater = create<{ state: UpdateState; dismissed: boolean; channel: UpdateChannel }>(() => ({
  state: { kind: "idle" },
  dismissed: false,
  channel: "stable",
}));
const set = (state: UpdateState) => useUpdater.setState({ state });

/** Asks the channel's feed for a newer signed version. `quiet`: a failure at launch is not shown as an error. */
export async function checkForUpdate(quiet = false) {
  const now = useUpdater.getState().state.kind;
  if (now === "checking" || now === "downloading" || now === "installed") return;
  set({ kind: "checking" });
  try {
    const found = await invoke<{ version: string; notes: string | null } | null>("update_check", { channel: useUpdater.getState().channel });
    if (found) {
      useUpdater.setState({ dismissed: false });
      set({ kind: "available", version: found.version, notes: found.notes });
    } else {
      set({ kind: "upToDate" });
    }
  } catch {
    set(quiet ? { kind: "idle" } : { kind: "failed", message: "No update information is published for this channel yet, or the connection failed. Try again later." });
  }
}

/** Downloads, verifies and installs the update found by the last check, then the app restarts on request. */
export async function installUpdate() {
  const cur = useUpdater.getState().state;
  if (cur.kind !== "available") return;
  const version = cur.version;
  set({ kind: "downloading", version, percent: null });
  // The updater plugin broadcasts its own progress event; it never passes Core's logged emit, so it is noted here.
  const off = await listen<number | null>("update-progress", (e) => { appEvent("update-progress"); set({ kind: "downloading", version, percent: e.payload }); });
  try {
    await invoke("update_install");
    set({ kind: "installed", version });
  } catch (e) {
    const why = e as Reject;
    set({ kind: "failed", message: why === "installFailed" ? "The update could not be installed. Nothing was changed; try again later." : "That update is no longer available. Check again." });
  } finally {
    off();
  }
}

export const restartApp = () => relaunch();

/** Once per launch, a short while after start, so it never slows opening the app. Skipped outside the desktop app. */
export function initUpdater() {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
  if (process.env.NODE_ENV !== "production") return;
  useUpdater.setState({ channel: readChannel() });
  window.setTimeout(() => void checkForUpdate(true), 5000);
}

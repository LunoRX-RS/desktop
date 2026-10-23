// Self-update. Only releases signed with the project's key are accepted (public key in tauri.conf.json); the plugin
// checks the signature before installing. Nothing installs without the user pressing Install.
import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; version: string; percent: number | null }
  | { kind: "installed"; version: string }
  | { kind: "failed"; message: string };

let pending: Update | null = null;

export const useUpdater = create<{ state: UpdateState; dismissed: boolean }>(() => ({ state: { kind: "idle" }, dismissed: false }));
const set = (state: UpdateState) => useUpdater.setState({ state });

/** Asks the release feed for a newer signed version. `quiet`: a failure at launch is not shown as an error. */
export async function checkForUpdate(quiet = false) {
  const now = useUpdater.getState().state.kind;
  if (now === "checking" || now === "downloading" || now === "installed") return;
  set({ kind: "checking" });
  try {
    const update = await check();
    pending = update;
    if (update) {
      useUpdater.setState({ dismissed: false });
      set({ kind: "available", version: update.version, notes: update.body?.trim() || null });
    } else {
      set({ kind: "upToDate" });
    }
  } catch {
    set(quiet ? { kind: "idle" } : { kind: "failed", message: "Could not check for updates. Check your connection and try again." });
  }
}

/** Downloads, verifies and installs the pending update, then the app restarts on request. */
export async function installUpdate() {
  if (!pending) return;
  const version = pending.version;
  let total: number | null = null;
  let done = 0;
  set({ kind: "downloading", version, percent: null });
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? null;
      else if (event.event === "Progress") {
        done += event.data.chunkLength;
        set({ kind: "downloading", version, percent: total ? Math.min(100, Math.round((done / total) * 100)) : null });
      }
    });
    set({ kind: "installed", version });
  } catch {
    set({ kind: "failed", message: "The update could not be installed. Nothing was changed; try again later." });
  }
}

export const restartApp = () => relaunch();

/** Once per launch, a short while after start, so it never slows opening the app. Skipped outside the desktop app. */
export function initUpdater() {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
  if (process.env.NODE_ENV !== "production") return;
  window.setTimeout(() => void checkForUpdate(true), 5000);
}

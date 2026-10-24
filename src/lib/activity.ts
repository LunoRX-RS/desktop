"use client";
import { activityFlush, activityRecord } from "../services/tauri/reports";

// User-action notes. Names follow the same slug rule Core validates (`action.<name>`, lowercase); a detail is free
// text that Core masks before it reaches disk. Records stay on this device unless the activity switch is on.

/** Notes one user action, e.g. `action.navigate.player`. Failures are ignored on purpose. */
export function action(name: string, detail = ""): void {
  void activityRecord({ kind: "action", name: `action.${name}`, detail }).catch(() => {});
}

/** Notes one app event that never passed Core's emit: the updater plugin broadcasts its own progress events. */
export function appEvent(name: string): void {
  void activityRecord({ kind: "event", name: `event.${name}` }).catch(() => {});
}

/** Uploads pending entries a minute after start and then hourly; also forces the throttled disk write. */
export function initActivity(): () => void {
  const flush = () => { void activityFlush().catch(() => {}); };
  const first = window.setTimeout(flush, 60_000);
  const every = window.setInterval(flush, 3_600_000);
  return () => { window.clearTimeout(first); window.clearInterval(every); };
}

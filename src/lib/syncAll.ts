"use client";
import { create } from "zustand";
import { statsCloudSync, statsLocal } from "../services/tauri/stats";
import { syncFlushPending, syncListLocalPlaylists, syncStatus, syncUploadPlaylist } from "../services/tauri/sync";
import { telemetryFlush, telemetryGet } from "../services/tauri/telemetry";
import { youtubeConnectionStatus, youtubeImport } from "../services/tauri/youtube";
import { refreshLikes } from "./library";
import { syncFollowedArtists } from "./likedArtists";

export type PartState = "ok" | "skipped" | "failed";
export interface Part { key: string; label: string; state: PartState; detail: string }

interface SyncAllState { running: boolean; parts: Part[]; finishedAt: number | null }
export const useSyncAll = create<SyncAllState>(() => ({ running: false, parts: [], finishedAt: null }));

type Outcome = Omit<Part, "key" | "label">;
const skipped = (detail: string): Outcome => ({ state: "skipped", detail });
const failed = (detail: string): Outcome => ({ state: "failed", detail });

async function account(): Promise<Outcome> {
  const s = await syncStatus();
  if (s.state.state === "signedOut") return skipped("Signed out of your LunoRX account");
  const sent = await syncFlushPending();
  return { state: "ok", detail: sent ? `${sent} queued ${sent === 1 ? "change" : "changes"} sent` : "Playlists up to date" };
}
/** Uploads every playlist not yet on the cloud, or only part-uploaded. One already there (including an imported
 * YouTube playlist uploaded before) has a remote id and nothing unsent, so it is skipped, never copied twice. */
async function playlists(): Promise<Outcome> {
  const s = await syncStatus();
  if (s.state.state === "signedOut") return skipped("Signed out of your LunoRX account");
  const todo = (await syncListLocalPlaylists()).filter((p) => p.remoteId === null || p.unsentEntries > 0);
  let done = 0;
  for (const p of todo) { try { await syncUploadPlaylist(p.id); done++; } catch { /* retried on the next sync */ } }
  if (done < todo.length) return failed(`${done} of ${todo.length} playlists uploaded; the rest are retried next time`);
  return { state: "ok", detail: todo.length ? `${done} ${done === 1 ? "playlist" : "playlists"} uploaded` : "Every playlist is on the cloud" };
}
async function artists(): Promise<Outcome> {
  const s = await syncStatus();
  if (s.state.state === "signedOut") return skipped("Signed out of your LunoRX account");
  const n = await syncFollowedArtists();
  return { state: "ok", detail: `${n} followed ${n === 1 ? "artist" : "artists"} in sync` };
}
async function youtube(): Promise<Outcome> {
  if ((await youtubeConnectionStatus()) !== "connected") return skipped("YouTube is not connected");
  try { await youtubeImport(); } catch (e) { if (e !== "inUse") throw e; }
  return { state: "ok", detail: "Likes, playlists and history imported; waiting likes sent" };
}
async function stats(): Promise<Outcome> {
  if (!(await statsLocal()).cloud) return skipped("Cloud stats are off");
  await statsCloudSync();
  return { state: "ok", detail: "Play counts uploaded" };
}
async function usage(): Promise<Outcome> {
  if (!(await telemetryGet()).enabled) return skipped("Usage data is off");
  await telemetryFlush();
  return { state: "ok", detail: "Usage counts sent" };
}

const STEPS: [string, string, () => Promise<Outcome>][] = [
  ["account", "LunoRX account", account],
  ["youtube", "YouTube and YouTube Music", youtube],
  ["playlists", "Playlists to the cloud", playlists],
  ["artists", "Followed artists", artists],
  ["stats", "Listening stats", stats],
  ["usage", "Usage data", usage],
];

/** One press: every service that is set up syncs, each on its own so one failure does not stop the others. */
export async function syncAll() {
  if (useSyncAll.getState().running) return;
  useSyncAll.setState({ running: true, parts: [] });
  const parts = await Promise.all(STEPS.map(async ([key, label, run]): Promise<Part> => {
    try { return { key, label, ...(await run()) }; }
    catch { return { key, label, ...failed("Could not sync. It will be retried; check your connection.") }; }
  }));
  void refreshLikes();
  useSyncAll.setState({ running: false, parts, finishedAt: Date.now() });
}

/** One line for the button's tooltip and screen readers. */
export function syncAllSummary(parts: Part[]): string {
  if (parts.length === 0) return "";
  const bad = parts.filter((p) => p.state === "failed").map((p) => p.label);
  const done = parts.filter((p) => p.state === "ok").length;
  return bad.length ? `Synced ${done}, failed: ${bad.join(", ")}` : `Synced ${done} ${done === 1 ? "service" : "services"}`;
}

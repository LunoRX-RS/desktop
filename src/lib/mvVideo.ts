"use client";

import { invoke } from "../services/tauri/invoke";
import { isVideoId } from "./mvMatch";

// The music video behind the backgrounds: the song's own YouTube video, found and ranked in Rust (core::mv). When the
// search finds nothing, the track's own YouTube id is used. Results are cached for the session.
const cache = new Map<string, Promise<string | null>>();

/** The embeddable music video, or null: the backdrop then shows the artwork ("Image Music"). */
export function mvVideoId(track: { provider: string; providerTrackId: string; title: string; artists?: string[]; durationMs?: number | null }): Promise<string | null> {
  const own = track.provider === "YouTube" && isVideoId(track.providerTrackId) ? track.providerTrackId : null;
  const title = track.title.trim();
  const artist = (track.artists ?? [])[0] ?? "";
  if (!title) return Promise.resolve(null);
  const key = `${title}|${artist}|${track.durationMs ?? ""}|${own ?? ""}`;
  let hit = cache.get(key);
  if (!hit) {
    hit = invoke<string | null>("mv_find", { title, artist, durationMs: track.durationMs ?? null, ownId: own })
      .catch(() => null);
    cache.set(key, hit);
  }
  return hit;
}

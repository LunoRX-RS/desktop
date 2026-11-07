"use client";

import { invoke } from "../services/tauri/invoke";
import { isVideoId } from "./mvMatch";

// The music video behind the backgrounds: the song's own YouTube video, found and ranked in Rust (core::mv). When the
// search finds nothing, the track's own YouTube id is used. Results are cached for the session.
const cache = new Map<string, Promise<string | null>>();

export function mvVideoId(track: { provider: string; providerTrackId: string; title: string; artists?: string[]; durationMs?: number | null }): Promise<string | null> {
  const own = track.provider === "YouTube" && isVideoId(track.providerTrackId) ? track.providerTrackId : null;
  const title = track.title.trim();
  const artist = (track.artists ?? [])[0] ?? "";
  if (!title) return Promise.resolve(own);
  const key = `${title}|${artist}|${track.durationMs ?? ""}`;
  let hit = cache.get(key);
  if (!hit) {
    hit = invoke<string | null>("mv_find", { title, artist, durationMs: track.durationMs ?? null })
      .catch(() => null)
      .then((id) => id ?? own);
    cache.set(key, hit);
  }
  return hit;
}

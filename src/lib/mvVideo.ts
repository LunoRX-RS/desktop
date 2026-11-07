"use client";

import { searchQuery } from "../services/tauri/search";
import { isVideoId, pickMvId } from "./mvMatch";

// The music video behind the ambient backdrop. A YouTube track's own id is its video; without one, the same title is
// searched on YouTube and the first hit with the same title is used. Results are cached for the session.
const cache = new Map<string, Promise<string | null>>();

export function mvVideoId(track: { provider: string; providerTrackId: string; title: string; artists?: string[] }): Promise<string | null> {
  if (track.provider === "YouTube" && isVideoId(track.providerTrackId)) return Promise.resolve(track.providerTrackId);
  const q = [track.title, ...(track.artists ?? []).slice(0, 1)].join(" ").trim();
  if (!q) return Promise.resolve(null);
  let hit = cache.get(q);
  if (!hit) {
    hit = searchQuery({ query: q, providers: ["YouTube"], limit: 5, cancellationId: `mv-${Date.now()}` })
      .then((r) => pickMvId(track.title, r.hits))
      .catch(() => null);
    cache.set(q, hit);
  }
  return hit;
}

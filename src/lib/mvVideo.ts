"use client";

import { searchQuery } from "../services/tauri/search";

// The music video behind the ambient backdrop. A YouTube track's own id is its video; without one, the same title is
// searched on YouTube and the first hit with the same title is used. Results are cached for the session.
const ID = /^[A-Za-z0-9_-]{11}$/;
const cache = new Map<string, Promise<string | null>>();
const norm = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();

export function mvVideoId(track: { provider: string; providerTrackId: string; title: string; artists?: string[] }): Promise<string | null> {
  if (track.provider === "YouTube" && ID.test(track.providerTrackId)) return Promise.resolve(track.providerTrackId);
  const q = [track.title, ...(track.artists ?? []).slice(0, 1)].join(" ").trim();
  if (!q) return Promise.resolve(null);
  let hit = cache.get(q);
  if (!hit) {
    const want = norm(track.title);
    hit = searchQuery({ query: q, providers: ["YouTube"], limit: 5, cancellationId: `mv-${Date.now()}` })
      .then((r) => r.hits.find((h) => h.provider === "YouTube" && ID.test(h.providerTrackId) && norm(h.title).includes(want))?.providerTrackId ?? null)
      .catch(() => null);
    cache.set(q, hit);
  }
  return hit;
}

"use client";

import { searchQuery } from "../services/tauri/search";
import { isVideoId, pickMvId } from "./mvMatch";

// The music video behind the backgrounds: a YouTube video with the same title (and artist), so the picture is the
// song's own video rather than the audio upload. Falls back to the track's own id when the search finds nothing.
// Results are cached for the session.
const cache = new Map<string, Promise<string | null>>();

export function mvVideoId(track: { provider: string; providerTrackId: string; title: string; artists?: string[] }): Promise<string | null> {
  const own = track.provider === "YouTube" && isVideoId(track.providerTrackId) ? track.providerTrackId : null;
  const q = [track.title, ...(track.artists ?? []).slice(0, 1), "official music video"].join(" ").trim();
  if (!track.title.trim()) return Promise.resolve(own);
  let hit = cache.get(q);
  if (!hit) {
    hit = searchQuery({ query: q, providers: ["YouTube"], limit: 5, cancellationId: `mv-${Date.now()}` })
      .then((r) => pickMvId(track.title, r.hits))
      .catch(() => null)
      .then((id) => id ?? own);
    cache.set(q, hit);
  }
  return hit;
}

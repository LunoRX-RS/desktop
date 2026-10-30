"use client";

import { create } from "zustand";
import type { ArtistRef } from "../services/tauri/search";
import { artistsCloudFollow, artistsCloudSync, type CloudArtist } from "../services/tauri/artists";

/** An artist the user follows. Kept on this device; the LunoRX server has no artist list to sync to yet. */
export interface LikedArtist extends ArtistRef {
  artworkUrl: string | null;
  addedAt: number;
}

const KEY = "lunorx.likedArtists";
export const artistKey = (a: Pick<ArtistRef, "provider" | "id" | "name">) => `${a.provider}:${a.id ?? a.name}`;

function read(): LikedArtist[] {
  try {
    const raw = JSON.parse(window.localStorage.getItem(KEY) ?? "[]") as LikedArtist[];
    return Array.isArray(raw) ? raw.filter((a) => a && typeof a.name === "string" && typeof a.provider === "string") : [];
  } catch {
    return [];
  }
}

interface LikedArtists {
  artists: LikedArtist[];
  /** Follows or unfollows. Returns the new state; false when storage refused the write. */
  toggle: (artist: ArtistRef, artworkUrl?: string | null) => boolean;
}

export const useLikedArtists = create<LikedArtists>((set, get) => ({
  artists: typeof window === "undefined" ? [] : read(),
  toggle: (artist, artworkUrl = null) => {
    const key = artistKey(artist);
    const had = get().artists.some((a) => artistKey(a) === key);
    const next = had
      ? get().artists.filter((a) => artistKey(a) !== key)
      : [{ provider: artist.provider, id: artist.id, name: artist.name, artworkUrl, addedAt: Date.now() }, ...get().artists];
    try { window.localStorage.setItem(KEY, JSON.stringify(next)); } catch { return had; }
    set({ artists: next });
    // Straight to the account when signed in. An unfollow that cannot be sent now is remembered and sent on the next
    // sync, so the server cannot bring the artist back.
    const cloud = toCloud({ provider: artist.provider, id: artist.id, name: artist.name, artworkUrl, addedAt: Date.now() });
    if (had) setRemoved([...removed().filter((r) => r[0] !== cloud.provider || r[1] !== cloud.artistKey), [cloud.provider, cloud.artistKey]]);
    void artistsCloudFollow(cloud, !had).then(
      () => { if (had) setRemoved(removed().filter((r) => r[0] !== cloud.provider || r[1] !== cloud.artistKey)); },
      () => {},
    );
    return !had;
  },
}));

export const useIsLikedArtist = (artist: Pick<ArtistRef, "provider" | "id" | "name"> | null) =>
  useLikedArtists((s) => !!artist && s.artists.some((a) => artistKey(a) === artistKey(artist)));

const REMOVED = "lunorx.likedArtists.removed";
function removed(): [string, string][] {
  try { const r = JSON.parse(window.localStorage.getItem(REMOVED) ?? "[]"); return Array.isArray(r) ? r : []; } catch { return []; }
}
function setRemoved(list: [string, string][]) {
  try { window.localStorage.setItem(REMOVED, JSON.stringify(list)); } catch { /* retried from the follow list next time */ }
}
const toCloud = (a: LikedArtist): CloudArtist => ({
  provider: a.provider, artistKey: (a.id ?? a.name).trim(), artistId: a.id, name: a.name, artworkUrl: a.artworkUrl, followedAtMs: a.addedAt,
});

/** Brings this device and the account to the same list. Rejects (with the core error) when signed out or offline. */
export async function syncFollowedArtists(): Promise<number> {
  const local = useLikedArtists.getState().artists.map(toCloud).filter((a) => a.artistKey && a.name);
  const server = await artistsCloudSync(local, removed());
  setRemoved([]);
  const next: LikedArtist[] = server.map((a) => ({
    provider: a.provider, id: a.artistId, name: a.name, artworkUrl: a.artworkUrl, addedAt: a.followedAtMs || Date.now(),
  }));
  try { window.localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* the in-memory list is still right */ }
  useLikedArtists.setState({ artists: next });
  return next.length;
}

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

/** Loads the followed artists from storage. Call once after mount; a follow made before that is kept. */
export function hydrateLikedArtists() {
  if (useLikedArtists.getState().artists.length === 0) useLikedArtists.setState({ artists: read() });
}

interface LikedArtists {
  artists: LikedArtist[];
  /** Follows or unfollows. Returns the new state; false when storage refused the write. */
  toggle: (artist: ArtistRef, artworkUrl?: string | null) => boolean;
}

export const useLikedArtists = create<LikedArtists>((set, get) => ({
  // Empty on the first render so it matches the static HTML; the stored list comes in after mount (hydrateLikedArtists).
  artists: [],
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
  const sentRemoved = removed();
  const sent = useLikedArtists.getState().artists;
  const sentKeys = new Set(sent.map(artistKey));
  const server = await artistsCloudSync(sent.map(toCloud).filter((a) => a.artistKey && a.name), sentRemoved);
  // Only the removals this request carried are settled; a removal made while it ran stays queued for the next sync.
  setRemoved(removed().filter((r) => !sentRemoved.some((s) => s[0] === r[0] && s[1] === r[1])));
  // Changes made while the request ran are kept: an artist unfollowed meanwhile stays out, one followed meanwhile stays in.
  const now = useLikedArtists.getState().artists;
  const nowKeys = new Set(now.map(artistKey));
  const unfollowedMeanwhile = new Set([...sentKeys].filter((k) => !nowKeys.has(k)));
  const followedMeanwhile = now.filter((a) => !sentKeys.has(artistKey(a)));
  const fromServer: LikedArtist[] = server
    .map((a) => ({ provider: a.provider, id: a.artistId, name: a.name, artworkUrl: a.artworkUrl, addedAt: a.followedAtMs || Date.now() }))
    .filter((a) => !unfollowedMeanwhile.has(artistKey(a)));
  const next = [...followedMeanwhile, ...fromServer.filter((a) => !followedMeanwhile.some((b) => artistKey(b) === artistKey(a)))];
  try { window.localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* the in-memory list is still right */ }
  useLikedArtists.setState({ artists: next });
  return next.length;
}

/** Sign-out: the list on this device belongs to the account that just left. Clear it so the next account starts from its own server list. */
export function forgetFollowedArtists() {
  try { window.localStorage.removeItem(KEY); window.localStorage.removeItem(REMOVED); } catch { /* nothing stored */ }
  useLikedArtists.setState({ artists: [] });
}

"use client";

import { create } from "zustand";
import { SAMPLE_TRACKS, type SampleTrack } from "./demo-data";

// UI state only. Playback below is a simulation: nothing produces sound and no provider is involved.

export type View =
  | "home"
  | "explore"
  | "search"
  | "library"
  | "playlists"
  | "player"
  | "connections"
  | "accounts"
  | "settings"
  | "sync"
  | "details"
  | "artist"
  | "album";

export type DetailRef = { kind: "album" | "playlist"; id: string };
export type LibraryTab = "albums" | "artists" | "songs" | "playlists";
export type RepeatMode = "off" | "all" | "one";

interface UiState {
  selectedArtist: import("../services/tauri/search").ArtistRef | null;
  selectedAlbum: { provider: string; id: string; name: string; artworkUrl: string | null; year: number | null } | null;
  openAlbum: (album: NonNullable<UiState["selectedAlbum"]>) => void;
  openArtist: (artist: import("../services/tauri/search").ArtistRef) => void;
  view: View;
  back: View[];
  forward: View[];
  sidebarCollapsed: boolean;
  paletteOpen: boolean;
  nowPlayingOpen: boolean;
  lyricsOpen: boolean;
  queueOpen: boolean;
  selectedPlaylistId: string | null;
  detail: DetailRef | null;
  libraryTab: LibraryTab;
  likedOnly: boolean;
  query: string;
  pendingSearch: boolean;
  recentSearches: string[];

  // Demo playback (simulated UI state only).
  queue: SampleTrack[];
  index: number;
  playing: boolean;
  positionSec: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  liked: Record<string, true>;

  navigate: (view: View) => void;
  goBack: () => void;
  goForward: () => void;
  openLibrary: (tab: LibraryTab, likedOnly?: boolean) => void;
  setSelectedPlaylist: (id: string | null) => void;
  openDetail: (kind: DetailRef["kind"], id: string) => void;
  toggleSidebar: () => void;
  setPalette: (open: boolean) => void;
  setNowPlaying: (open: boolean) => void;
  setLyrics: (open: boolean) => void;
  toggleQueue: () => void;
  setQuery: (q: string) => void;
  searchFor: (q: string) => void;
  pushRecentSearch: (q: string) => void;
  clearRecentSearches: () => void;
  removeRecentSearch: (query: string) => void;
  playIndex: (i: number) => void;
  togglePlay: () => void;
  next: () => void;
  prev: () => void;
  seek: (sec: number) => void;
  tick: () => void;
  setVolume: (v: number) => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  toggleLike: (id: string) => void;
}

const RECENT_LIMIT = 20;
const RECENT_KEY = "lunorx.recentSearches";
function saveRecent(values: string[]) { try { window.localStorage.setItem(RECENT_KEY,JSON.stringify(values)); } catch { /* Keep session history when disk storage is blocked. */ } }
export function hydrateRecentSearches() {
  try { const parsed:unknown=JSON.parse(window.localStorage.getItem(RECENT_KEY) ?? "[]");if(Array.isArray(parsed)) useUi.setState({recentSearches:Array.from(new Set(parsed.filter((s):s is string=>typeof s==="string" && s.trim().length>0 && s.length<=256))).slice(0,RECENT_LIMIT)}); } catch { /* Ignore corrupt browser data. */ }
}

export const useUi = create<UiState>()((set, get) => ({
  selectedArtist: null,
  selectedAlbum: null,
  openAlbum: (album) => { set({ selectedAlbum: album, nowPlayingOpen: false }); get().navigate("album"); },
  openArtist: (artist) => { set({ selectedArtist: artist, nowPlayingOpen: false }); get().navigate("artist"); },
  view: "home",
  back: [],
  forward: [],
  sidebarCollapsed: false,
  paletteOpen: false,
  nowPlayingOpen: false,
  lyricsOpen: false,
  queueOpen: false,
  selectedPlaylistId: null,
  detail: null,
  libraryTab: "playlists",
  likedOnly: false,
  query: "",
  pendingSearch: false,
  recentSearches: [],

  queue: SAMPLE_TRACKS,
  // -1: nothing is loaded until the user plays something. No track is pre-selected at launch.
  index: -1,
  playing: false,
  positionSec: 0,
  volume: 100,
  shuffle: false,
  repeat: "off",
  liked: { "s-01": true, "s-04": true },

  navigate: (view) => {
    const { view: current, back } = get();
    if (view === current) return;
    set({ view, back: [...back, current], forward: [] });
  },
  goBack: () => {
    const { back, view, forward } = get();
    const prev = back[back.length - 1];
    if (!prev) return;
    set({ view: prev, back: back.slice(0, -1), forward: [...forward, view] });
  },
  goForward: () => {
    const { forward, view, back } = get();
    const nextView = forward[forward.length - 1];
    if (!nextView) return;
    set({ view: nextView, forward: forward.slice(0, -1), back: [...back, view] });
  },
  openLibrary: (tab, likedOnly = false) => {
    set({ libraryTab: tab, likedOnly });
    get().navigate("library");
  },
  setSelectedPlaylist: (id) => set({ selectedPlaylistId: id }),
  openDetail: (kind, id) => {
    set({ detail: { kind, id } });
    get().navigate("details");
  },
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setPalette: (open) => set({ paletteOpen: open }),
  setNowPlaying: (open) => set({ nowPlayingOpen: open, lyricsOpen: open ? get().lyricsOpen : false }),
  setLyrics: (open) => set({ lyricsOpen: open }),
  toggleQueue: () => set((s) => ({ queueOpen: !s.queueOpen })),
  setQuery: (q) => set({ query: q }),
  // The command palette opens the Search view and run the query once.
  searchFor: (q) => {
    set({ query: q, pendingSearch: true });
    get().navigate("search");
  },
  pushRecentSearch: (q) => {
    const trimmed = q.trim();
    if (trimmed === "" || trimmed.length > 256) return;
    const rest = get().recentSearches.filter((x) => x !== trimmed);
    const recentSearches = [trimmed,...rest].slice(0,RECENT_LIMIT);
    saveRecent(recentSearches);set({recentSearches});
  },
  clearRecentSearches: () => {saveRecent([]);set({recentSearches:[]});},
  removeRecentSearch: (query) => {const recentSearches=get().recentSearches.filter(s=>s!==query);saveRecent(recentSearches);set({recentSearches});},

  playIndex: (i) => {
    const { queue } = get();
    if (i < 0 || i >= queue.length) return;
    set({ index: i, playing: true, positionSec: 0 });
  },
  togglePlay: () => set((s) => ({ playing: !s.playing })),
  next: () => {
    const { queue, index, shuffle, repeat } = get();
    if (queue.length === 0) return;
    if (shuffle && queue.length > 1) {
      let pick = index;
      while (pick === index) pick = Math.floor(Math.random() * queue.length);
      set({ index: pick, positionSec: 0 });
      return;
    }
    if (index + 1 < queue.length) {
      set({ index: index + 1, positionSec: 0 });
    } else if (repeat === "all") {
      set({ index: 0, positionSec: 0 });
    } else {
      set({ playing: false, positionSec: 0 });
    }
  },
  prev: () => {
    const { index, positionSec, queue } = get();
    if (positionSec > 3) {
      set({ positionSec: 0 });
      return;
    }
    set({ index: index > 0 ? index - 1 : queue.length - 1, positionSec: 0 });
  },
  seek: (sec) => {
    const dur = get().queue[get().index]?.durationSec ?? 0;
    set({ positionSec: Math.min(Math.max(0, sec), dur) });
  },
  tick: () => {
    const { playing, positionSec, queue, index, repeat } = get();
    if (!playing) return;
    const dur = queue[index]?.durationSec ?? 0;
    if (positionSec + 1 >= dur) {
      if (repeat === "one") set({ positionSec: 0 });
      else get().next();
      return;
    }
    set({ positionSec: positionSec + 1 });
  },
  setVolume: (v) => set({ volume: Math.min(100, Math.max(0, Math.round(v))) }),
  toggleShuffle: () => set((s) => ({ shuffle: !s.shuffle })),
  cycleRepeat: () =>
    set((s) => ({ repeat: s.repeat === "off" ? "all" : s.repeat === "all" ? "one" : "off" })),
  toggleLike: (id) =>
    set((s) => {
      const liked = { ...s.liked };
      if (liked[id]) delete liked[id];
      else liked[id] = true;
      return { liked };
    }),
}));

"use client";
// Every screen except Listen Now loads on demand, so the app opens on a fraction of the JavaScript it used to parse.
// `preloadScreens` fetches the rest once the window is idle, so moving between screens stays instant.
import dynamic from "next/dynamic";

const load = {
  explore: () => import("../../features/explore/ExploreScreen"),
  search: () => import("../../features/search/SearchScreen"),
  artists: () => import("../../features/artists/ArtistsScreen"),
  library: () => import("../../features/library/LibraryScreen"),
  playlists: () => import("../../features/playlists/PlaylistEditor"),
  player: () => import("../../features/player/PlayerScreen"),
  connections: () => import("../../features/connections/ConnectionsScreen"),
  accounts: () => import("../../features/accounts/AccountsScreen"),
  login: () => import("../../features/accounts/LoginPage"),
  loginGate: () => import("../../features/accounts/LoginGate"),
  settings: () => import("../../features/settings/SettingsScreen"),
  stats: () => import("../../features/stats/StatsScreen"),
  shared: () => import("../../features/shared/SharedScreen"),
  sync: () => import("../../features/sync/SyncPanel"),
  nowPlaying: () => import("./NowPlaying"),
  palette: () => import("./CommandPalette"),
  queue: () => import("./QueuePanel"),
};

export const ExploreScreen = dynamic(load.explore, { ssr: false });
export const SearchScreen = dynamic(load.search, { ssr: false });
export const ArtistScreen = dynamic(load.artists, { ssr: false });
export const AlbumScreen = dynamic(() => load.artists().then((m) => m.AlbumScreen), { ssr: false });
export const SavedLibraryScreen = dynamic(() => load.artists().then((m) => m.SavedLibraryScreen), { ssr: false });
export const LibraryScreen = dynamic(load.library, { ssr: false });
export const PlaylistEditor = dynamic(load.playlists, { ssr: false });
export const PlayerScreen = dynamic(load.player, { ssr: false });
export const ConnectionsScreen = dynamic(load.connections, { ssr: false });
export const AccountsScreen = dynamic(load.accounts, { ssr: false });
export const LoginPage = dynamic(load.login, { ssr: false });
export const LoginGate = dynamic(() => load.loginGate().then((m) => m.LoginGate), { ssr: false });
export const SettingsScreen = dynamic(load.settings, { ssr: false });
export const StatsScreen = dynamic(load.stats, { ssr: false });
export const SharedScreen = dynamic(load.shared, { ssr: false });
export const SyncPanel = dynamic(load.sync, { ssr: false });
export const NowPlaying = dynamic(load.nowPlaying, { ssr: false });
export const CommandPalette = dynamic(load.palette, { ssr: false });
export const QueuePanel = dynamic(() => load.queue().then((m) => m.QueuePanel), { ssr: false });

/** Fetches every screen's code in the background, one at a time, once the window has nothing else to do. */
export function preloadScreens(): () => void {
  let stop = false;
  const queue = Object.values(load);
  const next = () => {
    const fn = queue.shift();
    if (stop || !fn) return;
    void fn().then(schedule, schedule);
  };
  const idle = (window as { requestIdleCallback?: (f: () => void, o?: { timeout: number }) => number }).requestIdleCallback;
  const schedule = () => { if (idle) idle(next, { timeout: 2000 }); else window.setTimeout(next, 200); };
  const start = window.setTimeout(schedule, 1500);
  return () => { stop = true; window.clearTimeout(start); };
}

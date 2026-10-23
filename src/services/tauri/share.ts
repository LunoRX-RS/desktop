import { invoke } from "@tauri-apps/api/core";

/** Emitted by the Rust side with the share token when a `lunorx://share/{token}` link opens the app. */
export const SHARE_OPEN_EVENT = "core://share/open";

export interface SharedEntryDto { title: string; artists: string[]; artworkUrl: string | null; durationMs: number | null }
export interface SharedPlaylistDto { name: string; description: string | null; entries: SharedEntryDto[] }

/** The token from a link that arrived before the window listened; null when there is none. */
export const sharedLinkTake = () => invoke<string | null>("shared_link_take");
export const sharedPlaylistGet = (token: string) => invoke<SharedPlaylistDto>("shared_playlist_get", { token });

import { invoke } from "@tauri-apps/api/core";

// Mirrors commands/sync.rs and sync/engine.rs. Tauri maps snake_case Rust argument names to camelCase JS keys.
// Errors cross IPC as the bare category string; no message, token or password ever reaches the frontend.

export type ErrorCategory =
  | "validation"
  | "unauthenticated"
  | "conflict"
  | "rateLimited"
  | "offline"
  | "storage"
  | "queueFull"
  | "internal";

export type SyncState =
  | { state: "signedOut" }
  | { state: "syncing" }
  | { state: "synced" }
  | { state: "offline" }
  | { state: "rateLimited"; retryAfterSecs: number | null }
  | { state: "conflict"; playlistId: string }
  | { state: "error"; kind: ErrorCategory };

export interface SignedInDto {
  userId: string | null;
  expiresAt: string;
}

export interface SignOutDto {
  warning: ErrorCategory | null;
}

export interface SyncStatusDto {
  state: SyncState;
  pendingWrites: number;
  conflicts: string[];
}

export type Provider = "Spotify" | "YouTube";

export interface LocalEntryDto {
  provider: Provider;
  providerTrackId: string;
  providerUrl: string;
  title: string | null;
  artists: string[] | null;
  artworkUrl: string | null;
  durationMs: number | null;
}

export interface LocalPlaylistDto {
  id: string;
  name: string;
  remoteId: string | null;
  entryCount: number;
  // Entries the server has accepted; fewer than entryCount means a partial upload that can resume.
  uploaded: number;
  // Queued writes for this playlist alone; they are not on the server yet.
  queuedWrites: number;
  // Entries past the server-held and queued ones: the unsent partial-upload tail.
  unsentEntries: number;
}

export type ResolutionDto = "keepServer" | "keepMine";

export function syncListLocalPlaylists(): Promise<LocalPlaylistDto[]> {
  return invoke<LocalPlaylistDto[]>("sync_list_local_playlists");
}

// Index is the entry's position in the local playlist.
export function syncRemoveLocalEntry(localId: string, index: number): Promise<void> {
  return invoke<void>("sync_remove_local_entry", { localId, index });
}

// Returns how many queued writes the server accepted.
export function syncFlushPending(): Promise<number> {
  return invoke<number>("sync_flush_pending");
}

export function syncSignIn(email: string, password: string): Promise<SignedInDto> {
  return invoke<SignedInDto>("sync_sign_in", { email, password });
}

export function syncSignOut(): Promise<SignOutDto> {
  return invoke<SignOutDto>("sync_sign_out");
}

// Returns how many queued writes were dropped.
export function syncDiscardPending(): Promise<number> {
  return invoke<number>("sync_discard_pending");
}

export function syncStatus(): Promise<SyncStatusDto> {
  return invoke<SyncStatusDto>("sync_status");
}

// Returns the local playlist id.
export function syncCreateLocalPlaylist(name: string, description: string | null): Promise<string> {
  return invoke<string>("sync_create_local_playlist", { name, description });
}

/** The playlist's entries in order, as this device holds them. */
export function syncPlaylistEntries(localId: string): Promise<LocalEntryDto[]> {
  return invoke<LocalEntryDto[]>("sync_playlist_entries", { localId });
}

export function syncAddLocalEntry(localId: string, entry: LocalEntryDto): Promise<void> {
  return invoke<void>("sync_add_local_entry", { localId, entry });
}

// Returns the remote playlist id.
export function syncUploadPlaylist(localId: string): Promise<string> {
  return invoke<string>("sync_upload_playlist", { localId });
}

export function syncResolveConflict(playlistId: string, resolution: ResolutionDto): Promise<void> {
  return invoke<void>("sync_resolve_conflict", { playlistId, resolution });
}

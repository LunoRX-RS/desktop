import { invoke } from "@tauri-apps/api/core";

// Mirrors commands/stats.rs. Counts stay on this device; the cloud calls send play counts only, and only after the user turns them on.

export interface PlayCountDto {
  provider: string;
  providerTrackId: string;
  title: string;
  artists: string[];
  artworkUrl?: string | null;
  plays: number;
  lastPlayedMs: number;
}

export interface StatsViewDto {
  deviceId: string;
  cloud: boolean;
  tracks: PlayCountDto[];
}

export const statsLocal = () => invoke<StatsViewDto>("stats_local");
export const statsClearLocal = () => invoke<StatsViewDto>("stats_clear_local");
export const statsSetCloud = (enabled: boolean) => invoke<StatsViewDto>("stats_set_cloud", { enabled });
/** Uploads this device's counts, then returns the totals over every device signed in to the account. */
export const statsCloudSync = () => invoke<{ tracks: PlayCountDto[] }>("stats_cloud_sync");
/** Deletes the account's stats on the server and turns cloud sync off. */
export const statsCloudClear = () => invoke<void>("stats_cloud_clear");

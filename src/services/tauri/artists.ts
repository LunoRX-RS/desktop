import { invoke } from "./invoke";

/** Mirrors sync::api::FollowedArtist. `artistKey` is the provider's artist id, or the name when there is none. */
export interface CloudArtist { provider: string; artistKey: string; artistId: string | null; name: string; artworkUrl: string | null; followedAtMs: number }

/** Sends unfollows and follows made here, then returns the account's whole list. Rejects with an ErrorCategory. */
export const artistsCloudSync = (local: CloudArtist[], removed: [string, string][]): Promise<CloudArtist[]> =>
  invoke("artists_cloud_sync", { local, removed });
export const artistsCloudFollow = (artist: CloudArtist, follow: boolean): Promise<void> =>
  invoke("artists_cloud_follow", { artist, follow });

import type { PlayCountDto } from "../services/tauri/stats";
import type { SearchHitDto } from "../services/tauri/search";

/** A song needs this many plays to count as "often". One play is just something that was tried. */
export const OFTEN = 2;

/** A YouTube video id: 11 letters, digits, underscores or hyphens. Anything else cannot be opened, and it ends up in a URL. */
const YOUTUBE_ID = /^[A-Za-z0-9_-]{11}$/;

/**
 * The songs played most on this device, most plays first (the more recently played one wins a tie). Only songs played at least
 * OFTEN times, and only ones that can be played from the app, so the shelf is a list of favourites and not of everything.
 */
export function topPlayed(tracks: PlayCountDto[], limit = 20): SearchHitDto[] {
	return tracks
		.filter((t) => t.provider === "YouTube" && t.plays >= OFTEN && YOUTUBE_ID.test(t.providerTrackId))
		.sort((a, b) => b.plays - a.plays || b.lastPlayedMs - a.lastPlayedMs)
		.slice(0, limit)
		.map((t) => ({
			provider: t.provider,
			providerTrackId: t.providerTrackId,
			canonicalUrl: `https://www.youtube.com/watch?v=${t.providerTrackId}`,
			title: t.title,
			artists: t.artists,
			artworkUrl: t.artworkUrl ?? null,
			durationMs: null,
		}));
}

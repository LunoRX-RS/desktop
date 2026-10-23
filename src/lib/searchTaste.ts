import type { SearchHitDto } from "../services/tauri/search";

/** Earlier searches that contain what is typed, closest matches (prefix) first. */
export function historyMatches(recent: string[], typed: string, limit = 4): string[] {
	const text = typed.trim().toLowerCase();
	if (text === "") return [];
	const lower = (s: string) => s.toLowerCase();
	const hits = recent.filter((r) => lower(r) !== text && lower(r).includes(text));
	return [...hits.filter((r) => lower(r).startsWith(text)), ...hits.filter((r) => !lower(r).startsWith(text))].slice(0, limit);
}

export type Artist = { name: string; artworkUrl: string | null; videoId: string; plays: number };

/**
 * The artists a person plays most, from their listening history (newest first). A recent play counts a little more than an old
 * one, so a new favourite rises quickly. Each artist carries the cover of the track that was played most recently.
 */
export function topArtists(history: SearchHitDto[], limit = 4): Artist[] {
	const byName = new Map<string, Artist & { score: number }>();
	history.forEach((hit, index) => {
		const weight = 1 + 1 / (1 + index / 10); // 2 for the newest play, towards 1 for old ones
		for (const raw of hit.artists.slice(0, 1)) {
			const name = raw.trim();
			if (name === "") continue;
			const key = name.toLowerCase();
			const seen = byName.get(key);
			if (seen) {
				seen.score += weight;
				seen.plays += 1;
			} else {
				byName.set(key, { name, artworkUrl: hit.artworkUrl, videoId: hit.providerTrackId, plays: 1, score: weight });
			}
		}
	});
	return [...byName.values()]
		.sort((a, b) => b.score - a.score)
		.slice(0, limit)
		.map(({ score: _score, ...artist }) => artist);
}

/** Suggested search terms, with those that mention an artist the person plays often moved to the front (order kept otherwise). */
export function personalizeTerms(terms: string[], artists: string[]): string[] {
	const names = artists.map((a) => a.toLowerCase()).filter((a) => a.length >= 2);
	const favourite = (t: string) => names.some((n) => t.toLowerCase().includes(n));
	return [...terms.filter(favourite), ...terms.filter((t) => !favourite(t))];
}

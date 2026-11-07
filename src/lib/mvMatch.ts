// The part of the music-video lookup that needs no network: the id check and the same-title match. Kept free of
// imports so it can be tested on its own.
const ID = /^[A-Za-z0-9_-]{11}$/;
export const isVideoId = (id: string) => ID.test(id);
export const normalizeTitle = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{M}\p{N}]+/gu, " ").trim();

/** The first YouTube hit whose title contains the wanted title, or null. */
export function pickMvId(title: string, hits: { provider: string; providerTrackId: string; title: string }[]): string | null {
  const want = normalizeTitle(title);
  if (!want) return null;
  return hits.find((h) => h.provider === "YouTube" && isVideoId(h.providerTrackId) && normalizeTitle(h.title).includes(want))?.providerTrackId ?? null;
}

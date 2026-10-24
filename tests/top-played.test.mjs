import { test } from "bun:test";
import assert from "node:assert/strict";
import { topPlayed } from "../src/lib/topPlayed.ts";

const row = (id, plays, last = 0, provider = "YouTube") => ({ provider, providerTrackId: id, title: id, artists: ["A"], artworkUrl: null, plays, lastPlayedMs: last });

test("songs are ordered by plays, a tie goes to the more recent one", () => {
	const out = topPlayed([row("a", 3, 10), row("b", 9, 1), row("c", 3, 20), row("d", 5, 0)]);
	assert.deepEqual(out.map((t) => t.providerTrackId), ["b", "d", "c", "a"]);
});

test("a song played once is not a favourite", () => {
	assert.deepEqual(topPlayed([row("once", 1), row("twice", 2)]).map((t) => t.providerTrackId), ["twice"]);
});

test("only songs that can be played from the app are listed", () => {
	assert.deepEqual(topPlayed([row("x", 9, 0, "Spotify"), row("y", 4), row("", 5)]).map((t) => t.providerTrackId), ["y"]);
});

test("the list is capped and each song can be opened", () => {
	const many = Array.from({ length: 40 }, (_, i) => row(`id${i}`, 40 - i));
	const out = topPlayed(many, 20);
	assert.equal(out.length, 20);
	assert.equal(out[0].canonicalUrl, "https://www.youtube.com/watch?v=id0");
	assert.equal(out[0].durationMs, null);
});

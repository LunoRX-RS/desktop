import { test } from "bun:test";
import assert from "node:assert/strict";
import { topPlayed } from "../src/lib/topPlayed.ts";

// Real ids are 11 characters; the short names in the tests are padded to that.
const vid = (s) => (s === "" ? s : s.padEnd(11, "_"));
const row = (id, plays, last = 0, provider = "YouTube") => ({ provider, providerTrackId: vid(id), title: id, artists: ["A"], artworkUrl: null, plays, lastPlayedMs: last });

test("songs are ordered by plays, a tie goes to the more recent one", () => {
	const out = topPlayed([row("a", 3, 10), row("b", 9, 1), row("c", 3, 20), row("d", 5, 0)]);
	assert.deepEqual(out.map((t) => t.providerTrackId), ["b", "d", "c", "a"].map(vid));
});

test("a song played once is not a favourite", () => {
	assert.deepEqual(topPlayed([row("once", 1), row("twice", 2)]).map((t) => t.providerTrackId), ["twice"].map(vid));
});

test("only songs that can be played from the app are listed", () => {
	assert.deepEqual(topPlayed([row("x", 9, 0, "Spotify"), row("y", 4), row("", 5)]).map((t) => t.providerTrackId), ["y"].map(vid));
});

test("ids that are not YouTube video ids are left out", () => {
	const raw = (id) => ({ ...row("x", 5), providerTrackId: id });
	const out = topPlayed([row("ok", 5), raw("tooshort10"), raw("abc&list=xyz1"), raw("a/b?c=d_efg"), raw("dQw4w9WgXcQ")]);
	assert.deepEqual(out.map((t) => t.providerTrackId), [vid("ok"), "dQw4w9WgXcQ"]);
});

test("the list is capped and each song can be opened", () => {
	const many = Array.from({ length: 40 }, (_, i) => row(`id${i}`, 40 - i));
	const out = topPlayed(many, 20);
	assert.equal(out.length, 20);
	assert.equal(out[0].canonicalUrl, "https://www.youtube.com/watch?v=id0________");
	assert.equal(out[0].durationMs, null);
});

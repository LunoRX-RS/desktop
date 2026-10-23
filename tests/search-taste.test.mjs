import { test } from "bun:test";
import assert from "node:assert/strict";
import { historyMatches, personalizeTerms, topArtists } from "../src/lib/searchTaste.ts";

const hit = (id, artist, art = null) => ({ provider: "YouTube", providerTrackId: id, canonicalUrl: "", title: id, artists: [artist], artworkUrl: art, durationMs: null });

test("history matches put prefix matches first and skip the exact query", () => {
	const recent = ["do you mind", "dj khaled", "my mind", "dj"];
	assert.deepEqual(historyMatches(recent, "dj"), ["dj khaled"]);
	assert.deepEqual(historyMatches(recent, "mind"), ["do you mind", "my mind"]);
	assert.deepEqual(historyMatches(["my mind", "mind games"], "mind"), ["mind games", "my mind"]);
	assert.deepEqual(historyMatches(recent, "  "), []);
	assert.equal(historyMatches(recent, "d", 2).length, 2);
});

test("top artists rank by plays", () => {
	const history = [hit("a1", "New Fav", "n.jpg"), hit("b1", "Old Fav"), hit("b2", "Old Fav"), hit("b3", "Old Fav"), hit("a2", "New Fav")];
	const artists = topArtists(history, 3);
	assert.deepEqual(artists.map((a) => a.name), ["Old Fav", "New Fav"]);
	assert.equal(artists.find((a) => a.name === "New Fav").artworkUrl, "n.jpg", "cover of the most recent play");
	assert.equal(artists[0].plays, 3);
});

test("with equal plays the more recent artist ranks first", () => {
	const history = [hit("a1", "Recent"), hit("b1", "Older"), hit("a2", "Recent"), hit("b2", "Older")];
	assert.deepEqual(topArtists(history).map((a) => a.name), ["Recent", "Older"]);
});

test("artists are merged ignoring case and empty names are skipped", () => {
	const artists = topArtists([hit("a", "Creepy Nuts"), hit("b", "creepy nuts"), hit("c", "  ")]);
	assert.equal(artists.length, 1);
	assert.equal(artists[0].plays, 2);
});

test("personalised terms move favourite artists up and keep the rest in order", () => {
	const terms = ["fame is a gun lyrics", "creepy nuts otonoke", "bad guy", "creepy nuts bling"];
	assert.deepEqual(personalizeTerms(terms, ["Creepy Nuts"]), ["creepy nuts otonoke", "creepy nuts bling", "fame is a gun lyrics", "bad guy"]);
	assert.deepEqual(personalizeTerms(terms, []), terms);
});

import { test } from "node:test";
import assert from "node:assert/strict";
import { isVideoId, normalizeTitle, pickMvId } from "../src/lib/mvMatch.ts";

test("only 11-character YouTube ids count as ids", () => {
  assert.equal(isVideoId("dQw4w9WgXcQ"), true);
  assert.equal(isVideoId("short"), false);
  assert.equal(isVideoId("dQw4w9WgXc!"), false);
  assert.equal(isVideoId(""), false);
});

test("titles compare ignoring case, punctuation and spacing", () => {
  assert.equal(normalizeTitle("  BU (Official MV)! "), "bu official mv");
  assert.equal(normalizeTitle("พอตเตอร์"), "พอตเตอร์");
});

test("picks the first YouTube hit whose title contains the wanted title", () => {
  const hits = [
    { provider: "Spotify", providerTrackId: "aaaaaaaaaaa", title: "BU" },
    { provider: "YouTube", providerTrackId: "bbbbbbbbbbb", title: "Cover of BU" },
    { provider: "YouTube", providerTrackId: "ccccccccccc", title: "BU - Official MV" },
  ];
  assert.equal(pickMvId("BU", hits), "bbbbbbbbbbb");
});

test("no match, bad ids or an empty title give null", () => {
  assert.equal(pickMvId("BU", [{ provider: "YouTube", providerTrackId: "bad", title: "BU" }]), null);
  assert.equal(pickMvId("BU", [{ provider: "YouTube", providerTrackId: "dQw4w9WgXcQ", title: "Other" }]), null);
  assert.equal(pickMvId("  ", [{ provider: "YouTube", providerTrackId: "dQw4w9WgXcQ", title: "x" }]), null);
  assert.equal(pickMvId("BU", []), null);
});

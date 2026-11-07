import { test } from "node:test";
import assert from "node:assert/strict";
import { isVideoId } from "../src/lib/mvMatch.ts";

test("only 11-character YouTube ids count as ids", () => {
  assert.equal(isVideoId("dQw4w9WgXcQ"), true);
  assert.equal(isVideoId("short"), false);
  assert.equal(isVideoId("dQw4w9WgXc!"), false);
  assert.equal(isVideoId(""), false);
});

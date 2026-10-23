import { test } from "bun:test";
import assert from "node:assert/strict";
import { ACTIONS, actionFor, bindingsFor, checkCombo, comboFromEvent, formatCombo, hasCommandModifier, parseOverrides } from "../src/lib/shortcuts.ts";

const press = (code, mods = {}) => ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

test("defaults use Command on macOS and Ctrl elsewhere", () => {
	assert.deepEqual(bindingsFor("mac", {}).palette, ["Meta+KeyK"]);
	assert.deepEqual(bindingsFor("linux", {}).palette, ["Ctrl+KeyK"]);
	assert.deepEqual(bindingsFor("other", {}).back, ["Ctrl+BracketLeft"]);
	assert.deepEqual(bindingsFor("linux", {}).playPause, ["Space"]);
});

test("a key press becomes a combo by physical key, in a fixed modifier order", () => {
	assert.equal(comboFromEvent(press("KeyK", { ctrlKey: true })), "Ctrl+KeyK");
	assert.equal(comboFromEvent(press("KeyB", { metaKey: true, shiftKey: true, altKey: true, ctrlKey: true })), "Ctrl+Alt+Shift+Meta+KeyB");
	assert.equal(comboFromEvent(press("Space")), "Space");
});

test("holding only modifiers is not a combo", () => {
	for (const code of ["ControlLeft", "ShiftRight", "AltLeft", "MetaLeft", "OSLeft"]) assert.equal(comboFromEvent(press(code, { ctrlKey: true })), null);
});

test("zoom in also answers to Shift+= and the numpad on every platform", () => {
	const b = bindingsFor("linux", {});
	assert.equal(actionFor(comboFromEvent(press("Equal", { ctrlKey: true, shiftKey: true })), b), "zoomIn");
	assert.equal(actionFor(comboFromEvent(press("NumpadAdd", { ctrlKey: true })), b), "zoomIn");
});

test("an override replaces all defaults of that action only", () => {
	const b = bindingsFor("linux", { zoomIn: "Alt+KeyZ" });
	assert.deepEqual(b.zoomIn, ["Alt+KeyZ"]);
	assert.equal(actionFor("Ctrl+Equal", b), null);
	assert.equal(actionFor("Ctrl+Minus", b), "zoomOut");
});

test("format follows the platform", () => {
	assert.equal(formatCombo("Meta+KeyK", "mac"), "⌘K");
	assert.equal(formatCombo("Ctrl+Alt+Shift+Meta+BracketLeft", "mac"), "⌃⌥⇧⌘[");
	assert.equal(formatCombo("Ctrl+KeyK", "linux"), "Ctrl+K");
	assert.equal(formatCombo("Meta+KeyK", "linux"), "Super+K");
	assert.equal(formatCombo("Meta+KeyK", "other"), "Win+K");
	assert.equal(formatCombo("Space", "linux"), "Space");
	assert.equal(formatCombo("Ctrl+Digit0", "other"), "Ctrl+0");
});

test("a combo needs a command modifier unless it is Space or an F-key", () => {
	const b = bindingsFor("linux", {});
	assert.equal(checkCombo("KeyA", "palette", b).ok, false);
	assert.equal(checkCombo("Shift+KeyA", "palette", b).ok, false);
	assert.equal(checkCombo("Alt+KeyA", "palette", b).ok, true);
	assert.equal(checkCombo("F5", "palette", b).ok, true);
	assert.equal(checkCombo("Space", "palette", { ...b, playPause: [] }).ok, true);
	assert.equal(checkCombo("Escape", "palette", b).ok, false);
	assert.equal(checkCombo("Ctrl+Escape", "palette", b).ok, false);
});

test("a combo already taken by another action is refused, but the owner may keep it", () => {
	const b = bindingsFor("linux", {});
	const taken = checkCombo("Ctrl+KeyB", "palette", b);
	assert.equal(taken.ok, false);
	assert.match(taken.reason, /sidebar/);
	assert.equal(checkCombo("Ctrl+KeyB", "toggleSidebar", b).ok, true);
});

test("saved choices are read defensively", () => {
	assert.deepEqual(parseOverrides(null), {});
	assert.deepEqual(parseOverrides("not json"), {});
	assert.deepEqual(parseOverrides("[1,2]"), {});
	assert.deepEqual(parseOverrides(JSON.stringify({ palette: "Alt+KeyP", nope: "Ctrl+KeyX", back: 5, forward: "" })), { palette: "Alt+KeyP" });
});

test("every action has defaults and a unique default combo per platform", () => {
	for (const p of ["mac", "linux", "other"]) {
		const seen = new Map();
		for (const a of ACTIONS) {
			const combos = a.defaults(p);
			assert.ok(combos.length > 0, a.id);
			for (const c of combos) {
				assert.equal(seen.has(c), false, `${c} is the default of both ${seen.get(c)} and ${a.id} on ${p}`);
				seen.set(c, a.id);
			}
		}
	}
});

test("hasCommandModifier", () => {
	assert.equal(hasCommandModifier("Ctrl+KeyK"), true);
	assert.equal(hasCommandModifier("Shift+KeyK"), false);
	assert.equal(hasCommandModifier("Space"), false);
});

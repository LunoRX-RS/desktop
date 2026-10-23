import { useSyncExternalStore } from "react";
import { detectPlatform, type Platform } from "./platform";

/**
 * Global keyboard shortcuts that the user can change. A combo is a string of modifiers in a fixed order followed by a physical
 * key code, for example "Ctrl+KeyK" or "Meta+BracketLeft". Physical codes keep the shortcuts working on any keyboard layout
 * (a Thai layout types other characters for the same key). Defaults follow the system: Command on macOS, Ctrl elsewhere.
 */
export type ActionId = "palette" | "back" | "forward" | "toggleSidebar" | "zoomIn" | "zoomOut" | "zoomReset" | "playPause";

export type ActionDef = {
	id: ActionId;
	label: string;
	group: "Search" | "Navigation" | "View" | "Playback";
	defaults: (platform: Platform) => string[];
};

const mod = (p: Platform) => (p === "mac" ? "Meta" : "Ctrl");

export const ACTIONS: ActionDef[] = [
	{ id: "palette", label: "Quick search and commands", group: "Search", defaults: (p) => [`${mod(p)}+KeyK`] },
	{ id: "back", label: "Back", group: "Navigation", defaults: (p) => [`${mod(p)}+BracketLeft`] },
	{ id: "forward", label: "Forward", group: "Navigation", defaults: (p) => [`${mod(p)}+BracketRight`] },
	{ id: "toggleSidebar", label: "Collapse or expand the sidebar", group: "Navigation", defaults: (p) => [`${mod(p)}+KeyB`] },
	// Shift+Equal is "+" on a US keyboard, so the plain "+" press is an alias of the default.
	{ id: "zoomIn", label: "Larger text", group: "View", defaults: (p) => [`${mod(p)}+Equal`, `${mod(p)}+Shift+Equal`, `${mod(p)}+NumpadAdd`] },
	{ id: "zoomOut", label: "Smaller text", group: "View", defaults: (p) => [`${mod(p)}+Minus`, `${mod(p)}+NumpadSubtract`] },
	{ id: "zoomReset", label: "Reset text size", group: "View", defaults: (p) => [`${mod(p)}+Digit0`, `${mod(p)}+Numpad0`] },
	{ id: "playPause", label: "Play or pause (not while typing)", group: "Playback", defaults: () => ["Space"] },
];

const MODIFIER_CODES = new Set(["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "OSLeft", "OSRight", "AltGraph", "CapsLock", "ContextMenu"]);

type KeyEventLike = { code: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean };

/** The combo a key press makes, or null while only modifiers are held. */
export function comboFromEvent(e: KeyEventLike): string | null {
	if (!e.code || e.code === "Unidentified" || MODIFIER_CODES.has(e.code)) return null;
	const parts: string[] = [];
	if (e.ctrlKey) parts.push("Ctrl");
	if (e.altKey) parts.push("Alt");
	if (e.shiftKey) parts.push("Shift");
	if (e.metaKey) parts.push("Meta");
	parts.push(e.code);
	return parts.join("+");
}

/** True when the combo holds Ctrl, Alt or Meta, so it cannot be typed into a text field by accident. */
export function hasCommandModifier(combo: string): boolean {
	const parts = combo.split("+");
	return parts.includes("Ctrl") || parts.includes("Alt") || parts.includes("Meta");
}

const NAMED_KEYS: Record<string, string> = {
	BracketLeft: "[",
	BracketRight: "]",
	Equal: "=",
	Minus: "-",
	Comma: ",",
	Period: ".",
	Slash: "/",
	Backslash: "\\",
	Semicolon: ";",
	Quote: "'",
	Backquote: "`",
	Space: "Space",
	ArrowLeft: "←",
	ArrowRight: "→",
	ArrowUp: "↑",
	ArrowDown: "↓",
	NumpadAdd: "Num +",
	NumpadSubtract: "Num −",
	Numpad0: "Num 0",
};

function keyLabel(code: string): string {
	if (NAMED_KEYS[code]) return NAMED_KEYS[code];
	if (code.startsWith("Key")) return code.slice(3);
	if (code.startsWith("Digit")) return code.slice(5);
	return code;
}

/** Human text for a combo: ⌃⌥⇧⌘K on macOS, Ctrl+Alt+Shift+Super+K (Win on Windows) elsewhere. */
export function formatCombo(combo: string, platform: Platform): string {
	const parts = combo.split("+");
	const code = parts[parts.length - 1];
	const has = (m: string) => parts.slice(0, -1).includes(m);
	if (platform === "mac") {
		return `${has("Ctrl") ? "⌃" : ""}${has("Alt") ? "⌥" : ""}${has("Shift") ? "⇧" : ""}${has("Meta") ? "⌘" : ""}${keyLabel(code)}`;
	}
	const names = [has("Ctrl") && "Ctrl", has("Alt") && "Alt", has("Shift") && "Shift", has("Meta") && (platform === "linux" ? "Super" : "Win")];
	return [...names.filter(Boolean), keyLabel(code)].join("+");
}

export type Bindings = Record<ActionId, string[]>;
export type Overrides = Partial<Record<ActionId, string>>;

export function bindingsFor(platform: Platform, overrides: Overrides): Bindings {
	const out = {} as Bindings;
	for (const a of ACTIONS) out[a.id] = overrides[a.id] ? [overrides[a.id] as string] : a.defaults(platform);
	return out;
}

export function actionFor(combo: string, bindings: Bindings): ActionId | null {
	for (const a of ACTIONS) if (bindings[a.id].includes(combo)) return a.id;
	return null;
}

export type Check = { ok: true } | { ok: false; reason: string };

/** Whether `combo` may be assigned to `action`: it needs a command modifier (except Space and F-keys), must not be Escape, and must be free. */
export function checkCombo(combo: string, action: ActionId, bindings: Bindings): Check {
	const code = combo.split("+").pop() ?? "";
	if (code === "Escape") return { ok: false, reason: "Esc is reserved for closing things." };
	const bare = code === "Space" || /^F\d{1,2}$/.test(code);
	if (!hasCommandModifier(combo) && !bare) return { ok: false, reason: "Hold Ctrl, Alt or ⌘ with the key, so it cannot clash with typing." };
	const owner = actionFor(combo, bindings);
	if (owner && owner !== action) {
		const label = ACTIONS.find((a) => a.id === owner)?.label ?? owner;
		return { ok: false, reason: `Already used by “${label}”.` };
	}
	return { ok: true };
}

const STORAGE_KEY = "lunorx.shortcuts";

function storage(): Storage | null {
	try {
		return typeof window === "undefined" ? null : window.localStorage;
	} catch {
		return null;
	}
}

/** Reads saved choices, ignoring anything that is not a known action with a text combo (a corrupt or edited store). */
export function parseOverrides(raw: string | null): Overrides {
	if (!raw) return {};
	try {
		const parsed: unknown = JSON.parse(raw);
		if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
		const out: Overrides = {};
		for (const a of ACTIONS) {
			const v = (parsed as Record<string, unknown>)[a.id];
			if (typeof v === "string" && v.length > 0 && v.length <= 64) out[a.id] = v;
		}
		return out;
	} catch {
		return {};
	}
}

let overrides: Overrides = {};
let loaded = false;
const listeners = new Set<() => void>();

function load() {
	if (loaded) return;
	loaded = true;
	overrides = parseOverrides(storage()?.getItem(STORAGE_KEY) ?? null);
}

function commit(next: Overrides) {
	overrides = next;
	try {
		if (Object.keys(next).length === 0) storage()?.removeItem(STORAGE_KEY);
		else storage()?.setItem(STORAGE_KEY, JSON.stringify(next));
	} catch {
		/* Private mode or a full store: the choice still holds for this session. */
	}
	for (const l of listeners) l();
}

export function setShortcut(action: ActionId, combo: string) {
	load();
	commit({ ...overrides, [action]: combo });
}

export function resetShortcut(action: ActionId) {
	load();
	const { [action]: _removed, ...rest } = overrides;
	commit(rest);
}

export function resetAllShortcuts() {
	load();
	commit({});
}

export function getOverrides(): Overrides {
	load();
	return overrides;
}

function subscribe(listener: () => void) {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** Current overrides, re-rendering when one changes. */
export function useOverrides(): Overrides {
	return useSyncExternalStore(subscribe, getOverrides, () => ({}));
}

export function currentBindings(): Bindings {
	return bindingsFor(detectPlatform(), getOverrides());
}

"use client";
import { useSyncExternalStore } from "react";

// The colour theme and the one accent. Applied as data attributes on <html>; app/themes.css re-points the tokens.
export const THEMES = [
  { id: "lunor", label: "LUNOR dark" },
  { id: "graphite", label: "Graphite" },
  { id: "midnight", label: "Midnight blue" },
  { id: "light", label: "Light" },
] as const;
export const ACCENTS = [
  { id: "lavender", label: "Lumen lavender" },
  { id: "blue", label: "Blue" },
  { id: "mint", label: "Mint" },
  { id: "rose", label: "Rose" },
  { id: "amber", label: "Amber" },
] as const;
export type ThemeId = (typeof THEMES)[number]["id"];
export type AccentId = (typeof ACCENTS)[number]["id"];
export type BackdropId = "artwork" | "image" | "plain";
export interface Theme {
  theme: ThemeId;
  accent: AccentId;
  /** What sits behind the glass: the playing artwork (default), the user's own image, or a plain canvas. */
  backdrop: BackdropId;
  /** How dark (or, in Light, how pale) the layer over the backdrop is, 0 to 90 percent. */
  dim: number;
  /** Extra blur on the user's image, 0 to 60 px. */
  blur: number;
  /** The user's own CSS, applied last. */
  customCss: string;
}
export const MAX_CSS = 20_000;

const KEY = "lunorx.theme";
const DEFAULT: Theme = { theme: "lunor", accent: "lavender", backdrop: "artwork", dim: 60, blur: 0, customCss: "" };
const IMAGE_KEY = "lunorx.theme.image";
const num = (v: unknown, lo: number, hi: number, d: number) => (typeof v === "number" && Number.isFinite(v) ? Math.round(Math.min(hi, Math.max(lo, v))) : d);
const clean = (v: unknown): Theme => {
  const o = (v ?? {}) as Partial<Theme>;
  return {
    theme: THEMES.some((t) => t.id === o.theme) ? (o.theme as ThemeId) : DEFAULT.theme,
    accent: ACCENTS.some((a) => a.id === o.accent) ? (o.accent as AccentId) : DEFAULT.accent,
    backdrop: o.backdrop === "image" || o.backdrop === "plain" ? o.backdrop : "artwork",
    dim: num(o.dim, 0, 90, DEFAULT.dim),
    blur: num(o.blur, 0, 60, DEFAULT.blur),
    customCss: typeof o.customCss === "string" ? o.customCss.slice(0, MAX_CSS) : "",
  };
};
let state: Theme = DEFAULT;
const listeners = new Set<() => void>();

let image: string | null = null;
let userSheet: CSSStyleSheet | null = null;
/** Why the custom CSS could not be applied, or null when it was. */
export let cssProblem: string | null = null;

function apply() {
  if (typeof document === "undefined") return;
  const html = document.documentElement;
  if (state.theme === "lunor") delete html.dataset.theme; else html.dataset.theme = state.theme;
  if (state.accent === "lavender") delete html.dataset.accent; else html.dataset.accent = state.accent;
  const bg = state.backdrop === "image" && image ? "image" : state.backdrop === "plain" ? "plain" : null;
  if (bg) html.dataset.backdrop = bg; else delete html.dataset.backdrop;
  // CSSOM, not a style attribute: the app's CSP allows no inline styles.
  if (bg === "image") html.style.setProperty("--lx-user-bg", `url("${image}")`); else html.style.removeProperty("--lx-user-bg");
  html.style.setProperty("--lx-bg-dim", String(state.dim / 100));
  html.style.setProperty("--lx-bg-blur", `${state.blur}px`);
  // The user's CSS goes into a constructed stylesheet, added after the app's own.
  cssProblem = null;
  try {
    if (!userSheet) { userSheet = new CSSStyleSheet(); document.adoptedStyleSheets = [...document.adoptedStyleSheets, userSheet]; }
    userSheet.replaceSync(state.customCss);
  } catch {
    cssProblem = "This system webview could not apply custom CSS.";
  }
}

/** The user's background image, scaled to at most 2560 px wide and stored as a JPEG on this device. */
export async function setBackdropImage(file: File): Promise<void> {
  if (!file.type.startsWith("image/")) throw new Error("not an image");
  const bitmap = await createImageBitmap(file);
  const k = Math.min(1, 2560 / bitmap.width, 1600 / bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(bitmap.width * k);
  canvas.height = Math.round(bitmap.height * k);
  canvas.getContext("2d")?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  const data = canvas.toDataURL("image/jpeg", 0.85);
  window.localStorage.setItem(IMAGE_KEY, data); // throws when storage is full; the caller says so
  image = data;
  setTheme({ backdrop: "image" });
}
export function clearBackdropImage() {
  try { window.localStorage.removeItem(IMAGE_KEY); } catch { /* nothing kept */ }
  image = null;
  setTheme({ backdrop: "artwork" });
}
export const hasBackdropImage = () => image !== null;

export function setTheme(patch: Partial<Theme>) {
  state = clean({ ...state, ...patch });
  try { window.localStorage.setItem(KEY, JSON.stringify(state)); } catch { /* lasts for this session */ }
  apply();
  listeners.forEach((l) => l());
}
/** Reads the saved theme and applies it. Call once on startup. */
export function initTheme() {
  try { state = clean(JSON.parse(window.localStorage.getItem(KEY) ?? "{}")); } catch { state = DEFAULT; }
  try { image = window.localStorage.getItem(IMAGE_KEY); } catch { image = null; }
  apply();
}
const subscribe = (cb: () => void) => { listeners.add(cb); return () => { listeners.delete(cb); }; };
export const useTheme = (): Theme => useSyncExternalStore(subscribe, () => state, () => DEFAULT);

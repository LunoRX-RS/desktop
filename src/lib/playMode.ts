"use client";
import { playerSetMode } from "../services/tauri/player";
import { usePlayback } from "./playback";

export type Repeat = "off" | "all" | "one";
/** How closely autoplay keeps to what plays: the same mood, language and genre; a balance; or YouTube's own mix. */
export type Vibe = "same" | "balanced" | "explore";
export const VIBES: Vibe[] = ["same", "balanced", "explore"];
const VIBE_KEY = "lunorx.vibe";
const REPEAT_KEY = "lunorx.repeat";
const AUTOPLAY_KEY = "lunorx.autoplay";
const REPEATS: Repeat[] = ["off", "all", "one"];

function read(key: string): string | null { try { return window.localStorage.getItem(key); } catch { return null; } }
function write(key: string, value: string) { try { window.localStorage.setItem(key, value); } catch { /* the choice still applies this session */ } }

/** What core reports now, falling back to the defaults (repeat off, autoplay on). */
export const currentMode = () => {
  const a = usePlayback.getState().autoplay;
  return { repeat: (a.repeat ?? "off") as Repeat, autoplay: a.radio ?? true, vibe: (a.vibe ?? "balanced") as Vibe };
};

export function setPlayMode(next: { repeat?: Repeat; autoplay?: boolean; vibe?: Vibe }) {
  const cur = currentMode();
  const repeat = next.repeat ?? cur.repeat;
  const autoplay = next.autoplay ?? cur.autoplay;
  const vibe = next.vibe ?? cur.vibe;
  write(REPEAT_KEY, repeat);
  write(AUTOPLAY_KEY, String(autoplay));
  write(VIBE_KEY, vibe);
  void playerSetMode(repeat, autoplay, vibe).catch(() => {});
}

export const nextRepeat = (r: Repeat): Repeat => REPEATS[(REPEATS.indexOf(r) + 1) % REPEATS.length];

/** Sends the saved choices to core once at start, since core forgets them when the app closes. */
export function applySavedMode() {
  const repeat = read(REPEAT_KEY);
  const autoplay = read(AUTOPLAY_KEY);
  const vibe = read(VIBE_KEY);
  if (repeat === null && autoplay === null && vibe === null) return;
  void playerSetMode(
    REPEATS.includes(repeat as Repeat) ? (repeat as Repeat) : "off",
    autoplay !== "false",
    VIBES.includes(vibe as Vibe) ? (vibe as Vibe) : "balanced",
  ).catch(() => {});
}

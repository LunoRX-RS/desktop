import type { AudioEffects } from "../services/tauri/player";
import type { DeckLoop } from "../services/tauri/dj";

/** What a DJ remembers about one track: where the cues are, the loop, and how it was shaped. Kept on this device. */
export interface TrackSet {
  cues: (number | null)[];
  loop: DeckLoop | null;
  effects: AudioEffects | null;
}

const KEY = "lunorx.djSets";
const MAX = 200;

function read(): Record<string, TrackSet> {
  try {
    const raw = window.localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as Record<string, TrackSet>) : {};
  } catch {
    return {};
  }
}

export function loadSet(videoId: string): TrackSet | null {
  const s = read()[videoId];
  if (!s || !Array.isArray(s.cues)) return null;
  return { cues: [0, 1, 2, 3].map((i) => (typeof s.cues[i] === "number" ? s.cues[i] : null)), loop: s.loop ?? null, effects: s.effects ?? null };
}

export function saveSet(videoId: string, set: TrackSet): void {
  try {
    const all = read();
    delete all[videoId];
    all[videoId] = set;
    // Oldest first: drop from the front when there are too many.
    const ids = Object.keys(all);
    for (const id of ids.slice(0, Math.max(0, ids.length - MAX))) delete all[id];
    window.localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* Storage can be full or blocked; the set is a convenience, never required. */
  }
}

/** A track to put on deck A the next time the DJ screen opens, continuing from where the player was. */
export interface Handoff { videoId: string; title: string; positionMs: number }
let handoff: Handoff | null = null;
export function setHandoff(h: Handoff | null): void { handoff = h; }
export function takeHandoff(): Handoff | null { const h = handoff; handoff = null; return h; }

/** A lyric line the DJ marked at a moment in the track. */
export interface LyricMark { line: number; ms: number; text: string }
const MARKS = "lunorx.djLyricMarks";

export function loadMarks(videoId: string): LyricMark[] {
  try {
    const all = JSON.parse(window.localStorage.getItem(MARKS) ?? "{}") as Record<string, LyricMark[]>;
    const list = all[videoId];
    return Array.isArray(list) ? list.filter((m) => typeof m.line === "number" && typeof m.ms === "number" && typeof m.text === "string") : [];
  } catch {
    return [];
  }
}

/** Saves the marks for one track. Returns false when storage refused, so the screen can say so. */
export function saveMarks(videoId: string, marks: LyricMark[]): boolean {
  try {
    const all = JSON.parse(window.localStorage.getItem(MARKS) ?? "{}") as Record<string, LyricMark[]>;
    if (marks.length) all[videoId] = marks; else delete all[videoId];
    window.localStorage.setItem(MARKS, JSON.stringify(all));
    return true;
  } catch {
    return false;
  }
}

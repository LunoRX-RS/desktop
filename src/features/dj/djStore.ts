"use client";

import { useCallback } from "react";
import { create } from "zustand";
import type { DeckId, DeckReport } from "../../services/tauri/dj";
import { FLAT_EFFECTS, type AudioEffects } from "../../services/tauri/player";

// The decks keep playing in their own windows when the DJ screen is closed, so what the screen shows lives here, for
// as long as the app runs, not in the screen's components. Cues, loops, templates and lyric marks are also saved on disk.

export interface DeckData {
  videoId: string | null;
  title: string;
  bpm: number | null;
  filter: number;
  trim: number;
  fader: number;
  echo: number;
  templateId: string | null;
  fx: AudioEffects;
  cues: (number | null)[];
  loop: { startMs: number | null; endMs: number | null; on: boolean };
}
export interface Tempo { bpm: number | null; speed: number }

const emptyDeck = (): DeckData => ({
  videoId: null, title: "", bpm: null, filter: 0, trim: 100, fader: 100, echo: 0, templateId: null,
  fx: FLAT_EFFECTS, cues: [null, null, null, null], loop: { startMs: null, endMs: null, on: false },
});

interface DjStore {
  decks: Record<DeckId, DeckData>;
  reports: Record<DeckId, DeckReport | null>;
  cross: number;
  master: number;
  tempo: Record<DeckId, Tempo>;
}

export const useDj = create<DjStore>(() => ({
  decks: { a: emptyDeck(), b: emptyDeck() },
  reports: { a: null, b: null },
  cross: 50,
  master: 100,
  tempo: { a: { bpm: null, speed: 100 }, b: { bpm: null, speed: 100 } },
}));

type Update<T> = T | ((prev: T) => T);
const resolve = <T,>(u: Update<T>, prev: T): T => (typeof u === "function" ? (u as (p: T) => T)(prev) : u);

/** One field of one deck, read and written like `useState`. */
export function useDeck<K extends keyof DeckData>(id: DeckId, key: K): [DeckData[K], (u: Update<DeckData[K]>) => void] {
  const value = useDj((s) => s.decks[id][key]);
  const set = useCallback((u: Update<DeckData[K]>) => {
    useDj.setState((s) => ({ decks: { ...s.decks, [id]: { ...s.decks[id], [key]: resolve(u, s.decks[id][key]) } } }));
  }, [id, key]);
  return [value, set];
}

/** One top-level field (reports, crossfader, master, tempo), read and written like `useState`. */
export function useDjField<K extends Exclude<keyof DjStore, "decks">>(key: K): [DjStore[K], (u: Update<DjStore[K]>) => void] {
  const value = useDj((s) => s[key]);
  const set = useCallback((u: Update<DjStore[K]>) => {
    useDj.setState((s) => ({ [key]: resolve(u, s[key]) }) as Partial<DjStore>);
  }, [key]);
  return [value, set];
}

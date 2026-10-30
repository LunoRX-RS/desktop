import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "./invoke";
import type { AudioEffects } from "./player";

export type DeckId = "a" | "b";
export type DeckState = "playing" | "paused" | "waiting" | "ended" | "error";
export interface DeckReport { deck: DeckId; videoId: string; positionMs: number; durationMs: number; state: DeckState }
export interface DeckLoop { startMs: number; endMs: number }

/** Loads a video onto a deck, paused. The deck is a YouTube page; nothing plays until `djPlay`. */
export const djLoad = (deck: DeckId, videoId: string): Promise<void> => invoke("dj_load", { deck, videoId });
export const djPlay = (deck: DeckId): Promise<void> => invoke("dj_play", { deck });
export const djPause = (deck: DeckId): Promise<void> => invoke("dj_pause", { deck });
export const djSeek = (deck: DeckId, positionMs: number): Promise<void> => invoke("dj_seek", { deck, positionMs: Math.max(0, Math.round(positionMs)) });
export const djSetEffects = (deck: DeckId, effects: AudioEffects): Promise<void> => invoke("dj_set_effects", { deck, effects });
export const djSetLoop = (deck: DeckId, looping: DeckLoop | null): Promise<void> => invoke("dj_set_loop", { deck, looping });
/** Crossfader 0 (all A) to 100 (all B), master volume 0 to 100. */
export const djSetMix = (crossfader: number, master: number): Promise<void> => invoke("dj_set_mix", { crossfader, master });
export type DeckControl =
  | { kind: "filter"; value: number }
  | { kind: "trim"; value: number }
  | { kind: "fader"; value: number }
  | { kind: "bend"; value: number }
  | { kind: "echo"; delayMs: number; feedback: number }
  | { kind: "brake" };
export const djControl = (deck: DeckId, control: DeckControl): Promise<void> => invoke("dj_control", { deck, control });
/** The decks core knows about, with their latest report. Used to catch up after the DJ screen was closed. */
export const djStatus = (): Promise<DeckReport[]> => invoke("dj_status");
export const djStop = (deck: DeckId): Promise<void> => invoke("dj_stop", { deck });

export function subscribeDeck(callback: (report: DeckReport) => void): Promise<UnlistenFn> {
  return listen<DeckReport>("core://dj/deck", (event) => callback(event.payload));
}

/** Low, mid and high energy (0 to 255), one sample every 50 ms from `atMs`, heard by the deck while it plays. */
export interface WaveBatch { deck: DeckId; videoId: string; atMs: number; bands: [number, number, number][] }
export function subscribeWave(callback: (batch: WaveBatch) => void): Promise<UnlistenFn> {
  return listen<WaveBatch>("core://dj/wave", (event) => callback(event.payload));
}

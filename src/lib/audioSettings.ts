"use client";
import { playerAudioSettings, playerSetAudioSettings, type AudioSettings } from "../services/tauri/player";

const KEY = "lunorx.audio";

export function saveAudioSettings(settings: AudioSettings) {
  try { window.localStorage.setItem(KEY, JSON.stringify(settings)); } catch { /* the choice still applies this session */ }
}

/** Sends the saved sound settings to core once at start, since core forgets them when the app closes. A bad or old file is ignored. */
export async function applySavedAudio() {
  let saved: unknown;
  try { saved = JSON.parse(window.localStorage.getItem(KEY) ?? "null"); } catch { return; }
  if (!saved || typeof saved !== "object") return;
  try {
    const current = (await playerAudioSettings()).settings;
    await playerSetAudioSettings({ ...current, ...(saved as Partial<AudioSettings>), effects: { ...current.effects, ...((saved as Partial<AudioSettings>).effects ?? {}) } });
  } catch { /* core refused it (not available, or out of range): keep the defaults */ }
}

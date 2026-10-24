"use client";
import { playerAudioDevices, playerAudioSettings, playerSetAudioDevice, playerSetAudioSettings, subscribeAudioSettings, type AudioSettings } from "../services/tauri/player";

export const AUDIO_CHANGED = "lunorx:audio-changed";

const KEY = "lunorx.audio";

export function saveAudioSettings(settings: AudioSettings) {
  try { window.localStorage.setItem(KEY, JSON.stringify(settings)); } catch { /* the choice still applies this session */ }
}

const DEVICE_KEY = "lunorx.audioDevice";
export const saveAudioDevice = (id: string) => { try { window.localStorage.setItem(DEVICE_KEY, id); } catch { /* the choice still applies this session */ } };

/** Sends sound back to the saved output device at start. The device list fills in a moment after the player starts, so it
 *  tries a few times; a device that is not plugged in today is skipped and sound stays on the automatic choice. */
async function applySavedDevice() {
  let id: string | null = null;
  try { id = window.localStorage.getItem(DEVICE_KEY); } catch { return; }
  if (!id || id === "auto") return;
  for (let i = 0; i < 6; i += 1) {
    try {
      if ((await playerAudioDevices()).devices.some((d) => d.id === id)) { await playerSetAudioDevice(id); return; }
    } catch { return; }
    await new Promise((r) => window.setTimeout(r, 1500));
  }
}

/** Sends the saved sound settings to core once at start, since core forgets them when the app closes. A bad or old file is ignored. */
export async function applySavedAudio() {
  void applySavedDevice();
  let saved: unknown;
  try { saved = JSON.parse(window.localStorage.getItem(KEY) ?? "null"); } catch { return; }
  if (!saved || typeof saved !== "object") return;
  try {
    const current = (await playerAudioSettings()).settings;
    await playerSetAudioSettings({ ...current, ...(saved as Partial<AudioSettings>), effects: { ...current.effects, ...((saved as Partial<AudioSettings>).effects ?? {}) } });
  } catch { /* core refused it (not available, or out of range): keep the defaults */ }
}

/** Keeps the saved sound settings in step with changes made outside the panel (the macOS Sound menu), and tells the panel. */
export function watchAudioSettings(): () => void {
  let off: (() => void) | null = null;
  let live = true;
  subscribeAudioSettings((dto) => {
    saveAudioSettings(dto.settings);
    window.dispatchEvent(new CustomEvent(AUDIO_CHANGED, { detail: dto }));
  }).then((fn) => { if (live) off = fn; else fn(); }, () => {});
  return () => { live = false; off?.(); };
}

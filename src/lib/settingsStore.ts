"use client";

import { create } from "zustand";

interface SettingsState {
  devMode: boolean;
  toggleDevMode: () => void;
  /** Off by default: the DJ screen, its sidebar entry and the player-bar button appear only once turned on. */
  djMode: boolean;
  setDjMode: (on: boolean) => void;
}

const DEV_MODE_KEY = "lunorx.settings.dev_mode";
const DJ_MODE_KEY = "lunorx.settings.dj_mode";

export const useSettingsStore = create<SettingsState>((set) => ({
  devMode: (() => {
    try {
      return localStorage.getItem(DEV_MODE_KEY) === "true";
    } catch {
      return false;
    }
  })(),
  toggleDevMode: () => set((s) => {
    const next = !s.devMode;
    try { localStorage.setItem(DEV_MODE_KEY, String(next)); } catch {}
    return { devMode: next };
  }),
  djMode: (() => {
    try { return localStorage.getItem(DJ_MODE_KEY) === "true"; } catch { return false; }
  })(),
  setDjMode: (on) => {
    try { localStorage.setItem(DJ_MODE_KEY, String(on)); } catch {}
    set({ djMode: on });
  },
}));

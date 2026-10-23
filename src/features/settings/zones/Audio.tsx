"use client";

import { AudioSettingsPanel } from "../AudioSettings";

/** Stream quality, output device format and sound shaping (the equalizer lives inside the panel). */
export function Audio() {
  return <AudioSettingsPanel />;
}

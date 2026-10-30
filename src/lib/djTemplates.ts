import { FLAT_EFFECTS, type AudioEffects } from "../services/tauri/player";

/** A named remix recipe for one deck: shaping, filter, gain and a tempo echo. Applies to any track. */
export interface DjTemplate {
  id: string;
  name: string;
  builtIn?: boolean;
  effects: AudioEffects;
  /** -100 (low-pass) to 100 (high-pass), 0 off. */
  filter: number;
  /** Gain in percent, 0 to 200. */
  trim: number;
  /** Echo length in beats (0.25, 0.5, 1), or 0 for none. Needs a tapped tempo. */
  echoBeats: number;
}

const fx = (p: Partial<AudioEffects>): AudioEffects => ({ ...FLAT_EFFECTS, ...p, enabled: true });
const t = (id: string, name: string, effects: Partial<AudioEffects>, filter = 0, trim = 100, echoBeats = 0): DjTemplate =>
  ({ id, name, builtIn: true, effects: fx(effects), filter, trim, echoBeats });

export const BUILT_IN: DjTemplate[] = [
  t("nightcore", "Nightcore", { speed: 125, keepPitch: false }),
  t("nightcore-hard", "Nightcore hard", { speed: 135, keepPitch: false, bands: [3, 2, 1, 0, 0, 1, 2, 3, 3, 2], preamp: -3 }),
  t("slowed-reverb", "Slowed + reverb", { speed: 82, keepPitch: false, reverb: "hall", bands: [2, 2, 1, 0, 0, 0, -1, -2, -2, -3] }),
  t("lofi", "Lo-fi", { speed: 92, keepPitch: false, compressor: "medium" }, -45),
  t("8d", "8D", { spatial: "8d" }),
  t("4d-club", "4D club", { spatial: "4d", bands: [5, 4, 2, 0, -1, 0, 1, 2, 3, 3], preamp: -4 }),
  t("radio", "Old radio", { stereoWidth: 0, compressor: "heavy" }, 55),
  t("bass", "Bass boost", { bands: [7, 6, 4, 1, 0, 0, 0, 0, 0, 0], preamp: -5 }),
  t("echo", "Dub echo", { reverb: "room" }, 0, 100, 0.5),
];

const KEY = "lunorx.djTemplates";

export function loadTemplates(): DjTemplate[] {
  try {
    const raw = window.localStorage.getItem(KEY);
    const mine = raw ? (JSON.parse(raw) as DjTemplate[]) : [];
    return [...BUILT_IN, ...mine.filter((m) => m && typeof m.name === "string" && m.effects).map((m) => ({ ...m, builtIn: false, effects: fx(m.effects) }))];
  } catch {
    return [...BUILT_IN];
  }
}

function saveMine(list: DjTemplate[]) {
  try { window.localStorage.setItem(KEY, JSON.stringify(list.filter((x) => !x.builtIn))); } catch { /* storage full or blocked */ }
}

export function addTemplate(all: DjTemplate[], tpl: Omit<DjTemplate, "id" | "builtIn">): DjTemplate[] {
  const next = [...all, { ...tpl, id: `mine-${Date.now()}`, builtIn: false, name: tpl.name.trim().slice(0, 40) || "My template" }];
  saveMine(next);
  return next;
}

export function removeTemplate(all: DjTemplate[], id: string): DjTemplate[] {
  const next = all.filter((x) => x.builtIn || x.id !== id);
  saveMine(next);
  return next;
}

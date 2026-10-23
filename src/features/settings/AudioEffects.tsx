"use client";
import { useEffect, useRef, useState } from "react";
import { Switch } from "../../components/Lyrics";
import { Button } from "../../components/ui";
import { FLAT_EFFECTS, type AudioEffects } from "../../services/tauri/player";
import { FIELD, Inset, NoteRow, SettingRow } from "./rows";

const BANDS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
const PRESETS: { key: string; label: string; fx: Partial<AudioEffects> }[] = [
  { key: "flat", label: "Flat", fx: {} },
  { key: "bass", label: "Bass boost", fx: { bands: [5, 4, 3, 1, 0, 0, 0, 0, 0, 0], preamp: -3 } },
  { key: "vocal", label: "Vocal", fx: { bands: [-2, -2, -1, 1, 3, 3, 2, 1, 0, -1] } },
  { key: "bright", label: "Bright", fx: { bands: [0, 0, 0, 0, 0, 1, 2, 3, 4, 5], preamp: -3 } },
  { key: "late", label: "Late night", fx: { bands: [2, 1, 0, 0, 0, 0, 0, 0, 0, 0], compressor: "heavy" } },
  { key: "head", label: "Headphones", fx: { crossfeed: 35, bands: [1, 1, 0, 0, 0, 0, 0, 1, 1, 0] } },
  { key: "live", label: "Live room", fx: { reverb: "room", stereoWidth: 130 } },
];
const db = (n: number) => `${n > 0 ? "+" : ""}${n} dB`;

/** The equaliser curve through the ten band gains, so a change is visible as well as audible. */
function Curve({ bands }: { bands: number[] }) {
  const pts = bands.map((g, i) => `${(i / 9) * 100},${20 - (g / 12) * 18}`);
  return (
    <svg role="img" aria-label="Equaliser curve" viewBox="0 0 100 40" preserveAspectRatio="none" className="h-16 w-full rounded-[8px] bg-frost-200">
      <line x1="0" x2="100" y1="20" y2="20" className="stroke-white/20" strokeWidth="0.4" />
      <polyline points={pts.join(" ")} fill="none" strokeWidth="1.2" strokeLinejoin="round" className="stroke-lumen-lavender" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Slider({ label, value, min, max, step = 1, onChange, show }: { label: string; value: number; min: number; max: number; step?: number; onChange: (v: number) => void; show: (v: number) => string }) {
  return (
    <SettingRow label={label} htmlFor={`fx-${label}`}>
      <input id={`fx-${label}`} type="range" min={min} max={max} step={step} value={value} aria-valuetext={show(value)} onChange={(e) => onChange(Number(e.target.value))} className="w-[200px]" />
      <span className="w-16 text-right text-[13px] tabular-nums text-secondary-text">{show(value)}</span>
    </SettingRow>
  );
}

/** Settings > Playback > Sound shaping. Edits apply live: changes are sent a moment after the last touch. */
export function AudioEffectsPanel({ value, disabled, onChange }: { value: AudioEffects; disabled: boolean; onChange: (fx: AudioEffects) => void }) {
  const [draft, setDraft] = useState(value);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => setDraft(value), [value]);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const set = (patch: Partial<AudioEffects>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => onChange(next), 150);
  };
  const setBand = (i: number, g: number) => set({ bands: draft.bands.map((b, j) => (j === i ? g : b)) });
  const lock = disabled || !draft.enabled;
  return (
    <Inset id="settings-effects" title="Sound shaping">
      <SettingRow label="Use sound shaping" hint="Off plays the song untouched and keeps your settings.">
        <Switch on={draft.enabled} onChange={(on) => set({ enabled: on })} name="Use sound shaping">{draft.enabled ? "On" : "Off"}</Switch>
      </SettingRow>
      <SettingRow label="Preset" htmlFor="fx-preset">
        <select id="fx-preset" className={FIELD} disabled={lock} value="" onChange={(e) => { const p = PRESETS.find((x) => x.key === e.target.value); if (p) set({ ...FLAT_EFFECTS, enabled: true, ...p.fx }); }}>
          <option value="">Choose a preset…</option>{PRESETS.map((p) => <option key={p.key} value={p.key}>{p.label}</option>)}
        </select>
      </SettingRow>
      <div className="px-4 pt-1 pb-3">
        <Curve bands={draft.bands} />
        <div className="mt-2 grid grid-cols-10 gap-1">
          {draft.bands.map((g, i) => (
            <label key={BANDS[i]} className="flex flex-col items-center gap-1 text-[11px] text-secondary-text">
              <input type="range" min={-12} max={12} step={1} value={g} disabled={lock} aria-label={`${BANDS[i]} Hz`} aria-valuetext={db(g)} onChange={(e) => setBand(i, Number(e.target.value))} className="h-28 w-full [writing-mode:vertical-lr] [direction:rtl]" />
              <span className="tabular-nums">{BANDS[i]}</span>
              <span className="tabular-nums">{g}</span>
            </label>
          ))}
        </div>
      </div>
      <Slider label="Preamp" value={draft.preamp} min={-12} max={12} onChange={(v) => set({ preamp: v })} show={db} />
      <Slider label="Bass" value={draft.bass} min={-12} max={12} onChange={(v) => set({ bass: v })} show={db} />
      <Slider label="Treble" value={draft.treble} min={-12} max={12} onChange={(v) => set({ treble: v })} show={db} />
      <Slider label="Stereo width" value={draft.stereoWidth} min={0} max={200} step={5} onChange={(v) => set({ stereoWidth: v })} show={(v) => (v === 0 ? "Mono" : `${v}%`)} />
      <Slider label="Headphone crossfeed" value={draft.crossfeed} min={0} max={100} step={5} onChange={(v) => set({ crossfeed: v })} show={(v) => (v === 0 ? "Off" : `${v}%`)} />
      <Slider label="Speed" value={draft.speed} min={50} max={200} step={5} onChange={(v) => set({ speed: v })} show={(v) => `${(v / 100).toFixed(2)}×`} />
      <SettingRow label="Room" htmlFor="fx-reverb">
        <select id="fx-reverb" className={FIELD} disabled={lock} value={draft.reverb} onChange={(e) => set({ reverb: e.target.value as AudioEffects["reverb"] })}>
          <option value="off">Off</option><option value="room">Small room</option><option value="hall">Hall</option>
        </select>
      </SettingRow>
      <SettingRow label="Compressor" htmlFor="fx-comp" hint="Narrows the gap between quiet and loud parts.">
        <select id="fx-comp" className={FIELD} disabled={lock} value={draft.compressor} onChange={(e) => set({ compressor: e.target.value as AudioEffects["compressor"] })}>
          <option value="off">Off</option><option value="light">Light</option><option value="medium">Medium</option><option value="heavy">Heavy</option>
        </select>
      </SettingRow>
      <SettingRow label="Reset">
        <Button disabled={disabled} onClick={() => set({ ...FLAT_EFFECTS })}>Reset sound shaping</Button>
      </SettingRow>
      <NoteRow tone="off">These filters run inside the player, so they work on every song. Boosts are followed by a limiter to avoid clipping. Speed keeps the pitch. Saved on this device.</NoteRow>
    </Inset>
  );
}

"use client";

import { useRef, useState } from "react";
import { Button } from "../../components/ui";
import { FxText } from "../../components/FxText";
import { BUILTIN_FONTS, EFFECTS, ZONES, importFont, removeFont, setZoneStyle, useCustomFonts, useZoneStyle, type Effect, type Zone } from "../../lib/textStyle";
import { FIELD, Inset, SettingRow } from "./rows";

function ZoneRows({ zone, label, hint }: { zone: Zone; label: string; hint: string }) {
  const style = useZoneStyle(zone);
  const custom = useCustomFonts();
  const [replay, setReplay] = useState(0);
  return (
    <>
      <SettingRow label={`${label} font`} htmlFor={`font-${zone}`} hint={hint}>
        <select id={`font-${zone}`} className={FIELD} value={style.font} onChange={(e) => setZoneStyle(zone, { font: e.target.value })}>
          {BUILTIN_FONTS.map((f) => <option key={f.id} value={f.id}>{f.label}</option>)}
          {custom.map((f) => <option key={f.slug} value={`custom:${f.slug}`}>{f.label} (imported)</option>)}
        </select>
      </SettingRow>
      <SettingRow label={`${label} motion`} htmlFor={`fx-${zone}`}>
        <select id={`fx-${zone}`} className={FIELD} value={style.effect} onChange={(e) => { setZoneStyle(zone, { effect: e.target.value as Effect }); setReplay((n) => n + 1); }}>
          {(["Basic", "Entrance", "Loop"] as const).map((g) => (
            <optgroup key={g} label={g === "Loop" ? "Looping" : g}>
              {EFFECTS.filter((f) => f.group === g).map((f) => <option key={f.id} value={f.id}>{f.label}</option>)}
            </optgroup>))}
        </select>
        <Button onClick={() => setReplay((n) => n + 1)}>Replay</Button>
      </SettingRow>
      <SettingRow label="Preview">
        <span key={replay} className={zone === "title" ? "ln-np-title text-[20px] font-semibold" : "ln-lyr-region text-[20px] font-bold"}>
          <span className={zone === "lyrics" ? "ln-lyr-line" : undefined}><FxText text="Tonight we light the sky ค่ำคืนนี้ฉันนับดาว" effect={style.effect} /></span>
        </span>
      </SettingRow>
    </>
  );
}

/** Settings > General > Text style: a font and a motion effect for each text zone, plus imported fonts kept on this device. */
export function TextStyleSettings() {
  const custom = useCustomFonts();
  const input = useRef<HTMLInputElement>(null);
  const [error, setError] = useState("");
  const pick = async (file: File | undefined) => {
    if (!file) return;
    setError("");
    try { await importFont(file); } catch (e) { setError(e instanceof Error ? e.message : "Could not import that font."); }
    if (input.current) input.current.value = "";
  };
  return (
    <>
      {ZONES.map((z) => (
        <Inset key={z.id} id={`settings-text-${z.id}`} title={`Text style · ${z.label}`}>
          <ZoneRows zone={z.id} label={z.label} hint={z.hint} />
        </Inset>
      ))}
      <Inset id="settings-fonts" title="Imported fonts">
        <SettingRow label="Add a font file" hint="TTF, OTF, WOFF or WOFF2, up to 8 MB. Stored on this device only. Check the font's license allows your use.">
          <input ref={input} type="file" accept=".ttf,.otf,.woff,.woff2" className="sr-only" id="font-file" onChange={(e) => void pick(e.target.files?.[0])} />
          <Button onClick={() => input.current?.click()}>Choose file…</Button>
        </SettingRow>
        {error && <p role="alert" className="px-4 py-2 text-[13px] text-critical-coral">{error}</p>}
        {custom.map((f) => (
          <SettingRow key={f.slug} label={f.label}>
            <Button onClick={() => void removeFont(f.slug)}>Remove</Button>
          </SettingRow>
        ))}
      </Inset>
    </>
  );
}

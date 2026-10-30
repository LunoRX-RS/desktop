"use client";

import { Inset, SettingRow } from "../rows";
import { TextStyleSettings } from "../TextStyleSettings";
import { FIELD } from "../rows";
import { useState } from "react";
import { Button } from "../../../components/ui";
import { ACCENTS, MAX_CSS, THEMES, clearBackdropImage, cssProblem, hasBackdropImage, setBackdropImage, setTheme, useTheme, type AccentId, type BackdropId, type ThemeId } from "../../../lib/theme";

export function Appearance() {
  const t = useTheme();
  const [css, setCss] = useState(t.customCss);
  const [note, setNote] = useState<string | null>(null);
  return (
    <>
      <Inset id="settings-appearance" title="Theme">
        <SettingRow label="Theme" htmlFor="theme-pick" hint="Applies at once and is kept on this device.">
          <select id="theme-pick" className={FIELD} value={t.theme} onChange={(e) => setTheme({ theme: e.target.value as ThemeId })}>
            {THEMES.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}
          </select>
        </SettingRow>
        <SettingRow label="Accent" hint="The one colour that marks what is selected or playing.">
          <div role="radiogroup" aria-label="Accent colour" className="flex gap-2">
            {ACCENTS.map((a) => (
              <button key={a.id} type="button" role="radio" aria-checked={t.accent === a.id} aria-label={a.label} title={a.label} onClick={() => setTheme({ accent: a.id as AccentId })} data-swatch={a.id} className={`lx-swatch size-6 rounded-full ${t.accent === a.id ? "ring-2 ring-primary-text ring-offset-2 ring-offset-night-canvas" : ""}`} />
            ))}
          </div>
        </SettingRow>
        <SettingRow label="Background" htmlFor="theme-backdrop" hint="What shows through the glass behind everything.">
          <select id="theme-backdrop" className={FIELD} value={t.backdrop === "image" && !hasBackdropImage() ? "artwork" : t.backdrop} onChange={(e) => { const v = e.target.value as BackdropId; if (v === "image" && !hasBackdropImage()) { document.getElementById("theme-image")?.click(); return; } setTheme({ backdrop: v }); }}>
            <option value="artwork">Playing artwork</option>
            <option value="image">My image</option>
            <option value="plain">Plain</option>
          </select>
        </SettingRow>
        <SettingRow label="Background image" hint="Kept on this device only, scaled down to save space.">
          <div className="flex items-center gap-2">
            <input id="theme-image" type="file" accept="image/*" className="sr-only" onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ""; if (f) void setBackdropImage(f).then(() => setNote(null), () => setNote("That image could not be used. Try a smaller JPEG or PNG.")); }} />
            <Button onClick={() => document.getElementById("theme-image")?.click()}>Choose image</Button>
            {hasBackdropImage() && <Button variant="ghost" onClick={clearBackdropImage}>Remove</Button>}
          </div>
        </SettingRow>
        <SettingRow label="Dim" htmlFor="theme-dim" hint="Darkens the background (lightens it in Light) so text stays readable.">
          <input id="theme-dim" type="range" min={0} max={90} value={t.dim} aria-valuetext={`${t.dim} percent`} onChange={(e) => setTheme({ dim: Number(e.target.value) })} className="w-[200px]" />
          <span className="w-12 text-right text-[13px] tabular-nums text-secondary-text">{t.dim}%</span>
        </SettingRow>
        {t.backdrop === "image" && (
          <SettingRow label="Image blur" htmlFor="theme-blur">
            <input id="theme-blur" type="range" min={0} max={60} value={t.blur} aria-valuetext={`${t.blur} pixels`} onChange={(e) => setTheme({ blur: Number(e.target.value) })} className="w-[200px]" />
            <span className="w-12 text-right text-[13px] tabular-nums text-secondary-text">{t.blur}px</span>
          </SettingRow>
        )}
        {note && <p role="alert" className="px-4 pb-2 text-[12px] text-caution-amber">{note}</p>}
        <SettingRow label="Reduced motion" hint="Follows your operating system setting.">System</SettingRow>
        <SettingRow label="Reduced transparency" hint="Follows your operating system setting. The sidebar and player become solid.">System</SettingRow>
      </Inset>
      <Inset id="settings-custom-css" title="Custom CSS">
        <div className="flex flex-col gap-2 px-4 py-3">
          <p className="text-[12px] text-secondary-text">Your own CSS, applied after the app's styles. Colour tokens such as <code>--color-lumen-lavender</code> or <code>--color-night-canvas</code> can be set on <code>:root</code>. Kept on this device.</p>
          <textarea aria-label="Custom CSS" spellCheck={false} maxLength={MAX_CSS} value={css} onChange={(e) => setCss(e.target.value)} rows={8} placeholder={":root {\n  --color-lumen-lavender: #ff8fb1;\n}"} className="min-h-40 w-full rounded-[8px] bg-frost-200 p-3 font-mono text-[12px] leading-5 text-primary-text placeholder:text-tertiary-text" />
          <div className="flex items-center gap-2">
            <Button disabled={css === t.customCss} onClick={() => { setTheme({ customCss: css }); setNote(cssProblem); }}>Apply</Button>
            <Button variant="ghost" disabled={!t.customCss && !css} onClick={() => { setCss(""); setTheme({ customCss: "" }); }}>Clear</Button>
            <span className="text-[11px] text-tertiary-text">{css.length.toLocaleString()} / {MAX_CSS.toLocaleString()}</span>
          </div>
        </div>
      </Inset>
      <TextStyleSettings />
    </>
  );
}

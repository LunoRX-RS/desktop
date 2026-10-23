"use client";
import { useEffect, useState } from "react";
import { Check, Copy, Plus, Trash2 } from "lucide-react";
import { Button } from "../../components/ui";
import { Tip } from "../../components/Tip";
import { discordPreview, PRESETS, type CardDesign, type CardPreview } from "../../services/tauri/discord";
import { FIELD, Inset, NoteRow, SettingRow } from "./rows";

// The fill-ins a text line can use. `sample` is what they would show for a song playing right now.
const FILLINS: { token: string; name: string; what: string; sample: string; where?: string }[] = [
	{ token: "{title}", name: "Song title", what: "The name of the song that is playing. “Clean up titles” can trim hashtags and emoji first.", sample: "Fame is a Gun" },
	{ token: "{artist}", name: "First artist", what: "Only the first artist of the song.", sample: "Addison Rae" },
	{ token: "{artists}", name: "All artists", what: "Every artist of the song, separated by commas.", sample: "Billie Eilish, Justin Bieber" },
	{ token: "{status}", name: "Play state", what: "The word “playing”, or “paused” while the song is paused.", sample: "playing" },
	{ token: "{app}", name: "App name", what: "The name of this app.", sample: "LunoRX" },
	{ token: "{track_url}", name: "Song link", what: "The YouTube link of the playing video. Only works as a button link, not in text lines.", sample: "https://www.youtube.com/watch?v=…", where: "Button links only" },
];

async function copyText(text: string): Promise<boolean> {
	try {
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		// Some webviews block the async clipboard; a hidden field and the old command still work from a click.
		const box = document.createElement("textarea");
		box.value = text;
		box.setAttribute("readonly", "");
		box.style.position = "fixed";
		box.style.opacity = "0";
		document.body.appendChild(box);
		box.select();
		const ok = document.execCommand("copy");
		box.remove();
		return ok;
	}
}

/** One fill-in as a chip. Hover or focus explains it; a click copies it so it can be pasted into any field. */
function FillIn({ token, name, what, sample, where }: (typeof FILLINS)[number]) {
	const [copied, setCopied] = useState(false);
	useEffect(() => {
		if (!copied) return;
		const t = window.setTimeout(() => setCopied(false), 1600);
		return () => window.clearTimeout(t);
	}, [copied]);
	return (
		<Tip
			content={
				<div className="flex flex-col gap-2 text-[12px] leading-[17px]">
					<div className="flex items-center justify-between gap-3">
						<span className="font-semibold text-primary-text">{name}</span>
						<code className="rounded-[6px] bg-frost-300 px-1.5 py-0.5 font-mono text-[11px] text-primary-text">{token}</code>
					</div>
					<p className="text-secondary-text">{what}</p>
					<p className="rounded-[8px] bg-frost-200 px-2 py-1.5 text-secondary-text">
						<span className="text-tertiary-text">Shows as </span>
						<span className="text-primary-text">{sample}</span>
					</p>
					{where && <p className="text-caution-amber">{where}</p>}
					<p className="flex items-center gap-1.5 border-t border-hairline pt-2 text-[11px] font-medium text-primary-text">
						{copied ? <Check size={12} aria-hidden="true" /> : <Copy size={12} aria-hidden="true" />}
						{copied ? "Copied to the clipboard" : "Click to copy"}
					</p>
				</div>
			}
		>
			<button
				type="button"
				onClick={() => void copyText(token).then((ok) => setCopied(ok))}
				aria-label={`${name}, ${token}. ${what} Press to copy.`}
				className="inline-flex items-center gap-1.5 rounded-[8px] bg-frost-200 px-2.5 py-1 font-mono text-[12px] text-primary-text transition-colors hover:bg-frost-300 focus-visible:bg-frost-300"
			>
				{copied ? <Check size={12} aria-hidden="true" /> : null}
				{token}
			</button>
		</Tip>
	);
}

function FillIns() {
	return (
		<SettingRow label="Fill-ins" hint="Type these into any text line and Discord shows the real value. Hover one to see what it does; click to copy it." stack>
			<div className="flex flex-wrap gap-2">
				{FILLINS.map((f) => (
					<FillIn key={f.token} {...f} />
				))}
			</div>
		</SettingRow>
	);
}
const FIELD_NAMES: Record<string, string> = { details: "First line", state: "Second line", "paused text": "Paused line", "image text": "Image text", "badge text": "Badge text", "paused badge text": "Paused badge text", "catalog name": "Catalog name (a-z, 0-9, _ or -; unique)", "catalog image": "Catalog image (https link or asset key)", "large image": "Big picture", "small image": "Badge picture", button: "Button (label up to 32 characters, https link)", buttons: "Buttons (at most 2)", catalog: "Catalog (at most 12)", clock: "Clock", "status line": "Status line" };
// The app's CSP only lets these hosts load pictures, so any other image is named instead of drawn. Discord still shows it.
const PREVIEW_HOSTS = [/^i\.ytimg\.com$/, /^yt3\.ggpht\.com$/, /\.googleusercontent\.com$/];
const thumb = (v: string) => {
  try { const u = new URL(v); return u.protocol === "https:" && PREVIEW_HOSTS.some((r) => r.test(u.hostname)) ? v : null; } catch { return null; }
};

/** What the card will look like, drawn from the payload core would send. The picture is only shown when it is an https link. */
function Preview({ p }: { p: CardPreview }) {
  const big = thumb(p.assets.large_image);
  const small = p.assets.small_image ? thumb(p.assets.small_image) : null;
  return (
    <div aria-label="Card preview" className="flex gap-3 rounded-[10px] bg-frost-200 p-3">
      <div className="relative size-[72px] shrink-0 overflow-hidden rounded-[8px] bg-frost-300">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        {big ? <img src={big} alt="" referrerPolicy="no-referrer" className="size-full object-cover" /> : <span className="grid size-full place-items-center px-1 text-center text-[10px] text-secondary-text">{p.assets.large_image}</span>}
        {/* eslint-disable-next-line @next/next/no-img-element */}
        {small && <img src={small} alt="" referrerPolicy="no-referrer" className="absolute right-0.5 bottom-0.5 size-6 rounded-full" />}
      </div>
      <div className="min-w-0 text-[13px] leading-[18px]">
        <p className="text-[11px] font-semibold tracking-wide text-secondary-text uppercase">Listening to LunoRX</p>
        {p.details && <p className="truncate font-semibold">{p.details}</p>}
        {p.state && <p className="truncate text-secondary-text">{p.state}</p>}
        {p.assets.large_text && <p className="truncate text-secondary-text">{p.assets.large_text}</p>}
        {p.timestamps?.end && <p className="text-[11px] text-secondary-text">progress bar</p>}
        {p.timestamps && !p.timestamps.end && <p className="text-[11px] text-secondary-text">elapsed timer</p>}
        {p.buttons && <div className="mt-1.5 flex flex-wrap gap-1.5">{p.buttons.map((b) => <span key={b.label} className="rounded-md bg-frost-300 px-2 py-0.5 text-[12px]">{b.label}</span>)}</div>}
      </div>
    </div>
  );
}

/** Settings > Discord > Discord card. Edits a draft; Save sends it to core, which checks every field. */
export function DiscordCard({ design, onSave }: { design: CardDesign; onSave: (d: CardDesign) => Promise<string | null> }) {
  const [draft, setDraft] = useState(design);
  const [preview, setPreview] = useState<CardPreview | null>(null);
  const [problem, setProblem] = useState("");
  const [saved, setSaved] = useState(false);
  const set = <K extends keyof CardDesign>(k: K, v: CardDesign[K]) => { setDraft((d) => ({ ...d, [k]: v })); setSaved(false); };
  useEffect(() => { setDraft(design); }, [design]);
  useEffect(() => {
    let live = true;
    const t = window.setTimeout(() => { discordPreview(draft).then((p) => { if (live) { setPreview(p); setProblem(""); } }, (e) => { if (live) setProblem(FIELD_NAMES[String(e)] ?? String(e)); }); }, 250);
    return () => { live = false; window.clearTimeout(t); };
  }, [draft]);
  const images = [["cover", "Song cover"], ["logo", "LunoRX logo"], ...draft.catalog.filter((e) => e.name).map((e) => [`catalog:${e.name}`, `Catalog: ${e.name}`])];
  const text = (k: "details" | "state" | "pausedState" | "largeText" | "smallText" | "pausedSmallText", label: string, hint?: string) => (
    <SettingRow label={label} htmlFor={`dc-${k}`} hint={hint}>
      <input id={`dc-${k}`} value={draft[k]} maxLength={128} spellCheck={false} onChange={(e) => set(k, e.target.value)} className={`${FIELD} w-[260px]`} />
    </SettingRow>
  );
  const pick = (k: "largeImage" | "smallImage", label: string, none: boolean) => (
    <SettingRow label={label} htmlFor={`dc-${k}`}>
      <select id={`dc-${k}`} className={FIELD} value={draft[k]} onChange={(e) => set(k, e.target.value)}>
        {none && <option value="none">None</option>}
        {images.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
    </SettingRow>
  );
  return (
    <Inset id="settings-discord-card" title="Discord card design">
      <div className="flex flex-col gap-3 px-4 py-3">{preview ? <Preview p={preview} /> : <p className="text-[13px] text-secondary-text">Preview unavailable.</p>}</div>
      <SettingRow label="Start from" htmlFor="dc-preset" hint="Keeps your catalog and custom buttons.">
        <select id="dc-preset" className={FIELD} value="" onChange={(e) => { const p = PRESETS.find((x) => x.key === e.target.value); if (p) { setDraft((d) => ({ ...d, ...p.design, buttons: d.buttons, catalog: d.catalog }) as CardDesign); setSaved(false); } }}>
          <option value="">Choose a preset…</option>{PRESETS.map((p) => <option key={p.key} value={p.key}>{p.label}</option>)}
        </select>
      </SettingRow>
      <FillIns />
      {text("details", "First line", "Leave empty to hide the line.")}
      {text("state", "Second line")}
      {text("pausedState", "Second line while paused")}
      {text("largeText", "Text under the lines")}
      {pick("largeImage", "Big picture", false)}
      {pick("smallImage", "Badge picture", true)}
      {text("smallText", "Badge text")}
      {text("pausedSmallText", "Badge text while paused")}
      <SettingRow label="Clock" htmlFor="dc-clock">
        <select id="dc-clock" className={FIELD} value={draft.timestamps} onChange={(e) => set("timestamps", e.target.value as CardDesign["timestamps"])}>
          <option value="bar">Progress bar</option><option value="elapsed">Elapsed timer</option><option value="off">Hidden</option>
        </select>
      </SettingRow>
      <SettingRow label="Name in the member list" htmlFor="dc-status">
        <select id="dc-status" className={FIELD} value={draft.statusDisplay} onChange={(e) => set("statusDisplay", e.target.value as CardDesign["statusDisplay"])}>
          <option value="artist">Artist</option><option value="title">Song title</option><option value="app">LunoRX</option>
        </select>
      </SettingRow>
      <SettingRow label="Clean up titles" hint="Removes #hashtags and trailing emoji from YouTube titles.">
        <input type="checkbox" aria-label="Clean up titles" checked={draft.tidyTitle} onChange={(e) => set("tidyTitle", e.target.checked)} />
      </SettingRow>
      <SettingRow label="Buttons" hint="Discord shows at most two. Yours come first. Use {track_url} as the link for the playing video.">
        <label className="flex items-center gap-2 text-[13px]"><input type="checkbox" checked={draft.listenButton} onChange={(e) => set("listenButton", e.target.checked)} />Listen on YouTube</label>
        <label className="flex items-center gap-2 text-[13px]"><input type="checkbox" checked={draft.appButton} onChange={(e) => set("appButton", e.target.checked)} />Get LunoRX</label>
      </SettingRow>
      {draft.buttons.map((b, i) => (
        <SettingRow key={i} label={`Your button ${i + 1}`}>
          <input aria-label={`Button ${i + 1} label`} value={b.label} maxLength={32} placeholder="Label" onChange={(e) => set("buttons", draft.buttons.map((x, j) => (j === i ? { ...x, label: e.target.value } : x)))} className={`${FIELD} w-[120px]`} />
          <input aria-label={`Button ${i + 1} link`} value={b.url} placeholder="https://… or {track_url}" onChange={(e) => set("buttons", draft.buttons.map((x, j) => (j === i ? { ...x, url: e.target.value } : x)))} className={`${FIELD} w-[200px]`} />
          <Button variant="ghost" aria-label={`Remove button ${i + 1}`} onClick={() => set("buttons", draft.buttons.filter((_, j) => j !== i))}><Trash2 size={14} aria-hidden="true" /></Button>
        </SettingRow>
      ))}
      {draft.buttons.length < 2 && <SettingRow label=""><Button onClick={() => set("buttons", [...draft.buttons, { label: "", url: "https://" }])}><Plus size={14} aria-hidden="true" />Add a button</Button></SettingRow>}
      <NoteRow tone="off">My images. Add a picture by https link, or by the key of an Art Asset you uploaded to your own Application ID (Developer Portal, Rich Presence, Art Assets). Then pick it above.</NoteRow>
      {draft.catalog.map((e, i) => (
        <SettingRow key={i} label={`Image ${i + 1}`}>
          <input aria-label={`Image ${i + 1} name`} value={e.name} maxLength={24} placeholder="name" spellCheck={false} onChange={(ev) => set("catalog", draft.catalog.map((x, j) => (j === i ? { ...x, name: ev.target.value } : x)))} className={`${FIELD} w-[110px]`} />
          <input aria-label={`Image ${i + 1} source`} value={e.value} placeholder="https://… or asset_key" spellCheck={false} onChange={(ev) => set("catalog", draft.catalog.map((x, j) => (j === i ? { ...x, value: ev.target.value } : x)))} className={`${FIELD} w-[210px]`} />
          <Button variant="ghost" aria-label={`Remove image ${i + 1}`} onClick={() => setDraft((d) => ({ ...d, catalog: d.catalog.filter((_, j) => j !== i), largeImage: d.largeImage === `catalog:${e.name}` ? "cover" : d.largeImage, smallImage: d.smallImage === `catalog:${e.name}` ? "logo" : d.smallImage }))}><Trash2 size={14} aria-hidden="true" /></Button>
        </SettingRow>
      ))}
      {draft.catalog.length < 12 && <SettingRow label=""><Button onClick={() => set("catalog", [...draft.catalog, { name: "", value: "" }])}><Plus size={14} aria-hidden="true" />Add an image</Button></SettingRow>}
      {problem && <NoteRow tone="bad" role="alert">Check this field: {problem}</NoteRow>}
      <SettingRow label="">
        <Button variant="primary" disabled={!!problem} onClick={async () => { const err = await onSave(draft); setProblem(err ? (FIELD_NAMES[err] ?? err) : ""); setSaved(!err); }}>Save design</Button>
        <Button onClick={() => { setDraft(PRESETS[0].design as CardDesign); setSaved(false); }}>Reset to default</Button>
        {saved && <span role="status" className="text-[13px] text-secondary-text">Saved.</span>}
      </SettingRow>
    </Inset>
  );
}

"use client";

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Pause, Play, Repeat, Square } from "lucide-react";
import { Button, IconButton, Screen, StatusNote } from "../../components/ui";
import { TrackCover } from "../../components/TrackCover";
import { togglePlayback, usePlayback } from "../../lib/playback";
import { loadSet, saveSet, takeHandoff, type TrackSet } from "../../lib/djSets";
import { parseYouTubeLink } from "../../lib/ytLink";
import { TrackPicker, type Pick } from "./TrackPicker";
import { useDeck, useDjField, useDj, type Tempo } from "./djStore";
import { Knob, VFader } from "./Controls";
import { Waveform } from "./Waveform";
import { DeckLyrics } from "./DeckLyrics";
import { addTemplate, loadTemplates, removeTemplate, type DjTemplate } from "../../lib/djTemplates";
import {
  djControl, djLoad, djPause, djPlay, djSeek, djSetEffects, djSetLoop, djSetMix, djStatus, djStop, subscribeDeck,
  type DeckId, type DeckReport,
} from "../../services/tauri/dj";
import { FLAT_EFFECTS, playerEffectPresets, type AudioEffects, type EffectPreset } from "../../services/tauri/player";

const fmt = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const EQ = [
  { key: "low", label: "Low", band: [0, 1, 2, 3], at: 1 },
  { key: "mid", label: "Mid", band: [4, 5, 6], at: 5 },
  { key: "high", label: "High", band: [7, 8, 9], at: 8 },
];

const QUICK: { label: string; fx: Partial<AudioEffects> }[] = [
  { label: "Nightcore", fx: { speed: 125, keepPitch: false } },
  { label: "Slowed", fx: { speed: 80, keepPitch: false } },
  { label: "8D", fx: { spatial: "8d" } },
  { label: "4D", fx: { spatial: "4d" } },
  { label: "Flat", fx: {} },
];

const BEATS = [1, 2, 4, 8, 16];
/** The wall-clock beats per minute a track plays at, given its own beats per minute and the speed it is set to. */
const heard = (t: Tempo) => (t.bpm ? t.bpm * (t.speed / 100) : null);

export interface DeckKeys { playPause: () => void; cue: (i: number) => void; brake: () => void }

function Deck({ id, report, onReport, presets, other, onTempo, keys }: { id: DeckId; report: DeckReport | null; onReport: (r: DeckReport | null) => void; presets: EffectPreset[]; other: Tempo; onTempo: (t: Tempo) => void; keys: React.MutableRefObject<DeckKeys | null> }) {
  const nowPlaying = usePlayback((s) => s.track);
  const upNext = usePlayback((s) => s.autoplay.queue);
  const [link, setLink] = useState("");
  const [videoId, setVideoId] = useDeck(id, "videoId");
  const [title, setTitle] = useDeck(id, "title");
  const [bpm, setBpm] = useDeck(id, "bpm");
  const taps = useRef<number[]>([]);
  const [filter, setFilter] = useDeck(id, "filter");
  const [trim, setTrim] = useDeck(id, "trim");
  const [fader, setFader] = useDeck(id, "fader");
  const [echo, setEcho] = useDeck(id, "echo");
  const start = useRef<number | null>(null);
  const [picking, setPicking] = useState(false);
  const [lyricMarks, setLyricMarks] = useState<number[]>([]);
  const [templates, setTemplates] = useState<DjTemplate[]>(() => loadTemplates());
  const [templateId, setTemplateId] = useDeck(id, "templateId");
  const [naming, setNaming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fx, setFx] = useDeck(id, "fx");
  const [cues, setCues] = useDeck(id, "cues");
  const [loop, setLoop] = useDeck(id, "loop");
  const timer = useRef<number | undefined>(undefined);
  const label = id.toUpperCase();

  const applyFx = useCallback((next: AudioEffects) => {
    setFx(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => { void djSetEffects(id, next).catch(() => setError("The deck could not take that sound setting.")); }, 120);
  }, [id]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  // What this deck remembers about the track follows it: cues, loop and shaping are saved as they change.
  useEffect(() => {
    if (!videoId) return;
    const set: TrackSet = { cues, loop: loop.startMs !== null && loop.endMs !== null ? { startMs: loop.startMs, endMs: loop.endMs } : null, effects: fx };
    saveSet(videoId, set);
  }, [videoId, cues, loop.startMs, loop.endMs, fx]);

  const load = async (vid: string, name = "", fromMs: number | null = null) => {
    setError(null);
    try {
      await djLoad(id, vid);
      setVideoId(vid);
      setTitle(name);
      setBpm(null);
      taps.current = [];
      start.current = fromMs;
      setFilter(0); setTrim(100); setFader(100); setEcho(0);
      onReport(null);
      const saved = loadSet(vid);
      setCues(saved?.cues ?? [null, null, null, null]);
      setLoop({ startMs: saved?.loop?.startMs ?? null, endMs: saved?.loop?.endMs ?? null, on: false });
      const next = saved?.effects ?? FLAT_EFFECTS;
      setFx(next);
      if (saved?.effects) void djSetEffects(id, saved.effects).catch(() => {});
    } catch {
      setError("The deck could not open that track.");
    }
  };
  const fromLink = () => {
    const parsed = parseYouTubeLink(link);
    if (!parsed) { setError("That is not a YouTube link or video id."); return; }
    void load(parsed.id);
  };

  useEffect(() => { onTempo({ bpm, speed: fx.speed }); }, [bpm, fx.speed, onTempo]);
  // Opened from the player bar: deck A takes the playing track and carries on from the same moment.
  useEffect(() => {
    if (id !== "a") return;
    const h = takeHandoff();
    if (h) void load(h.videoId, h.title, h.positionMs);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (report === null || start.current === null) return;
    const at = start.current;
    start.current = null;
    const main = usePlayback.getState().player.state;
    if (main === "playing" || main === "buffering") togglePlayback();
    void djSeek(id, at).then(() => djPlay(id)).catch(() => {});
  }, [report, id]);
  const ctl = (c: Parameters<typeof djControl>[1]) => void djControl(id, c).catch(() => setError("The deck did not respond."));

  const state = report?.state;
  const playing = state === "playing";
  const pos = report?.positionMs ?? 0;
  const dur = report?.durationMs ?? 0;
  const ready = videoId !== null && report !== null;

  const setLoopPoint = (which: "startMs" | "endMs") => {
    const next = { ...loop, [which]: pos, on: false };
    setLoop(next);
    void djSetLoop(id, null).catch(() => {});
  };
  const toggleLoop = () => {
    if (loop.on) { setLoop({ ...loop, on: false }); void djSetLoop(id, null).catch(() => {}); return; }
    if (loop.startMs === null || loop.endMs === null || loop.endMs <= loop.startMs) { setError("Set the loop start and end first, end after start."); return; }
    setError(null);
    void djSetLoop(id, { startMs: loop.startMs, endMs: loop.endMs }).then(() => setLoop({ ...loop, on: true }), () => setError("That loop is too short or too long."));
  };

  const tap = () => {
    const now = performance.now();
    const t = taps.current;
    if (t.length && now - t[t.length - 1] > 2000) t.length = 0;
    t.push(now);
    if (t.length > 8) t.shift();
    if (t.length >= 3) {
      const gaps = t.slice(1).map((x, i) => x - t[i]);
      const wall = 60000 / (gaps.reduce((a, b) => a + b, 0) / gaps.length);
      // Taps are heard at the current speed; the track's own tempo is what it would be at 100 percent.
      setBpm(Math.min(300, Math.max(40, wall / (fx.speed / 100))));
    }
  };
  const beatLoop = (n: number) => {
    if (!bpm) return;
    const startMs = Math.round(pos);
    const endMs = Math.round(pos + (n * 60000) / bpm);
    setLoop({ startMs, endMs, on: false });
    void djSetLoop(id, { startMs, endMs }).then(() => setLoop({ startMs, endMs, on: true }), () => setError("That loop is too short or too long."));
  };
  const sync = () => {
    const target = heard(other);
    if (!bpm || target === null) return;
    const speed = Math.round((target / bpm) * 100);
    if (speed < 50 || speed > 200) { setError("These tempos are too far apart to match."); return; }
    setError(null);
    applyFx({ ...fx, speed, keepPitch: true });
  };
  const eqValue = (at: number) => fx.bands[at];
  const setEq = (band: number[], gain: number) => applyFx({ ...fx, bands: fx.bands.map((b, i) => (band.includes(i) ? gain : b)) });

  const echoFor = (beats: number) => {
    if (!bpm) return;
    const ms = Math.round((beats * 60000) / (bpm * (fx.speed / 100)));
    setEcho(beats);
    ctl({ kind: "echo", delayMs: Math.min(4000, ms), feedback: 45 });
  };
  const applyTemplate = (tpl: DjTemplate) => {
    setTemplateId(tpl.id);
    applyFx(tpl.effects);
    setFilter(tpl.filter); ctl({ kind: "filter", value: tpl.filter });
    setTrim(tpl.trim); ctl({ kind: "trim", value: tpl.trim });
    if (tpl.echoBeats && bpm) echoFor(tpl.echoBeats);
    else { setEcho(0); ctl({ kind: "echo", delayMs: 0, feedback: 0 }); }
    if (tpl.echoBeats && !bpm) setError("Tap the tempo to hear this template's echo.");
  };
  const saveTemplate = () => {
    if (naming === null) return;
    setTemplates((all) => addTemplate(all, { name: naming, effects: fx, filter, trim, echoBeats: echo }));
    setNaming(null);
  };
  keys.current = {
    playPause: () => { if (ready) void (playing ? djPause(id) : djPlay(id)).catch(() => {}); },
    cue: (i) => { if (!ready) return; const c = cues[i]; if (c === null) setCues(cues.map((x, j) => (j === i ? pos : x))); else void djSeek(id, c).catch(() => {}); },
    brake: () => ctl({ kind: "brake" }),
  };

  const quickNow = QUICK.find((q) => JSON.stringify({ ...FLAT_EFFECTS, ...q.fx, bands: fx.bands }) === JSON.stringify({ ...fx, bands: fx.bands }))?.label ?? null;
  const remaining = Math.max(0, dur - pos);
  const tempoText = bpm ? `${(bpm * (fx.speed / 100)).toFixed(1)} BPM` : "Tap for BPM";
  const showPicker = !videoId || picking;

  return (
    <section aria-label={`Deck ${label}`} className="flex min-w-0 flex-1 flex-col gap-5">
      <div className="flex items-center gap-3">
        <span aria-hidden className="w-5 font-display text-[22px] font-bold text-tertiary-text">{label}</span>
        {videoId
          ? <TrackCover videoId={videoId} title={title} className="size-14 shrink-0 rounded-[6px] object-cover" />
          : <div aria-hidden className="size-14 shrink-0 rounded-[6px] bg-frost-200" />}
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold" title={title}>{videoId ? title || "YouTube video" : "Empty deck"}</p>
          <p className="text-[11px] tabular-nums text-secondary-text">{videoId ? `${tempoText} · ${fx.speed}%${fx.keepPitch ? "" : " · pitch follows"}` : "Choose a track below"}</p>
        </div>
        {videoId && <Button variant="ghost" onClick={() => setPicking((v) => !v)}>{picking ? "Done" : "Change track"}</Button>}
        {videoId && <IconButton label={`Eject deck ${label}`} onClick={() => { void djStop(id).catch(() => {}); setVideoId(null); setPicking(false); onReport(null); }}><Square size={15} /></IconButton>}
      </div>

      {showPicker && (
        <div className="flex flex-col gap-3">
          <TrackPicker label={label} onPick={(p: Pick) => { setPicking(false); void load(p.videoId, p.title); }} />
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="ghost" disabled={!nowPlaying} onClick={() => { if (nowPlaying) { setPicking(false); void load(nowPlaying.providerTrackId, nowPlaying.title); } }}>Use now playing</Button>
            <select aria-label={`Deck ${label} from up next`} value="" onChange={(e) => { const t = upNext.find((x) => x.id === e.target.value); if (t) { setPicking(false); void load(t.id, t.title); } }} className="h-8 max-w-[220px] rounded-[8px] bg-frost-200 px-2 text-[13px]" disabled={upNext.length === 0}>
              <option value="">{upNext.length ? "From up next" : "Up next is empty"}</option>
              {upNext.slice(0, 30).map((t) => <option key={t.id} value={t.id}>{t.title}</option>)}
            </select>
            <input aria-label={`Deck ${label} link or video id`} value={link} onChange={(e) => setLink(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { setPicking(false); fromLink(); } }} placeholder="or paste a YouTube link" className="h-8 min-w-0 flex-1 rounded-[8px] bg-frost-200 px-3 text-[13px] placeholder:text-tertiary-text" />
          </div>
        </div>
      )}

      {error && <StatusNote tone="amber" role="alert">{error}</StatusNote>}

      {videoId && (
        <>
          {!ready && <StatusNote role="status">Loading the deck. YouTube may play an advert first.</StatusNote>}
          {state === "waiting" && <StatusNote role="status">Buffering.</StatusNote>}
          {state === "ended" && <StatusNote role="status">This track has ended.</StatusNote>}

          <Waveform deck={id} videoId={videoId} pos={pos} dur={dur} playing={playing} rate={fx.enabled ? fx.speed / 100 : 1} cues={cues} marks={lyricMarks} loop={loop} bpm={bpm} onSeek={(ms) => void djSeek(id, ms).catch(() => {})} />
          <Overview pos={pos} dur={dur} cues={cues} loop={loop} disabled={!ready} onSeek={(ms) => void djSeek(id, ms).catch(() => {})} label={label} />

          <div className="flex items-center gap-4">
            <button type="button" aria-label={playing ? `Pause deck ${label}` : `Play deck ${label}`} disabled={!ready} onClick={() => void (playing ? djPause(id) : djPlay(id)).catch(() => setError("The deck did not respond."))} className="grid size-11 place-items-center rounded-full text-primary-text transition-transform duration-100 hover:bg-frost-200 active:scale-[0.97] disabled:text-disabled-text">
              {playing ? <Pause size={26} fill="currentColor" aria-hidden /> : <Play size={26} fill="currentColor" aria-hidden />}
            </button>
            <div className="flex flex-col">
              <span className="text-[22px] font-semibold tabular-nums leading-7">{fmt(pos)}</span>
              <span className="text-[11px] tabular-nums text-secondary-text">-{fmt(remaining)} of {fmt(dur)}</span>
            </div>
            <div className="ml-auto flex items-center gap-1">
              <Button variant="ghost" disabled={!ready} onPointerDown={() => ctl({ kind: "bend", value: -4 })} onPointerUp={() => ctl({ kind: "bend", value: 0 })} onPointerLeave={() => ctl({ kind: "bend", value: 0 })} aria-label={`Nudge deck ${label} slower while held`}>−</Button>
              <span className="text-[11px] text-secondary-text">Nudge</span>
              <Button variant="ghost" disabled={!ready} onPointerDown={() => ctl({ kind: "bend", value: 4 })} onPointerUp={() => ctl({ kind: "bend", value: 0 })} onPointerLeave={() => ctl({ kind: "bend", value: 0 })} aria-label={`Nudge deck ${label} faster while held`}>+</Button>
              <Button variant="ghost" disabled={!ready} onClick={() => ctl({ kind: "brake" })}>Brake</Button>
            </div>
          </div>

          <div role="group" aria-label={`Deck ${label} hot cues`} className="grid grid-cols-4 gap-2">
            {cues.map((c, i) => (
              <button
                key={i}
                type="button"
                disabled={!ready}
                title={c === null ? "Set a cue here" : "Jump. Shift-click or right-click to clear."}
                onClick={(e) => { if (c !== null && e.shiftKey) { setCues(cues.map((x, j) => (j === i ? null : x))); return; } if (c === null) setCues(cues.map((x, j) => (j === i ? pos : x))); else void djSeek(id, c).catch(() => {}); }}
                onContextMenu={(e) => { if (c === null) return; e.preventDefault(); setCues(cues.map((x, j) => (j === i ? null : x))); }}
                className={`flex h-14 flex-col items-start justify-between rounded-[8px] px-3 py-2 text-left transition-colors duration-150 active:scale-[0.97] disabled:opacity-40 ${c === null ? "bg-frost-100 text-secondary-text hover:bg-frost-200" : "bg-frost-200 text-primary-text hover:bg-frost-300"}`}
              >
                <span className="text-[13px] font-semibold">{i + 1}</span>
                <span className="text-[11px] tabular-nums">{c === null ? "Set" : fmt(c)}</span>
              </button>
            ))}
          </div>

          <Group title="Lyrics">
            <DeckLyrics videoId={videoId} pos={pos} ready={ready} onSeek={(ms) => void djSeek(id, ms).catch(() => {})} onMarks={setLyricMarks} />
          </Group>

          <Group title="Loop">
            <div className="flex flex-wrap items-center gap-2">
              <Button disabled={!ready} onClick={() => setLoopPoint("startMs")}>In{loop.startMs !== null ? ` ${fmt(loop.startMs)}` : ""}</Button>
              <Button disabled={!ready} onClick={() => setLoopPoint("endMs")}>Out{loop.endMs !== null ? ` ${fmt(loop.endMs)}` : ""}</Button>
              <IconButton label={loop.on ? `Stop looping deck ${label}` : `Loop deck ${label}`} active={loop.on} aria-pressed={loop.on} disabled={!ready} onClick={toggleLoop}><Repeat size={17} /></IconButton>
              <span className="mx-1 h-5 w-px bg-hairline" aria-hidden />
              <span className="text-[11px] text-secondary-text">Beats</span>
              {BEATS.map((n) => <Button key={n} variant="ghost" disabled={!ready || !bpm} onClick={() => beatLoop(n)}>{n}</Button>)}
            </div>
          </Group>

          <Group title="Tempo">
            <div className="flex flex-wrap items-center gap-2">
              <Button disabled={!ready} onClick={tap}>Tap</Button>
              <span className="w-20 text-[13px] tabular-nums">{bpm ? `${bpm.toFixed(1)}` : "–"} <span className="text-secondary-text">BPM</span></span>
              <Button variant="ghost" disabled={!bpm || heard(other) === null} onClick={sync}>Sync to other deck</Button>
              {bpm && <Button variant="ghost" onClick={() => { setBpm(null); taps.current = []; }}>Reset</Button>}
            </div>
            <Fader label="Speed" min={50} max={200} value={fx.speed} show={(v) => `${v}%`} onChange={(v) => applyFx({ ...fx, speed: v })} />
            <label className="flex items-center gap-2 text-[13px] text-secondary-text">
              <input type="checkbox" checked={!fx.keepPitch} onChange={(e) => applyFx({ ...fx, keepPitch: !e.target.checked })} />
              Pitch follows speed (nightcore and slowed)
            </label>
          </Group>

          <Group title="Mixer">
            <div className="flex items-end gap-5 rounded-[10px] bg-frost-100 px-4 py-4">
              <div className="grid flex-1 grid-cols-3 gap-y-4 sm:grid-cols-5">
                <Knob label="Gain" min={0} max={200} reset={100} value={trim} show={(v) => `${v}%`} onChange={(v) => { setTrim(v); ctl({ kind: "trim", value: v }); }} />
                {EQ.map((g) => (
                  <div key={g.key} className="flex flex-col items-center gap-1">
                    <Knob label={g.label} min={-12} max={12} reset={0} value={eqValue(g.at)} show={(v) => `${v > 0 ? "+" : ""}${v} dB`} onChange={(v) => setEq(g.band, v)} />
                    <button type="button" onClick={() => setEq(g.band, eqValue(g.at) === -12 ? 0 : -12)} aria-pressed={eqValue(g.at) === -12} aria-label={`Kill ${g.label} deck ${label}`} className={`h-5 rounded-full px-2 text-[10px] font-semibold transition-colors duration-150 ${eqValue(g.at) === -12 ? "bg-lumen-wash text-lumen-lavender" : "bg-frost-200 text-secondary-text hover:text-primary-text"}`}>Kill</button>
                  </div>
                ))}
                <Knob label="Filter" min={-100} max={100} reset={0} value={filter} show={(v) => (v === 0 ? "Off" : v < 0 ? `Low ${-v}` : `High ${v}`)} onChange={(v) => { const snap = Math.abs(v) < 4 ? 0 : v; setFilter(snap); ctl({ kind: "filter", value: snap }); }} />
              </div>
              <VFader label="Channel" min={0} max={100} reset={100} value={fader} show={(v) => `${v}`} onChange={(v) => { setFader(v); ctl({ kind: "fader", value: v }); }} />
            </div>
          </Group>

          <Group title="Templates">
            <div role="radiogroup" aria-label={`Deck ${label} templates`} className="flex flex-wrap gap-1.5">
              {templates.map((tpl) => {
                const on = tpl.id === templateId;
                return (
                  <span key={tpl.id} className={`inline-flex h-7 items-center rounded-full text-[13px] transition-colors duration-150 ${on ? "bg-frost-300 text-primary-text" : "bg-frost-100 text-secondary-text hover:bg-frost-200 hover:text-primary-text"}`}>
                    <button type="button" role="radio" aria-checked={on} disabled={!ready} onClick={() => applyTemplate(tpl)} className="h-full px-3 disabled:opacity-40">{tpl.name}</button>
                    {!tpl.builtIn && <button type="button" aria-label={`Delete template ${tpl.name}`} onClick={() => { setTemplates((all) => removeTemplate(all, tpl.id)); if (on) setTemplateId(null); }} className="h-full pr-2.5 text-tertiary-text hover:text-primary-text">×</button>}
                  </span>
                );
              })}
            </div>
            {naming === null ? (
              <Button variant="ghost" className="self-start" disabled={!ready} onClick={() => setNaming("")}>Save this sound as a template</Button>
            ) : (
              <div className="flex gap-2">
                <input autoFocus aria-label="Template name" maxLength={40} value={naming} onChange={(e) => setNaming(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") saveTemplate(); if (e.key === "Escape") setNaming(null); }} placeholder="Name it, for example Late night slowed" className="h-8 min-w-0 flex-1 rounded-[8px] bg-frost-200 px-3 text-[13px] placeholder:text-tertiary-text" />
                <Button onClick={saveTemplate}>Save</Button>
                <Button variant="ghost" onClick={() => setNaming(null)}>Cancel</Button>
              </div>
            )}
            <p className="text-[11px] text-tertiary-text">A template holds the effects, filter, gain and echo. Cue points and loops stay with each song.</p>
          </Group>

          <Group title="Sound">
            <div role="radiogroup" aria-label={`Deck ${label} remix style`} className="flex flex-wrap gap-1.5">
              {QUICK.map((q) => {
                const on = q.label === quickNow;
                return <button key={q.label} type="button" role="radio" aria-checked={on} onClick={() => applyFx({ ...FLAT_EFFECTS, ...q.fx })} className={`h-7 rounded-full px-3 text-[13px] transition-colors duration-150 ${on ? "bg-frost-300 text-primary-text" : "bg-frost-100 text-secondary-text hover:bg-frost-200 hover:text-primary-text"}`}>{q.label}</button>;
              })}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-[11px] text-secondary-text">Echo</span>
              {[0.25, 0.5, 1].map((b) => <Button key={b} variant={echo === b ? "secondary" : "ghost"} aria-pressed={echo === b} disabled={!ready || !bpm} onClick={() => echoFor(b)}>{b === 1 ? "1 beat" : `1/${1 / b}`}</Button>)}
              {echo !== 0 && <Button variant="ghost" onClick={() => { setEcho(0); ctl({ kind: "echo", delayMs: 0, feedback: 0 }); }}>Off</Button>}
              {!bpm && <span className="text-[11px] text-tertiary-text">needs BPM</span>}
            </div>
            <select aria-label={`Deck ${label} preset`} value="" onChange={(e) => { const p = presets.find((x) => x.id === e.target.value); if (p) applyFx({ ...FLAT_EFFECTS, ...p.effects, enabled: true }); }} className="h-8 rounded-[8px] bg-frost-200 px-2 text-[13px]">
              <option value="">More presets</option>
              {[...new Set(presets.map((p) => p.group))].map((g) => <optgroup key={g} label={g}>{presets.filter((p) => p.group === g).map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}</optgroup>)}
            </select>
          </Group>
        </>
      )}
    </section>
  );
}

/** A small titled group, separated by space rather than a box. */
function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2.5">
      <h3 className="text-[13px] font-semibold text-secondary-text">{title}</h3>
      {children}
    </div>
  );
}

/** One labelled horizontal fader with its value. Double-click resets when `onReset` is given. */
function Fader({ label, min, max, value, show, onChange, onReset }: { label: string; min: number; max: number; value: number; show: (v: number) => string; onChange: (v: number) => void; onReset?: () => void }) {
  return (
    <label className="flex items-center gap-3 text-[13px]">
      <span className="w-14 shrink-0 text-secondary-text">{label}</span>
      <input type="range" min={min} max={max} step={1} value={value} aria-valuetext={show(value)} onDoubleClick={onReset} onChange={(e) => onChange(Number(e.target.value))} className="min-w-0 flex-1" />
      <span className="w-16 shrink-0 text-right text-[11px] tabular-nums text-secondary-text">{show(value)}</span>
    </label>
  );
}

/** The track at a glance: where the playhead is, the cue points and the loop. Click anywhere to jump. */
function Overview({ pos, dur, cues, loop, disabled, onSeek, label }: { pos: number; dur: number; cues: (number | null)[]; loop: { startMs: number | null; endMs: number | null; on: boolean }; disabled: boolean; onSeek: (ms: number) => void; label: string }) {
  const pct = (ms: number) => (dur > 0 ? Math.min(100, Math.max(0, (ms / dur) * 100)) : 0);
  const hasLoop = loop.startMs !== null && loop.endMs !== null && loop.endMs > loop.startMs;
  return (
    <div
      role="slider"
      aria-label={`Deck ${label} position`}
      aria-valuemin={0}
      aria-valuemax={Math.max(dur, 1)}
      aria-valuenow={Math.round(pos)}
      aria-valuetext={`${fmt(pos)} of ${fmt(dur)}`}
      aria-disabled={disabled || dur === 0}
      tabIndex={disabled ? -1 : 0}
      onClick={(e) => { if (disabled || dur === 0) return; const r = e.currentTarget.getBoundingClientRect(); onSeek(((e.clientX - r.left) / r.width) * dur); }}
      onKeyDown={(e) => { if (disabled || dur === 0) return; if (e.key === "ArrowLeft") onSeek(pos - 5000); if (e.key === "ArrowRight") onSeek(pos + 5000); }}
      className="relative h-5 cursor-pointer overflow-hidden rounded-[6px] bg-frost-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-text"
    >
      <div aria-hidden className="absolute inset-y-0 left-0 bg-frost-200" style={{ width: `${pct(pos)}%` }} />
      {hasLoop && <div aria-hidden className={`absolute inset-y-0 ${loop.on ? "bg-lumen-wash" : "bg-frost-200"}`} style={{ left: `${pct(loop.startMs!)}%`, width: `${pct(loop.endMs!) - pct(loop.startMs!)}%` }} />}
      {cues.map((c, i) => c !== null && (
        <span key={i} aria-hidden className="absolute top-0 flex h-full flex-col items-start" style={{ left: `${pct(c)}%` }}>
          <span className="h-full w-px bg-secondary-text" />
          <span className="absolute top-0.5 left-1 text-[10px] font-semibold text-secondary-text">{i + 1}</span>
        </span>
      ))}
      <span aria-hidden className="absolute inset-y-0 w-0.5 bg-primary-text" style={{ left: `${pct(pos)}%` }} />
    </div>
  );
}

/** Two YouTube decks, a crossfader and a master volume. Each deck is a YouTube page, so its adverts play as usual. */
export default function DjScreen() {
  const [reports, setReports] = useDjField("reports");
  const [presets, setPresets] = useState<EffectPreset[]>([]);
  const [cross, setCross] = useDjField("cross");
  const [master, setMaster] = useDjField("master");
  const mixTimer = useRef<number | undefined>(undefined);
  const [tempo, setTempo] = useDjField("tempo");
  const tempoA = useCallback((t: Tempo) => setTempo((p) => (p.a.bpm === t.bpm && p.a.speed === t.speed ? p : { ...p, a: t })), []);
  const tempoB = useCallback((t: Tempo) => setTempo((p) => (p.b.bpm === t.bpm && p.b.speed === t.speed ? p : { ...p, b: t })), []);
  const keysA = useRef<DeckKeys | null>(null);
  const keysB = useRef<DeckKeys | null>(null);
  // Bare keys, only on this screen and never while typing: A is the left hand, B the right.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (e.metaKey || e.ctrlKey || e.altKey || e.repeat || (t && (t.tagName === "INPUT" || t.tagName === "SELECT" || t.tagName === "TEXTAREA" || t.isContentEditable || t.getAttribute("role") === "slider"))) return;
      const a = keysA.current, b = keysB.current;
      const map: Record<string, () => void> = {
        KeyQ: () => a?.playPause(), Digit1: () => a?.cue(0), Digit2: () => a?.cue(1), Digit3: () => a?.cue(2), Digit4: () => a?.cue(3), KeyZ: () => a?.brake(),
        KeyP: () => b?.playPause(), Digit7: () => b?.cue(0), Digit8: () => b?.cue(1), Digit9: () => b?.cue(2), Digit0: () => b?.cue(3), Slash: () => b?.brake(),
        ArrowLeft: () => setCross((c) => Math.max(0, c - 5)), ArrowRight: () => setCross((c) => Math.min(100, c + 5)),
      };
      const fn = map[e.code];
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const fade = useRef<number | undefined>(undefined);
  useEffect(() => () => window.cancelAnimationFrame(fade.current ?? 0), []);
  const fadeTo = (target: number, seconds = 8) => {
    window.cancelAnimationFrame(fade.current ?? 0);
    const from = cross;
    const start = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / (seconds * 1000));
      setCross(Math.round(from + (target - from) * k));
      if (k < 1) fade.current = window.requestAnimationFrame(step);
    };
    fade.current = window.requestAnimationFrame(step);
  };

  useEffect(() => {
    let off: (() => void) | undefined;
    let live = true;
    void subscribeDeck((r) => setReports((prev) => ({ ...prev, [r.deck]: r }))).then((u) => { if (live) off = u; else u(); });
    return () => { live = false; off?.(); };
  }, []);
  useEffect(() => { void playerEffectPresets().then(setPresets, () => {}); }, []);
  // The decks may have kept playing while this screen was closed: take their latest state from core.
  useEffect(() => {
    void djStatus().then((list) => {
      for (const r of list) {
        setReports((prev) => ({ ...prev, [r.deck]: r }));
        const d = useDj.getState().decks[r.deck];
        if (d.videoId !== r.videoId) useDj.setState((st) => ({ decks: { ...st.decks, [r.deck]: { ...st.decks[r.deck], videoId: r.videoId } } }));
      }
    }, () => {});
  }, [setReports]);
  useEffect(() => {
    window.clearTimeout(mixTimer.current);
    mixTimer.current = window.setTimeout(() => { void djSetMix(cross, master).catch(() => {}); }, 40);
    return () => window.clearTimeout(mixTimer.current);
  }, [cross, master]);

  const setDeck = useMemo(() => (deck: DeckId) => (r: DeckReport | null) => setReports((prev) => ({ ...prev, [deck]: r })), []);
  return (
    <Screen id="dj-title" title="DJ" caption="Two YouTube decks with loops, cue points, tempo and effects. Each deck is a YouTube page, so adverts play and the mix cannot be saved as audio.">
      <section aria-label="Mixer" className="flex flex-col gap-3 rounded-[10px] bg-frost-100 px-5 py-4">
        <div className="flex items-center gap-4">
          <span className="w-6 text-[13px] font-semibold text-secondary-text">A</span>
          <input type="range" min={0} max={100} step={1} value={cross} aria-label="Crossfader" aria-valuetext={cross === 50 ? "Centre" : cross < 50 ? `${100 - cross} percent towards A` : `${cross} percent towards B`} onDoubleClick={() => setCross(50)} onChange={(e) => { window.cancelAnimationFrame(fade.current ?? 0); setCross(Number(e.target.value)); }} className="min-w-0 flex-1" />
          <span className="w-6 text-right text-[13px] font-semibold text-secondary-text">B</span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="ghost" onClick={() => fadeTo(0)}>Fade to A</Button>
          <Button variant="ghost" onClick={() => { window.cancelAnimationFrame(fade.current ?? 0); setCross(50); }}>Centre</Button>
          <Button variant="ghost" onClick={() => fadeTo(100)}>Fade to B</Button>
          <label className="ml-auto flex w-full items-center gap-3 text-[13px] sm:w-[280px]">
            <span className="text-secondary-text">Master</span>
            <input type="range" min={0} max={100} step={1} value={master} aria-label="Master volume" aria-valuetext={`${master} percent`} onChange={(e) => setMaster(Number(e.target.value))} className="min-w-0 flex-1" />
            <span className="w-8 text-right text-[11px] tabular-nums text-secondary-text">{master}</span>
          </label>
        </div>
      </section>
      <div className="grid grid-cols-1 gap-10 xl:grid-cols-2 xl:gap-12">
        <Deck id="a" report={reports.a} onReport={setDeck("a")} presets={presets} other={tempo.b} onTempo={tempoA} keys={keysA} />
        <Deck id="b" report={reports.b} onReport={setDeck("b")} presets={presets} other={tempo.a} onTempo={tempoB} keys={keysB} />
      </div>
      <p className="text-[11px] text-tertiary-text">Keys: Q and P play deck A and B · 1 to 4 and 7 to 0 hot cues · Z and / brake · ← → crossfader · double-click a fader to reset.</p>
    </Screen>
  );
}

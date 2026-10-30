"use client";

import { useEffect, useRef } from "react";
import { subscribeWave } from "../../services/tauri/dj";
import type { DeckId } from "../../services/tauri/dj";

const STEP = 50;
/** What each track has sounded like so far, kept while the app runs so a reloaded track keeps its scan. */
const scans = new Map<string, Map<number, [number, number, number]>>();
const scanFor = (videoId: string) => {
  let m = scans.get(videoId);
  if (!m) { m = new Map(); scans.set(videoId, m); if (scans.size > 40) scans.delete(scans.keys().next().value!); }
  return m;
};

/**
 * A scrolling three-band waveform centred on the playhead, drawn as the deck plays. YouTube gives no audio ahead of the
 * playhead, so only what has been heard is drawn; the rest is a flat line until the deck reaches it.
 */
export function Waveform({ deck, videoId, pos, dur, playing, rate, cues, marks = [], loop, bpm, onSeek, windowMs = 10_000 }: {
  deck: DeckId; videoId: string; pos: number; dur: number; playing: boolean; rate: number;
  cues: (number | null)[]; marks?: number[]; loop: { startMs: number | null; endMs: number | null; on: boolean }; bpm: number | null;
  onSeek: (ms: number) => void; windowMs?: number;
}) {
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const anchor = useRef({ pos, at: performance.now() });
  const live = useRef({ playing, rate, cues, marks, loop, bpm, dur, windowMs });
  live.current = { playing, rate, cues, marks, loop, bpm, dur, windowMs };
  useEffect(() => { anchor.current = { pos, at: performance.now() }; }, [pos]);

  useEffect(() => {
    let off: (() => void) | undefined;
    let on = true;
    void subscribeWave((b) => {
      if (b.deck !== deck || b.videoId !== videoId) return;
      const scan = scanFor(videoId);
      b.bands.forEach((v, i) => scan.set(Math.round((b.atMs + i * STEP * (live.current.rate || 1)) / STEP), v));
    }).then((u) => { if (on) off = u; else u(); });
    return () => { on = false; off?.(); };
  }, [deck, videoId]);

  useEffect(() => {
    const el = canvas.current;
    if (!el) return;
    const ctx = el.getContext("2d");
    if (!ctx) return;
    const css = getComputedStyle(document.documentElement);
    const ink = css.getPropertyValue("--color-primary-text").trim() || "#f5f5f7";
    const lumen = css.getPropertyValue("--color-lumen-lavender").trim() || "#c5b4ff";
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let frame = 0;
    const draw = () => {
      const { playing: p, rate: r, cues: cs, marks: mk, loop: lp, bpm: b, dur: d, windowMs: w } = live.current;
      const dpr = window.devicePixelRatio || 1;
      const W = el.clientWidth, H = el.clientHeight;
      if (el.width !== Math.round(W * dpr) || el.height !== Math.round(H * dpr)) { el.width = Math.round(W * dpr); el.height = Math.round(H * dpr); }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, H);
      const now = anchor.current.pos + (p ? (performance.now() - anchor.current.at) * r : 0);
      const head = Math.min(d || Infinity, Math.max(0, now));
      const x = (ms: number) => W / 2 + ((ms - head) / w) * W;
      const mid = H / 2;
      const scan = scans.get(videoId);

      // Loop region.
      if (lp.startMs !== null && lp.endMs !== null && lp.endMs > lp.startMs) {
        ctx.globalAlpha = lp.on ? 0.18 : 0.08;
        ctx.fillStyle = lp.on ? lumen : ink;
        ctx.fillRect(x(lp.startMs), 0, x(lp.endMs) - x(lp.startMs), H);
      }
      // Beat grid from the tapped tempo, counted from cue 1 (or the start). Every fourth beat is a bar line.
      const origin = cs[0] ?? 0;
      if (b) {
        const beat = 60000 / b;
        const first = Math.ceil((head - w / 2 - origin) / beat);
        const last = Math.floor((head + w / 2 - origin) / beat);
        for (let n = first; n <= last; n++) {
          const bx = Math.round(x(origin + n * beat)) + 0.5;
          ctx.globalAlpha = n % 4 === 0 ? 0.35 : 0.12;
          ctx.fillStyle = ink;
          ctx.fillRect(bx, 0, 1, H);
        }
      }
      // Unheard: a quiet centre line.
      ctx.globalAlpha = 0.15;
      ctx.fillStyle = ink;
      ctx.fillRect(0, mid, W, 1);
      // Heard: three layers, lows widest and softest, highs thinnest and brightest.
      if (scan) {
        const from = Math.floor((head - w / 2) / STEP), to = Math.ceil((head + w / 2) / STEP);
        const bw = Math.max(1, (STEP / w) * W);
        for (let i = from; i <= to; i++) {
          const v = scan.get(i);
          if (!v) continue;
          const bx = x(i * STEP);
          const past = i * STEP <= head;
          const layers: [number, number][] = [[v[0], 0.28], [v[1], 0.55], [v[2], 0.95]];
          for (const [e, a] of layers) {
            const h = (e / 255) * (mid - 2);
            ctx.globalAlpha = past ? a : a * 0.5;
            ctx.fillStyle = ink;
            ctx.fillRect(bx, mid - h, Math.ceil(bw), h * 2);
          }
        }
      }
      // Lyric marks: short lavender ticks along the bottom edge.
      ctx.globalAlpha = 0.9;
      ctx.fillStyle = lumen;
      mk.forEach((m) => { const mx = Math.round(x(m)); if (mx >= -2 && mx <= W + 2) ctx.fillRect(mx - 1, H - 8, 2, 8); });
      // Cue points.
      cs.forEach((c, i) => {
        if (c === null) return;
        const cx = Math.round(x(c)) + 0.5;
        if (cx < -10 || cx > W + 10) return;
        ctx.globalAlpha = 0.9;
        ctx.fillStyle = ink;
        ctx.fillRect(cx, 0, 1, H);
        ctx.font = "600 10px -apple-system, Inter, sans-serif";
        ctx.fillText(String(i + 1), cx + 3, 11);
      });
      // Playhead: fixed in the centre.
      ctx.globalAlpha = 1;
      ctx.fillStyle = ink;
      ctx.fillRect(Math.round(W / 2) - 1, 0, 2, H);
      ctx.globalAlpha = 1;
      if (!reduce || p) frame = window.requestAnimationFrame(draw);
    };
    frame = window.requestAnimationFrame(draw);
    return () => window.cancelAnimationFrame(frame);
  }, [videoId, playing]);

  return (
    <canvas
      ref={canvas}
      role="img"
      aria-label={`Deck ${deck.toUpperCase()} waveform. Click to jump.`}
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        const now = anchor.current.pos + (playing ? (performance.now() - anchor.current.at) * rate : 0);
        const ms = now + ((e.clientX - r.left - r.width / 2) / r.width) * windowMs;
        onSeek(Math.max(0, dur ? Math.min(dur, ms) : ms));
      }}
      className="h-28 w-full cursor-pointer rounded-[8px] bg-black/40"
    />
  );
}

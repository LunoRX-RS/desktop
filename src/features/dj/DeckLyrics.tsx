"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Bookmark, BookmarkCheck } from "lucide-react";
import { Button, StatusNote } from "../../components/ui";
import { loadMarks, saveMarks, type LyricMark } from "../../lib/djSets";
import { playerTrackInfo, type LyricLineDto } from "../../services/tauri/player";

const fmt = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

interface Lyrics { lines: { text: string; startMs: number | null }[]; synced: boolean; source: string | null }

/** Picks timed lines when any source has them, otherwise the plain text, one line per row. */
function pick(info: Awaited<ReturnType<typeof playerTrackInfo>>): Lyrics | null {
  const timed = (l: LyricLineDto[] | null | undefined) => (l && l.length ? l : null);
  const fromSource = info.lyricsSources?.find((s) => s.available && s.synced && timed(s.lines));
  const lines = info.lyricsSynced ? timed(info.lyricsLines) : null;
  if (lines) return { lines: lines.map((l) => ({ text: l.text, startMs: l.startMs })), synced: true, source: info.lyricsProvider };
  if (fromSource?.lines) return { lines: fromSource.lines.map((l) => ({ text: l.text, startMs: l.startMs })), synced: true, source: fromSource.label };
  const text = info.lyrics ?? info.lyricsSources?.find((s) => s.available && s.text)?.text ?? null;
  if (!text) return null;
  return { lines: text.split("\n").map((t) => ({ text: t, startMs: null })), synced: false, source: info.lyricsProvider };
}

/**
 * The deck's lyrics, following the deck. Click a timed line to jump to it. Mark a line to pin it to a moment: a timed
 * line keeps its own time, an untimed one takes the deck's current position, so plain lyrics can be timed by hand.
 */
export function DeckLyrics({ videoId, pos, ready, onSeek, onMarks }: { videoId: string; pos: number; ready: boolean; onSeek: (ms: number) => void; onMarks: (ms: number[]) => void }) {
  const [lyrics, setLyrics] = useState<Lyrics | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "none" | "failed">("loading");
  const [marks, setMarks] = useState<LyricMark[]>([]);
  const [saved, setSaved] = useState<LyricMark[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const list = useRef<HTMLOListElement | null>(null);

  useEffect(() => {
    let live = true;
    setState("loading");
    setLyrics(null);
    const m = loadMarks(videoId);
    setMarks(m);
    setSaved(m);
    setNote(null);
    playerTrackInfo(videoId).then(
      (info) => { if (!live) return; const l = pick(info); setLyrics(l); setState(l ? "ready" : "none"); },
      () => { if (live) setState("failed"); },
    );
    return () => { live = false; };
  }, [videoId]);

  useEffect(() => { onMarks(marks.map((m) => m.ms)); }, [marks, onMarks]);

  // The line being sung: the last timed line (or mark) at or before the playhead.
  const current = useMemo(() => {
    if (!lyrics) return -1;
    const at = (i: number) => marks.find((m) => m.line === i)?.ms ?? lyrics.lines[i].startMs;
    let best = -1;
    lyrics.lines.forEach((_, i) => { const t = at(i); if (t !== null && t !== undefined && t <= pos) best = i; });
    return best;
  }, [lyrics, marks, pos]);

  useEffect(() => {
    const el = list.current?.querySelector<HTMLElement>(`[data-line="${current}"]`);
    if (!el || !list.current) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    list.current.scrollTo({ top: el.offsetTop - list.current.clientHeight / 2 + el.clientHeight / 2, behavior: reduce ? "auto" : "smooth" });
  }, [current]);

  const dirty = JSON.stringify(marks) !== JSON.stringify(saved);
  const toggle = (i: number) => {
    if (!lyrics) return;
    const has = marks.some((m) => m.line === i);
    if (has) { setMarks(marks.filter((m) => m.line !== i)); return; }
    const ms = lyrics.lines[i].startMs ?? Math.round(pos);
    setMarks([...marks, { line: i, ms, text: lyrics.lines[i].text }].sort((a, b) => a.ms - b.ms));
  };
  const save = () => {
    if (saveMarks(videoId, marks)) { setSaved(marks); setNote(`Saved ${marks.length} mark${marks.length === 1 ? "" : "s"} on this device.`); }
    else setNote("The marks could not be saved. Storage may be full.");
  };

  if (state === "loading") return <StatusNote role="status">Looking for lyrics.</StatusNote>;
  if (state === "failed") return <StatusNote tone="amber" role="status">Lyrics could not be loaded.</StatusNote>;
  if (state === "none" || !lyrics) return <p className="text-[13px] text-secondary-text">No lyrics found for this track.</p>;

  return (
    <div className="flex flex-col gap-2.5">
      <p className="text-[11px] text-tertiary-text">
        {lyrics.synced ? "Timed lyrics" : "Untimed lyrics: mark a line as it is sung to time it"}{lyrics.source ? ` · ${lyrics.source}` : ""}
      </p>
      <ol ref={list} aria-label="Lyrics" className="relative flex max-h-64 flex-col overflow-y-auto rounded-[8px] bg-frost-100 py-2">
        {lyrics.lines.map((l, i) => {
          const mark = marks.find((m) => m.line === i);
          const on = i === current;
          if (!l.text.trim()) return <li key={i} data-line={i} aria-hidden className="h-3" />;
          return (
            <li key={i} data-line={i} className={`group flex items-center gap-2 px-3 py-1 ${on ? "bg-frost-200" : ""}`}>
              <button type="button" disabled={!ready || (l.startMs === null && !mark)} onClick={() => { const t = mark?.ms ?? l.startMs; if (t !== null) onSeek(t); }} className={`min-w-0 flex-1 text-left text-[14px] leading-5 transition-colors duration-150 disabled:cursor-default ${on ? "font-semibold text-primary-text" : "text-secondary-text hover:text-primary-text"}`}>
                {l.text}
              </button>
              <span className="w-9 shrink-0 text-right text-[10px] tabular-nums text-tertiary-text">{mark ? fmt(mark.ms) : l.startMs !== null ? fmt(l.startMs) : ""}</span>
              <button type="button" disabled={!ready} aria-pressed={!!mark} aria-label={mark ? `Remove mark from line: ${l.text}` : `Mark line: ${l.text}`} onClick={() => toggle(i)} className={`grid size-7 shrink-0 place-items-center rounded-[6px] transition-colors duration-150 ${mark ? "text-lumen-lavender" : "text-tertiary-text opacity-0 hover:text-primary-text group-hover:opacity-100 focus-visible:opacity-100"}`}>
                {mark ? <BookmarkCheck size={15} aria-hidden /> : <Bookmark size={15} aria-hidden />}
              </button>
            </li>
          );
        })}
      </ol>
      {marks.length > 0 && (
        <div className="flex flex-wrap gap-1.5" aria-label="Marked lines">
          {marks.map((m) => (
            <button key={m.line} type="button" disabled={!ready} onClick={() => onSeek(m.ms)} title={m.text} className="h-7 max-w-[200px] truncate rounded-full bg-frost-200 px-3 text-[12px] hover:bg-frost-300">
              {fmt(m.ms)} · {m.text}
            </button>
          ))}
        </div>
      )}
      <div className="flex items-center gap-2">
        <Button disabled={!dirty} onClick={save}>{dirty ? "Save marks" : "Saved"}</Button>
        {marks.length > 0 && <Button variant="ghost" onClick={() => setMarks([])}>Clear marks</Button>}
        {dirty && <span className="text-[11px] text-secondary-text">Unsaved changes</span>}
      </div>
      {note && <p role="status" className="text-[11px] text-secondary-text">{note}</p>}
    </div>
  );
}

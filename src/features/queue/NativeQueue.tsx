"use client";

import { GripVertical, ListMusic, Loader2, X } from "lucide-react";
import { playerQueueEdit, playerQueueMore } from "../../services/tauri/player";
import { attempt } from "../../lib/notice";
import { usePlayback, playTrack } from "../../lib/playback";
import { useUi } from "../../lib/store";
import { TrackCover } from "../../components/TrackCover";
import { LikeButton } from "../../components/LibraryActions";
import { formatDuration } from "../../lib/media";
import { Button, EmptyState, IconButton, cx } from "../../components/ui";
import { VibePicker } from "../../components/PlayModeButtons";
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type Ref, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import type { OfflineTrack } from "../../services/tauri/offline";
import { addToPlaylist } from "../../lib/playlistActions";
import { syncCreateLocalPlaylist } from "../../services/tauri/sync";

const NEXT_STATUS: Record<string, string> = {
  ready: "Ready to play after this song",
  preparing: "Preparing the audio",
  finding: "Finding related music",
  unavailable: "No related track could be prepared",
};

// Rows are drawn a screenful at a time. The next batch is added when a row a little before the end of what is drawn comes
// into view, so a normal scroll never reaches the edge; only a very fast one shows the placeholder rows for a moment.
/** The most songs core keeps up next (QUEUE_MAX in autoplay.rs); at this size there is nothing more to load. */
const QUEUE_CAP = 500;
const FIRST_ROWS = 12;
const MORE_ROWS = 16;
const LOOKAHEAD_ROWS = 6;

type RowProps = {
  item: OfflineTrack;
  index: number;
  busy: boolean;
  dragging: boolean;
  target: "top" | "bottom" | null;
  dy: number;
  status: string | null;
  rowRef: (el: HTMLLIElement | null) => void;
  onDown: (e: ReactPointerEvent<HTMLButtonElement>, index: number) => void;
  onMove: (e: ReactPointerEvent<HTMLButtonElement>) => void;
  onUp: (e: ReactPointerEvent<HTMLButtonElement>) => void;
  onCancel: () => void;
  onKey: (e: ReactKeyboardEvent<HTMLButtonElement>, index: number) => void;
};

const toHit = (item: OfflineTrack) => ({ provider: item.provider, source: "YouTube Music" as const, providerTrackId: item.id, canonicalUrl: `https://www.youtube.com/watch?v=${item.id}`, title: item.title, artists: item.artists, artworkUrl: item.artworkUrl, durationMs: item.durationMs });

/** One Up next row. Memoised: it is drawn again only when what it shows changes, not on every progress update of the player. */
const QueueRow = memo(function QueueRow({ item, index, busy, dragging, target, dy, status, rowRef, onDown, onMove, onUp, onCancel, onKey }: RowProps) {
  const hit = toHit(item);
  return <li ref={rowRef}
    style={dragging ? { transform: `translateY(${dy}px)` } : undefined}
    className={cx("group/row grid min-h-16 grid-cols-[24px_40px_minmax(0,1fr)_auto] items-center gap-2 rounded-lg px-2 hover:bg-frost-200 focus-within:bg-frost-200",
      dragging && "relative z-10 bg-frost-300 opacity-90 shadow-[0_8px_24px_rgba(0,0,0,0.5)]",
      target === "top" && "shadow-[inset_0_2px_0_rgba(255,255,255,0.45)]", target === "bottom" && "shadow-[inset_0_-2px_0_rgba(255,255,255,0.45)]")}>
    <button type="button" aria-label={`Move ${item.title}. Drag it, or press the up and down arrow keys.`}
      onPointerDown={(e) => onDown(e, index)} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={onCancel}
      onKeyDown={(e) => onKey(e, index)}
      className="group/h flex size-6 cursor-grab touch-none items-center justify-center rounded text-secondary-text select-none active:cursor-grabbing">
      <span className="text-[12px] tabular-nums group-hover/row:hidden group-focus-visible/h:hidden">{index + 1}</span>
      <GripVertical size={14} aria-hidden="true" className="hidden group-hover/row:block group-focus-visible/h:block" />
    </button>
    <button type="button" disabled={busy} onClick={() => void playTrack(hit, true)} aria-label={`Play ${item.title} from autoplay queue`} className="rounded-md disabled:opacity-40"><TrackCover url={item.artworkUrl} videoId={item.id} className="size-10 rounded-[6px]" /></button>
    <button type="button" disabled={busy} onClick={() => void playTrack(hit, true)} className="min-w-0 rounded-md text-left disabled:opacity-40">
      <p className="truncate text-[13px] font-semibold" title={item.title}>{item.title}</p>
      <p className="truncate text-[12px] text-secondary-text">{item.artists.join(", ")} · YouTube Music</p>
      {status && <p role="status" className="text-[11px] text-secondary-text">{status}</p>}
    </button>
    <div className="flex items-center gap-1">
      <span className="text-[11px] tabular-nums text-secondary-text">{item.durationMs != null ? formatDuration(item.durationMs / 1000) : ""}</span>
      <IconButton label={`Remove ${item.title} from the queue`} onClick={() => attempt(playerQueueEdit({ op: "remove", id: item.id }), "Could not remove the song from the queue.")} className="size-7"><X size={14} /></IconButton>
      <LikeButton track={hit} />
    </div>
  </li>;
}, (a, b) =>
  a.item.id === b.item.id && a.item.title === b.item.title && a.item.artworkUrl === b.item.artworkUrl && a.item.durationMs === b.item.durationMs
  && a.item.artists.join() === b.item.artists.join()
  && a.index === b.index && a.busy === b.busy && a.dragging === b.dragging && a.target === b.target && a.status === b.status
  && (!a.dragging || a.dy === b.dy)
  && a.onDown === b.onDown && a.onMove === b.onMove && a.onUp === b.onUp && a.onCancel === b.onCancel && a.onKey === b.onKey && a.rowRef === b.rowRef);

/** Two grey rows shown for the moment it takes to draw the next batch. */
function PlaceholderRows({ liRef }: { liRef: Ref<HTMLLIElement> }) {
  return <li ref={liRef} aria-hidden="true" className="flex flex-col gap-2 px-2 py-2">
    {[0, 1].map((i) => <div key={i} className="flex items-center gap-3"><div className="size-10 rounded-[6px] bg-frost-200" /><div className="flex flex-1 flex-col gap-1.5"><div className="h-3 w-3/5 rounded bg-frost-200" /><div className="h-3 w-2/5 rounded bg-frost-100" /></div></div>)}
  </li>;
}

/** Loads more related songs when the end of the list scrolls into view, one request at a time. It never gives up:
 * after an empty answer it waits a little longer each time and asks again while the end is still on screen. */
function useLoadMore(enabled: boolean, resetKey: string | undefined, remaining: number, sentinelShown: boolean) {
  const sentinel = useRef<HTMLLIElement | null>(null);
  const [loading, setLoading] = useState(false);
  const [retry, setRetry] = useState(0);
  const misses = useRef(0);
  const inFlight = useRef(false);
  useEffect(() => { misses.current = 0; }, [resetKey]);
  const load = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setLoading(true);
    let added = 0;
    try { added = await playerQueueMore(); } catch { /* counted as a miss below */ }
    misses.current = added > 0 ? 0 : misses.current + 1;
    inFlight.current = false;
    setLoading(false);
    if (added === 0) window.setTimeout(() => setRetry((n) => n + 1), Math.min(800 * 2 ** misses.current, 8000));
  }, []);
  useEffect(() => {
    const node = sentinel.current;
    if (!enabled || !sentinelShown || !node) return;
    const observer = new IntersectionObserver((entries) => { if (entries.some((e) => e.isIntersecting)) void load(); }, { rootMargin: "900px" });
    observer.observe(node);
    return () => observer.disconnect();
  }, [enabled, sentinelShown, load, retry]); // not `loading`: the observer stays put while a load runs; `retry` re-arms it after an empty answer
  // Keep a good stretch of songs ready ahead of the scroll, so the end of the list is rarely reached.
  useEffect(() => { if (enabled && remaining < 30) void load(); }, [enabled, remaining, load, retry]);
  return { sentinel, loading };
}

export default function NativeQueue() {
  const navigate = useUi((u) => u.navigate);
  const toggleQueue = useUi((u) => u.toggleQueue);
  // Slices, not the whole store: the whole store changes about four times a second while a song plays.
  const requested = usePlayback((p) => p.requested);
  const current = usePlayback((p) => p.track);
  const stored = usePlayback((p) => p.info);
  const playerState = usePlayback((p) => p.player.state);
  const busy = usePlayback((p) => p.busy);
  const autoplayState = usePlayback((p) => p.autoplay.state);
  // Keyed on what a row shows (id, title, artists, cover, length), so an edit to a queued track refreshes the list too.
  const queueKey = usePlayback((p) => (p.autoplay.queue ?? []).map((q) => JSON.stringify([q.id, q.title, q.artists, q.artworkUrl, q.durationMs])).join("\n"));
  // The list itself is read when the queue changes, not each time the player reports.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const upcoming = useMemo<OfflineTrack[]>(() => usePlayback.getState().autoplay.queue ?? [], [queueKey]);
  const track = requested ?? current;
  const info = stored?.providerTrackId === track?.providerTrackId ? stored : null;
  const title = info?.title ?? track?.title ?? "YouTube";
  const artists = (info?.artists ?? track?.artists)?.join(", ") ?? "";
  const playing = playerState === "playing" || playerState === "buffering";
  const next = { state: autoplayState };
  const [shown, setShown] = useState(FIRST_ROWS);
  const [saved, setSaved] = useState<"" | "saving" | "done" | "failed">("");
  // Reordering by dragging the handle of a row. The pointer is captured by the handle, so the drag keeps working outside it.
  const list = useRef<HTMLOListElement>(null);
  const rows = useRef<(HTMLLIElement | null)[]>([]);
  const startY = useRef(0);
  const [drag, setDrag] = useState<{ from: number; over: number; dy: number } | null>(null);
  // One move at a time: a second press before the list has caught up would send the same indices against the old order.
  const moving = useRef(false);
  const order = upcoming.map((u) => u.id).join(",");
  useEffect(() => { moving.current = false; }, [order]);
  const move = (from: number, to: number) => {
    if (moving.current || to < 0 || to >= upcoming.length || to === from) return;
    moving.current = true;
    void playerQueueEdit({ op: "move", from, to }).catch(() => { moving.current = false; });
    window.setTimeout(() => { moving.current = false; }, 600); // a move that changed nothing never updates the order
  };
  const rowUnder = (clientY: number): number => {
    const ol = list.current;
    if (!ol) return 0;
    const y = clientY - ol.getBoundingClientRect().top + ol.offsetTop; // same offset parent as the rows
    let over = 0;
    rows.current.forEach((li, i) => {
      if (li && y >= li.offsetTop) over = i;
    });
    return Math.min(over, upcoming.length - 1);
  };
  const dragRef = useRef(drag);
  const moveRef = useRef(move);
  const rowUnderRef = useRef(rowUnder);
  // Written after the commit, not during render, so a render that is thrown away never leaves its handlers in the refs.
  useLayoutEffect(() => {
    dragRef.current = drag;
    moveRef.current = move;
    rowUnderRef.current = rowUnder;
  });
  const onHandleDown = useRef((e: ReactPointerEvent<HTMLButtonElement>, from: number) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    startY.current = e.clientY;
    setDrag({ from, over: from, dy: 0 });
  }).current;
  const onHandleMove = useRef((e: ReactPointerEvent<HTMLButtonElement>) => {
    const d = dragRef.current;
    if (d) setDrag({ ...d, over: rowUnderRef.current(e.clientY), dy: e.clientY - startY.current });
  }).current;
  const onHandleUp = useRef((e: ReactPointerEvent<HTMLButtonElement>) => {
    const d = dragRef.current;
    if (d) moveRef.current(d.from, rowUnderRef.current(e.clientY)); // where the pointer was let go, not the last move event
    setDrag(null);
  }).current;
  const onHandleCancel = useRef(() => setDrag(null)).current;
  const onHandleKey = useRef((e: ReactKeyboardEvent<HTMLButtonElement>, index: number) => {
    if (e.key === "ArrowUp") { e.preventDefault(); moveRef.current(index, index - 1); }
    else if (e.key === "ArrowDown") { e.preventDefault(); moveRef.current(index, index + 1); }
    else if (e.key === "Escape" && dragRef.current) { e.preventDefault(); e.stopPropagation(); setDrag(null); }
  }).current;
  // Stable per-row ref callbacks (a new function each render would make every row look changed).
  const rowRefs = useRef(new Map<number, (el: HTMLLIElement | null) => void>());
  const rowRef = (index: number) => {
    let fn = rowRefs.current.get(index);
    if (!fn) {
      fn = (el) => { rows.current[index] = el; };
      rowRefs.current.set(index, fn);
    }
    return fn;
  };
  // When a row a few before the end of what is drawn is seen, draw the next batch.
  const total = upcoming.length;
  const lookahead = Math.max(0, Math.min(shown, total) - LOOKAHEAD_ROWS);
  const placeholder = useRef<HTMLLIElement>(null);
  useEffect(() => {
    if (shown >= total) return;
    // The placeholder at the end too: a jump straight to the bottom (End key, dragging the scrollbar) can pass the lookahead row
    // without it ever being seen, and the placeholder is then what comes into view.
    const els = [rows.current[lookahead], placeholder.current].filter((el): el is HTMLLIElement => !!el);
    if (els.length === 0) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) setShown((n) => Math.min(total, n + MORE_ROWS));
    });
    els.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [shown, total, lookahead, upcoming]); // upcoming: a reorder puts another row at the lookahead index
  const more = useLoadMore(total > 0 && total < QUEUE_CAP && next.state !== "finding", track?.providerTrackId, total, shown >= total);
  async function saveQueue() {
    if (!track) return;
    // Read before the first await: the song can change while the playlist is being created.
    const hits = [{ ...track, source: "YouTube Music" as const, artists: track.artists ?? [], artworkUrl: track.artworkUrl ?? null, durationMs: usePlayback.getState().durationMs || null }, ...upcoming.map(toHit)];
    setSaved("saving");
    try {
      const id = await syncCreateLocalPlaylist(`Queue · ${new Date().toLocaleDateString()}`, null);
      for (const hit of hits) await addToPlaylist(id, hit);
      setSaved("done");
    } catch { setSaved("failed"); }
  }

  if (!track) {
    return (
      <section aria-labelledby="queue-title" className="flex flex-col gap-4">
        <h2 id="queue-title" className="font-display text-[22px] leading-7 font-semibold tracking-[-0.015em]">Queue</h2>
        <EmptyState
          icon={<ListMusic size={24} />}
          title="Nothing in the queue"
          action={<Button variant="secondary" className="mt-1" onClick={() => { toggleQueue(); navigate("search"); }}>Find songs</Button>}
        >
          Play a song and it appears here.
        </EmptyState>
      </section>
    );
  }

  return (
    <section aria-labelledby="queue-title" className="flex flex-col gap-4">
      <header className="flex flex-col gap-0.5">
        <h2 id="queue-title" className="font-display text-[22px] leading-7 font-semibold tracking-[-0.015em]">Queue</h2>
        <p className="text-[13px] leading-[18px] text-secondary-text">
          Autoplay · {upcoming.length} songs up next. Select any song to continue from there.
        </p>
        <div className="mt-2"><VibePicker /></div>
        <Button variant="secondary" className="mt-2 self-start" disabled={saved === "saving"} onClick={() => void saveQueue()}>
          {saved === "saving" ? "Saving…" : saved === "done" ? "Saved to Playlists" : saved === "failed" ? "Could not save. Retry" : "Save queue as playlist"}
        </Button>
      </header>

      <section aria-labelledby="queue-now" className="flex flex-col gap-1">
        <h3 id="queue-now" className="px-2 text-[13px] leading-[18px] font-semibold text-secondary-text">Now playing</h3>
        <div className="grid h-14 grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 rounded-[8px] bg-lumen-wash px-2" aria-current="true">
          <TrackCover key={track.providerTrackId} url={info?.artworkUrl ?? track.artworkUrl} videoId={track.providerTrackId} className="size-10 rounded-[6px]" />
          <div className="min-w-0">
            <p className="truncate text-[13px] leading-[18px] font-semibold" title={title}>{title}</p>
            <p className="truncate text-[13px] leading-[18px] text-secondary-text" title={artists}>{artists || "YouTube Music"}</p>
          </div>
          <p className="text-right text-[12px] text-secondary-text">{playing ? "Playing" : playerState === "paused" ? "Paused" : ""}</p>
        </div>
      </section>

      <section aria-labelledby="queue-next" className="flex flex-col gap-1">
        <h3 id="queue-next" className="px-2 text-[13px] leading-[18px] font-semibold text-secondary-text">Up next</h3>
        {next.state === "finding" && <p role="status" className="flex items-center gap-2 px-2 py-2 text-[13px] text-secondary-text"><Loader2 size={16} className="lx-spin" aria-hidden="true" />Finding up to 50 related songs…</p>}
        {upcoming.length > 0 ? <ol ref={list} aria-label="Autoplay playlist" className="flex flex-col">
          {upcoming.slice(0, shown).map((item, index) => {
            const dragging = drag?.from === index;
            const target = drag && drag.over === index && drag.from !== index ? (drag.over < drag.from ? "top" as const : "bottom" as const) : null;
            return <QueueRow key={item.id} item={item} index={index} busy={busy} dragging={dragging} target={target} dy={dragging ? drag.dy : 0}
              status={index === 0 ? (NEXT_STATUS[next.state] ?? "Up next") : null} rowRef={rowRef(index)}
              onDown={onHandleDown} onMove={onHandleMove} onUp={onHandleUp} onCancel={onHandleCancel} onKey={onHandleKey} />;
          })}
          {shown < total && <PlaceholderRows liRef={placeholder} />}
          {shown >= total && total < QUEUE_CAP && <li ref={more.sentinel} className="flex h-12 items-center justify-center text-[12px] text-secondary-text"><span role="status" className="flex items-center gap-2"><Loader2 size={14} className="lx-spin" aria-hidden="true" />Loading more…</span></li>}
        </ol> : next.state !== "finding" && <p role="status" className="px-2 py-2 text-[13px] text-secondary-text">{NEXT_STATUS[next.state] ?? "Nothing after this song yet."}</p>}

      </section>
    </section>
  );
}

"use client";

import { GripVertical, ListMusic, Loader2, X } from "lucide-react";
import { playerQueueEdit } from "../../services/tauri/player";
import { attempt } from "../../lib/notice";
import { usePlayback, playTrack } from "../../lib/playback";
import { useUi } from "../../lib/store";
import { TrackCover } from "../../components/TrackCover";
import { LikeButton } from "../../components/LibraryActions";
import { formatDuration } from "../../lib/media";
import { Button, EmptyState, IconButton, cx } from "../../components/ui";
import { VibePicker } from "../../components/PlayModeButtons";
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { addToPlaylist } from "../../lib/playlistActions";
import { syncCreateLocalPlaylist } from "../../services/tauri/sync";

const NEXT_STATUS: Record<string, string> = {
  ready: "Ready to play after this song",
  preparing: "Preparing the audio",
  finding: "Finding related music",
  unavailable: "No related track could be prepared",
};

export default function NativeQueue() {
  const s = usePlayback();
  const navigate = useUi((u) => u.navigate);
  const toggleQueue = useUi((u) => u.toggleQueue);
  const track = s.requested ?? s.track;
  const info = s.info?.providerTrackId === track?.providerTrackId ? s.info : null;
  const title = info?.title ?? track?.title ?? "YouTube";
  const artists = (info?.artists ?? track?.artists)?.join(", ") ?? "";
  const playing = s.player.state === "playing" || s.player.state === "buffering";
  const next = s.autoplay;
  const upcoming = next.queue ?? [];
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
  function rowUnder(clientY: number): number {
    const ol = list.current;
    if (!ol) return 0;
    const y = clientY - ol.getBoundingClientRect().top + ol.offsetTop; // same offset parent as the rows
    let over = 0;
    rows.current.forEach((li, i) => {
      if (li && y >= li.offsetTop) over = i;
    });
    return Math.min(over, upcoming.length - 1);
  }
  function onHandleDown(e: ReactPointerEvent<HTMLButtonElement>, from: number) {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    startY.current = e.clientY;
    setDrag({ from, over: from, dy: 0 });
  }
  function onHandleMove(e: ReactPointerEvent<HTMLButtonElement>) {
    if (drag) setDrag({ ...drag, over: rowUnder(e.clientY), dy: e.clientY - startY.current });
  }
  function onHandleUp(e: ReactPointerEvent<HTMLButtonElement>) {
    if (drag) move(drag.from, rowUnder(e.clientY)); // where the pointer was let go, not the last move event
    setDrag(null);
  }
  function onHandleKey(e: ReactKeyboardEvent<HTMLButtonElement>, index: number) {
    if (e.key === "ArrowUp") { e.preventDefault(); move(index, index - 1); }
    else if (e.key === "ArrowDown") { e.preventDefault(); move(index, index + 1); }
    else if (e.key === "Escape" && drag) { e.preventDefault(); e.stopPropagation(); setDrag(null); }
  }
  const toHit = (item: (typeof upcoming)[number]) => ({ provider: item.provider, source: "YouTube Music" as const, providerTrackId: item.id, canonicalUrl: `https://www.youtube.com/watch?v=${item.id}`, title: item.title, artists: item.artists, artworkUrl: item.artworkUrl, durationMs: item.durationMs });
  async function saveQueue() {
    if (!track) return;
    setSaved("saving");
    try {
      const id = await syncCreateLocalPlaylist(`Queue · ${new Date().toLocaleDateString()}`, null);
      for (const hit of [{ ...track, source: "YouTube Music" as const, artists: track.artists ?? [], artworkUrl: track.artworkUrl ?? null, durationMs: s.durationMs || null }, ...upcoming.map(toHit)]) await addToPlaylist(id, hit);
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
          <p className="text-right text-[12px] text-secondary-text">{playing ? "Playing" : s.player.state === "paused" ? "Paused" : ""}</p>
        </div>
      </section>

      <section aria-labelledby="queue-next" className="flex flex-col gap-1">
        <h3 id="queue-next" className="px-2 text-[13px] leading-[18px] font-semibold text-secondary-text">Up next</h3>
        {next.state === "finding" && <p role="status" className="flex items-center gap-2 px-2 py-2 text-[13px] text-secondary-text"><Loader2 size={16} className="lx-spin" aria-hidden="true" />Finding up to 50 related songs…</p>}
        {upcoming.length > 0 ? <ol ref={list} aria-label="Autoplay playlist" className="flex flex-col">
          {upcoming.map((item, index) => {
            const hit = { provider: item.provider, source: "YouTube Music" as const, providerTrackId: item.id, canonicalUrl: `https://www.youtube.com/watch?v=${item.id}`, title: item.title, artists: item.artists, artworkUrl: item.artworkUrl, durationMs: item.durationMs };
            const dragging = drag?.from === index;
            const target = drag && drag.over === index && drag.from !== index ? (drag.over < drag.from ? "top" : "bottom") : null;
            return <li key={item.id} ref={(el) => { rows.current[index] = el; }}
              style={dragging ? { transform: `translateY(${drag.dy}px)` } : undefined}
              className={cx("group/row grid min-h-16 grid-cols-[24px_40px_minmax(0,1fr)_auto] items-center gap-2 rounded-lg px-2 hover:bg-frost-200 focus-within:bg-frost-200",
                dragging && "relative z-10 bg-frost-300 opacity-90 shadow-[0_8px_24px_rgba(0,0,0,0.5)]",
                target === "top" && "shadow-[inset_0_2px_0_var(--color-lumen-lavender)]", target === "bottom" && "shadow-[inset_0_-2px_0_var(--color-lumen-lavender)]")}>
              <button type="button" aria-label={`Move ${item.title}. Drag it, or press the up and down arrow keys.`}
                onPointerDown={(e) => onHandleDown(e, index)} onPointerMove={onHandleMove} onPointerUp={onHandleUp} onPointerCancel={() => setDrag(null)}
                onKeyDown={(e) => onHandleKey(e, index)}
                className="group/h flex size-6 cursor-grab touch-none items-center justify-center rounded text-secondary-text select-none active:cursor-grabbing">
                <span className="text-[12px] tabular-nums group-hover/row:hidden group-focus-visible/h:hidden">{index + 1}</span>
                <GripVertical size={14} aria-hidden="true" className="hidden group-hover/row:block group-focus-visible/h:block" />
              </button>
              <button type="button" disabled={s.busy} onClick={() => void playTrack(hit, true)} aria-label={`Play ${item.title} from autoplay queue`} className="rounded-md disabled:opacity-40"><TrackCover url={item.artworkUrl} videoId={item.id} className="size-10" /></button>
              <button type="button" disabled={s.busy} onClick={() => void playTrack(hit, true)} className="min-w-0 rounded-md text-left disabled:opacity-40">
                <p className="truncate text-[13px] font-semibold" title={item.title}>{item.title}</p>
                <p className="truncate text-[12px] text-secondary-text">{item.artists.join(", ")} · YouTube Music</p>
                {index === 0 && <p role="status" className="text-[11px] text-secondary-text">{NEXT_STATUS[next.state] ?? "Up next"}</p>}
              </button>
              <div className="flex items-center gap-1">
                <span className="text-[11px] tabular-nums text-secondary-text">{item.durationMs != null ? formatDuration(item.durationMs / 1000) : ""}</span>
                <IconButton label={`Remove ${item.title} from the queue`} onClick={() => attempt(playerQueueEdit({ op: "remove", id: item.id }), "Could not remove the song from the queue.")} className="size-7"><X size={14} /></IconButton>
                <LikeButton track={hit} />
              </div>
            </li>;
          })}
        </ol> : next.state !== "finding" && <p role="status" className="px-2 py-2 text-[13px] text-secondary-text">{NEXT_STATUS[next.state] ?? "Nothing after this song yet."}</p>}

      </section>
    </section>
  );
}

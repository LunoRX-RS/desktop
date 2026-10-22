"use client";

import { ListMusic, Loader2 } from "lucide-react";
import { usePlayback, playTrack } from "../../lib/playback";
import { useUi } from "../../lib/store";
import { TrackCover } from "../../components/TrackCover";
import { LikeButton } from "../../components/LibraryActions";
import { formatDuration } from "../../lib/demo-data";
import { Button, EmptyState } from "../../components/ui";

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
        {upcoming.length > 0 ? <ol aria-label="Autoplay playlist" className="flex flex-col">
          {upcoming.map((item, index) => {
            const hit = { provider: item.provider, source: "YouTube Music" as const, providerTrackId: item.id, canonicalUrl: `https://www.youtube.com/watch?v=${item.id}`, title: item.title, artists: item.artists, artworkUrl: item.artworkUrl, durationMs: item.durationMs };
            return <li key={item.id} className="grid min-h-16 grid-cols-[24px_40px_minmax(0,1fr)_auto] items-center gap-2 rounded-lg px-2 hover:bg-frost-200 focus-within:bg-frost-200">
              <span className="text-[12px] tabular-nums text-secondary-text">{index + 1}</span>
              <button type="button" disabled={s.busy} onClick={() => void playTrack(hit, true)} aria-label={`Play ${item.title} from autoplay queue`} className="rounded-md disabled:opacity-40"><TrackCover url={item.artworkUrl} videoId={item.id} className="size-10" /></button>
              <button type="button" disabled={s.busy} onClick={() => void playTrack(hit, true)} className="min-w-0 rounded-md text-left disabled:opacity-40">
                <p className="truncate text-[13px] font-semibold" title={item.title}>{item.title}</p>
                <p className="truncate text-[12px] text-secondary-text">{item.artists.join(", ")} · YouTube Music</p>
                {index === 0 && <p role="status" className="text-[11px] text-secondary-text">{NEXT_STATUS[next.state] ?? "Up next"}</p>}
              </button>
              <div className="flex items-center gap-1"><span className="text-[11px] tabular-nums text-secondary-text">{item.durationMs != null ? formatDuration(item.durationMs / 1000) : ""}</span><LikeButton track={hit} /></div>
            </li>;
          })}
        </ol> : next.state !== "finding" && <p role="status" className="px-2 py-2 text-[13px] text-secondary-text">{NEXT_STATUS[next.state] ?? "Nothing after this song yet."}</p>}

      </section>
    </section>
  );
}

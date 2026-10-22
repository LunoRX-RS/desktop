"use client";

import { ChevronLeft, Heart, ListMusic, Pause, Play, Shuffle } from "lucide-react";
import { useUi } from "../../lib/store";
import { MIX_TRACKS, SAMPLE_ALBUMS, SAMPLE_MIXES, SAMPLE_TRACKS, providerSummary, type SampleTrack, type Tone } from "../../lib/demo-data";
import { Art } from "../../components/Art";
import { SongTable, trackById, usePlayTrack } from "../../components/Tracks";
import { PopOnTrue } from "../../components/Motion";
import { Button, EmptyState, IconButton, Skeleton, cx, useMounted } from "../../components/ui";

type Found = { title: string; by: string; seed: string; tone: Tone; tracks: SampleTrack[]; kind: string; year?: number };

// Album / playlist details. Sample data only; Play is simulated and no audio plays.
export default function DetailsScreen() {
  const ready = useMounted();
  const detail = useUi((s) => s.detail);
  const navigate = useUi((s) => s.navigate);
  const goBack = useUi((s) => s.goBack);
  const canGoBack = useUi((s) => s.back.length > 0);
  const liked = useUi((s) => s.liked);
  const toggleLike = useUi((s) => s.toggleLike);
  const shuffle = useUi((s) => s.shuffle);
  const toggleShuffle = useUi((s) => s.toggleShuffle);
  const playIndex = useUi((s) => s.playIndex);
  const queue = useUi((s) => s.queue);
  const p = usePlayTrack();

  let found: Found | null = null;
  if (detail?.kind === "album") {
    const a = SAMPLE_ALBUMS.find((x) => x.id === detail.id);
    if (a) found = { title: a.title, by: a.artist, seed: a.title, tone: a.tone, kind: "Album", year: a.year, tracks: SAMPLE_TRACKS.filter((t) => t.album === a.title) };
  } else if (detail?.kind === "playlist") {
    const m = SAMPLE_MIXES.find((x) => x.id === detail.id);
    if (m)
      found = {
        title: m.title.replace("Sample Mix: ", ""),
        by: m.blurb,
        seed: m.title,
        tone: m.tone,
        kind: "Playlist",
        tracks: (MIX_TRACKS[m.id] ?? []).map(trackById).filter((t): t is SampleTrack => t !== undefined),
      };
  }

  if (!found) {
    return (
      <section aria-labelledby="details-title" className="flex flex-col gap-8">
        <h1 id="details-title" className="font-display text-[34px] leading-[40px] font-bold tracking-[-0.022em]">
          Not found
        </h1>
        <EmptyState
          icon={<ListMusic size={24} />}
          title="This item is not available"
          action={
            <Button variant="secondary" onClick={() => navigate("home")} className="mt-2">
              Back to Listen Now
            </Button>
          }
        >
          It may have been removed from the sample data.
        </EmptyState>
      </section>
    );
  }

  const ids = found.tracks.map((t) => t.id);
  const empty = ids.length === 0;
  const playing = ids.some((id) => p.isPlaying(id));
  const allLiked = !empty && ids.every((id) => liked[id]);
  const summary = providerSummary(found.tracks);
  const minutes = Math.max(1, Math.round(found.tracks.reduce((n, t) => n + t.durationSec, 0) / 60));
  const meta = [
    found.kind,
    found.year ? String(found.year) : null,
    `${ids.length} ${ids.length === 1 ? "song" : "songs"}`,
    empty ? null : `${minutes} min`,
    summary || null,
  ]
    .filter(Boolean)
    .join(" · ");

  function shuffleAll() {
    if (!shuffle) toggleShuffle();
    const pick = ids[Math.floor(Math.random() * ids.length)];
    playIndex(Math.max(0, queue.findIndex((t) => t.id === pick)));
  }
  function likeAll() {
    for (const id of ids) if (allLiked || !liked[id]) toggleLike(id);
  }

  return (
    <section aria-labelledby="details-title" className="flex flex-col gap-8">
      {canGoBack && (
        <div className="-mb-4">
          <Button variant="ghost" onClick={goBack} aria-label="Go back" className="-ml-2 h-8 gap-1 pr-3 pl-2">
            <ChevronLeft size={16} aria-hidden="true" />
            Back
          </Button>
        </div>
      )}
      {!ready ? (
        <div aria-hidden="true" className="flex flex-col gap-8 sm:flex-row sm:items-end">
          <Skeleton className="size-[240px] shrink-0 rounded-[10px]" />
          <div className="flex flex-1 flex-col gap-3">
            <Skeleton className="h-10 w-2/3" />
            <Skeleton className="h-6 w-1/3" />
            <Skeleton className="h-11 w-32 rounded-[12px]" />
          </div>
        </div>
      ) : (
        <header className="flex flex-col gap-8 sm:flex-row sm:items-end">
          <Art seed={found.seed} tone={found.tone} rounded="rounded-[10px]" className="size-[240px] shrink-0 shadow-[0_8px_28px_rgba(0,0,0,0.5)]" />
          <div className="flex min-w-0 flex-1 flex-col gap-1 pb-1">
            <h1 id="details-title" title={found.title} className="line-clamp-2 font-display text-[34px] leading-[40px] font-bold tracking-[-0.022em] [overflow-wrap:anywhere]">
              {found.title}
            </h1>
            <p className="truncate text-[17px] leading-6 text-secondary-text" title={found.by}>
              {found.by}
            </p>
            <p className="truncate text-[13px] leading-[18px] text-secondary-text" title={meta}>
              {meta}
            </p>
            <p className="text-[13px] leading-[18px] text-secondary-text">Demo content: sample songs, no audio.</p>
            <div className="mt-4 flex flex-wrap items-center gap-3">
              <Button
                variant="primary"
                disabled={empty}
                onClick={() => p.startList(ids)}
                aria-label={`${playing ? "Pause" : "Play"} ${found.title}, demo, no audio`}
                className="h-11 gap-2 rounded-[12px] px-6"
              >
                {playing ? <Pause size={16} fill="currentColor" aria-hidden="true" /> : <Play size={16} fill="currentColor" aria-hidden="true" />}
                {playing ? "Pause" : "Play"}
              </Button>
              <IconButton label="Shuffle (demo)" disabled={empty} onClick={shuffleAll} className={cx("size-11 rounded-[12px] bg-frost-200 hover:bg-frost-300")}>
                <Shuffle size={18} aria-hidden="true" />
              </IconButton>
              <IconButton
                label={allLiked ? "Remove from library" : "Add to library"}
                aria-pressed={allLiked}
                active={allLiked}
                disabled={empty}
                onClick={likeAll}
                className="size-11 rounded-[12px] bg-frost-200 hover:bg-frost-300"
              >
                <PopOnTrue on={allLiked}>
                  <Heart size={18} fill={allLiked ? "currentColor" : "none"} aria-hidden="true" />
                </PopOnTrue>
              </IconButton>
            </div>
          </div>
        </header>
      )}
      {empty ? (
        <EmptyState icon={<ListMusic size={24} />} title="No songs yet">
          This {found.kind.toLowerCase()} has no songs. Add songs from Search and they will show up here.
        </EmptyState>
      ) : (
        <div className="glass-card rounded-[12px] px-2 pb-2">
          <SongTable tracks={found.tracks} label={`${found.title}, sample songs`} />
        </div>
      )}
    </section>
  );
}

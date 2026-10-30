"use client";

import { MicVocal } from "lucide-react";
import { ArtistLinks, LikeButton } from "../LibraryActions";
import { TrackCover } from "../TrackCover";
import { Button, cx } from "../ui";
import { usePlayback } from "../../lib/playback";
import { useUi } from "../../lib/store";

/** Only this part follows the playback position (four updates a second); the cover and title above it do not redraw. */
function LyricWindow({ lines, synced, plain }: { lines: { startMs: number; text: string }[] | null; synced: boolean; plain: string | null }) {
  const position = usePlayback((s) => s.positionMs);
  if (synced && lines && lines.length > 0) {
    let at = -1;
    for (let i = 0; i < lines.length && lines[i].startMs <= position; i++) at = i;
    const from = Math.max(0, at - 1);
    const shown = lines.slice(from, from + 4);
    return (
      <ol aria-label="Lyrics" className="flex flex-col gap-1.5">
        {shown.map((line, k) => {
          const current = from + k === at;
          return (
            <li key={line.startMs} aria-current={current ? "true" : undefined}
              className={cx("font-display text-[17px] leading-[24px] font-bold tracking-[-0.01em] transition-opacity duration-300", current ? "text-ink-bright [text-shadow:0_0_22px_rgba(255,255,255,0.3)]" : "opacity-40")}>
              {line.text}
            </li>
          );
        })}
      </ol>
    );
  }
  if (plain) return <p className="line-clamp-4 whitespace-pre-line text-[14px] leading-5 text-secondary-text">{plain}</p>;
  return <p className="text-[13px] leading-[18px] text-secondary-text">No lyrics for this song.</p>;
}

/** The right dock's first tab: what is playing, big, with the lines being sung. The full lyrics view is one press away. */
export function NowPanel() {
  const track = usePlayback((s) => s.requested ?? s.track);
  const info = usePlayback((s) => s.info);
  const matched = info && track && info.providerTrackId === track.providerTrackId ? info : null;
  if (!track) {
    return <p className="py-6 text-[13px] leading-[18px] text-secondary-text">Nothing is playing. Pick a song and it shows up here.</p>;
  }
  const title = matched?.title ?? track.title;
  const artists = matched?.artists ?? track.artists ?? [];
  const artwork = matched?.artworkUrl ?? track.artworkUrl;
  const like = { provider: track.provider, providerTrackId: track.providerTrackId, canonicalUrl: track.canonicalUrl, title, artists, artistRefs: matched?.artistRefs ?? track.artistRefs ?? [], artworkUrl: artwork ?? null, durationMs: null };
  return (
    <div className="flex flex-col gap-5">
      <TrackCover url={artwork} videoId={track.providerTrackId} title={title} className="aspect-square w-full rounded-[10px] shadow-[0_18px_40px_rgba(0,0,0,0.5)]" />
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <h2 className="line-clamp-2 font-display text-[22px] leading-7 font-bold tracking-[-0.022em]" title={title}>{title}</h2>
          <p className="truncate text-[14px] leading-5 text-secondary-text">{artists.length > 0 ? <ArtistLinks track={like} /> : "Artist not provided"}</p>
        </div>
        <LikeButton track={like} />
      </div>
      <section aria-label="Lyrics" className="flex flex-col gap-3 rounded-[10px] bg-frost-100 p-4 shadow-[inset_0_0_0_1px_rgba(255,255,255,0.06)]">
        <LyricWindow lines={matched?.lyricsLines ?? null} synced={matched?.lyricsSynced ?? false} plain={matched?.lyrics ?? null} />
        <Button variant="secondary" className="self-start" onClick={() => { useUi.getState().setLyrics(true); useUi.getState().setNowPlaying(true); }}>
          <MicVocal size={14} aria-hidden="true" />Full lyrics
        </Button>
      </section>
    </div>
  );
}

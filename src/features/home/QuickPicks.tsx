"use client";
import { Play } from "lucide-react";
import type { ReactNode } from "react";
import { Section, useMounted } from "../../components/ui";
import { formatDuration } from "../../lib/media";
import { TrackCover } from "../../components/TrackCover";
import { ArtistLinks } from "../../components/LibraryActions";
import { playTrack, resumeLast, usePlayback } from "../../lib/playback";
import type { SearchHitDto } from "../../services/tauri/search";

/** The songs you played last, one tap away: a compact list that sits beside the featured song. Playing one queues the ones after it. */
export function QuickPicks({ id, title, hits, action }: { id: string; title: string; hits: SearchHitDto[]; action?: ReactNode }) {
  const busy = usePlayback((s) => s.busy);
  // A song left part-way (saved at the last quit, nothing loaded yet): its row says how much is left and plays from that minute.
  const resume = usePlayback((s) => s.resume);
  const mounted = useMounted();
  return (
    <Section id={id} title={title} action={action}>
      <ul aria-label={title} className="flex flex-col gap-2">
        {hits.map((hit, i) => {
          const left = resume && resume.track.providerTrackId === hit.providerTrackId && resume.positionMs > 5000 && resume.durationMs > resume.positionMs
            ? { remaining: formatDuration((resume.durationMs - resume.positionMs) / 1000), pct: (resume.positionMs / resume.durationMs) * 100 } : null;
          return (
          <li key={`${hit.provider}:${hit.providerTrackId}`} className="group relative flex items-center gap-3 rounded-[10px] bg-frost-100 pr-3 shadow-[inset_0_0_0_1px_rgba(255,255,255,0.06)] hover:bg-frost-200">
            <TrackCover url={hit.artworkUrl} videoId={hit.providerTrackId} className="size-14 rounded-l-[10px] rounded-r-[4px]" />
            <div className="pointer-events-none relative z-10 min-w-0 flex-1 [&_button]:pointer-events-auto">
              <p className="truncate text-[13px] leading-[18px] font-semibold" title={hit.title}>{hit.title}</p>
              <p className="truncate text-[12px] leading-4 text-secondary-text">{hit.artists.length > 0 ? <ArtistLinks track={hit} /> : "Artist not provided"}</p>
            </div>
            {left && <span className="relative z-10 pointer-events-none shrink-0 text-[11px] leading-4 tabular-nums text-secondary-text group-focus-within:hidden group-hover:hidden">{left.remaining} left</span>}
            <span aria-hidden="true" className={`pointer-events-none relative z-10 size-8 shrink-0 items-center justify-center rounded-full bg-primary-text text-on-ink ${left ? "hidden group-focus-within:flex group-hover:flex" : "flex opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"}`}>
              <Play size={14} fill="currentColor" className="translate-x-[0.5px]" />
            </span>
            {left && mounted && (
              <span aria-hidden="true" className="pointer-events-none absolute inset-x-3 bottom-0 h-0.5 overflow-hidden rounded-full bg-frost-300">
                <span className="block h-full rounded-full bg-secondary-text" style={{ width: `${left.pct}%` }} />
              </span>
            )}
            {/* The row is one big button laid under the artist links, so the links stay separate focus targets. */}
            <button type="button" disabled={busy} aria-label={left ? `Continue ${hit.title}, ${left.remaining} left` : `Play ${hit.title}`} title={left ? "Continue" : "Play"} onClick={() => void (left ? resumeLast(hits.slice(i + 1)) : playTrack(hit, false, hits.slice(i + 1)))}
              className="absolute inset-0 rounded-[10px] disabled:opacity-60" />
          </li>
          );
        })}
      </ul>
    </Section>
  );
}

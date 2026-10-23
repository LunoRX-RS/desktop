"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Play } from "lucide-react";
import { TrackCover } from "./TrackCover";
import { ArtistLinks } from "./LibraryActions";
import { playTrack, usePlayback } from "../lib/playback";
import type { SearchHitDto } from "../services/tauri/search";
import { cx } from "./ui";

/**
 * Songs in one sideways row, like the shelves of YouTube Music: a cover tile per song with the title and artist under it. The
 * row snaps tile by tile, and arrows at its edges (shown on hover or keyboard focus, only while there is more to see) page
 * through it. Playing a tile queues the songs after it, as the list rows do.
 */
export function TrackRail({ label, hits }: { label: string; hits: SearchHitDto[] }) {
	const scroller = useRef<HTMLDivElement>(null);
	const busy = usePlayback((s) => s.busy);
	const [more, setMore] = useState({ left: false, right: false });

	const measure = useCallback(() => {
		const el = scroller.current;
		if (!el) return;
		setMore({ left: el.scrollLeft > 1, right: el.scrollLeft + el.clientWidth < el.scrollWidth - 1 });
	}, []);

	useEffect(() => {
		measure();
		const el = scroller.current;
		if (!el) return;
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		return () => observer.disconnect();
	}, [measure, hits.length]);

	function page(direction: 1 | -1) {
		const el = scroller.current;
		if (!el) return;
		const calm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		el.scrollBy({ left: direction * el.clientWidth * 0.85, behavior: calm ? "auto" : "smooth" });
	}

	const arrow = "absolute top-[84px] z-10 flex size-9 -translate-y-1/2 items-center justify-center rounded-full bg-black/70 text-white shadow-[0_2px_8px_rgba(0,0,0,0.5)] transition-opacity hover:bg-black/85 focus-visible:opacity-100";
	return (
		<div className="group/rail relative">
			<div ref={scroller} onScroll={measure} role="list" aria-label={label} className="lx-rail flex snap-x scroll-px-1 gap-4 overflow-x-auto px-1 pb-2">
				{hits.map((hit, i) => (
					<div key={`${hit.provider}:${hit.providerTrackId}`} role="listitem" className="lx-reveal-host flex w-[168px] shrink-0 snap-start flex-col gap-2 2xl:w-[188px]">
						<div className="relative aspect-square w-full">
							<TrackCover url={hit.artworkUrl} videoId={hit.providerTrackId} className="size-full rounded-[8px] shadow-[0_1px_4px_rgba(0,0,0,0.45)]" />
							<button
								type="button"
								disabled={busy}
								onClick={() => void playTrack(hit, false, hits.slice(i + 1))}
								aria-label={`Play ${hit.title}`}
								title="Play"
								className="lx-reveal absolute inset-0 flex items-center justify-center rounded-[8px] bg-black/50 text-white hover:bg-black/60 disabled:opacity-60"
							>
								<span className="flex size-11 items-center justify-center rounded-full bg-white/90 text-black">
									<Play size={18} fill="currentColor" className="translate-x-[1px]" aria-hidden="true" />
								</span>
							</button>
						</div>
						<div className="min-w-0 px-0.5">
							<p className="truncate text-[13px] leading-[18px] font-semibold" title={hit.title}>{hit.title}</p>
							<p className="truncate text-[12px] leading-4 text-secondary-text">
								{hit.artists.length > 0 ? <ArtistLinks track={hit} /> : "Artist not provided"}
							</p>
						</div>
					</div>
				))}
			</div>
			<button
				type="button"
				aria-label={`${label}: scroll left`}
				onClick={() => page(-1)}
				tabIndex={more.left ? 0 : -1}
				className={cx(arrow, "left-1", more.left ? "opacity-0 group-hover/rail:opacity-100" : "pointer-events-none opacity-0")}
			>
				<ChevronLeft size={20} aria-hidden="true" />
			</button>
			<button
				type="button"
				aria-label={`${label}: scroll right`}
				onClick={() => page(1)}
				tabIndex={more.right ? 0 : -1}
				className={cx(arrow, "right-1", more.right ? "opacity-0 group-hover/rail:opacity-100" : "pointer-events-none opacity-0")}
			>
				<ChevronRight size={20} aria-hidden="true" />
			</button>
		</div>
	);
}

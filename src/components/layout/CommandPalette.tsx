"use client";

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { BadgeCheck, Clock, Flame, Search, TrendingUp, User, X } from "lucide-react";
import { useUi, type View } from "../../lib/store";
import { useLocalPlaylists } from "../../features/playlists/usePlaylists";
import { cx } from "../ui";
import { DUR, tween } from "../../lib/motion";
import { TrackCover } from "../TrackCover";
import { useHistory } from "../../lib/history";
import { useFeed } from "../../lib/discovery";
import { playTrack } from "../../lib/playback";
import { searchSuggestions } from "../../services/tauri/discovery";
import { searchCancel, searchQuery, type SearchHitDto } from "../../services/tauri/search";
import { historyMatches, personalizeTerms, topArtists } from "../../lib/searchTaste";
import { currentBindings, formatCombo } from "../../lib/shortcuts";
import { detectPlatform } from "../../lib/platform";

type Section = "Search" | "Recent searches" | "For you" | "Trending" | "Suggestions" | "Songs" | "Go to";

type Entry = {
	key: string;
	section: Section;
	label: string;
	sub?: string;
	hint?: string;
	icon?: "search" | "clock" | "artist" | "trend";
	cover?: { url: string | null; videoId: string };
	official?: boolean;
	run: () => void;
	remove?: () => void;
};

// A short pause after the last key before asking YouTube Music, so fast typing sends one request, not one per letter.
const TYPING_PAUSE_MS = 150;
const SONGS_SHOWN = 4;

const ICONS = { search: Search, clock: Clock, artist: User, trend: TrendingUp } as const;

/** The top bar's search: earlier searches, suggested words, songs with covers, and the app's commands. */
export default function CommandPalette() {
	const s = useUi();
	const reduce = useReducedMotion();
	const { playlists } = useLocalPlaylists();
	const history = useHistory((h) => h.tracks);
	const trending = useFeed("explore");
	const [text, setText] = useState("");
	const [active, setActive] = useState(0);
	const [terms, setTerms] = useState<string[]>([]);
	const [songs, setSongs] = useState<SearchHitDto[]>([]);
	const listRef = useRef<HTMLUListElement>(null);
	const inputRef = useRef<HTMLInputElement>(null);

	const close = () => s.setPalette(false);
	const typed = text.trim();
	const artists = useMemo(() => topArtists(history, 4), [history]);

	// Suggested words and songs for what is typed. The song request is the one the Search page sends, so choosing "Search" after
	// it is answered from the core's memory.
	useEffect(() => {
		setTerms([]);
		setSongs([]);
		if (!s.paletteOpen || typed.length < 2 || typed.length > 200) return;
		let live = true;
		const id = crypto.randomUUID();
		let started = false;
		const timer = window.setTimeout(() => {
			started = true;
			searchSuggestions(typed)
				.then((values) => live && setTerms(personalizeTerms(values, artists.map((a) => a.name)).slice(0, 5)))
				.catch(() => {});
			searchQuery({ query: typed, providers: [], limit: 10, cancellationId: id })
				.then((result) => live && setSongs(result.hits.slice(0, SONGS_SHOWN)))
				.catch(() => {});
		}, TYPING_PAUSE_MS);
		return () => {
			live = false;
			window.clearTimeout(timer);
			if (started) void searchCancel(id).catch(() => {});
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [typed, s.paletteOpen]);

	const entries = useMemo<Entry[]>(() => {
		const go = (view: View) => () => {
			s.navigate(view);
			close();
		};
		const search = (q: string) => () => {
			s.searchFor(q);
			close();
		};
		const play = (hit: SearchHitDto, following: SearchHitDto[]) => () => {
			close();
			void playTrack(hit, false, following);
		};
		const platform = detectPlatform();
		const sidebarKey = formatCombo(currentBindings().toggleSidebar[0], platform);
		const commands: Entry[] = [
			{ key: "nav-home", label: "Go to Listen Now", run: go("home") },
			{ key: "nav-explore", label: "Go to Explore", run: go("explore") },
			{ key: "nav-search", label: "Go to Search", run: go("search") },
			{ key: "nav-library", label: "Go to Library", run: () => { s.openLibrary("playlists"); close(); } },
			{ key: "nav-create", label: "Create playlist", run: go("playlists") },
			{ key: "nav-connections", label: "Provider connections", run: go("connections") },
			{ key: "nav-accounts", label: "Accounts", run: go("accounts") },
			{ key: "nav-settings", label: "Settings", run: go("settings") },
			{ key: "nav-sync", label: "Sync", run: go("sync") },
			{ key: "nav-player", label: "Playback status", run: go("player") },
			{ key: "now", label: "Open Now Playing (demo)", hint: "Demo", run: () => { s.setNowPlaying(true); close(); } },
			{ key: "sidebar", label: "Toggle sidebar", hint: sidebarKey, run: () => { s.toggleSidebar(); close(); } },
			...playlists.map((p) => ({
				key: `pl-${p.id}`,
				label: `Open playlist: ${p.name}`,
				hint: `${p.entryCount} ${p.entryCount === 1 ? "entry" : "entries"}`,
				run: () => { s.setSelectedPlaylist(p.id); s.navigate("playlists"); close(); },
			})),
		].map((c) => ({ ...c, section: "Go to" as const }));

		const out: Entry[] = [];
		if (typed === "") {
			for (const r of s.recentSearches.slice(0, 5)) {
				out.push({ key: `h-${r}`, section: "Recent searches", label: r, icon: "clock", run: search(r), remove: () => s.removeRecentSearch(r) });
			}
			for (const a of artists) {
				out.push({ key: `a-${a.name}`, section: "For you", label: a.name, sub: `${a.plays} ${a.plays === 1 ? "play" : "plays"} lately`, icon: "artist", cover: { url: a.artworkUrl, videoId: a.videoId }, run: search(a.name) });
			}
			const chart = trending.sections.find((x) => x.tracks.length > 0);
			chart?.tracks.slice(0, 4).forEach((t, i, all) => {
				out.push({ key: `t-${t.providerTrackId}`, section: "Trending", label: t.title, sub: t.artists.join(", "), icon: "trend", cover: { url: t.artworkUrl, videoId: t.providerTrackId }, run: play(t, all.slice(i + 1)) });
			});
			return [...out, ...commands.slice(0, 4)];
		}
		out.push({ key: "search", section: "Search", label: `Search music for “${typed}”`, icon: "search", run: search(typed) });
		for (const r of historyMatches(s.recentSearches, typed, 3)) {
			out.push({ key: `h-${r}`, section: "Recent searches", label: r, icon: "clock", run: search(r), remove: () => s.removeRecentSearch(r) });
		}
		for (const t of terms) {
			if (t.toLowerCase() !== typed.toLowerCase()) out.push({ key: `s-${t}`, section: "Suggestions", label: t, icon: "search", run: search(t) });
		}
		songs.forEach((hit, i) => {
			out.push({ key: `song-${hit.providerTrackId}`, section: "Songs", label: hit.title, sub: hit.artists.join(", "), official: hit.official, cover: { url: hit.artworkUrl, videoId: hit.providerTrackId }, run: play(hit, songs.slice(i + 1)) });
		});
		const needle = typed.toLowerCase();
		return [...out, ...commands.filter((c) => c.label.toLowerCase().includes(needle)).slice(0, 4)];
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [playlists, typed, terms, songs, artists, trending.sections, s.recentSearches]);

	useEffect(() => {
		if (!s.paletteOpen) return;
		setText("");
		setActive(0);
		const id = window.setTimeout(() => inputRef.current?.focus(), 0);
		return () => window.clearTimeout(id);
	}, [s.paletteOpen]);

	useEffect(() => {
		setActive(0);
	}, [typed]);

	useEffect(() => {
		listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
	}, [active]);

	function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
		if (e.key === "ArrowDown") {
			e.preventDefault();
			setActive((i) => Math.min(i + 1, entries.length - 1));
		} else if (e.key === "ArrowUp") {
			e.preventDefault();
			setActive((i) => Math.max(i - 1, 0));
		} else if (e.key === "Enter") {
			e.preventDefault();
			entries[active]?.run();
		} else if (e.key === "Escape") {
			e.preventDefault();
			close();
		}
	}

	return (
		<AnimatePresence>
			{s.paletteOpen && (
				<motion.div
					key="palette"
					className="fixed inset-0 z-50 flex items-start justify-center bg-scrim pt-[10vh]"
					initial={reduce ? false : { opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0, transition: tween(DUR.exit) }}
					transition={tween(0.16)}
					onMouseDown={(e) => {
						if (e.target === e.currentTarget) close();
					}}
				>
					<motion.div
						role="dialog"
						aria-modal="true"
						aria-label="Quick search"
						initial={reduce ? false : { opacity: 0, scale: 0.98 }}
						animate={{ opacity: 1, scale: 1 }}
						exit={{ opacity: 0, scale: 0.98, transition: tween(DUR.exit) }}
						transition={tween(0.16)}
						className="glass-solid w-[min(680px,calc(100vw-32px))] overflow-hidden rounded-[12px] shadow-[0_24px_64px_rgba(0,0,0,0.55)]"
					>
						<div className="flex items-center gap-3 px-4 py-3">
							<Search size={18} aria-hidden="true" className="text-secondary-text" />
							<input
								ref={inputRef}
								value={text}
								onChange={(e) => setText(e.currentTarget.value)}
								onKeyDown={onKeyDown}
								role="combobox"
								aria-expanded="true"
								aria-controls="palette-list"
								aria-activedescendant={entries[active] ? `palette-${active}` : undefined}
								aria-label="Quick search"
								placeholder="Search songs, artists, or type a command"
								className="h-10 min-w-0 flex-1 bg-transparent text-[17px] text-primary-text outline-none placeholder:text-tertiary-text"
							/>
							<kbd className="rounded-[6px] bg-frost-200 px-1.5 py-0.5 text-[11px] text-secondary-text">Esc</kbd>
						</div>
						<ul id="palette-list" role="listbox" aria-label="Results" ref={listRef} className="max-h-[460px] overflow-y-auto border-t border-hairline p-2">
							{entries.length === 0 && <li className="px-3 py-6 text-center text-sm text-tertiary-text">No matches.</li>}
							{entries.map((c, i) => {
								const Icon = c.icon ? ICONS[c.icon] : null;
								return (
									<li key={c.key} role="presentation">
										{(i === 0 || entries[i - 1].section !== c.section) && c.section !== "Search" && (
											<p className="flex items-center gap-1.5 px-3 pt-3 pb-1 text-[11px] font-semibold text-secondary-text">
												{c.section === "For you" && <Flame size={12} aria-hidden="true" />}
												{c.section}
											</p>
										)}
										<div
											id={`palette-${i}`}
											role="option"
											aria-selected={i === active}
											data-index={i}
											onMouseEnter={() => setActive(i)}
											onClick={c.run}
											className={cx(
												"flex cursor-pointer items-center gap-3 rounded-[8px] px-3 text-[13px] text-primary-text",
												c.cover ? "h-[52px]" : "h-10",
												i === active && "bg-lumen-wash",
											)}
										>
											{c.cover ? (
												<TrackCover url={c.cover.url} videoId={c.cover.videoId} className={cx("size-10", c.icon === "artist" && "rounded-full")} />
											) : (
												Icon && <Icon size={16} aria-hidden="true" className="shrink-0 text-secondary-text" />
											)}
											<span className="min-w-0 flex-1">
												<span className="flex items-center gap-2">
													<span className="truncate">{c.label}</span>
													{c.official && (
														<span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-frost-300 px-2 py-0.5 text-[10px] font-medium">
															<BadgeCheck size={11} aria-hidden="true" />
															Official
														</span>
													)}
												</span>
												{c.sub && <span className="block truncate text-[12px] text-secondary-text">{c.sub}</span>}
											</span>
											{c.hint && <span className="shrink-0 text-[11px] tabular-nums text-secondary-text">{c.hint}</span>}
											{c.remove && (
												<button
													type="button"
													aria-label={`Remove ${c.label} from recent searches`}
													onClick={(e) => {
														e.stopPropagation();
														c.remove?.();
													}}
													className="flex size-6 shrink-0 items-center justify-center rounded-full text-secondary-text hover:bg-frost-200 hover:text-primary-text"
												>
													<X size={14} aria-hidden="true" />
												</button>
											)}
										</div>
									</li>
								);
							})}
						</ul>
					</motion.div>
				</motion.div>
			)}
		</AnimatePresence>
	);
}

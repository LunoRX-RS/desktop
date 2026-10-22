"use client";

import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { ChevronRight, Heart, LayoutGrid, List, ListMusic, Search, X } from "lucide-react";
import { useUi, type LibraryTab } from "../../lib/store";
import { SAMPLE_ALBUMS, SAMPLE_ARTISTS, SAMPLE_TRACKS, type SampleTrack, type Tone } from "../../lib/demo-data";
import { Art } from "../../components/Art";
import { MediaCard } from "../../components/MediaCard";
import { ActivePill, PopOnTrue } from "../../components/Motion";
import { SongTable, thumbSeed, usePlayTrack } from "../../components/Tracks";
import { Button, EmptyState, IconButton, Screen, Skeleton, StatusNote, cx } from "../../components/ui";
import { describeUpload, entriesLabel, errorText, useLocalPlaylists } from "../playlists/usePlaylists";

// Albums, Artists and Songs are sample data. Playlists are the user's real local playlists from the sync engine.
type SortKey = "added" | "name" | "artist";
type ViewMode = "grid" | "list";

const TABS: { key: LibraryTab; label: string }[] = [
  { key: "albums", label: "Albums" },
  { key: "artists", label: "Artists" },
  { key: "songs", label: "Songs" },
  { key: "playlists", label: "Playlists" },
];

const SORTS: Record<LibraryTab, { key: SortKey; label: string }[]> = {
  albums: [
    { key: "added", label: "Recently added" },
    { key: "name", label: "Name" },
    { key: "artist", label: "Artist" },
  ],
  artists: [
    { key: "added", label: "Recently added" },
    { key: "name", label: "Name" },
  ],
  songs: [
    { key: "added", label: "Recently added" },
    { key: "name", label: "Name" },
    { key: "artist", label: "Artist" },
  ],
  playlists: [
    { key: "added", label: "Recently added" },
    { key: "name", label: "Name" },
  ],
};

const GRID = "lx-stagger grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-x-5 gap-y-6";
const VIEW_KEY = "lunorx.library.view";
const byText = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });

type Props = {
  onOpenPlaylist: (id: string) => void;
};

// Segmented control with arrow-key switching (roving tabindex). Same look as the shared Segmented.
function Tabs({ value, onChange }: { value: LibraryTab; onChange: (t: LibraryTab) => void }) {
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});
  function onKeyDown(e: KeyboardEvent) {
    const i = TABS.findIndex((t) => t.key === value);
    const to =
      e.key === "ArrowRight" ? (i + 1) % TABS.length : e.key === "ArrowLeft" ? (i + TABS.length - 1) % TABS.length : e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : -1;
    if (to < 0) return;
    e.preventDefault();
    onChange(TABS[to].key);
    refs.current[TABS[to].key]?.focus();
  }
  return (
    <div role="tablist" aria-label="Library sections" onKeyDown={onKeyDown} className="inline-flex h-9 items-center gap-0.5 rounded-[10px] bg-frost-200 p-0.5">
      {TABS.map((t) => {
        const on = value === t.key;
        return (
          <button
            key={t.key}
            ref={(el) => {
              refs.current[t.key] = el;
            }}
            type="button"
            role="tab"
            id={`lib-tab-${t.key}`}
            aria-selected={on}
            aria-controls="lib-panel"
            tabIndex={on ? 0 : -1}
            onClick={() => onChange(t.key)}
            className={cx(
              "relative isolate h-8 rounded-[8px] px-3.5 text-[13px] font-medium transition-colors duration-150",
              on ? "text-primary-text" : "text-secondary-text hover:text-primary-text",
            )}
          >
            {on && <ActivePill layoutId="library-tab-pill" className="bg-frost-300 shadow-[0_1px_2px_rgba(0,0,0,0.3)]" />}
            {t.label}
          </button>
        );
      })}
    </div>
  );
}

// 56px list row. Interactive rows are one button, so nothing is nested.
function Row({
  seed,
  tone,
  round,
  title,
  subtitle,
  onClick,
  label,
}: {
  seed: string;
  tone: Tone;
  round?: boolean;
  title: string;
  subtitle: string;
  onClick?: () => void;
  label?: string;
}) {
  const body = (
    <>
      <Art seed={seed} tone={tone} grain={false} rounded={round ? "rounded-full" : "rounded-[6px]"} className="size-10 shrink-0" />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] leading-[18px] font-semibold" title={title}>
          {title}
        </span>
        <span className="block truncate text-[13px] leading-[18px] text-secondary-text" title={subtitle}>
          {subtitle}
        </span>
      </span>
    </>
  );
  const cls = "flex h-14 w-full items-center gap-3 rounded-[8px] px-2 text-left";
  return onClick ? (
    <li>
      <button type="button" onClick={onClick} aria-label={label} className={cx(cls, "transition-colors duration-150 hover:bg-frost-200")}>
        {body}
        <ChevronRight size={14} className="shrink-0 text-tertiary-text" aria-hidden="true" />
      </button>
    </li>
  ) : (
    <li className={cls}>{body}</li>
  );
}

function Loading({ mode }: { mode: ViewMode }) {
  return mode === "grid" ? (
    <div className={GRID} role="status" aria-label="Loading playlists">
      {Array.from({ length: 8 }, (_, i) => (
        <div key={i} className="flex flex-col gap-2">
          <Skeleton className="aspect-square w-full" />
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-4 w-1/2" />
        </div>
      ))}
    </div>
  ) : (
    <div className="flex flex-col" role="status" aria-label="Loading playlists">
      {Array.from({ length: 6 }, (_, i) => (
        <div key={i} className="flex h-14 items-center gap-3 px-2">
          <Skeleton className="size-10 shrink-0 rounded-[6px]" />
          <div className="flex flex-1 flex-col gap-1.5">
            <Skeleton className="h-3.5 w-1/3" />
            <Skeleton className="h-3.5 w-1/4" />
          </div>
        </div>
      ))}
    </div>
  );
}

export default function LibraryScreen({ onOpenPlaylist }: Props) {
  const tab = useUi((s) => s.libraryTab);
  const likedOnly = useUi((s) => s.likedOnly);
  const liked = useUi((s) => s.liked);
  const navigate = useUi((s) => s.navigate);
  const openDetail = useUi((s) => s.openDetail);
  const play = usePlayTrack();
  const { loading, playlists, error, reload } = useLocalPlaylists();

  const [sort, setSort] = useState<SortKey>("added");
  const [mode, setMode] = useState<ViewMode>("grid");
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLowerCase();

  // Per-viewer convenience only: the saved view comes back after mount so the static HTML stays the same.
  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(VIEW_KEY);
      if (saved === "grid" || saved === "list") setMode(saved);
    } catch {
      // Storage can be blocked; the default view is fine.
    }
  }, []);
  function chooseMode(m: ViewMode) {
    setMode(m);
    try {
      window.localStorage.setItem(VIEW_KEY, m);
    } catch {
      // Not saved; the choice still applies for this visit.
    }
  }

  const setTab = (t: LibraryTab) => useUi.setState({ libraryTab: t, likedOnly: false });
  const sorts = SORTS[tab];
  const sortKey = sorts.some((o) => o.key === sort) ? sort : "added";

  const albums = useMemo(() => {
    const l = SAMPLE_ALBUMS.filter((a) => `${a.title} ${a.artist}`.toLowerCase().includes(needle));
    if (sortKey === "name") l.sort((a, b) => byText(a.title, b.title));
    else if (sortKey === "artist") l.sort((a, b) => byText(a.artist, b.artist) || byText(a.title, b.title));
    else l.sort((a, b) => b.year - a.year);
    return l;
  }, [needle, sortKey]);

  const artists = useMemo(() => {
    const l = SAMPLE_ARTISTS.filter((a) => `${a.name} ${a.genre}`.toLowerCase().includes(needle));
    if (sortKey === "name") l.sort((a, b) => byText(a.name, b.name));
    return l;
  }, [needle, sortKey]);

  const songs = useMemo(() => {
    const l: SampleTrack[] = SAMPLE_TRACKS.filter(
      (t) => (!likedOnly || liked[t.id]) && (needle === "" || `${t.title} ${t.artist} ${t.album}`.toLowerCase().includes(needle)),
    );
    if (sortKey === "name") l.sort((a, b) => byText(a.title, b.title));
    else if (sortKey === "artist") l.sort((a, b) => byText(a.artist, b.artist) || byText(a.title, b.title));
    return l;
  }, [needle, sortKey, likedOnly, liked]);

  // The engine returns playlists in the order they were made, so "Recently added" is that order reversed.
  const shownPlaylists = useMemo(() => {
    const l = playlists.filter((p) => p.name.toLowerCase().includes(needle));
    if (sortKey === "name") l.sort((a, b) => byText(a.name, b.name));
    else l.reverse();
    return l;
  }, [playlists, needle, sortKey]);

  const isSample = tab !== "playlists";
  const filterText = filter.trim();
  const noMatch = (what: string): ReactNode => (
    <EmptyState
      icon={<Search size={24} />}
      title={`No ${what} match “${filterText}”`}
      action={
        <Button variant="secondary" onClick={() => setFilter("")} className="mt-1">
          Clear filter
        </Button>
      }
    />
  );

  let panel: ReactNode;
  if (tab === "playlists") {
    if (loading && playlists.length === 0) panel = <Loading mode={mode} />;
    else if (error) {
      panel = (
        <StatusNote role="alert" tone="coral">
          <p>{errorText(error)}</p>
          <Button variant="secondary" onClick={reload} className="mt-2">
            Retry
          </Button>
        </StatusNote>
      );
    } else if (playlists.length === 0) {
      panel = (
        <EmptyState
          icon={<ListMusic size={24} />}
          title="No playlists yet"
          action={
            <Button variant="primary" onClick={() => navigate("playlists")} className="mt-1">
              Create your first playlist
            </Button>
          }
        >
          Playlists are saved on this device first. Add tracks from Search.
        </EmptyState>
      );
    } else if (shownPlaylists.length === 0) panel = noMatch("playlists");
    else if (mode === "grid") {
      panel = (
        <div className={GRID}>
          {shownPlaylists.map((p) => (
            <MediaCard
              key={p.id}
              seed={p.id}
              tone="ink"
              title={p.name}
              subtitle={`${entriesLabel(p.entryCount)} · ${describeUpload(p)}`}
              onOpen={() => onOpenPlaylist(p.id)}
              openLabel={`Open playlist ${p.name}`}
            />
          ))}
        </div>
      );
    } else {
      panel = (
        <ul className="lx-stagger flex flex-col" aria-label="Playlists">
          {shownPlaylists.map((p) => (
            <Row
              key={p.id}
              seed={p.id}
              tone="ink"
              title={p.name}
              subtitle={`${entriesLabel(p.entryCount)} · ${describeUpload(p)}`}
              onClick={() => onOpenPlaylist(p.id)}
              label={`Open playlist ${p.name}`}
            />
          ))}
        </ul>
      );
    }
  } else if (tab === "albums") {
    if (albums.length === 0) panel = noMatch("albums");
    else if (mode === "grid") {
      panel = (
        <div className={GRID}>
          {albums.map((a) => (
            <MediaCard
              key={a.id}
              seed={a.title}
              tone={a.tone}
              title={a.title}
              subtitle={`${a.artist} · ${a.trackCount} songs`}
              onOpen={() => openDetail("album", a.id)}
              openLabel={`Open album ${a.title} by ${a.artist}`}
            />
          ))}
        </div>
      );
    } else {
      panel = (
        <ul className="lx-stagger flex flex-col" aria-label="Sample albums">
          {albums.map((a) => (
            <Row
              key={a.id}
              seed={a.title}
              tone={a.tone}
              title={a.title}
              subtitle={`${a.artist} · ${a.year} · ${a.trackCount} songs`}
              onClick={() => openDetail("album", a.id)}
              label={`Open album ${a.title} by ${a.artist}`}
            />
          ))}
        </ul>
      );
    }
  } else if (tab === "artists") {
    if (artists.length === 0) panel = noMatch("artists");
    else if (mode === "grid") {
      panel = (
        <div className={GRID}>
          {artists.map((a) => (
            <MediaCard key={a.id} round seed={a.name} tone={a.tone} title={a.name} subtitle={a.genre} />
          ))}
        </div>
      );
    } else {
      panel = (
        <ul className="lx-stagger flex flex-col" aria-label="Sample artists">
          {artists.map((a) => (
            <Row key={a.id} round seed={a.name} tone={a.tone} title={a.name} subtitle={a.genre} />
          ))}
        </ul>
      );
    }
  } else if (songs.length === 0) {
    panel =
      filterText !== "" ? (
        noMatch("songs")
      ) : (
        <EmptyState
          icon={<Heart size={24} />}
          title="No liked songs yet"
          action={
            <Button variant="secondary" onClick={() => useUi.setState({ likedOnly: false })} className="mt-1">
              Show all songs
            </Button>
          }
        >
          Like a sample song with the heart in the player bar. Likes stay in this window.
        </EmptyState>
      );
  } else if (mode === "grid") {
    panel = (
      <div className={GRID}>
        {songs.map((t) => (
          <MediaCard
            key={t.id}
            seed={thumbSeed(t)}
            tone={t.tone}
            title={t.title}
            subtitle={t.artist}
            onPlay={() => play.activate(t.id)}
            playLabel={`${play.isPlaying(t.id) ? "Pause" : "Play"} ${t.title} by ${t.artist}, demo, no audio`}
          />
        ))}
      </div>
    );
  } else {
    panel = <SongTable tracks={songs} label={likedOnly ? "Liked songs (sample)" : "Songs (sample)"} />;
  }

  return (
    <Screen
      id="library-title"
      title={likedOnly ? "Liked Songs" : "Library"}
      caption={isSample ? "Demo: albums, artists and songs are sample data. Only playlists are yours." : undefined}
    >
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <Tabs value={tab} onChange={setTab} />
          {tab === "songs" && (
            <Button variant="secondary" aria-pressed={likedOnly} onClick={() => useUi.setState({ likedOnly: !likedOnly })} className={cx(likedOnly && "bg-frost-300")}>
              <PopOnTrue on={likedOnly}>
                <Heart size={14} fill={likedOnly ? "currentColor" : "none"} aria-hidden="true" />
              </PopOnTrue>
              Liked songs
            </Button>
          )}
          <div className="flex-1" />
          <div className="flex h-8 w-[220px] items-center gap-2 rounded-[8px] bg-frost-200 pr-1 pl-2.5 text-secondary-text focus-within:bg-frost-300">
            <Search size={14} className="shrink-0" aria-hidden="true" />
            <label htmlFor="library-filter" className="sr-only">
              Filter {TABS.find((t) => t.key === tab)?.label.toLowerCase()}
            </label>
            <input
              id="library-filter"
              value={filter}
              onChange={(e) => setFilter(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Escape" && setFilter("")}
              placeholder="Filter"
              autoComplete="off"
              className="h-8 min-w-0 flex-1 bg-transparent text-[13px] text-primary-text outline-none placeholder:text-tertiary-text"
            />
            {filter !== "" && (
              <IconButton label="Clear filter" onClick={() => setFilter("")} className="size-6">
                <X size={13} />
              </IconButton>
            )}
          </div>
          <label htmlFor="library-sort" className="sr-only">
            Sort by
          </label>
          <select
            id="library-sort"
            value={sortKey}
            onChange={(e) => setSort(e.currentTarget.value as SortKey)}
            className="h-8 rounded-[8px] bg-frost-200 px-2.5 text-[13px] text-primary-text"
          >
            {sorts.map((o) => (
              <option key={o.key} value={o.key}>
                {o.label}
              </option>
            ))}
          </select>
          <div className="flex items-center" role="group" aria-label="View">
            <IconButton label="Grid view" className={mode === "grid" ? "bg-frost-300 text-primary-text!" : undefined} aria-pressed={mode === "grid"} onClick={() => chooseMode("grid")}>
              <LayoutGrid size={16} />
            </IconButton>
            <IconButton label="List view" className={mode === "list" ? "bg-frost-300 text-primary-text!" : undefined} aria-pressed={mode === "list"} onClick={() => chooseMode("list")}>
              <List size={16} />
            </IconButton>
          </div>
        </div>
        <div role="tabpanel" id="lib-panel" aria-labelledby={`lib-tab-${tab}`} className="pt-2">
          {panel}
        </div>
      </div>
    </Screen>
  );
}

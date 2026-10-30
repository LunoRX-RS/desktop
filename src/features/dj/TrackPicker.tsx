"use client";

import { useEffect, useRef, useState } from "react";
import { Search } from "lucide-react";
import { Button, Segmented, StatusNote } from "../../components/ui";
import { libraryLikes } from "../../services/tauri/library";
import { searchCancel, searchQuery, type SearchHitDto } from "../../services/tauri/search";
import { syncListLocalPlaylists, syncPlaylistEntries, type LocalPlaylistDto } from "../../services/tauri/sync";

export interface Pick { videoId: string; title: string; artist: string }
type Tab = "search" | "playlists" | "liked";

const asPick = (t: { providerTrackId: string; title: string | null; artists: string[] | null }): Pick => ({
  videoId: t.providerTrackId,
  title: t.title || "YouTube video",
  artist: (t.artists ?? []).join(", "),
});

/** Where a deck gets its track: a search, one of the user's playlists, or their liked songs. Only YouTube tracks can be mixed. */
export function TrackPicker({ label, onPick }: { label: string; onPick: (p: Pick) => void }) {
  const [tab, setTab] = useState<Tab>("search");
  const [text, setText] = useState("");
  const [rows, setRows] = useState<Pick[]>([]);
  const [playlists, setPlaylists] = useState<LocalPlaylistDto[]>([]);
  const [playlistId, setPlaylistId] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const searchId = useRef<string | null>(null);

  useEffect(() => () => { if (searchId.current) void searchCancel(searchId.current).catch(() => {}); }, []);

  useEffect(() => {
    setRows([]);
    setNote(null);
    if (tab === "playlists") {
      void syncListLocalPlaylists().then(setPlaylists, () => setNote("Your playlists could not be read."));
    } else if (tab === "liked") {
      setBusy(true);
      void libraryLikes().then(
        (t: SearchHitDto[]) => { setRows(t.filter((x) => x.provider === "YouTube").map(asPick)); setBusy(false); },
        () => { setNote("Your liked songs could not be read."); setBusy(false); },
      );
    }
  }, [tab]);

  useEffect(() => {
    if (tab !== "playlists" || !playlistId) return;
    let live = true;
    setBusy(true);
    void syncPlaylistEntries(playlistId).then(
      (entries) => { if (live) { setRows(entries.filter((e) => e.provider === "YouTube").map(asPick)); setBusy(false); } },
      () => { if (live) { setNote("That playlist could not be opened."); setBusy(false); } },
    );
    return () => { live = false; };
  }, [tab, playlistId]);

  const search = async () => {
    const q = text.trim();
    if (!q) return;
    if (searchId.current) void searchCancel(searchId.current).catch(() => {});
    const id = `dj-${label}-${Date.now()}`;
    searchId.current = id;
    setBusy(true);
    setNote(null);
    try {
      const r = await searchQuery({ query: q, providers: ["YouTube"], limit: 10, cancellationId: id });
      if (searchId.current !== id) return;
      setRows(r.hits.filter((h) => h.provider === "YouTube").map(asPick));
      if (r.hits.length === 0) setNote("Nothing found.");
    } catch {
      if (searchId.current === id) setNote("The search failed. Try again.");
    } finally {
      if (searchId.current === id) setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-2" role="group" aria-label={`Deck ${label} track picker`}>
      <Segmented<Tab> label={`Deck ${label} source`} value={tab} onChange={setTab} options={[{ key: "search", label: "Search" }, { key: "playlists", label: "Playlists" }, { key: "liked", label: "Liked" }]} />
      {tab === "search" && (
        <div className="flex gap-2">
          <input aria-label={`Deck ${label} search`} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void search(); }} placeholder="Search songs on YouTube" className="h-9 min-w-0 flex-1 rounded-[9px] bg-frost-200 px-3 text-[13px]" />
          <Button onClick={() => void search()} disabled={busy}><Search size={14} aria-hidden /> Search</Button>
        </div>
      )}
      {tab === "playlists" && (
        <select aria-label={`Deck ${label} playlist`} value={playlistId} onChange={(e) => setPlaylistId(e.target.value)} className="h-9 rounded-[9px] bg-frost-200 px-2 text-[13px]">
          <option value="">{playlists.length ? "Choose a playlist" : "No playlists yet"}</option>
          {playlists.map((p) => <option key={p.id} value={p.id}>{p.name} ({p.entryCount})</option>)}
        </select>
      )}
      {note && <StatusNote role="status">{note}</StatusNote>}
      {busy && <StatusNote role="status">Loading.</StatusNote>}
      {rows.length > 0 && (
        <ul className="flex max-h-48 flex-col overflow-y-auto rounded-[9px] bg-frost-200" aria-label={`Deck ${label} tracks`}>
          {rows.slice(0, 200).map((r, i) => (
            <li key={`${r.videoId}-${i}`}>
              <button type="button" onClick={() => onPick(r)} className="flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-frost-300">
                <span className="w-full truncate text-[13px]">{r.title}</span>
                {r.artist && <span className="w-full truncate text-[11px] text-secondary-text">{r.artist}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

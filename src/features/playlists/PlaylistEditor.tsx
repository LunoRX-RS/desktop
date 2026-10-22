"use client";
import { OfflinePlaylist, useOffline } from "../offline/OfflinePlaylist";
import { ResultRow } from "../search/SourceGroup";
import { playTrack, usePlayback } from "../../lib/playback";
import { useImport } from "../../lib/youtubeImport";
import { offlineKeep } from "../../services/tauri/offline";
import { syncPlaylistEntries, type LocalEntryDto } from "../../services/tauri/sync";
import type { SearchHitDto } from "../../services/tauri/search";

import { useEffect, useRef, useState, type FormEvent } from "react";
import { ArrowLeft, Cloud, Download, ListMusic, Play, Send, Upload } from "lucide-react";
import { Art } from "../../components/Art";
import type { View } from "../../lib/store";
import {
  syncCreateLocalPlaylist,
  syncFlushPending,
  syncResolveConflict,
  syncUploadPlaylist,
  type LocalPlaylistDto,
  type ResolutionDto,
} from "../../services/tauri/sync";
import { describeUpload, entriesLabel, errorText, needsUpload, useLocalPlaylists, useSyncStatus } from "./usePlaylists";
import { Button, EmptyState, Group, GroupRow, Screen, Skeleton, StatusNote, cx } from "../../components/ui";

type Props = {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onNavigate: (view: View) => void;
};

// One feedback line per scope. Pending and success are polite; errors are assertive. Copy is fixed, never backend text.
type Feedback = { scope: "create" | "detail"; kind: "pending" | "ok" | "error"; text: string } | null;

function FeedbackLine({ feedback, scope }: { feedback: Feedback; scope: "create" | "detail" }) {
  const mine = feedback && feedback.scope === scope ? feedback : null;
  return (
    <>
      <p role="status" className="min-h-0 text-[13px] leading-[18px] text-secondary-text">
        {mine && mine.kind !== "error" ? <span key={mine.text} className="lx-note">{mine.text}</span> : null}
      </p>
      <p role="alert" className="text-[13px] leading-[18px] text-critical-coral">
        {mine && mine.kind === "error" ? <span key={mine.text} className="lx-note">{mine.text}</span> : null}
      </p>
    </>
  );
}

function ListRow({ p, selected, onSelect }: { p: LocalPlaylistDto; selected: boolean; onSelect: () => void }) {
  const sub = `${entriesLabel(p.entryCount)} · ${describeUpload(p)}`;
  return (
    <li>
      <button
        type="button"
        data-playlist={p.id}
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
        className={cx(
          "flex h-14 w-full items-center gap-3 rounded-[8px] px-2 text-left transition-colors duration-150",
          selected ? "bg-frost-200" : "hover:bg-frost-100",
        )}
      >
        <Art seed={p.id} tone="ink" grain={false} rounded="rounded-[6px]" className="size-10 shrink-0" />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13px] leading-[18px] font-semibold" title={p.name}>
            {p.name}
          </span>
          <span className="block truncate text-[13px] leading-[18px] text-secondary-text" title={sub}>
            {sub}
          </span>
        </span>
      </button>
    </li>
  );
}

const toHit = (e: LocalEntryDto): SearchHitDto => ({
  provider: e.provider, providerTrackId: e.providerTrackId, canonicalUrl: e.providerUrl, title: e.title ?? e.providerTrackId,
  artists: e.artists ?? [], artworkUrl: e.artworkUrl, durationMs: e.durationMs,
});

/** The playlist's tracks, read from this device. Only YouTube entries can play; Spotify entries keep ids only. */
function usePlaylistTracks(id: string, count: number) {
  const [tracks, setTracks] = useState<SearchHitDto[] | null>(null);
  const [failed, setFailed] = useState(false);
  const imported = useImport((s) => s.finished);
  useEffect(() => {
    let live = true;
    setFailed(false);
    syncPlaylistEntries(id).then((list) => { if (live) setTracks(list.map(toHit)); }, () => { if (live) { setTracks([]); setFailed(true); } });
    return () => { live = false; };
  }, [id, count, imported]);
  return { tracks, failed };
}

function PlaylistActions({ playlist, tracks, busy, conflict, onUpload }: {
  playlist: LocalPlaylistDto;
  tracks: SearchHitDto[] | null; busy: boolean; conflict: boolean; onUpload: () => void;
}) {
  const playing = usePlayback((s) => s.busy);
  const { status, busy: offlineBusy, error, act } = useOffline(playlist.id, 0, null);
  const playable = (tracks ?? []).filter((t) => t.provider === "YouTube");
  const kept = status?.keeping ?? false;
  const downloading = status?.running ?? false;
  const ready = status?.tracks.filter((t) => t.state === "ready").length ?? 0;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="primary" disabled={playing || playable.length === 0} onClick={() => void playTrack(playable[0], false, playable.slice(1))}>
          <Play size={14} fill="currentColor" aria-hidden="true" />Play now
        </Button>
        <Button variant="secondary" disabled={busy || conflict || !needsUpload(playlist)} onClick={onUpload}>
          {needsUpload(playlist) ? <Upload size={14} aria-hidden="true" /> : <Cloud size={14} aria-hidden="true" />}
          {!needsUpload(playlist) ? "On the cloud" : playlist.remoteId ? "Resume upload" : "Upload to cloud"}
        </Button>
        <Button variant="secondary" disabled={offlineBusy || !status || downloading || playlist.entryCount === 0} onClick={() => void act(async () => { await offlineKeep(playlist.id); })}>
          <Download size={14} aria-hidden="true" />
          {downloading ? "Downloading…" : kept ? `Saved on device (${ready})` : "Download to local"}
        </Button>
      </div>
      {error && <p role="alert" className="text-[13px] text-critical-coral">{error}</p>}
    </div>
  );
}

export default function PlaylistEditor({ selectedId, onSelect, onNavigate }: Props) {
  const { loading, playlists, error, reload } = useLocalPlaylists();
  const { status, reload: reloadStatus } = useSyncStatus();
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback>(null);
  const [confirmServer, setConfirmServer] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const focusNext = useRef<"heading" | string | null>(null);

  const selected = playlists.find((p) => p.id === selectedId) ?? null;
  const { tracks: playlistTracks, failed: tracksFailed } = usePlaylistTracks(selected?.id ?? "", selected?.entryCount ?? 0);
  const [shown, setShown] = useState(100);
  useEffect(() => setShown(100), [selectedId]);
  const conflict = selected !== null && (status?.conflicts.includes(selected.id) ?? false);
  const syncState = status?.state.state;

  // Move focus after a state change has rendered: the new playlist's heading, or the row we came back from.
  useEffect(() => {
    const target = focusNext.current;
    if (!target) return;
    if (target === "heading" && selected) headingRef.current?.focus();
    else if (target !== "heading") document.querySelector<HTMLElement>(`[data-playlist="${CSS.escape(target)}"]`)?.focus();
    else return;
    focusNext.current = null;
  });

  useEffect(() => setConfirmServer(false), [selectedId]);

  // Runs one write, then re-reads the list and sync status so the view matches the engine.
  async function run<T>(scope: "create" | "detail", pending: string, done: (r: T) => string, action: () => Promise<T>): Promise<T | null> {
    setBusy(true);
    setFeedback({ scope, kind: "pending", text: pending });
    let result: T | null = null;
    try {
      result = await action();
      setFeedback({ scope, kind: "ok", text: done(result) });
    } catch (e) {
      setFeedback({ scope, kind: "error", text: errorText(e) });
    }
    setBusy(false);
    reload();
    reloadStatus();
    return result;
  }

  async function createPlaylist(e: FormEvent) {
    e.preventDefault();
    const name = newName.trim();
    if (name === "") {
      setFeedback({ scope: "create", kind: "error", text: "Enter a name for the playlist." });
      nameRef.current?.focus();
      return;
    }
    const id = await run("create", "Creating playlist…", () => "Playlist created on this device.", () => syncCreateLocalPlaylist(name, null));
    if (id !== null) {
      setNewName("");
      focusNext.current = "heading";
      onSelect(id);
    } else nameRef.current?.focus();
  }

  function resolve(resolution: ResolutionDto) {
    if (!selected) return;
    setConfirmServer(false);
    void run("detail", "Resolving…", () => (resolution === "keepMine" ? "Kept your version." : "Kept the server version."), () => syncResolveConflict(selected.id, resolution));
  }

  const nameInvalid = feedback?.scope === "create" && feedback.kind === "error";

  const createForm = (
    <form onSubmit={createPlaylist} noValidate className="flex flex-col gap-1.5" aria-label="New playlist">
      <label htmlFor="editor-new-name" className="text-[13px] leading-[18px] font-semibold text-secondary-text">
        New playlist
      </label>
      <div className="flex items-center gap-2">
        <input
          ref={nameRef}
          id="editor-new-name"
          value={newName}
          onChange={(e) => setNewName(e.currentTarget.value)}
          placeholder="Name"
          autoComplete="off"
          aria-invalid={nameInvalid || undefined}
          className="h-8 min-w-0 flex-1 rounded-[8px] bg-frost-200 px-3 text-[13px] text-primary-text outline-none placeholder:text-tertiary-text focus:bg-frost-300"
        />
        <Button variant="primary" type="submit" disabled={busy}>
          Create
        </Button>
      </div>
      <FeedbackLine feedback={feedback} scope="create" />
    </form>
  );

  const detail = selected && (
    <div className="flex min-w-0 flex-col gap-6">
      <Button variant="ghost" onClick={() => { focusNext.current = selected.id; onSelect(null); }} className="self-start px-2 min-[1100px]:hidden">
        <ArrowLeft size={14} aria-hidden="true" />
        All playlists
      </Button>

      <section aria-labelledby="editor-detail" className="flex items-end gap-5">
        <Art seed={selected.id} tone="ink" rounded="rounded-[10px]" className="size-[152px] shrink-0 shadow-[0_2px_10px_rgba(0,0,0,0.4)]" />
        <div className="flex min-w-0 flex-col gap-1 pb-1">
          <h2
            id="editor-detail"
            ref={headingRef}
            tabIndex={-1}
            title={selected.name}
            className="line-clamp-2 font-display text-[22px] leading-7 font-semibold tracking-[-0.015em] [overflow-wrap:anywhere] outline-none"
          >
            {selected.name}
          </h2>
          <p className="truncate text-[13px] leading-[18px] text-secondary-text">
            {entriesLabel(selected.entryCount)} · {describeUpload(selected)}
          </p>
        </div>
      </section>

      <PlaylistActions
        playlist={selected}
        tracks={playlistTracks}
        busy={busy}
        conflict={conflict}
        onUpload={() => void run("detail", "Uploading…", () => "Uploaded.", () => syncUploadPlaylist(selected.id))}
      />

      <OfflinePlaylist key={selected.id} playlist={selected} />

      {conflict && (
        <StatusNote role="alert" tone="amber">
          <p className="font-semibold text-primary-text">This playlist changed on the server and on this device.</p>
          <p className="mt-0.5">Keep mine uploads this device&rsquo;s version. Keep server replaces it with the server&rsquo;s version.</p>
          <div className="mt-3 flex flex-wrap items-center gap-2">
            {confirmServer ? (
              <>
                <Button variant="primary" disabled={busy} onClick={() => resolve("keepServer")}>
                  Replace my version
                </Button>
                <Button variant="ghost" disabled={busy} onClick={() => setConfirmServer(false)}>
                  Cancel
                </Button>
              </>
            ) : (
              <>
                <Button variant="primary" disabled={busy} onClick={() => resolve("keepMine")}>
                  Keep mine
                </Button>
                <Button variant="secondary" disabled={busy} onClick={() => setConfirmServer(true)}>
                  Keep server
                </Button>
              </>
            )}
          </div>
        </StatusNote>
      )}

      {syncState === "offline" && (
        <StatusNote tone="amber">No connection to the server. Changes stay on this device until it returns.</StatusNote>
      )}
      {syncState === "signedOut" && needsUpload(selected) && (
        <StatusNote tone="blue">
          Sign in to upload this playlist.
          <Button variant="ghost" onClick={() => onNavigate("sync")} className="ml-1 h-8 px-2">
            Open Sync
          </Button>
        </StatusNote>
      )}

      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          {selected.remoteId && selected.queuedWrites > 0 && (
            <Button
              variant="secondary"
              disabled={busy || conflict}
              onClick={() => run("detail", "Sending changes…", (n: number) => (n === 0 ? "No changes were accepted." : `Sent ${n} ${n === 1 ? "change" : "changes"}.`), () => syncFlushPending())}
            >
              <Send size={14} aria-hidden="true" />
              Send {selected.queuedWrites} waiting {selected.queuedWrites === 1 ? "change" : "changes"}
            </Button>
          )}
          {!needsUpload(selected) && selected.queuedWrites === 0 && !conflict && (
            <p className="text-[13px] leading-[18px] text-secondary-text">Everything is on the server.</p>
          )}
        </div>
        <FeedbackLine feedback={feedback} scope="detail" />
      </div>

      <section aria-label="Tracks" className="flex flex-col gap-2">
        <h3 className="px-1 text-[13px] leading-[18px] font-semibold text-secondary-text">Tracks</h3>
        {playlistTracks === null && <p role="status" className="text-[13px] text-secondary-text">Reading tracks…</p>}
        {tracksFailed && <p role="alert" className="text-[13px] text-critical-coral">Could not read this playlist's tracks.</p>}
        {playlistTracks?.length === 0 && !tracksFailed && <p className="text-[13px] text-secondary-text">No tracks yet. Add some from Search.</p>}
        <ul>{playlistTracks?.slice(0, shown).map((hit, i) => <ResultRow key={`${hit.providerTrackId}:${i}`} hit={hit} />)}</ul>
        {playlistTracks && playlistTracks.length > shown && <Button variant="ghost" onClick={() => setShown((n) => n + 100)}>Show more</Button>}
      </section>

      <Group id="editor-facts" title="Details">
        <GroupRow label="Entries">{entriesLabel(selected.entryCount)}</GroupRow>
        <GroupRow label="Server">{describeUpload(selected)}</GroupRow>
        {selected.remoteId && (
          <GroupRow label="Uploaded">
            {selected.uploaded} of {selected.entryCount}
          </GroupRow>
        )}
      </Group>


      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <Button variant="secondary" onClick={() => onNavigate("search")}>
          Add tracks in Search
        </Button>
        <p className="text-[13px] leading-[18px] text-secondary-text">Rename, delete and reorder are not available yet.</p>
      </div>
    </div>
  );

  let body;
  if (loading && playlists.length === 0) {
    body = (
      <div className="flex flex-col gap-1" role="status" aria-label="Loading playlists">
        {Array.from({ length: 5 }, (_, i) => (
          <div key={i} className="flex h-14 items-center gap-3 px-2">
            <Skeleton className="size-10 shrink-0 rounded-[6px]" />
            <div className="flex flex-1 flex-col gap-1.5">
              <Skeleton className="h-3.5 w-1/2" />
              <Skeleton className="h-3.5 w-1/3" />
            </div>
          </div>
        ))}
      </div>
    );
  } else if (error) {
    body = (
      <StatusNote role="alert" tone="coral">
        <p>{errorText(error)}</p>
        <Button variant="secondary" onClick={reload} className="mt-2">
          Retry
        </Button>
      </StatusNote>
    );
  } else if (playlists.length === 0) {
    body = (
      <p className="px-2 text-[13px] leading-[18px] text-secondary-text">No playlists yet.</p>
    );
  } else {
    body = (
      <ul className="lx-stagger flex flex-col" aria-label="Playlists">
        {playlists.map((p) => (
          <ListRow
            key={p.id}
            p={p}
            selected={p.id === selectedId}
            onSelect={() => {
              focusNext.current = "heading";
              onSelect(p.id);
            }}
          />
        ))}
      </ul>
    );
  }

  const showList = !selected;
  return (
    <Screen id="editor-title" title="Playlist Editor" caption="Playlists are saved on this device first, then uploaded when you choose.">
      <div className="grid grid-cols-1 gap-8 min-[1100px]:grid-cols-[320px_minmax(0,1fr)]">
        <div className={cx("flex min-w-0 flex-col gap-5", !showList && "max-[1099px]:hidden")}>
          {createForm}
          {body}
        </div>
        <div className="min-w-0">
          {detail ||
            (!loading && !error && (
              <div className="max-[1099px]:hidden">
                {playlists.length === 0 ? (
                  <EmptyState
                    icon={<ListMusic size={24} />}
                    title="No playlists yet"
                    action={
                      <Button variant="primary" onClick={() => nameRef.current?.focus()} className="mt-1">
                        Create your first playlist
                      </Button>
                    }
                  >
                    Name one on the left. It is saved on this device first.
                  </EmptyState>
                ) : (
                  <p className="py-6 text-[13px] leading-[18px] text-secondary-text">Select a playlist to see its details.</p>
                )}
              </div>
            ))}
        </div>
      </div>
    </Screen>
  );
}

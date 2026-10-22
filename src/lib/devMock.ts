// Dev-only mock of the Tauri IPC so every screen can be previewed in a plain browser (`bun run dev`).
// Installed only when NODE_ENV is development and the page is not inside Tauri. Data is fake; nothing plays.
import type { SearchHitDto } from "../services/tauri/search";

// Generated cover art as a data: URI (the app CSP allows data: images), so the dev preview looks like real covers.
const cover = (seed: string): string => {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619) >>> 0;
  const a = h % 360, b = (a + 40 + (h >> 8) % 90) % 360, k = (h >> 16) % 3;
  const shape = k === 0 ? `<circle cx="150" cy="140" r="90" fill="hsl(${b} 80% 70%)" opacity=".55"/>` : k === 1 ? `<rect x="40" y="60" width="150" height="150" rx="24" transform="rotate(${h % 40} 120 120)" fill="hsl(${b} 80% 72%)" opacity=".5"/>` : `<path d="M0 220 L120 60 L240 220Z" fill="hsl(${b} 80% 70%)" opacity=".55"/>`;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 240"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${a} 70% 38%)"/><stop offset="1" stop-color="hsl(${b} 70% 16%)"/></linearGradient></defs><rect width="240" height="240" fill="url(#g)"/>${shape}</svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
};
const ART = (n: number) => ({ provider: "YouTube", id: `UC${String(n).padStart(22, "a")}`, name: ["Palmy", "Big Ass", "P6ICK", "MILLI", "YOUNGOHM", "PiXXiE"][n % 6] });
const hit = (i: number): SearchHitDto => {
  const a = ART(i);
  const id = `dQw4w9Wg${String(i).padStart(3, "0")}`;
  return { source: "YouTube Music", provider: "YouTube", providerTrackId: id, canonicalUrl: `https://www.youtube.com/watch?v=${id}`, title: ["ซ่อนกลิ่น", "ก่อนตาย", "1 OF 1", "ขึ้นใจ (Final Call)", "ติดฝน (rain)", "BU", "Proud", "นครดารา"][i % 8], artists: [a.name], artistRefs: [a], artworkUrl: cover(id), durationMs: 190000 + i * 7000 };
};
const HITS = Array.from({ length: 12 }, (_, i) => hit(i));
const profile = (id: string) => ({ provider: "YouTube", id, name: "YOUNGOHM", description: "Thai rapper. Mock profile for dev preview.", artworkUrl: cover("artist-" + id), subscribers: 1250000, tracks: HITS.slice(0, 8), albums: [0, 1, 2, 3].map((n) => ({ id: `MPREb_mock${n}`, name: ["ไฟกลางคืน", "ธาตุทองซาวด์", "BANGKOK LEGACY", "แค่เสร็จ"][n], year: 2020 + n, artworkUrl: cover("album" + n) })) });

// Fake playback: a clock that advances while "playing", with timed lyrics from two sources.
const LYR = ["Baby, can you text me when you feel alone", "เรียน BU ละนี่เธออยู่ปีไหน", "Never see you ก็เพราะพี่ถูกรีไทร์", "ไม่มีคำหวานหรือว่าประโยคกินใจ", "Baby ขอสามคำเลยเธอ โสด จริง ไหม", "โดนทีเดียวผมได้ไปอยู่ Sea side", "โฟดีโฟดีโฟดีฟาย", "ROV หรือเธออยาก Free Fire", "จัดให้ Feel โดจินเลย เธออยากฟินไหม"];
const lines = (words: boolean) => LYR.map((text, i) => {
  const startMs = 8000 + i * 6500;
  const parts = text.split(" ");
  return { startMs, text, words: words ? parts.map((t, k) => ({ text: t + (k < parts.length - 1 ? " " : ""), startMs: startMs + k * 700, endMs: startMs + k * 700 + 650 })) : null };
});
const src = (id: string, label: string, o: object) => ({ id, label, available: false, text: null, lines: null, synced: false, words: false, attribution: null, status: "noMatch", ...o });
const sources = () => [
  src("lyricsplus", "LyricsPlus", { available: true, text: LYR.join("\n"), lines: lines(true), synced: true, words: true, attribution: "Lyrics from LyricsPlus", status: "available" }),
  src("lrclib", "LRCLIB", { available: true, text: LYR.join("\n"), lines: lines(false), synced: true, attribution: "Lyrics from LRCLIB", status: "available" }),
  src("youtubeMusic", "YouTube Music", { available: true, text: LYR.join("\n"), attribution: "Source: LyricFind", status: "available" }),
  src("netease", "Netease Cloud Music", { status: "unreachable" }),
  src("qq", "QQ Music", { status: "disabled" }),
  src("kugou", "Kugou", { status: "noMatch" }),
];

export function installDevMock(): void {
  if (typeof window === "undefined" || "__TAURI_INTERNALS__" in window) return;
  let cb = 1;
  const handlers: Record<string, number> = {};
  let likes = HITS.slice(0, 5);
  let signedIn = false;
  let ytStatus = "disconnected";
  let keeping = false;
  const lrcs: { providerTrackId: string; title: string; artists: string[]; lines: { startMs: number; text: string }[]; modifiedMs: number }[] = [];
  let playlists: { id: string; name: string; remoteId: string | null; entryCount: number; uploaded: number; queuedWrites: number; unsentEntries: number }[] = [{ id: "pl1", name: "Late night", remoteId: null, entryCount: 3, uploaded: 0, queuedWrites: 0, unsentEntries: 0 }, { id: "pl2", name: "Work focus", remoteId: null, entryCount: 12, uploaded: 12, queuedWrites: 0, unsentEntries: 0 }];
  const w = window as unknown as Record<string, unknown>;
  let track: SearchHitDto | null = null;
  let queue: SearchHitDto[] = [];
  let state = "idle";
  let revision = 1;
  let pos = 0;
  let volume = 100;
  const DUR = 166000;
  const st = () => ({ version: 1, state, revision });
  const emit = (event: string, payload: unknown) => { const id = handlers[event]; const f = id ? (w["_" + id] as ((e: unknown) => void) | undefined) : undefined; f?.({ event, id, payload }); };
  const prog = () => ({ player: st(), track, positionMs: pos, durationMs: track ? DUR : 0, available: !!track, volume, autoplay: { state: queue.length ? "ready" : "", nextTitle: queue[0]?.title ?? null, queue: queue.map((t) => ({ provider: "YouTube", id: t.providerTrackId, title: t.title, artists: t.artists, artworkUrl: null, durationMs: t.durationMs })) } });
  const set = (next: string) => { state = next; revision++; emit("core://player/state", st()); emit("core://player/progress", prog()); };
  window.setInterval(() => {
    if (state !== "playing") return;
    pos += 500;
    if (pos >= DUR) { const n = queue.shift(); if (n) { track = n; pos = 0; } else { pos = DUR; set("ended"); return; } }
    emit("core://player/progress", prog());
  }, 500);
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  w.__TAURI_INTERNALS__ = {
    transformCallback(f: unknown) { const id = cb++; w["_" + id] = f; return id; },
    unregisterCallback() {},
    convertFileSrc: (p: string) => p,
    async invoke(cmd: string, a: Record<string, unknown> = {}) {
      if (cmd === "plugin:event|listen") { handlers[a.event as string] = a.handler as number; return cb++; }
      if (cmd.startsWith("plugin:")) return null;
      switch (cmd) {
        case "library_likes": return likes;
        case "library_set_like": { const t = a.track as SearchHitDto; likes = a.liked ? [t, ...likes.filter((x) => x.providerTrackId !== t.providerTrackId)] : likes.filter((x) => x.providerTrackId !== t.providerTrackId); return likes; }
        case "library_history": return HITS.slice(0, 9);
        case "library_clear_history": return [];
        case "library_artist": return profile(a.id as string);
        case "library_album": return { provider: "YouTube", id: a.id, name: "ธาตุทองซาวด์", kind: "Album", year: 2023, artists: [ART(4)], artworkUrl: cover("album1"), tracks: HITS.slice(0, 7).map((t) => ({ ...t, artworkUrl: cover("album1") })) };
        case "discovery_feed": return [{ title: "Thai hits", tracks: HITS.slice(0, 8) }, { title: "Because you liked P6ICK", tracks: HITS.slice(4, 12) }];
        case "search_suggestions": return ["ซ่อนกลิ่น", "P6ICK", "YOUNGOHM"];
        case "search_query": return { query: (a.args as { query: string }).query, groups: [{ kind: "ok", provider: "YouTube", hits: HITS }], hits: HITS };
        case "search_cancel": return null;
        case "providers_list": return [{ provider: "YouTube", search: "Supported", playback: "Supported", externalPlayback: "Supported" }, { provider: "Spotify", search: "Unsupported", playback: "Unsupported", externalPlayback: "Unsupported" }];
        case "sync_status": return { state: signedIn ? { state: "synced" } : { state: "signedOut" }, pendingWrites: playlists.filter((p) => p.remoteId === null && p.entryCount > 0).length, conflicts: [] };
        case "sync_sign_in": await new Promise((r) => setTimeout(r, 500)); signedIn = true; return { userId: "mock-user", expiresAt: new Date(Date.now() + 864e5).toISOString() };
        case "sync_sign_out": signedIn = false; return { warning: null };
        case "sync_upload_playlist": { if (!signedIn) throw "unauthenticated"; await new Promise((r) => setTimeout(r, 600)); playlists = playlists.map((p) => p.id === a.localId ? { ...p, remoteId: "remote-" + p.id, uploaded: p.entryCount } : p); return "remote-" + a.localId; }
        case "sync_flush_pending": return 0;
        case "sync_discard_pending": return 0;
        case "sync_remove_local_entry": return null;
        case "youtube_connect": ytStatus = "connecting"; return ytStatus;
        case "youtube_connect_finish": ytStatus = "connected"; return ytStatus;
        case "youtube_disconnect": ytStatus = "disconnected"; return ytStatus;
        case "lrc_save": { const g = a.args as { providerTrackId: string; title: string; artists: string[]; lines: { startMs: number; text: string }[] }; const i = lrcs.findIndex((x) => x.providerTrackId === g.providerTrackId); const rec = { ...g, modifiedMs: Date.now() }; if (i >= 0) lrcs[i] = rec; else lrcs.push(rec); return { path: `~/Documents/LUNOR-X Lyrics/${g.artists[0] ?? "Artist"} - ${g.title} [${g.providerTrackId}].lrc`, count: g.lines.length, bytes: g.lines.length * 32 }; }
        case "lrc_delete": { const i = lrcs.findIndex((x) => x.providerTrackId === (a.args as { providerTrackId: string }).providerTrackId); if (i >= 0) lrcs.splice(i, 1); return { deleted: i >= 0 }; }
        case "lrc_reveal": return null;
        case "offline_keep": keeping = true; return null;
        case "offline_remove": case "offline_cancel": keeping = false; return null;
        case "offline_set_limit": return null;
        case "sync_list_local_playlists": return playlists;
        case "offline_status": return { keeping, running: false, changed: false, tracks: keeping ? HITS.slice(0, 3).map((t) => ({ track: { provider: "YouTube", id: t.providerTrackId, title: t.title, artists: t.artists, artworkUrl: t.artworkUrl, durationMs: t.durationMs }, error: null, state: "ready" })) : [], usedBytes: keeping ? 18_400_000 : 0, pinnedBytes: keeping ? 18_400_000 : 0, limitBytes: 2_000_000_000 };
        case "sync_create_local_playlist": { const id = "pl" + (playlists.length + 1); playlists = [...playlists, { id, name: a.name as string, remoteId: null, entryCount: 0, uploaded: 0, queuedWrites: 0, unsentEntries: 0 }]; return id; }
        case "sync_add_local_entry": playlists = playlists.map((p) => p.id === a.localId ? { ...p, entryCount: p.entryCount + 1 } : p); return null;
        case "player_state": return st();
        case "player_progress": return prog();
        case "player_load": {
          const g = a.args as SearchHitDto & { queueSelection?: boolean; following?: { providerTrackId: string }[] };
          const found = HITS.find((h) => h.providerTrackId === g.providerTrackId) ?? { ...g, artistRefs: [], artworkUrl: null, durationMs: DUR };
          if (g.queueSelection) queue = queue.slice(Math.max(0, queue.findIndex((t) => t.providerTrackId === g.providerTrackId) + 1));
          else queue = (g.following ?? []).map((f) => HITS.find((h) => h.providerTrackId === f.providerTrackId) ?? hit(0)).concat(HITS.filter((h) => h.providerTrackId !== g.providerTrackId)).slice(0, 20);
          track = found; pos = 0; set("ready"); return st();
        }
        case "player_play": case "player_resume": set("playing"); return st();
        case "player_pause": set("paused"); return st();
        case "player_stop": track = null; queue = []; pos = 0; set("idle"); return st();
        case "player_seek": pos = Number(a.positionMs) || 0; emit("core://player/progress", prog()); return st();
        case "player_set_volume": volume = Number(a.volume ?? 100); return st();
        case "player_track_info": return { providerTrackId: track?.providerTrackId ?? "", title: track?.title ?? "", artists: track?.artists ?? [], artworkUrl: null, lyrics: LYR.join("\n"), lyricsSource: "Lyrics from LyricsPlus", lyricsProvider: "LRCLIB", lyricsStatus: "available", lyricsLines: lines(true), lyricsSynced: true, lyricsSources: sources() };
        case "lyrics_source_fetch": return sources().find((x) => x.id === (a.args as { sourceId: string }).sourceId) ?? src("kugou", "Kugou", {});
        case "lyrics_source_search": return { candidates: [{ sourceId: "lrclib", key: "k1", title: "BU", artist: "SURIYA MQT", durationMs: DUR, synced: true, words: false }] };
        case "lrc_load": { const r = lrcs.find((x) => x.providerTrackId === (a.args as { providerTrackId: string }).providerTrackId); return r ? { exists: true, path: "~/Documents/LUNOR-X Lyrics/" + r.title + ".lrc", lines: r.lines, title: r.title, artists: r.artists } : { exists: false, path: null, lines: [], title: null, artists: [] }; }
        case "lrc_list": return { items: lrcs };
        case "youtube_connection_status": return ytStatus;
        case "provider_credential_status": return { present: false };
        case "player_audio_settings": return { settings: { quality: "high", pcmFormat: "float32", sampleRate: "auto", loudness: "balanced" }, available: true, output: { driver: "coreaudio", format: "float32", sampleRate: 48000, sourceFormat: "opus", sourceSampleRate: 48000 } };
        case "player_set_audio_settings": return { settings: a.settings, available: true, output: { driver: "coreaudio", format: "float32", sampleRate: 48000, sourceFormat: "opus", sourceSampleRate: 48000 } };
        case "lyrics_settings_get": return { order: sources().map((x) => x.id), disabled: ["qq"], sources: sources().map((x) => ({ id: x.id, label: x.label })) };
        default: console.debug("[devMock] unmocked command", cmd); return null;
      }
    },
  };
}

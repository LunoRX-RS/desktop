"use client";

import { loadCustomFonts } from "../../lib/textStyle";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AnimatePresence, MotionConfig, motion, useReducedMotion } from "motion/react";
import { applySavedAudio, watchAudioSettings } from "../../lib/audioSettings";
import { initHistory } from "../../lib/history";
import { detectPlatform } from "../../lib/platform";
import { actionFor, comboFromEvent, currentBindings, hasCommandModifier } from "../../lib/shortcuts";
import { initTelemetry, track } from "../../lib/telemetry";
import { action as noteAction, initActivity } from "../../lib/activity";
import { initErrorReport } from "../../lib/errorReport";
import { initRemote } from "../../lib/remote";
import { initUpdater } from "../../lib/updater";
import { UpdateBanner } from "./UpdateBanner";
import { NoticeBar } from "./NoticeBar";
import { hydrateRecentSearches } from "../../lib/store";
import { useUi } from "../../lib/store";
import { Button } from "../ui";
import { useLibrary } from "../../lib/library";
import { refreshLikes } from "../../lib/library";
import HomeScreen from "../../features/home/HomeScreen";
import {
  AccountsScreen, AlbumScreen, ArtistScreen, CommandPalette, ConnectionsScreen, ExploreScreen, LibraryScreen, LoginGate, LoginPage,
  NowPlaying, PlayerScreen, PlaylistEditor, QueuePanel, ReplayScreen, SavedLibraryScreen, SearchScreen, SettingsScreen, SharedScreen,
  SyncPanel, preloadScreens,
} from "./screens";
import { SHARE_OPEN_EVENT, sharedLinkTake } from "../../services/tauri/share";
import Sidebar from "./Sidebar";
import TopBar from "./TopBar";
import PlayerBar from "./PlayerBar";
import { PlaybackEvents } from "./NativePlayer";
import { usePlayback, togglePlayback } from "../../lib/playback";
import { DUR, tween } from "../../lib/motion";
import { ContextMenuHost } from "../ContextMenu";
import { loadFeed } from "../../lib/discovery";
import { initYouTubeImport } from "../../lib/youtubeImport";
import { applySavedMode } from "../../lib/playMode";
import { initAppStyle, resetAppScale, stepAppScale } from "../../lib/appStyle";


export type { View } from "../../lib/store";

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(target.tagName);
}

// Keyboard shortcuts. The keys come from lib/shortcuts (Command on macOS, Ctrl elsewhere, changeable in Settings).
function useGlobalKeys() {
	useEffect(() => {
		function onKey(e: KeyboardEvent) {
			const combo = comboFromEvent(e);
			if (!combo) return;
			const action = actionFor(combo, currentBindings());
			if (!action) return;
			// Keys without Ctrl, Alt or Command (Space) must not fire while typing or on a focused button.
			if (!hasCommandModifier(combo) && isTypingTarget(e.target)) return;
			const s = useUi.getState();
			if (action === "playPause" && s.paletteOpen) return;
			e.preventDefault();
			// Shortcuts are actions too; the name is lowercased to the slug shape the log validates.
			noteAction(`shortcut.${action.toLowerCase()}`);
			switch (action) {
				case "palette": s.setPalette(!s.paletteOpen); break;
				case "back": s.goBack(); break;
				case "forward": s.goForward(); break;
				case "toggleSidebar": s.toggleSidebar(); break;
				case "zoomIn": stepAppScale(1); break;
				case "zoomOut": stepAppScale(-1); break;
				case "zoomReset": resetAppScale(); break;
				case "playPause":
					if (usePlayback.getState().requested || usePlayback.getState().track) void togglePlayback();
					else s.togglePlay();
					break;
			}
		}
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, []);
}

// Motion writes inline style attributes when rendered on the server, and the Tauri CSP forbids those.
// The animated wrapper therefore mounts only after the first client render; the static HTML stays plain.
function ViewFrame({ mounted, children }: { mounted: boolean; children: ReactNode }) {
  const reduce = useReducedMotion();
  if (!mounted) return <div>{children}</div>;
  return (
    <motion.div
      initial={{ opacity: 0, y: reduce ? 0 : 8 }}
      animate={{ opacity: 1, y: 0, transition: tween(DUR.base) }}
      exit={{ opacity: 0, transition: tween(DUR.exit) }}
    >
      {children}
    </motion.div>
  );
}

export default function AppShell() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => { void loadCustomFonts(); }, []);
  // CSS hook for per-OS rendering differences (globals.css: html[data-platform="linux"]).
  useEffect(() => { document.documentElement.dataset.platform = detectPlatform(); }, []);
  useEffect(() => {
    setMounted(true);
    hydrateRecentSearches();
    void refreshLikes();
    const stopImport = initYouTubeImport();
    applySavedMode();
    void applySavedAudio();
    const stopAudio = watchAudioSettings();
    const stopPreload = preloadScreens();
    initAppStyle();
    // Warm the first screens while the window is still settling: both feeds, and Explore's first topic.
    const warm = () => { void loadFeed("home"); void loadFeed("explore", "charts"); };
    const idle = (window as { requestIdleCallback?: (f: () => void) => number }).requestIdleCallback;
    if (idle) idle(warm); else window.setTimeout(warm, 300);
    return () => { stopImport(); stopAudio(); stopPreload(); };
  }, []);
  const mainRef = useRef<HTMLElement>(null);
  const libraryError = useLibrary(s => s.error);
  const libraryTab = useUi(s => s.libraryTab);
  const view = useUi((s) => s.view);
  const selectedPlaylistId = useUi((s) => s.selectedPlaylistId);
  const queueOpen = useUi((s) => s.queueOpen);
  const nowPlayingOpen = useUi((s) => s.nowPlayingOpen);
  const navigate = useUi((s) => s.navigate);
  const openShared = useUi((s) => s.openShared);
  // Share links open the shared-playlist view: live events, plus a link that arrived before this window listened.
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    void listen<string>(SHARE_OPEN_EVENT, (event) => openShared(event.payload)).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    void sharedLinkTake().then((token) => { if (token && active) openShared(token); }).catch(() => {});
    return () => {
      active = false;
      unlisten?.();
    };
  }, [openShared]);
  const setSelectedPlaylist = useUi((s) => s.setSelectedPlaylist);
  useGlobalKeys();
  useEffect(() => initHistory(), []);
  useEffect(() => initTelemetry(), []);
  useEffect(() => initErrorReport(), []);
  useEffect(() => initActivity(), []);
  useEffect(() => initRemote(), []);
  useEffect(() => initUpdater(), []);
  useEffect(() => { track(`feature.view.${view}`); }, [view]);

  // Demo playback clock. Only advances while the simulated state is "playing".
  useEffect(() => {
    const id = window.setInterval(() => useUi.getState().tick(), 1000);
    return () => window.clearInterval(id);
  }, []);

  return (
    <MotionConfig reducedMotion="user">
      <PlaybackEvents />
      <UpdateBanner />
      <NoticeBar />
      <LoginGate />
      <a
        href="#main-content"
        className="glass-solid sr-only top-2 left-2 z-[60] rounded-[8px] px-3 py-2 text-[13px] focus:not-sr-only focus:fixed focus:inline-flex focus:min-h-8 focus:items-center"
      >
        Skip to content
      </a>
      <div className="flex h-dvh flex-col bg-night-canvas">
        <TopBar />
        <div className="glass flex min-h-0 flex-1">
          <Sidebar />
          <div className="mr-3 mb-3 flex min-w-0 flex-1 flex-col overflow-hidden rounded-[12px] bg-night-canvas">
            {libraryError && <div role="alert" className="mx-8 mt-2 mb-3 flex shrink-0 items-center justify-between gap-4 rounded-lg bg-frost-200 px-4 py-2 text-[13px] text-secondary-text"><p>{libraryError}</p><Button className="shrink-0" onClick={() => void refreshLikes()}>Retry likes</Button></div>}
            <main ref={mainRef} id="main-content" tabIndex={-1} className="min-h-0 flex-1 overflow-y-auto outline-none">
              <div className="mx-auto w-full max-w-content px-8 pt-4 pb-16">
                <AnimatePresence mode="wait" initial={false} onExitComplete={() => mainRef.current?.scrollTo({ top: 0 })}>
                  <ViewFrame key={view} mounted={mounted}>
                    {view === "home" && <HomeScreen onNavigate={navigate} onOpenPlaylist={(id) => { setSelectedPlaylist(id); navigate("playlists"); }} />}
                    {view === "explore" && <ExploreScreen />}
                    {view === "search" && <SearchScreen />}
                    {view === "artist" && <ArtistScreen />}
                    {view === "album" && <AlbumScreen />}
                    {view === "library" && (libraryTab === "songs" || libraryTab === "artists") && <SavedLibraryScreen artists={libraryTab === "artists"} />}
                    {view === "library" && libraryTab !== "songs" && libraryTab !== "artists" && <LibraryScreen onOpenPlaylist={(id) => { setSelectedPlaylist(id); navigate("playlists"); }} />}
                    {view === "playlists" && (
                      <PlaylistEditor selectedId={selectedPlaylistId} onSelect={setSelectedPlaylist} onNavigate={navigate} />
                    )}
                    {view === "player" && <div className="lx-scope"><PlayerScreen /></div>}
                    {view === "connections" && <div className="lx-scope"><ConnectionsScreen /></div>}
                    {view === "accounts" && <AccountsScreen />}
                    {view === "login" && <LoginPage />}
                    {view === "settings" && <SettingsScreen />}
                    {view === "replay" && <ReplayScreen />}
                    {view === "shared" && <SharedScreen />}
                    {view === "sync" && <div className="lx-scope"><SyncPanel /></div>}
                  </ViewFrame>
                </AnimatePresence>
              </div>
            </main>
          </div>
        </div>
        <PlayerBar />
      </div>
      <AnimatePresence>{queueOpen && !nowPlayingOpen && <QueuePanel />}</AnimatePresence>
      <NowPlaying />
      <CommandPalette />
      <ContextMenuHost />
    </MotionConfig>
  );
}

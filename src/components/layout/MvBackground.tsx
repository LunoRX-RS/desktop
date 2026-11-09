"use client";

import { useEffect, useState } from "react";
import { useTheme } from "../../lib/theme";
import { mvVideoId } from "../../lib/mvVideo";
import { usePlayback } from "../../lib/playback";
import { invoke } from "../../services/tauri/invoke";

/** The music video id for a track when the Background is set to Music video, else null. */
export function useMvId(track: { provider: string; providerTrackId: string; title: string; artists?: string[] } | null | undefined): string | null {
  const { backdrop } = useTheme();
  const [mv, setMv] = useState<string | null>(null);
  const key = track ? `${track.provider}:${track.providerTrackId}:${track.title}` : "";
  useEffect(() => {
    let live = true;
    setMv(null);
    if (backdrop === "video" && track) {
      const durationMs = usePlayback.getState().durationMs || null;
      void mvVideoId({ ...track, durationMs }).then((id) => { if (live) setMv(id); });
    }
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backdrop, key]);
  return backdrop === "video" ? mv : null;
}

// Loopback origin of the app's own host page (platform/mv_host.rs). Null outside the app or if the server did not start.
let mvHost: Promise<string | null> | null = null;
function mvHostOrigin(): Promise<string | null> {
  mvHost ??= invoke<string | null>("mv_host_origin").catch(() => null);
  return mvHost;
}

/**
 * The video, muted and looped, behind the glass. The sound stays with the player; not synced to the playhead.
 * The frame loads a local page that embeds YouTube: the WebView sends no Referer, and YouTube refuses an embed without one.
 */
export function MvFrame({ id, className = "" }: { id: string; className?: string }) {
  const [origin, setOrigin] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void mvHostOrigin().then((o) => { if (live) setOrigin(o); });
    return () => { live = false; };
  }, []);
  if (!origin) return null;
  return (
    <iframe
      key={id}
      title=""
      tabIndex={-1}
      src={`${origin}/mv/${id}`}
      allow="autoplay; encrypted-media"
      className={`pointer-events-none border-0 ${className}`}
    />
  );
}

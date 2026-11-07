"use client";

import { useEffect, useState } from "react";
import { useTheme } from "../../lib/theme";
import { mvVideoId } from "../../lib/mvVideo";

/** The music video id for a track when the Background is set to Music video, else null. */
export function useMvId(track: { provider: string; providerTrackId: string; title: string; artists?: string[] } | null | undefined): string | null {
  const { backdrop } = useTheme();
  const [mv, setMv] = useState<string | null>(null);
  const key = track ? `${track.provider}:${track.providerTrackId}:${track.title}` : "";
  useEffect(() => {
    let live = true;
    setMv(null);
    if (backdrop === "video" && track) void mvVideoId(track).then((id) => { if (live) setMv(id); });
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backdrop, key]);
  return backdrop === "video" ? mv : null;
}

/** The video, muted and looped, behind the glass. The sound stays with the player; not synced to the playhead. */
export function MvFrame({ id, className = "" }: { id: string; className?: string }) {
  return (
    <iframe
      key={id}
      title=""
      tabIndex={-1}
      src={`https://www.youtube.com/embed/${id}?autoplay=1&mute=1&loop=1&playlist=${id}&controls=0&playsinline=1&modestbranding=1&rel=0`}
      allow="autoplay; encrypted-media"
      className={`pointer-events-none border-0 ${className}`}
    />
  );
}

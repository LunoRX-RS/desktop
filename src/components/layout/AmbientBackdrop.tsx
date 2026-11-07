"use client";

import { useEffect, useState } from "react";
import { usePlayback } from "../../lib/playback";
import { useTheme } from "../../lib/theme";
import { mvVideoId } from "../../lib/mvVideo";
import { TrackCover } from "../TrackCover";

// App-wide ambient layer under every glass surface: the current artwork, heavily blurred, under a scrim.
// Static on purpose: a looping drift here would repaint every backdrop-filter above it on each frame.
// With nothing playing the slate/violet gradient (.lx-app-ambient) shows through.
export function AmbientBackdrop() {
  const track = usePlayback((s) => s.requested ?? s.track);
  const info = usePlayback((s) => s.info);
  const artwork = info?.providerTrackId === track?.providerTrackId ? info?.artworkUrl ?? track?.artworkUrl : track?.artworkUrl;
  const { backdrop } = useTheme();
  const [mv, setMv] = useState<string | null>(null);
  // Muted and looped: the sound stays with the player. Not synced to the playhead (the default).
  useEffect(() => {
    let live = true;
    setMv(null);
    if (backdrop === "video" && track) void mvVideoId(track).then((id) => { if (live) setMv(id); });
    return () => { live = false; };
  }, [backdrop, track?.providerTrackId]);
  if (backdrop === "video" && mv) {
    return (
      <div aria-hidden="true" className="lx-app-ambient pointer-events-none fixed inset-0 overflow-hidden">
        <iframe
          key={mv}
          title=""
          src={`https://www.youtube.com/embed/${mv}?autoplay=1&mute=1&loop=1&playlist=${mv}&controls=0&playsinline=1&modestbranding=1&rel=0`}
          allow="autoplay; encrypted-media"
          tabIndex={-1}
          className="absolute top-1/2 left-1/2 h-[120vh] w-[213vh] -translate-x-1/2 -translate-y-1/2 border-0 opacity-60 blur-[12px]"
        />
        <div className="lx-app-ambient-scrim absolute inset-0" />
      </div>
    );
  }
  return (
    <div aria-hidden="true" className="lx-app-ambient pointer-events-none fixed inset-0 overflow-hidden">
      {track && (
        <div key={track.providerTrackId} className="lx-ambient-in absolute inset-0">
          <div className="absolute -inset-32 scale-125 opacity-60 blur-[96px] saturate-150">
            <TrackCover url={artwork} videoId={track.providerTrackId} className="size-full rounded-none bg-transparent" />
          </div>
        </div>
      )}
      <div className="lx-app-ambient-scrim absolute inset-0" />
    </div>
  );
}

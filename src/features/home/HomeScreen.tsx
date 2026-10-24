"use client";
import type { View } from "../../lib/store";
import { useLibrary } from "../../lib/library";
import { useHistory, clearHistory, refreshHistory } from "../../lib/history";
import { Button, Screen, Section, StatusNote } from "../../components/ui";
import { TrackRail } from "../../components/TrackRail";
import { statsLocal } from "../../services/tauri/stats";
import { topPlayed } from "../../lib/topPlayed";
import type { SearchHitDto } from "../../services/tauri/search";
import { useEffect, useState } from "react";
import LiveFeed from "../discovery/LiveFeed";
type Props = { onNavigate: (view: View) => void; onOpenPlaylist: (id: string) => void };
export default function HomeScreen(_props: Props) {
  const history = useHistory(s => s.tracks);
  const historyError = useHistory(s => s.error);
  const likes = useLibrary(s => s.tracks);
  // Play counts live in core; they are read again when a new play lands in the history.
  const [often, setOften] = useState<SearchHitDto[]>([]);
  const [oftenFailed, setOftenFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  // Read again whenever the history changes (not only when the top song does), when saving a play to the history fails (the
  // history array does not change then, but the count may), and when Retry is pressed.
  useEffect(() => {
    let live = true;
    statsLocal()
      .then((v) => { if (live) { setOften(topPlayed(v.tracks)); setOftenFailed(false); } })
      .catch(() => { if (live) setOftenFailed(true); });
    return () => { live = false; };
  }, [history, historyError, attempt]);
  return <Screen id="home-title" title="Listen Now" caption="Real music from YouTube Music, your likes and listening on this device.">
    <div className="flex flex-col gap-8">
      {historyError && <StatusNote role="alert">{historyError}<Button onClick={() => void refreshHistory()}>Retry history</Button></StatusNote>}
      {history.length > 0 && <Section id="recently-played" title="Recently played" action={<Button variant="ghost" onClick={() => void clearHistory()}>Clear history</Button>}><TrackRail label="Recently played" hits={history.slice(0, 24)} /><p className="mt-2 text-[12px] text-secondary-text">Saved on this device. Recorded after playback starts.</p></Section>}
      {oftenFailed && <StatusNote role="alert">Could not read your play counts, so Most played may be out of date.<Button onClick={() => setAttempt((n) => n + 1)}>Retry Most played</Button></StatusNote>}
      {often.length >= 3 && <Section id="home-most-played" title="Most played"><TrackRail label="Most played" hits={often} /></Section>}
      <LiveFeed kind="home" featured rail />
      {likes.length > 0 && <Section id="home-liked" title="Your liked songs"><TrackRail label="Your liked songs" hits={likes.slice(0, 24)} /></Section>}
    </div>
  </Screen>;
}

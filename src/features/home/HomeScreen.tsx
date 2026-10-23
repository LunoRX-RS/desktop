"use client";
import type { View } from "../../lib/store";
import { useLibrary } from "../../lib/library";
import { useHistory, clearHistory, refreshHistory } from "../../lib/history";
import { Button, Screen, Section, StatusNote } from "../../components/ui";
import { TrackRail } from "../../components/TrackRail";
import LiveFeed from "../discovery/LiveFeed";
type Props = { onNavigate: (view: View) => void; onOpenPlaylist: (id: string) => void };
export default function HomeScreen(_props: Props) {
  const history = useHistory(s => s.tracks);
  const historyError = useHistory(s => s.error);
  const likes = useLibrary(s => s.tracks);
  return <Screen id="home-title" title="Listen Now" caption="Real music from YouTube Music, your likes and listening on this device.">
    <div className="flex flex-col gap-8">
      {historyError && <StatusNote role="alert">{historyError}<Button onClick={() => void refreshHistory()}>Retry history</Button></StatusNote>}
      {history.length > 0 && <Section id="recently-played" title="Recently played" action={<Button variant="ghost" onClick={() => void clearHistory()}>Clear history</Button>}><TrackRail label="Recently played" hits={history.slice(0, 24)} /><p className="mt-2 text-[12px] text-secondary-text">Saved on this device. Recorded after playback starts.</p></Section>}
      <LiveFeed kind="home" featured rail />
      {likes.length > 0 && <Section id="home-liked" title="Your liked songs"><TrackRail label="Your liked songs" hits={likes.slice(0, 24)} /></Section>}
    </div>
  </Screen>;
}

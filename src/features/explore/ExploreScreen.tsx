"use client";
import { useState } from "react";
import { Button, Screen } from "../../components/ui";
import LiveFeed from "../discovery/LiveFeed";
import { loadFeed } from "../../lib/discovery";
const TOPICS = [["charts","Discover"],["thai","Thai music"],["pop","Pop"],["hiphop","Hip-hop"],["focus","Focus"],["chill","Chill"],["acoustic","Acoustic"]] as const;
export default function ExploreScreen() {
  const [topic,setTopic]=useState("charts");
  return <Screen id="explore-title" title="Explore" caption="Browse real songs from YouTube Music. Choose a song to play or like."><div className="flex flex-col gap-8"><nav aria-label="Explore music categories" className="flex flex-wrap gap-2">{TOPICS.map(([id,title])=><Button key={id} variant={topic===id?"primary":"secondary"} aria-pressed={topic===id} onPointerEnter={()=>void loadFeed("explore",id)} onFocus={()=>void loadFeed("explore",id)} onClick={()=>setTopic(id)}>{title}</Button>)}</nav><LiveFeed kind="explore" topic={topic} limit={20} /></div></Screen>;
}

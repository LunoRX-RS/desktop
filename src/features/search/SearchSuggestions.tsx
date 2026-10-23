"use client";
import { useEffect, useState } from "react";
import { Clock } from "lucide-react";
import { searchSuggestions } from "../../services/tauri/discovery";
import { searchQuery, searchCancel, type SearchHitDto } from "../../services/tauri/search";
import { useUi } from "../../lib/store";
import { Button } from "../../components/ui";
import { ResultRow } from "./SourceGroup";
import { historyMatches } from "../../lib/searchTaste";

// Short, so the panel feels instant; the keystroke that follows cancels the request that was in flight.
const TYPING_PAUSE_MS = 180;

export default function SearchSuggestions({ query, onChoose }: { query: string; onChoose: (query: string) => void }) {
  const recent = useUi((s) => s.recentSearches);
  const [terms,setTerms]=useState<string[]>([]);
  const [tracks,setTracks]=useState<SearchHitDto[]>([]);
  const [loading,setLoading]=useState(false);
  const [failed,setFailed]=useState(false);
  useEffect(()=>{
    let active=true;
    const text=query.trim();
    setTerms([]);setTracks([]);setFailed(false);
    if (text.length<2 || text.length>256) {setLoading(false);return;}
    const id=crypto.randomUUID();let started=false;
    setLoading(true);
    const timer=window.setTimeout(()=>{
      started=true;
      searchSuggestions(text).then(values=>{if(active)setTerms(values);}).catch(()=>{});
      // The same request the Search button sends, so pressing Enter afterwards is answered from the core's memory.
      searchQuery({query:text,providers:[],limit:10,cancellationId:id}).then(result=>{
        if(active){setTracks(result.hits.slice(0,3));setFailed(result.groups.some(g=>g.kind==="failed"));}
      }).catch(()=>{if(active)setFailed(true);}).finally(()=>{if(active)setLoading(false);});
    },TYPING_PAUSE_MS);
    return()=>{active=false;window.clearTimeout(timer);if(started)void searchCancel(id).catch(()=>{});};
  },[query]);
  const history=historyMatches(recent,query);
  if(query.trim().length<1)return null;
  if(query.trim().length<2&&history.length===0)return null;
  return <div aria-label="Search suggestions" className="flex flex-col gap-3">
    {history.length>0 && <div className="flex flex-col gap-1.5"><p className="text-[13px] font-semibold text-secondary-text">Searched before</p>
      <div className="flex flex-wrap gap-2">{history.map(item=><Button key={item} variant="ghost" onClick={()=>onChoose(item)}><Clock size={14} aria-hidden="true" />{item}</Button>)}</div></div>}
    {query.trim().length>=2 && <p className="text-[13px] font-semibold text-secondary-text">Suggestions</p>}
    {terms.length>0 && <div className="flex flex-wrap gap-2">{terms.map(term=><Button key={term} variant="ghost" onClick={()=>onChoose(term)}>{term}</Button>)}</div>}
    {loading && <p role="status" className="text-[12px] text-secondary-text">Finding suggested songs…</p>}
    {tracks.length>0 && <ul aria-label="Songs you can play now">{tracks.map(hit=><ResultRow key={`${hit.provider}:${hit.providerTrackId}`} hit={hit} />)}</ul>}
    {failed && <p role="status" className="text-[12px] text-secondary-text">Suggested songs are unavailable. You can still submit your search.</p>}
  </div>;
}

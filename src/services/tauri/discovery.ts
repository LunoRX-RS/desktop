import { invoke } from "@tauri-apps/api/core";
import type { SearchHitDto } from "./search";
import { prefetchCovers } from "../../lib/prefetch";
export interface FeedSection { title: string; tracks: SearchHitDto[] }
export const discoveryFeed = (kind: "home" | "explore", topic = "charts") =>
  invoke<FeedSection[]>("discovery_feed", { kind, topic }).then((sections) => { for (const s of sections) prefetchCovers(s.tracks); return sections; });
export const searchSuggestions = (query: string) => invoke<string[]>("search_suggestions", { query });

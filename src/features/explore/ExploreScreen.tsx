"use client";
import { useEffect, useState } from "react";
import { Art, SCRIM_BG } from "../../components/Art";
import { Button, Screen, Section, cx } from "../../components/ui";
import LiveFeed from "../discovery/LiveFeed";
import { loadFeed } from "../../lib/discovery";
import { toneFor } from "../../lib/media";
import { discoveryGenres, type Genre } from "../../services/tauri/discovery";

// Shown until YouTube Music's own moods and genres arrive, and kept if they never do.
const FALLBACK: Genre[] = [["thai", "Thai music"], ["pop", "Pop"], ["hiphop", "Hip-hop"], ["focus", "Focus"], ["chill", "Chill"], ["acoustic", "Acoustic"]]
  .map(([id, name]) => ({ id: `fixed:${id}`, name, mood: false }));
const KEY = "lunorx.exploreGenres";
// Two rows of four on a wide window; the rest open with "Show all".
const SHOWN = 8;
const topicOf = (g: Genre) => (g.id.startsWith("fixed:") ? g.id.slice(6) : `genre:${g.id}`);

function readGenres(): Genre[] | null {
  try { const v = JSON.parse(window.localStorage.getItem(KEY) ?? "null"); return Array.isArray(v) && v.length ? v : null; } catch { return null; }
}

/** DESIGN.md Mood tile: 96px, 12px radius, generated art under its tone scrim, the label inside at 20px bold. */
function MoodTile({ id, title, selected, onChoose }: { id: string; title: string; selected: boolean; onChoose: () => void }) {
  const tone = toneFor(id);
  return (
    <button
      type="button"
      aria-pressed={selected}
      onPointerEnter={() => void loadFeed("explore", id)}
      onFocus={() => void loadFeed("explore", id)}
      onClick={onChoose}
      className={cx(
        "lx-press group relative isolate h-24 overflow-hidden rounded-[12px] text-left",
        "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-text focus-visible:outline-solid",
        selected && "ring-2 ring-primary-text ring-offset-2 ring-offset-night-canvas",
      )}
    >
      <Art seed={id} tone={tone} grain={false} rounded="rounded-none" className="absolute inset-0 -z-10 transition-transform duration-200 group-hover:scale-[1.04]" />
      <span aria-hidden="true" className={cx("absolute inset-0 -z-10", SCRIM_BG[tone])} />
      <span className="absolute inset-x-4 bottom-3 line-clamp-2 text-[20px] leading-6 font-bold tracking-[-0.01em] text-primary-text">
        {title}
      </span>
    </button>
  );
}

export default function ExploreScreen() {
  const [topic, setTopic] = useState("charts");
  const [genres, setGenres] = useState<Genre[]>(FALLBACK);
  const [all, setAll] = useState(false);
  useEffect(() => {
    let live = true;
    const cached = readGenres();
    if (cached) setGenres(cached);
    discoveryGenres().then((list) => {
      if (!live || list.length === 0) return;
      setGenres(list);
      try { window.localStorage.setItem(KEY, JSON.stringify(list)); } catch { /* uncached */ }
    }, () => {});
    return () => { live = false; };
  }, []);

  const tiles = [{ id: "charts", name: "Discover" }, ...genres.map((g) => ({ id: topicOf(g), name: g.name }))];
  const shown = all ? tiles : tiles.slice(0, SHOWN);
  // A topic chosen from the hidden part stays visible after "Fewer".
  const picked = tiles.find((t) => t.id === topic);
  if (!all && picked && !shown.includes(picked)) shown.push(picked);
  const title = picked?.name ?? "Discover";

  return (
    <Screen id="explore-title" title="Explore" caption="New releases, charts, moods and genres from YouTube Music. Choose a song to play or like.">
      <div className="flex flex-col gap-10">
        <Section
          id="explore-moods"
          title="Moods and genres"
          action={tiles.length > SHOWN ? <Button variant="ghost" onClick={() => setAll(!all)}>{all ? "Fewer" : `Show all ${tiles.length}`}</Button> : undefined}
        >
          <nav aria-label="Explore music categories" className="grid grid-cols-2 gap-3 min-[1024px]:grid-cols-4">
            {shown.map((t) => (
              <MoodTile key={t.id} id={t.id} title={t.name} selected={topic === t.id} onChoose={() => setTopic(t.id)} />
            ))}
          </nav>
        </Section>

        <div className="flex flex-col gap-2">
          <h2 className="font-display text-[22px] leading-7 font-semibold tracking-[-0.015em] text-primary-text" aria-live="polite">
            {title}
          </h2>
          <LiveFeed key={topic} kind="explore" topic={topic} featured rail />
        </div>
      </div>
    </Screen>
  );
}

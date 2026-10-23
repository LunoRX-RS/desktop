"use client";
import { Infinity as InfinityIcon, Repeat, Repeat1 } from "lucide-react";
import { IconButton } from "./ui";
import { usePlayback } from "../lib/playback";
import { nextRepeat, setPlayMode, VIBES, type Repeat as RepeatMode, type Vibe } from "../lib/playMode";
import { cx } from "./ui";

/** Repeat: off, the whole list, or this track. */
export function RepeatButton({ className }: { className?: string }) {
  const repeat = usePlayback((s) => (s.autoplay.repeat ?? "off") as RepeatMode);
  const label = repeat === "off" ? "Repeat off" : repeat === "all" ? "Repeat all" : "Repeat this track";
  return (
    <IconButton label={label} active={repeat !== "off"} aria-pressed={repeat !== "off"} title={`${label}. Click to change.`} className={className} onClick={() => setPlayMode({ repeat: nextRepeat(repeat) })}>
      {repeat === "one" ? <Repeat1 size={18} /> : <Repeat size={18} />}
    </IconButton>
  );
}

/** Autoplay: keep playing related tracks once the list is used up. */
export function AutoplayButton({ className }: { className?: string }) {
  const on = usePlayback((s) => s.autoplay.radio ?? true);
  const label = on ? "Autoplay on" : "Autoplay off";
  return (
    <IconButton label={label} active={on} aria-pressed={on} title={`${label}. Plays related music when your list ends.`} className={className} onClick={() => setPlayMode({ autoplay: !on })}>
      <InfinityIcon size={18} />
    </IconButton>
  );
}

const VIBE_TEXT: Record<Vibe, { label: string; hint: string }> = {
  same: { label: "Same vibe", hint: "Keeps the mood, language and genre of what is playing. Sad stays sad, Isan stays Isan." },
  balanced: { label: "Balanced", hint: "Mostly the same, with a little room for a surprise." },
  explore: { label: "Discover", hint: "Follows YouTube Music's own mix, only without covers and long compilations." },
};

/** How closely the songs autoplay adds keep to what plays. Changing it re-plans the related part of the queue. */
export function VibePicker() {
  const vibe = usePlayback((s) => (s.autoplay.vibe ?? "balanced") as Vibe);
  return (
    <div role="radiogroup" aria-label="Autoplay style" className="flex flex-col gap-1.5">
      <div className="inline-flex gap-1 rounded-[10px] bg-frost-100 p-1">
        {VIBES.map((v) => (
          <button
            key={v}
            type="button"
            role="radio"
            aria-checked={vibe === v}
            title={VIBE_TEXT[v].hint}
            onClick={() => setPlayMode({ vibe: v })}
            className={cx(
              "h-8 flex-1 rounded-[8px] px-3 text-[12px] font-medium transition-colors",
              vibe === v ? "bg-frost-300 text-primary-text" : "text-secondary-text hover:bg-frost-200",
            )}
          >
            {VIBE_TEXT[v].label}
          </button>
        ))}
      </div>
      <p className="text-[12px] leading-4 text-secondary-text">{VIBE_TEXT[vibe].hint}</p>
    </div>
  );
}

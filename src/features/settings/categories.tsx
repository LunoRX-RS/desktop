"use client";

import {
  AudioLines,
  Bot,
  Bug,
  HardDrive,
  Info,
  Keyboard,
  MessageCircle,
  MicVocal,
  Palette,
  Play,
  ShieldCheck,
  UserRound,
  type LucideIcon,
} from "lucide-react";
import { About } from "./zones/About";
import { Accounts } from "./zones/Accounts";
import { AiControl } from "./zones/AiControl";
import { Appearance } from "./zones/Appearance";
import { Audio } from "./zones/Audio";
import { DevMode } from "./zones/DevMode";
import { Discord } from "./zones/Discord";
import { Lyrics } from "./zones/Lyrics";
import { Playback } from "./zones/Playback";
import { Privacy } from "./zones/Privacy";
import { Shortcuts } from "./zones/Shortcuts";
import { Storage } from "./zones/Storage";

// One zone per concern, each in its own file under zones/. The rail order is the order below.
export type CategoryKey =
  | "appearance"
  | "playback"
  | "audio"
  | "lyrics"
  | "storage"
  | "accounts"
  | "discord"
  | "ai"
  | "privacy"
  | "shortcuts"
  | "dev"
  | "about";

export interface Category {
  key: CategoryKey;
  label: string;
  description: string;
  Icon: LucideIcon;
}

export const CATEGORIES: Category[] = [
  { key: "appearance", label: "Appearance", description: "Theme, fonts, text size and text motion.", Icon: Palette },
  { key: "playback", label: "Playback", description: "What can play in the app and what happens at startup.", Icon: Play },
  { key: "audio", label: "Audio", description: "Stream quality, audio output and sound shaping.", Icon: AudioLines },
  { key: "lyrics", label: "Lyrics", description: "Which lyrics sources are used, and in what order.", Icon: MicVocal },
  { key: "storage", label: "Storage", description: "Disk cache and playlists kept for offline listening.", Icon: HardDrive },
  { key: "accounts", label: "Accounts & sync", description: "Music provider sign-in, your LunoRX account and playlist sync.", Icon: UserRound },
  { key: "discord", label: "Discord", description: "Show what you play on your Discord profile.", Icon: MessageCircle },
  { key: "ai", label: "AI control", description: "Let AI tools on this computer play, search and queue music.", Icon: Bot },
  { key: "privacy", label: "Privacy & data", description: "What the app may send about how it is used, what failed and what you did.", Icon: ShieldCheck },
  { key: "shortcuts", label: "Keyboard shortcuts", description: "Keys that work across the app.", Icon: Keyboard },
  { key: "dev", label: "Dev Mode", description: "Raw failure reasons, recent errors and what the app can reach.", Icon: Bug },
  { key: "about", label: "About", description: "Version, platform and what the app can report about itself.", Icon: Info },
];

/** Saved keys from the six-category layout, so a remembered choice still lands somewhere sensible. */
export const LEGACY_KEYS: Record<string, CategoryKey> = { general: "appearance", sync: "accounts" };

// Search index: row labels per zone and the group heading to focus. Static on purpose; keep in step with zones/.
export const SEARCH_INDEX: { cat: CategoryKey; group: string; label: string }[] = [
  { cat: "appearance", group: "settings-appearance", label: "Theme" },
  { cat: "appearance", group: "settings-appearance", label: "Reduced motion" },
  { cat: "appearance", group: "settings-appearance", label: "Reduced transparency" },
  { cat: "appearance", group: "settings-appstyle", label: "App font" },
  { cat: "appearance", group: "settings-appstyle", label: "Text size" },
  { cat: "appearance", group: "settings-text-lyrics", label: "Lyrics font" },
  { cat: "appearance", group: "settings-text-lyrics", label: "Lyrics motion" },
  { cat: "appearance", group: "settings-text-title", label: "Song title font" },
  { cat: "appearance", group: "settings-text-title", label: "Song title motion" },
  { cat: "appearance", group: "settings-fonts", label: "Imported fonts" },
  { cat: "privacy", group: "settings-usage", label: "Usage data" },
  { cat: "privacy", group: "settings-error-reports", label: "Error reports" },
  { cat: "privacy", group: "settings-activity", label: "Activity log" },
  { cat: "discord", group: "settings-discord", label: "Discord Rich Presence" },
  { cat: "discord", group: "settings-discord", label: "Discord Application ID" },
  { cat: "discord", group: "settings-discord-card", label: "Discord card design" },
  { cat: "ai", group: "settings-ai", label: "AI control MCP server Claude" },
  { cat: "ai", group: "settings-ai", label: "MCP port and token" },
  { cat: "playback", group: "settings-startup", label: "Restore last track" },
  { cat: "audio", group: "settings-effects", label: "Sound shaping equalizer bass treble speed" },
  { cat: "audio", group: "settings-effects", label: "Nightcore slowed reverb 8D 4D moving sound keep pitch" },
  { cat: "audio", group: "settings-audio", label: "Stream quality" },
  { cat: "audio", group: "settings-audio", label: "Output device speakers headphones bluetooth" },
  { cat: "storage", group: "settings-storage-usage", label: "Disk cache" },
  { cat: "storage", group: "settings-storage-usage", label: "Cache limit" },
  { cat: "storage", group: "settings-storage-usage", label: "Cached songs not kept offline" },
  { cat: "storage", group: "settings-storage-offline", label: "Playlists kept offline" },
  { cat: "audio", group: "settings-audio", label: "PCM output" },
  { cat: "audio", group: "settings-audio", label: "Sample rate" },
  { cat: "audio", group: "settings-audio", label: "Negotiated audio output" },
  { cat: "lyrics", group: "settings-lyrics", label: "Auto order" },
  { cat: "lyrics", group: "settings-lyrics", label: "Lyrics sources" },
  { cat: "lyrics", group: "settings-lyrics", label: "Use this source" },
  { cat: "playback", group: "settings-playback", label: "YouTube audio in the app" },
  { cat: "playback", group: "settings-playback", label: "Spotify audio" },
  { cat: "accounts", group: "settings-providers", label: "YouTube Music" },
  { cat: "accounts", group: "settings-providers", label: "Spotify" },
  { cat: "accounts", group: "settings-lunor-account", label: "LunoRX account" },
  { cat: "accounts", group: "settings-sync-server", label: "Server endpoint" },
  { cat: "accounts", group: "settings-sync", label: "Sync state" },
  { cat: "accounts", group: "settings-sync", label: "Pending writes and conflicts" },
  { cat: "shortcuts", group: "settings-keys-nav", label: "Back and forward" },
  { cat: "shortcuts", group: "settings-keys-nav", label: "Collapse or expand the sidebar" },
  { cat: "shortcuts", group: "settings-keys-play", label: "Play or pause" },
  { cat: "shortcuts", group: "settings-keys-maker", label: "Lyric Maker" },
  { cat: "shortcuts", group: "settings-keys-search", label: "Quick search and commands" },
  { cat: "shortcuts", group: "settings-keys-mac", label: "Playback menu (macOS)" },
  { cat: "dev", group: "settings-dev-mode", label: "Dev Mode" },
  { cat: "dev", group: "settings-dev-status", label: "YouTube session" },
  { cat: "dev", group: "settings-dev-status", label: "Native player (libmpv)" },
  { cat: "dev", group: "settings-dev-errors", label: "Recent errors" },
  { cat: "about", group: "settings-about", label: "App version" },
  { cat: "about", group: "settings-about", label: "Check for updates" },
  { cat: "about", group: "settings-about", label: "Update channel stable dev pre-release" },
  { cat: "about", group: "settings-about", label: "Platform" },
  { cat: "about", group: "settings-about", label: "Native player (libmpv)" },
  { cat: "about", group: "settings-about", label: "Cache management" },
  { cat: "about", group: "settings-about", label: "Core status" },
];

export function CategoryPane({ cat, mac, platform }: { cat: CategoryKey; mac: boolean; platform: string }) {
  switch (cat) {
    case "appearance": return <Appearance />;
    case "playback": return <Playback />;
    case "audio": return <Audio />;
    case "lyrics": return <Lyrics />;
    case "storage": return <Storage />;
    case "accounts": return <Accounts />;
    case "discord": return <Discord />;
    case "ai": return <AiControl />;
    case "privacy": return <Privacy />;
    case "shortcuts": return <Shortcuts mac={mac} />;
    case "dev": return <DevMode />;
    case "about": return <About platform={platform} />;
  }
}

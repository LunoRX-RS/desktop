import { invoke } from "@tauri-apps/api/core";

export type DiscordStatus = "off" | "connecting" | "connected" | "unavailable";

/** The user's card. Text fields are templates: {title} {artist} {artists} {status} {app}. Images: cover, logo, none or catalog:<name>. */
export interface CardDesign {
  details: string;
  state: string;
  pausedState: string;
  largeText: string;
  smallText: string;
  pausedSmallText: string;
  largeImage: string;
  smallImage: string;
  timestamps: "bar" | "elapsed" | "off";
  statusDisplay: "app" | "artist" | "title";
  tidyTitle: boolean;
  listenButton: boolean;
  appButton: boolean;
  /** Up to two. `url` is an https link or {track_url}. */
  buttons: { label: string; url: string }[];
  /** The user's own images: an https URL or an Art Asset key of their Application ID. Up to 12. */
  catalog: { name: string; value: string }[];
}

export interface DiscordSettings { enabled: boolean; clientId: string; defaultClientId: string; status: DiscordStatus; design: CardDesign }
export const discordSettingsGet = () => invoke<DiscordSettings>("discord_settings_get");
/** Rejects with "invalid application id", or the name of the design field that failed. Without `design` the saved one is kept. */
export const discordSettingsSet = (enabled: boolean, clientId: string, design?: CardDesign) => invoke<void>("discord_settings_set", { args: { enabled, clientId, design } });

/** The Rich Presence payload a design would send for the song playing now. Nothing is saved or sent. */
export interface CardPreview {
  details?: string; state?: string; details_url?: string;
  timestamps?: { start: number; end?: number };
  assets: { large_image: string; large_text?: string; small_image?: string; small_text?: string };
  buttons?: { label: string; url: string }[];
}
export const discordPreview = (design: CardDesign) => invoke<CardPreview>("discord_preview", { design });

export const DEFAULT_DESIGN: CardDesign = {
  details: "{title}", state: "{artists}", pausedState: "Paused · {artists}", largeText: "on {app}", smallText: "{app}", pausedSmallText: "{app} - paused",
  largeImage: "cover", smallImage: "logo", timestamps: "bar", statusDisplay: "artist", tidyTitle: true, listenButton: true, appButton: true, buttons: [], catalog: [],
};

/** Starting points. Applying one keeps the user's catalog and custom buttons. */
export const PRESETS: { key: string; label: string; design: Partial<CardDesign> }[] = [
  { key: "default", label: "LunoRX default", design: DEFAULT_DESIGN },
  { key: "minimal", label: "Minimal", design: { ...DEFAULT_DESIGN, state: "{artist}", largeText: "", smallImage: "none", timestamps: "off", listenButton: false, appButton: false } },
  { key: "artist", label: "Artist first", design: { ...DEFAULT_DESIGN, details: "{artist}", state: "{title}", statusDisplay: "artist", appButton: false } },
  { key: "elapsed", label: "Elapsed timer", design: { ...DEFAULT_DESIGN, timestamps: "elapsed", statusDisplay: "app" } },
];

// SAMPLE DATA. Every title, artist and album here is invented for the demo.
// It is not provider catalogue data and is never played: there is no audio source in this build.

export type Tone = "slate" | "violet" | "amber" | "rose" | "moss" | "ink";

export interface SampleLyricLine {
  at: number; // seconds
  text: string;
}

export type ProviderLabel = "YouTube Music" | "Spotify";

export interface SampleTrack {
  id: string;
  provider: ProviderLabel;
  title: string;
  artist: string;
  album: string;
  durationSec: number;
  tone: Tone;
  lyrics?: SampleLyricLine[];
}

export interface SampleAlbum {
  id: string;
  title: string;
  artist: string;
  year: number;
  tone: Tone;
  trackCount: number;
}

export interface SampleArtist {
  id: string;
  name: string;
  tone: Tone;
  genre: string;
}

export const SAMPLE_TRACKS: SampleTrack[] = [
  {
    id: "s-01",
    provider: "YouTube Music",
    title: "Low Tide Seasons",
    artist: "Halden Pike",
    album: "Harbour Lamps",
    durationSec: 214,
    tone: "slate",
    lyrics: [
      { at: 0, text: "Sample lyrics for the demo view." },
      { at: 8, text: "The tide goes out and leaves the lamps behind" },
      { at: 15, text: "Every window keeps a little light" },
      { at: 22, text: "We counted boats until the count was gone" },
      { at: 30, text: "And waited for the water to come back" },
      { at: 38, text: "Low tide seasons, slow and kind" },
      { at: 46, text: "Nothing here is hurried, nothing lost" },
    ],
  },
  {
    id: "s-02",
    provider: "Spotify",
    title: "Paper Orchard",
    artist: "Marisol Quen",
    album: "Quiet Machinery",
    durationSec: 187,
    tone: "amber",
  },
  {
    id: "s-03",
    provider: "YouTube Music",
    title: "Night Ferry",
    artist: "Halden Pike",
    album: "Harbour Lamps",
    durationSec: 242,
    tone: "slate",
  },
  {
    id: "s-04",
    provider: "Spotify",
    title: "Glass Corridor",
    artist: "The Ambers Choir",
    album: "Interior Weather",
    durationSec: 203,
    tone: "violet",
  },
  {
    id: "s-05",
    provider: "YouTube Music",
    title: "Second Hand Sun",
    artist: "Odile Marsh",
    album: "Bright Repairs",
    durationSec: 176,
    tone: "rose",
  },
  {
    id: "s-06",
    provider: "Spotify",
    title: "Moss Clock",
    artist: "Tamsin Vell",
    album: "Field Notes",
    durationSec: 258,
    tone: "moss",
  },
  {
    id: "s-07",
    provider: "Spotify",
    title: "Lantern Hour",
    artist: "Marisol Quen",
    album: "Quiet Machinery",
    durationSec: 221,
    tone: "amber",
  },
  {
    id: "s-08",
    provider: "YouTube Music",
    title: "Static Harbour",
    artist: "Ilan Strand",
    album: "Pale Frequencies",
    durationSec: 195,
    tone: "ink",
  },
];

export const SAMPLE_ALBUMS: SampleAlbum[] = [
  { id: "a-01", title: "Harbour Lamps", artist: "Halden Pike", year: 2024, tone: "slate", trackCount: 10 },
  { id: "a-02", title: "Quiet Machinery", artist: "Marisol Quen", year: 2025, tone: "amber", trackCount: 9 },
  { id: "a-03", title: "Interior Weather", artist: "The Ambers Choir", year: 2023, tone: "violet", trackCount: 12 },
  { id: "a-04", title: "Bright Repairs", artist: "Odile Marsh", year: 2026, tone: "rose", trackCount: 8 },
  { id: "a-05", title: "Field Notes", artist: "Tamsin Vell", year: 2022, tone: "moss", trackCount: 11 },
  { id: "a-06", title: "Pale Frequencies", artist: "Ilan Strand", year: 2025, tone: "ink", trackCount: 7 },
];

export const SAMPLE_ARTISTS: SampleArtist[] = [
  { id: "r-01", name: "Halden Pike", tone: "slate", genre: "Coastal folk" },
  { id: "r-02", name: "Marisol Quen", tone: "amber", genre: "Chamber pop" },
  { id: "r-03", name: "The Ambers Choir", tone: "violet", genre: "Choral ambient" },
  { id: "r-04", name: "Odile Marsh", tone: "rose", genre: "Bedroom soul" },
  { id: "r-05", name: "Tamsin Vell", tone: "moss", genre: "Field recordings" },
];

// Shown in Home "Made for you" and Explore. Titles are sample names only.
export const SAMPLE_MIXES: { id: string; title: string; blurb: string; tone: Tone }[] = [
  { id: "m-01", title: "Sample Mix: Late Harbour", blurb: "Slow coastal folk and choral wash", tone: "slate" },
  { id: "m-02", title: "Sample Mix: Bright Repairs", blurb: "Soul, chamber pop and small hooks", tone: "rose" },
  { id: "m-03", title: "Sample Mix: Field Day", blurb: "Acoustic, outdoors, unhurried", tone: "moss" },
  { id: "m-04", title: "Sample Mix: Night Shift", blurb: "Ambient and low synth for focus", tone: "violet" },
];

// Track ids per sample mix, for the details view. Sample data only.
export const MIX_TRACKS: Record<string, string[]> = {
  "m-01": ["s-01", "s-03", "s-04", "s-08"],
  "m-02": ["s-05", "s-02", "s-07"],
  "m-03": ["s-06", "s-02", "s-07", "s-03"],
  "m-04": ["s-08", "s-04", "s-06", "s-01"],
};

export function formatDuration(totalSec: number): string {
  const s = Math.max(0, Math.floor(totalSec));
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
}

// "Spotify and YouTube Music" style origin line for a set of sample tracks.
export function providerSummary(tracks: SampleTrack[]): string {
  return [...new Set(tracks.map((t) => t.provider))].sort().join(" and ");
}

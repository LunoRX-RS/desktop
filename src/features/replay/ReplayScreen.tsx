"use client";

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight, Music2, Play } from "lucide-react";
import { useUi } from "../../lib/store";
import { TrackCover } from "../../components/TrackCover";
import { Button, EmptyState, IconButton, Screen, Segmented, StatusNote } from "../../components/ui";
import { statsLocal, statsReport, type PlayCountDto, type ListeningReport } from "../../services/tauri/stats";

type Tab = "replay" | "dna" | "crossover" | "timeline" | "alltime";

const MS_HOUR = 3_600_000;

const monthRange = (y: number, m: number) => ({
  from: new Date(y, m, 1).getTime(),
  to: new Date(y, m + 1, 1).getTime(),
});
const yearRange = (y: number) => ({ from: new Date(y, 0, 1).getTime(), to: new Date(y + 1, 0, 1).getTime() });

function fmtMs(ms: number): string {
  const h = ms / MS_HOUR;
  if (h >= 1) return `${h.toFixed(1)}h`;
  return `${Math.round(ms / 60000)}m`;
}

/** The big figure: hours and minutes, the units small and quiet next to the numbers. */
function BigTime({ ms }: { ms: number }) {
  const total = Math.round(ms / 60000);
  const h = Math.floor(total / 60);
  const m = total % 60;
  const unit = "ml-1 mr-3 text-[26px] font-semibold tracking-normal text-secondary-text";
  return (
    <p className="font-display text-[84px] leading-none font-bold tracking-[-0.045em] tabular-nums" aria-label={`${h} hours ${m} minutes`}>
      {h > 0 && (<><span>{h}</span><span className={unit}>h</span></>)}
      <span>{m}</span><span className={unit}>m</span>
    </p>
  );
}

/** Ranked rows where the bar behind each row is its share of the leader: the numbers are readable and comparable at a glance. */
function RankedBars({ rows }: { rows: { key: string; label: string; sub?: string; plays: number; cover?: ReactNode }[] }) {
  const max = Math.max(1, ...rows.map((r) => r.plays));
  return (
    <ol className="flex flex-col gap-1.5">
      {rows.map((r, i) => (
        <li key={r.key} className="relative grid min-h-12 grid-cols-[22px_auto_minmax(0,1fr)_auto] items-center gap-3 overflow-hidden rounded-[10px] px-3">
          <span aria-hidden="true" className="absolute inset-y-0 left-0 rounded-[10px] bg-frost-200" style={{ width: `${(r.plays / max) * 100}%` }} />
          <span className="relative text-[12px] tabular-nums text-secondary-text">{i + 1}</span>
          <span className="relative">{r.cover}</span>
          <span className="relative min-w-0">
            <span className="block truncate text-[13px] leading-[18px] font-semibold">{r.label}</span>
            {r.sub && <span className="block truncate text-[12px] leading-4 text-secondary-text">{r.sub}</span>}
          </span>
          <span className="relative text-[13px] tabular-nums text-secondary-text">{r.plays}</span>
        </li>
      ))}
    </ol>
  );
}

const providerName = (p: string) => (p === "youtube" ? "YouTube" : p.charAt(0).toUpperCase() + p.slice(1));
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** A plain bar chart: the tallest bar in off-white, the rest Frost 300, the value in each bar's title. */
function Bars({ values, labels, label, height = 72, axis }: { values: number[]; labels?: string[]; label: string; height?: number; axis?: string[] }) {
  const max = Math.max(1, ...values);
  return (
    <div>
      <div role="img" aria-label={label} className="flex items-end gap-[3px]" style={{ height }}>
        {values.map((v, i) => (
          <div key={i} className={`min-w-[3px] flex-1 rounded-t-[3px] hover:bg-primary-text ${v === max && v > 0 ? "bg-primary-text" : "bg-frost-300"}`}
            style={{ height: `${Math.max(4, (v / max) * 100)}%` }} title={`${labels?.[i] ?? i}: ${v} plays`} />
        ))}
      </div>
      {axis && <div aria-hidden="true" className="mt-2 flex justify-between text-[11px] leading-4 text-tertiary-text">{axis.map((t, i) => <span key={`${i}:${t}`}>{t}</span>)}</div>}
    </div>
  );
}

/** Figures with no boxes: label over a large value, a hairline above the group. */
function Facts({ items }: { items: [string, ReactNode][] }) {
  return (
    <dl className="grid grid-cols-2 gap-x-6 gap-y-5 border-t border-hairline pt-6 @3xl:grid-cols-3">
      {items.map(([k, v]) => (
        <div key={k}>
          <dt className="text-[12px] leading-4 text-secondary-text">{k}</dt>
          <dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function Block({ title, aside, children }: { title: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between gap-4">
        <h3 className="font-display text-[20px] leading-7 font-bold tracking-[-0.02em]">{title}</h3>
        {aside && <p className="text-[13px] leading-[18px] text-secondary-text">{aside}</p>}
      </div>
      {children}
    </section>
  );
}

/** Shares of a whole as labelled bars (0 to 100). */
function ShareBars({ rows }: { rows: { label: string; value: number }[] }) {
  return (
    <ul className="flex flex-col gap-3">
      {rows.map((r, i) => (
        <li key={r.label} className="flex flex-col gap-1.5">
          <div className="flex items-baseline justify-between text-[13px] leading-[18px]"><span className="text-secondary-text">{r.label}</span><span className="font-semibold tabular-nums">{r.value}%</span></div>
          <div className="h-2 rounded-full bg-frost-200"><div className={`h-full rounded-full ${i === 0 ? "bg-primary-text" : "bg-secondary-text"}`} style={{ width: `${Math.min(100, r.value)}%` }} /></div>
        </li>
      ))}
    </ul>
  );
}

const bigNumber = "font-display text-[84px] leading-none font-bold tracking-[-0.045em] tabular-nums";
const bigUnit = "ml-2 text-[26px] font-semibold tracking-normal text-secondary-text";
const lead = "text-[14px] leading-5 text-secondary-text";
const noData = <EmptyState icon={<Music2 size={24} />} title="Nothing to show yet"><p>Play some songs and this fills in.</p></EmptyState>;
const loadingState = <p role="status" className="text-[13px] leading-[18px] text-secondary-text">Loading your listening…</p>;
const failedState = <EmptyState icon={<Music2 size={24} />} title="Could not read your stats"><p>Try again in a moment.</p></EmptyState>;

function keyOf(t: { title: string; artists: string[] }): string {
  return `${t.title.trim().toLowerCase()}::${t.artists.map((a) => a.trim().toLowerCase()).join(",")}`;
}

export default function ReplayScreen() {
  const navigate = useUi((s) => s.navigate);
  const [tab, setTab] = useState<Tab>("replay");
  const [local, setLocal] = useState<{ tracks: PlayCountDto[] } | null>(null);
  const [alltime, setAlltime] = useState<ListeningReport | null>(null);
  const [monthly, setMonthly] = useState<ListeningReport | null>(null);
  const [yearly, setYearly] = useState<ListeningReport | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);
  // Each load gets a number; only the newest one may write, so an earlier month cannot land on top of the current one.
  const latest = useRef(0);

  const now = new Date();
  const [viewYear, setViewYear] = useState(now.getFullYear());
  const [viewMonth, setViewMonth] = useState(now.getMonth());

  const load = useCallback(async () => {
    const id = ++latest.current;
    setBusy(true);
    try {
      const lv = await statsLocal();
      const at = await statsReport(0, Number.MAX_SAFE_INTEGER);
      const mr = monthRange(viewYear, viewMonth);
      const yr = yearRange(viewYear);
      const [m, y] = await Promise.all([
        statsReport(mr.from, mr.to),
        statsReport(yr.from, yr.to),
      ]);
      if (id !== latest.current) return;
      setLocal({ tracks: lv.tracks });
      setAlltime(at);
      setMonthly(m);
      setYearly(y);
      setNote(null);
    } catch {
      if (id === latest.current) setNote("Could not read listening stats. Play some songs first.");
    } finally {
      if (id === latest.current) setBusy(false);
    }
  }, [viewYear, viewMonth]);

  useEffect(() => { void load(); }, [load]);

  const allTracks = local?.tracks ?? [];
  const totalPlays = useMemo(() => allTracks.reduce((n, t) => n + t.plays, 0), [allTracks]);

  const wrapped = useMemo(() => {
    if (allTracks.length === 0 || totalPlays === 0) return null;
    const sorted = [...allTracks].sort((a, b) => b.plays - a.plays);
    const cumShare = (frac: number) => {
      const n = Math.max(1, Math.ceil(sorted.length * frac));
      const s = sorted.slice(0, n).reduce((x, t) => x + t.plays, 0);
      return Math.round((s / totalPlays) * 100);
    };
    const topTrack = sorted[0];
    const topArtistMap = new Map<string, number>();
    for (const t of allTracks) for (const a of t.artists) topArtistMap.set(a, (topArtistMap.get(a) ?? 0) + t.plays);
    const topArtist = [...topArtistMap].sort((a, b) => b[1] - a[1])[0];
    return {
      topTrackShare: Math.round((topTrack.plays / totalPlays) * 100),
      topArtistShare: topArtist ? Math.round((topArtist[1] / totalPlays) * 100) : 0,
      topTrack,
      topArtist: topArtist?.[0] ?? null,
      top1: cumShare(0.01),
      top5: cumShare(0.05),
      top10: cumShare(0.10),
      artistCount: topArtistMap.size,
    };
  }, [allTracks, totalPlays]);

  const crossover = useMemo(() => {
    const byProvider = new Map<string, { plays: number; tracks: number }>();
    for (const t of allTracks) {
      const cur = byProvider.get(t.provider) ?? { plays: 0, tracks: 0 };
      cur.plays += t.plays;
      cur.tracks += 1;
      byProvider.set(t.provider, cur);
    }
    const sets = new Map<string, Set<string>>();
    for (const t of allTracks) {
      if (!sets.has(t.provider)) sets.set(t.provider, new Set());
      sets.get(t.provider)!.add(keyOf(t));
    }
    const providers = [...sets.keys()];
    let overlap = 0;
    if (providers.length >= 2) {
      const a = sets.get(providers[0])!;
      const b = sets.get(providers[1])!;
      for (const k of a) if (b.has(k)) overlap += 1;
    }
    return { byProvider, providers, overlap };
  }, [allTracks]);

  const isCurrentMonth = viewYear === now.getFullYear() && viewMonth === now.getMonth();
  // Every calendar day of the viewed month, zero-filled. Report days are day numbers in the local zone (Date.UTC of the same calendar date).
  const monthDays = useMemo(() => {
    const count = new Date(viewYear, viewMonth + 1, 0).getDate();
    const byIndex = new Map((monthly?.byDay ?? []).map((d) => [d.day, d.plays]));
    return Array.from({ length: count }, (_, i) => {
      const day = Math.round(Date.UTC(viewYear, viewMonth, i + 1) / 86_400_000);
      return { day, plays: byIndex.get(day) ?? 0 };
    });
  }, [monthly, viewYear, viewMonth]);
  const monthLabel = new Date(viewYear, viewMonth, 1).toLocaleString(undefined, { month: "long", year: "numeric" });

  const peakHour = alltime ? alltime.byHour.indexOf(Math.max(...alltime.byHour)) : 0;

  return (
    <>
    <Screen
      id="replay-title"
      title="Unified Replay"
      caption="Built from this device's play log. Nothing leaves it unless cloud sync is on."
    >
      {note && <StatusNote tone="amber">{note}</StatusNote>}

      <div className="self-start">
      <Segmented<Tab>
        label="Replay section"
        value={tab}
        options={[
          { key: "replay", label: "Replay" },
          { key: "dna", label: "Music DNA" },
          { key: "crossover", label: "Crossover" },
          { key: "timeline", label: "Timeline" },
          { key: "alltime", label: "All-time" },
        ]}
        onChange={setTab}
      />
      </div>

      {tab === "replay" && (
        <div className="flex flex-col gap-12">
          <div className="flex items-center justify-between gap-4">
            <h2 className="font-display text-[30px] leading-9 font-bold tracking-[-0.025em]" aria-live="polite">{monthLabel}</h2>
            <div className="flex items-center gap-1">
              <IconButton label="Previous month" className="size-9" onClick={() => { const d = new Date(viewYear, viewMonth - 1, 1); setViewYear(d.getFullYear()); setViewMonth(d.getMonth()); }}><ChevronLeft size={20} /></IconButton>
              <IconButton label="Next month" className="size-9" disabled={isCurrentMonth} onClick={() => { const d = new Date(viewYear, viewMonth + 1, 1); setViewYear(d.getFullYear()); setViewMonth(d.getMonth()); }}><ChevronRight size={20} /></IconButton>
            </div>
          </div>

          {monthly && monthly.plays > 0 && (
            <>
              <section aria-label="This month" className="grid gap-10 @3xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
                <div className="flex flex-col gap-3">
                  <p className="text-[14px] leading-5 text-secondary-text">You listened for</p>
                  <BigTime ms={monthly.listenedMs} />
                  <p className="text-[15px] leading-[22px] text-secondary-text">
                    {monthly.plays} plays · {monthly.uniqueTracks} songs · {monthly.uniqueArtists} artists{monthly.newTracks > 0 ? ` · ${monthly.newTracks} new to you` : ""}
                  </p>
                  {monthly.byDay.length > 0 && (
                    <div className="mt-4">
                      <div role="img" aria-label="Plays per day this month" className="flex h-20 items-end gap-[3px]">
                        {monthDays.map((d) => {
                          const max = Math.max(1, ...monthDays.map((x) => x.plays));
                          const date = new Date(d.day * 86_400_000);
                          return <div key={d.day} className={`min-w-[3px] flex-1 rounded-t-[3px] hover:bg-primary-text ${d.plays === max ? "bg-primary-text" : "bg-frost-300"}`} style={{ height: `${Math.max(6, (d.plays / max) * 100)}%` }} title={`${date.toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "UTC" })}: ${d.plays} plays`} />;
                        })}
                      </div>
                      <p className="mt-2 text-[12px] leading-4 text-secondary-text">{monthly.activeDays} days with music{isCurrentMonth && monthly.currentStreak > 1 ? ` · ${monthly.currentStreak} day streak` : ""}</p>
                    </div>
                  )}
                </div>
                {monthly.topTracks[0] && (
                  <figure className="flex flex-col gap-4">
                    <TrackCover url={monthly.topTracks[0].artworkUrl ?? null} videoId={monthly.topTracks[0].providerTrackId} title={monthly.topTracks[0].title} className="aspect-square w-full max-w-[300px] rounded-[10px] shadow-[0_18px_40px_rgba(0,0,0,0.5)]" />
                    <figcaption>
                      <p className="text-[13px] leading-[18px] text-secondary-text">Song of the month</p>
                      <p className="font-display text-[22px] leading-7 font-bold tracking-[-0.02em]">{monthly.topTracks[0].title}</p>
                      <p className="text-[14px] leading-5 text-secondary-text">{monthly.topTracks[0].artists.join(", ")} · {monthly.topTracks[0].plays} plays</p>
                    </figcaption>
                  </figure>
                )}
              </section>

              <div className="grid gap-10 @3xl:grid-cols-2">
                {monthly.topArtists.length > 0 && (
                  <section className="flex flex-col gap-3">
                    <h3 className="font-display text-[20px] leading-7 font-bold tracking-[-0.02em]">Top artists</h3>
                    <RankedBars rows={monthly.topArtists.slice(0, 5).map((a) => ({ key: a.name, label: a.name, sub: fmtMs(a.listenedMs), plays: a.plays }))} />
                  </section>
                )}
                {monthly.topTracks.length > 0 && (
                  <section className="flex flex-col gap-3">
                    <h3 className="font-display text-[20px] leading-7 font-bold tracking-[-0.02em]">Top songs</h3>
                    <RankedBars rows={monthly.topTracks.slice(0, 5).map((t) => ({ key: `${t.provider}:${t.providerTrackId}`, label: t.title, sub: t.artists.join(", "), plays: t.plays, cover: <TrackCover url={t.artworkUrl ?? null} videoId={t.providerTrackId} className="size-9" /> }))} />
                  </section>
                )}
              </div>

              <section className="flex flex-col gap-3">
                <div className="flex items-baseline justify-between gap-4">
                  <h3 className="font-display text-[20px] leading-7 font-bold tracking-[-0.02em]">When you listen</h3>
                  <p className="text-[13px] leading-[18px] text-secondary-text">Most around {monthly.byHour.indexOf(Math.max(...monthly.byHour))}:00</p>
                </div>
                <div role="img" aria-label="Plays by hour of day" className="flex h-16 items-end gap-[3px]">
                  {monthly.byHour.map((v, i) => {
                    const max = Math.max(1, ...monthly.byHour);
                    return <div key={i} className={`flex-1 rounded-t-[3px] ${v === max ? "bg-primary-text" : "bg-frost-300"}`} style={{ height: `${Math.max(4, (v / max) * 100)}%` }} title={`${i}:00: ${v} plays`} />;
                  })}
                </div>
                <div aria-hidden="true" className="flex justify-between text-[11px] leading-4 text-tertiary-text"><span>0:00</span><span>6:00</span><span>12:00</span><span>18:00</span><span>23:00</span></div>
              </section>
            </>
          )}

          {monthly && monthly.plays === 0 && allTracks.length > 0 && (
            <EmptyState icon={<Music2 size={24} />} title={`No plays in ${monthLabel}`}>
              <p>Pick another month, or play something and it is counted here.</p>
            </EmptyState>
          )}

          {yearly && yearly.plays > 0 && (
            <dl aria-label={`${viewYear} in numbers`} className="grid grid-cols-2 gap-x-6 gap-y-4 border-t border-hairline pt-6 @3xl:grid-cols-4">
              {([["Plays", yearly.plays], ["Listened", fmtMs(yearly.listenedMs)], ["Songs", yearly.uniqueTracks], ["Artists", yearly.uniqueArtists]] as [string, string | number][]).map(([k, v]) => (
                <div key={k}>
                  <dt className="text-[12px] leading-4 text-secondary-text">{viewYear} · {k}</dt>
                  <dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{v}</dd>
                </div>
              ))}
            </dl>
          )}

          {allTracks.length === 0 && (
            <EmptyState icon={<Play size={24} />} title="No plays counted yet">
              <p>Songs you play are counted here from now on.</p>
              <Button variant="primary" onClick={() => navigate("search")}>Find songs</Button>
            </EmptyState>
          )}
        </div>
      )}

      {tab === "dna" && (
        <div className="flex flex-col gap-12">
          {!alltime || !wrapped ? (busy ? loadingState : note ? failedState : noData) : (
            <>
              <section aria-label="Your sound" className="grid gap-10 @3xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
                <div className="flex flex-col gap-3">
                  <p className={lead}>Your sound is led by</p>
                  <p className="font-display line-clamp-2 text-[56px] leading-[1.02] font-bold tracking-[-0.04em]" title={wrapped.topArtist ?? undefined}>{wrapped.topArtist ?? "Many artists"}</p>
                  <p className="text-[15px] leading-[22px] text-secondary-text">{wrapped.topArtistShare}% of your plays · {wrapped.artistCount} artists across {allTracks.length} songs</p>
                </div>
                {allTracks.length >= 20 ? (
                <div className="flex flex-col gap-4">
                  <p className={lead}>How much of your listening the top songs make up</p>
                  <ShareBars rows={[{ label: "Top 1% of songs", value: wrapped.top1 }, { label: "Top 5%", value: wrapped.top5 }, { label: "Top 10%", value: wrapped.top10 }]} />
                  <p className="text-[13px] leading-[18px] text-secondary-text">Your #1 song, {wrapped.topTrack.title}, is {wrapped.topTrackShare}% of everything you play.</p>
                </div>
                ) : (
                  <p className="self-end text-[15px] leading-[22px] text-secondary-text">Your #1 song, {wrapped.topTrack.title}, is {wrapped.topTrackShare}% of everything you play. Once you have 20 songs, this shows how concentrated your taste is.</p>
                )}
              </section>

              <Facts items={[
                ["Peak hour", `${peakHour}:00`],
                ["Active days", alltime.activeDays],
                ["Current streak", `${alltime.currentStreak} days`],
                ["Longest streak", `${alltime.longestStreak} days`],
                ["Avg. completion", `${alltime.avgCompletion}%`],
                ["Skips", alltime.skips],
              ]} />

              <div className="grid gap-10 @3xl:grid-cols-2">
                <Block title="By weekday" aside={`Most on ${WEEKDAYS[alltime.byWeekday.indexOf(Math.max(...alltime.byWeekday))]}`}>
                  <Bars values={alltime.byWeekday} labels={WEEKDAYS} label="Plays by weekday" height={96} axis={["S", "M", "T", "W", "T", "F", "S"]} />
                </Block>
                {alltime.topArtists.length > 0 && (
                  <Block title="Top artists">
                    <RankedBars rows={alltime.topArtists.slice(0, 6).map((a) => ({ key: a.name, label: a.name, sub: fmtMs(a.listenedMs), plays: a.plays }))} />
                  </Block>
                )}
              </div>
            </>
          )}
        </div>
      )}

      {tab === "crossover" && (
        <div className="flex flex-col gap-12">
          {crossover.providers.length === 0 ? noData : (
            <>
              <section aria-label="Providers" className="flex flex-col gap-4">
                <p className={lead}>{crossover.providers.length >= 2 ? "Songs you play on more than one provider" : "All your plays are on"}</p>
                <p className="font-display text-[56px] leading-[1.02] font-bold tracking-[-0.04em]">{crossover.providers.length >= 2 ? `${crossover.overlap} in common` : providerName(crossover.providers[0])}</p>
                <p className="text-[15px] leading-[22px] text-secondary-text">
                  {crossover.providers.length >= 2
                    ? `Matched by title and artists between ${crossover.providers.map(providerName).join(" and ")}.`
                    : "Connect and play from another provider to compare them here."}
                </p>
                {(() => {
                  const entries = [...crossover.byProvider.entries()];
                  const total = Math.max(1, entries.reduce((n, [, v]) => n + v.plays, 0));
                  const tones = ["bg-primary-text", "bg-secondary-text", "bg-frost-300", "bg-frost-200"];
                  return (
                    <div className="mt-2 flex flex-col gap-3">
                      <div role="img" aria-label="Plays by provider" className="flex h-3 gap-0.5 overflow-hidden rounded-full">
                        {entries.map(([name, v], i) => <div key={name} className={tones[Math.min(i, 3)]} style={{ width: `${(v.plays / total) * 100}%` }} title={`${name}: ${v.plays} plays`} />)}
                      </div>
                      <ul className="flex flex-wrap gap-x-8 gap-y-2">
                        {entries.map(([name, v], i) => (
                          <li key={name} className="flex items-center gap-2 text-[13px] leading-[18px]">
                            <span aria-hidden="true" className={`size-2.5 rounded-full ${tones[Math.min(i, 3)]}`} />
                            <span className="font-semibold">{providerName(name)}</span>
                            <span className="text-secondary-text tabular-nums">{Math.round((v.plays / total) * 100)}% · {v.plays} plays · {v.tracks} songs</span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  );
                })()}
              </section>

              {alltime && alltime.topTracks.length > 0 && (
                <div className={`grid gap-10 ${crossover.providers.length > 1 ? "@3xl:grid-cols-2" : ""}`}>
                  {crossover.providers.map((p) => {
                    const ts = alltime.topTracks.filter((t) => t.provider === p).slice(0, 5);
                    if (ts.length === 0) return null;
                    return (
                      <Block key={p} title={`Top on ${providerName(p)}`}>
                        <RankedBars rows={ts.map((t) => ({ key: `${t.provider}:${t.providerTrackId}`, label: t.title, sub: t.artists.join(", "), plays: t.plays, cover: <TrackCover url={t.artworkUrl ?? null} videoId={t.providerTrackId} className="size-9" /> }))} />
                      </Block>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>
      )}

      {tab === "timeline" && (
        <div className="flex flex-col gap-12">
          {!alltime || alltime.plays === 0 ? (busy ? loadingState : note ? failedState : noData) : (
            <>
              <section aria-label="Streak" className="grid gap-10 @3xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)] @3xl:items-end">
                <div className="flex flex-col gap-3">
                  <p className={lead}>Current streak</p>
                  <p className={bigNumber}>{alltime.currentStreak}<span className={bigUnit}>{alltime.currentStreak === 1 ? "day" : "days"}</span></p>
                  <p className="text-[15px] leading-[22px] text-secondary-text">Days in a row with at least one song.</p>
                </div>
                <dl className="grid grid-cols-2 gap-6">
                  <div><dt className="text-[12px] leading-4 text-secondary-text">Longest streak</dt><dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{alltime.longestStreak} days</dd></div>
                  <div><dt className="text-[12px] leading-4 text-secondary-text">Days with music</dt><dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{alltime.activeDays}</dd></div>
                </dl>
              </section>

              {(() => {
                const days = alltime.byDay.slice(-45);
                const fmt = (d: number) => new Date(d * 86_400_000).toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "UTC" });
                if (days.length === 0) return null;
                return (
                  <Block title="Recent active days" aside={(() => { const top = days.reduce((a, b) => (b.plays > a.plays ? b : a)); return `Busiest: ${fmt(top.day)}, ${top.plays} plays`; })()}>
                    <Bars values={days.map((d) => d.plays)} labels={days.map((d) => fmt(d.day))} label="Plays per day" height={96} axis={[fmt(days[0].day), fmt(days[Math.floor(days.length / 2)].day), fmt(days[days.length - 1].day)]} />
                  </Block>
                );
              })()}

              <div className="grid gap-10 @3xl:grid-cols-2">
                <Block title="By hour of day" aside={`Peak at ${peakHour}:00`}>
                  <Bars values={alltime.byHour} labels={alltime.byHour.map((_, i) => `${i}:00`)} label="Plays by hour of day" height={96} axis={["0:00", "6:00", "12:00", "18:00", "23:00"]} />
                </Block>
                <Block title="By weekday" aside={`Most on ${WEEKDAYS[alltime.byWeekday.indexOf(Math.max(...alltime.byWeekday))]}`}>
                  <Bars values={alltime.byWeekday} labels={WEEKDAYS} label="Plays by weekday" height={96} axis={["S", "M", "T", "W", "T", "F", "S"]} />
                </Block>
              </div>
            </>
          )}
        </div>
      )}

      {tab === "alltime" && (
        <div className="flex flex-col gap-12">
          {!alltime || alltime.plays === 0 ? (
            allTracks.length === 0 ? (
              <EmptyState icon={<Play size={24} />} title="No plays counted yet">
                <p>Songs you play are counted here from now on.</p>
                <Button variant="primary" onClick={() => navigate("search")}>Find songs</Button>
              </EmptyState>
            ) : noData
          ) : (
            <>
              <section aria-label="Totals" className="grid gap-10 @3xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)] @3xl:items-end">
                <div className="flex flex-col gap-3">
                  <p className={lead}>You have listened for</p>
                  <BigTime ms={alltime.listenedMs} />
                  <p className="text-[15px] leading-[22px] text-secondary-text">
                    {alltime.plays} plays · {alltime.uniqueTracks} songs · {alltime.uniqueArtists} artists
                    {alltime.byDay[0] ? ` · since ${new Date(alltime.byDay[0].day * 86_400_000).toLocaleDateString(undefined, { month: "long", year: "numeric", timeZone: "UTC" })}` : ""}
                  </p>
                </div>
                <dl className="grid grid-cols-2 gap-6">
                  <div><dt className="text-[12px] leading-4 text-secondary-text">Avg. completion</dt><dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{alltime.avgCompletion}%</dd></div>
                  <div><dt className="text-[12px] leading-4 text-secondary-text">Skips</dt><dd className="font-display text-[26px] leading-8 font-bold tracking-[-0.02em] tabular-nums">{alltime.skips}</dd></div>
                </dl>
              </section>

              <div className="grid gap-10 @3xl:grid-cols-2">
                {alltime.topTracks.length > 0 && (
                  <Block title="Top songs">
                    <RankedBars rows={alltime.topTracks.slice(0, 10).map((t) => ({ key: `${t.provider}:${t.providerTrackId}`, label: t.title, sub: t.artists.join(", "), plays: t.plays, cover: <TrackCover url={t.artworkUrl ?? null} videoId={t.providerTrackId} className="size-9" /> }))} />
                  </Block>
                )}
                {alltime.topArtists.length > 0 && (
                  <Block title="Top artists">
                    <RankedBars rows={alltime.topArtists.slice(0, 10).map((a) => ({ key: a.name, label: a.name, sub: fmtMs(a.listenedMs), plays: a.plays }))} />
                  </Block>
                )}
              </div>
            </>
          )}
        </div>
      )}

      {busy && <p className="text-[12px] text-secondary-text">Loading…</p>}
    </Screen>
    </>
  );
}
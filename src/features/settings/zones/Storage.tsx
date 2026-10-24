"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Button } from "../../../components/ui";
import { Banner, Inset, NoteRow, SettingRow, SkeletonRows } from "../rows";
import {
  offlineClearTransient,
  offlineError,
  offlineRemove,
  offlineSetLimit,
  offlineUsage,
  type CacheUsage,
} from "../../../services/tauri/offline";

const KiB = 1024;
const MiB = KiB ** 2;
const GiB = KiB ** 3;

function formatBytes(bytes: number): string {
  if (bytes >= GiB) return `${(bytes / GiB).toFixed(1).replace(/\.0$/, "")} GB`;
  if (bytes >= MiB) return `${Math.round(bytes / MiB)} MB`;
  return `${Math.round(bytes / KiB)} KB`;
}

const MIN_LIMIT_MB = 64;
const MAX_LIMIT_MB = 20480;

export function Storage() {
  const [usage, setUsage] = useState<CacheUsage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [freed, setFreed] = useState<number | null>(null);
  const [limit, setLimit] = useState(String(MIN_LIMIT_MB));
  const version = useRef(0);

  const refresh = useCallback(async () => {
    const current = ++version.current;
    try {
      const value = await offlineUsage();
      if (current !== version.current) return;
      setUsage(value);
      setError(null);
    } catch (e) {
      if (current === version.current) setError(offlineError(e));
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    void listen("core://offline/changed", () => void refresh()).then(
      (fn) => {
        if (disposed) fn();
        else {
          unlisten = fn;
          void refresh();
        }
      },
      () => {
        if (!disposed) setError("Storage is unavailable.");
      },
    );
    return () => {
      disposed = true;
      version.current++;
      unlisten?.();
    };
  }, [refresh]);

  useEffect(() => {
    if (usage) setLimit(String(Math.round(usage.limitBytes / MiB)));
  }, [usage?.limitBytes]);

  const act = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      setError(null);
      try {
        await action();
        await refresh();
      } catch (e) {
        setError(offlineError(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const limitMb = Number(limit);
  const limitValid = Number.isInteger(limitMb) && limitMb >= MIN_LIMIT_MB && limitMb <= MAX_LIMIT_MB;
  const over = usage !== null && usage.totalBytes >= usage.limitBytes * 0.9;

  if (usage === null) {
    return error ? (
      <Banner tone="bad" role="alert">
        {error}
      </Banner>
    ) : (
      <Inset id="settings-storage-usage" title="Disk cache" hideTitle>
        <SkeletonRows rows={5} />
      </Inset>
    );
  }

  const share = (bytes: number) => Math.min(100, (bytes / usage.limitBytes) * 100);

  return (
    <>
      <Inset id="settings-storage-usage" title="Disk cache">
        <div className="flex flex-col gap-2 border-b border-hairline px-4 py-3">
          <div
            role="meter"
            aria-valuemin={0}
            aria-valuemax={usage.limitBytes}
            aria-valuenow={usage.totalBytes}
            aria-valuetext={`${formatBytes(usage.totalBytes)} of ${formatBytes(usage.limitBytes)} used`}
            className="flex h-1.5 w-full overflow-hidden rounded-full bg-frost-300"
          >
            <div
              className={over ? "bg-caution-amber" : "bg-secondary-text"}
              style={{ width: `${share(usage.audioBytes)}%` }}
            />
            <div
              className={over ? "bg-caution-amber/70" : "bg-secondary-text/50"}
              style={{ width: `${share(usage.artworkBytes + usage.lyricBytes)}%` }}
            />
          </div>
          <p className="text-[12px] leading-4 text-secondary-text">
            {formatBytes(usage.totalBytes)} of {formatBytes(usage.limitBytes)} · {formatBytes(usage.audioBytes)} songs ·{" "}
            {formatBytes(usage.artworkBytes)} artwork · {formatBytes(usage.lyricBytes)} lyrics
          </p>
          <p className="text-[12px] leading-4 text-secondary-text">
            Songs you play are cached and the least used are dropped first. Playlists kept offline and the
            song that is playing are never dropped.
          </p>
        </div>
        <SettingRow
          label="Cache limit"
          htmlFor="cache-limit"
          hint="Between 64 MB and 20 GB. Lowering it removes cached songs at once."
        >
          <input
            id="cache-limit"
            type="number"
            min={MIN_LIMIT_MB}
            max={MAX_LIMIT_MB}
            step={1}
            value={limit}
            onChange={(e) => setLimit(e.target.value)}
            className="w-28 rounded bg-frost-200 px-3 py-2 text-[13px] text-primary-text"
          />
          <Button variant="ghost" disabled={busy || !limitValid} onClick={() => void act(() => offlineSetLimit(limitMb * MiB))}>
            Apply
          </Button>
        </SettingRow>
        <SettingRow
          label="Cached songs not kept offline"
          hint="Clearing frees these now. Kept playlists and the song that is playing stay."
        >
          <span>{formatBytes(usage.clearableBytes)}</span>
          <Button
            variant="ghost"
            disabled={busy || usage.clearableBytes === 0}
            onClick={() => void act(async () => setFreed(await offlineClearTransient()))}
          >
            Clear cached
          </Button>
        </SettingRow>
        {freed !== null && <NoteRow tone="off" role="status">Freed {formatBytes(freed)}.</NoteRow>}
      </Inset>
      <Inset id="settings-storage-offline" title="Playlists kept offline">
        {usage.playlists.length === 0 ? (
          <NoteRow tone="off">Nothing is kept offline yet. Choose Keep offline on a playlist in Library.</NoteRow>
        ) : (
          <>
            {usage.sharedBytes > 0 && (
              <NoteRow tone="off">
                {formatBytes(usage.sharedBytes)} is kept by more than one playlist. Those songs count in every
                playlist that keeps them, so these sizes are not a split of the total.
              </NoteRow>
            )}
            {usage.playlists.map((p) => (
              <SettingRow key={p.id} label={p.name} hint={`${p.tracks} songs`}>
                <span>{formatBytes(p.bytes)}</span>
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() =>
                    void act(async () => {
                      await offlineRemove(p.id);
                      setFreed(null);
                    })
                  }
                >
                  Remove
                </Button>
              </SettingRow>
            ))}
          </>
        )}
      </Inset>
      {error && (
        <Banner tone="bad" role="alert">
          {error}
        </Banner>
      )}
    </>
  );
}

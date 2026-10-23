"use client";

import { Download, RotateCw, X } from "lucide-react";
import { Button } from "../ui";
import { installUpdate, restartApp, useUpdater } from "../../lib/updater";

/** A quiet note above the player bar when a newer signed version exists. Install only on the user's word. */
export function UpdateBanner() {
  const state = useUpdater((s) => s.state);
  const dismissed = useUpdater((s) => s.dismissed);
  if (dismissed) return null;
  if (state.kind !== "available" && state.kind !== "downloading" && state.kind !== "installed") return null;

  return (
    <div role="status" aria-live="polite" className="glass-solid lx-rise fixed right-4 bottom-[104px] z-[70] flex w-[min(380px,calc(100vw-32px))] flex-col gap-3 rounded-[12px] p-4 shadow-[0_18px_48px_rgba(0,0,0,0.5)]">
      <div className="flex items-start gap-3">
        <Download size={18} aria-hidden="true" className="mt-0.5 shrink-0 text-primary-text" />
        <div className="min-w-0 flex-1">
          <p className="text-[13px] leading-[18px] font-semibold text-primary-text">
            {state.kind === "installed" ? `LunoRX ${state.version} is installed` : `LunoRX ${state.version} is available`}
          </p>
          <p className="text-[12px] leading-4 text-secondary-text">
            {state.kind === "available" && "Signed by the LunoRX team. Your playlists and settings stay as they are."}
            {state.kind === "downloading" && (state.percent === null ? "Downloading…" : `Downloading… ${state.percent}%`)}
            {state.kind === "installed" && "Restart to use it. Playback stops while the app restarts."}
          </p>
        </div>
        {state.kind === "available" && (
          <button type="button" aria-label="Not now" onClick={() => useUpdater.setState({ dismissed: true })} className="-m-1 flex size-8 shrink-0 items-center justify-center rounded-[8px] text-secondary-text hover:bg-frost-100 hover:text-primary-text">
            <X size={16} aria-hidden="true" />
          </button>
        )}
      </div>
      {state.kind === "available" && (
        <Button variant="primary" className="w-full" onClick={() => void installUpdate()}>Install update</Button>
      )}
      {state.kind === "installed" && (
        <Button variant="primary" className="w-full" onClick={() => void restartApp()}>
          <RotateCw size={14} aria-hidden="true" />
          Restart now
        </Button>
      )}
    </div>
  );
}

"use client";
import { useEffect, useState } from "react";
import { CircleAlert, X } from "lucide-react";
import { Button } from "../ui";
import { reportsAsked, reportsFlush, reportsGet, reportsSetEnabled, type ErrorLogViewDto } from "../../services/tauri/reports";

const SESSION_KEY = "lunorx.reportPrompt";

/** Asks once about the failures waiting on this device, then never again: Settings governs from there. Records stay
 * on the device whatever the answer, and the literal masked text is shown before any button is pressed. */
export function ErrorReportBanner() {
  const [view, setView] = useState<ErrorLogViewDto | null>(null);
  useEffect(() => {
    try {
      if (sessionStorage.getItem(SESSION_KEY)) return;
    } catch { /* storage blocked: ask normally */ }
    reportsGet()
      .then((v) => { if (!v.asked && v.unsent > 0) setView(v); })
      .catch(() => {});
  }, []);
  if (!view) return null;
  const close = () => {
    try { sessionStorage.setItem(SESSION_KEY, "1"); } catch { /* this session only */ }
    setView(null);
  };
  async function decide(answer: "once" | "always" | "never") {
    try {
      if (answer === "always") {
        await reportsSetEnabled(true);
        await reportsAsked();
      } else {
        // The click is the consent for one send; the switch stays off and the records that fail to upload stay here.
        if (answer === "once") await reportsFlush(true);
        await reportsAsked();
      }
    } catch {
      // Could not reach the store or the server: nothing was sent, ask again next launch.
    }
    close();
  }
  return (
    <div role="status" aria-live="polite" className="glass-solid lx-rise fixed right-4 bottom-[104px] z-[70] flex w-[min(380px,calc(100vw-32px))] flex-col gap-3 rounded-[12px] p-4 shadow-[0_18px_48px_rgba(0,0,0,0.5)]">
      <div className="flex items-start gap-3">
        <CircleAlert size={18} aria-hidden="true" className="mt-0.5 shrink-0 text-primary-text" />
        <div className="min-w-0 flex-1">
          <p className="text-[13px] leading-[18px] font-semibold text-primary-text">
            {view.unsent === 1 ? "One failure was recorded" : `${view.unsent} failures were recorded`}
          </p>
          <p className="text-[12px] leading-4 text-secondary-text">
            Sending helps fix what broke. It includes the error message and what was on screen — song titles or search
            terms included. Credentials and tokens are removed before anything is stored.
          </p>
        </div>
        <button type="button" aria-label="Not now" onClick={close} className="-m-1 flex size-8 shrink-0 items-center justify-center rounded-[8px] text-secondary-text hover:bg-frost-100 hover:text-primary-text">
          <X size={16} aria-hidden="true" />
        </button>
      </div>
      <div className="rounded-[8px] bg-white/5 px-3 py-2 text-[12px] leading-4 text-secondary-text">
        {view.records.slice(0, 3).map((r) => (
          <p key={r.uid} className="truncate">
            {r.kind} — {r.message || r.detail || "no message"}
          </p>
        ))}
      </div>
      <div className="flex flex-col gap-2">
        <Button variant="primary" onClick={() => void decide("once")}>Send these once</Button>
        <Button onClick={() => void decide("always")}>Always send</Button>
        <Button variant="ghost" onClick={() => void decide("never")}>Never</Button>
      </div>
    </div>
  );
}

"use client";

// Last resort for a screen that fails while drawing: say so, keep the user's data safe, offer a way back. Without it
// a render error leaves the window blank. No error text is shown (it can carry paths or ids); it goes to the console.
import { useEffect } from "react";

export default function AppError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    console.error("render failed:", error.name);
  }, [error]);
  return (
    <main role="alert" className="flex min-h-dvh flex-col items-center justify-center gap-4 bg-night-canvas p-8 text-center text-primary-text">
      <h1 className="font-display text-[28px] leading-9 font-bold tracking-[-0.02em]">Something went wrong</h1>
      <p className="max-w-[44ch] text-[14px] leading-5 text-secondary-text">
        LunoRX could not draw this screen. Your playlists and settings are stored on this device and were not changed.
      </p>
      <button type="button" onClick={reset} className="lx-press h-11 rounded-[12px] bg-primary-text px-6 text-[14px] font-semibold text-on-ink hover:bg-ink-bright">
        Try again
      </button>
    </main>
  );
}

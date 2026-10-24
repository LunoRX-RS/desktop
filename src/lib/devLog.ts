"use client";

import { create } from "zustand";
import { reportIpcFailure } from "./errorReport";

/** One failure exactly as Core reported it, kept in memory for the Dev Mode screen. */
export interface DevError {
  atMs: number;
  where: string;
  category: string;
  message: string;
}

const KEEP = 20;

interface DevLog {
  entries: DevError[];
  record: (where: string, error: unknown) => void;
  clear: () => void;
}

export const useDevLog = create<DevLog>((set) => ({
  entries: [],
  record: (where, error) => {
    set((s) => ({ entries: [describeFailure(where, error), ...s.entries].slice(0, KEEP) }));
    // The same failure joins the error log, where Core masks it before it reaches disk.
    const described = describeFailure(where, error);
    reportIpcFailure(where, described.category, described.message);
  },
  clear: () => set({ entries: [] }),
}));

/** IPC errors are typed DTOs: a fixed category plus fixed text. Anything else is recorded as-is. */
export function describeFailure(where: string, error: unknown): DevError {
  const e = error as { category?: unknown; message?: unknown } | null;
  const category = typeof e?.category === "string" ? e.category : typeof error === "string" ? error : "unknown";
  const message = typeof e?.message === "string" ? e.message : typeof error === "string" ? "" : safe(error);
  return { atMs: Date.now(), where, category, message };
}

function safe(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

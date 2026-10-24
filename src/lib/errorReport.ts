"use client";
import { reportsFlush, reportsRecord } from "../services/tauri/reports";
import { useUi } from "./store";

// Frontend failure capture: window.onerror, unhandledrejection and the render boundary all funnel through
// reportFailure into the Core error log, where text is masked and bounded before it reaches disk. Nothing here
// decides about uploading; Core refuses while consent is off.

const DEBOUNCE_MS = 10_000;
const DEBOUNCE_SLOTS = 64;
const recent = new Map<string, number>();

function currentView(): string {
  return useUi.getState().view;
}

/** One failure, deduplicated by kind so a render loop cannot fill the store. */
export function reportFailure(input: {
  source: string;
  area: string;
  name: string;
  site: string;
  message: string;
  stack?: string;
  detail?: string;
}): void {
  const key = `${input.area}.${input.name}`;
  const now = Date.now();
  const last = recent.get(key);
  if (last !== undefined && now - last < DEBOUNCE_MS) return;
  if (recent.size >= DEBOUNCE_SLOTS) recent.clear();
  recent.set(key, now);
  void reportsRecord({
    source: input.source,
    kind: `error.${input.area}.${input.name}`,
    site: input.site,
    view: currentView(),
    message: input.message,
    stack: input.stack ?? "",
    detail: input.detail ?? "",
  }).catch(() => {});
}

/** The single funnel every typed IPC failure already flows through (useDevLog.record). */
export function reportIpcFailure(where: string, category: string, message: string): void {
  // `where` is a human label like "player command"; the slug shape is what Core validates for kind and site.
  const slug = where.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 58) || "unknown";
  reportFailure({
    source: "ipc",
    area: "ipc",
    name: slug,
    site: slug,
    message,
    detail: category,
  });
}

function describe(reason: unknown): { message: string; stack: string } {
  if (reason instanceof Error) {
    return { message: reason.message, stack: reason.stack ?? "" };
  }
  return { message: String(reason), stack: "" };
}

/** Registers the global listeners and the upload timer. Returns the teardown. */
export function initErrorReport(): () => void {
  const onError = (e: ErrorEvent) => {
    const d = describe(e.error ?? e.message);
    reportFailure({
      source: "frontend",
      area: "frontend",
      name: "exception",
      site: "window.error",
      message: d.message,
      stack: d.stack,
    });
  };
  const onRejection = (e: PromiseRejectionEvent) => {
    const d = describe(e.reason);
    reportFailure({
      source: "frontend",
      area: "frontend",
      name: "promise",
      site: "window.unhandledrejection",
      message: d.message,
      stack: d.stack,
    });
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  const flush = () => { void reportsFlush().catch(() => {}); };
  const first = window.setTimeout(flush, 60_000);
  const every = window.setInterval(flush, 3_600_000);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
    window.clearTimeout(first);
    window.clearInterval(every);
  };
}

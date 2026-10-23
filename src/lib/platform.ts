import { useSyncExternalStore } from "react";

export type Platform = "mac" | "linux" | "other";

export function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "other";
  if (/Mac/i.test(navigator.userAgent)) return "mac";
  if (/Linux|X11/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent)) return "linux";
  return "other";
}

const subscribe = () => () => {};

/** The OS the webview runs on. "other" during the static export and the first server render. */
export function usePlatform(): Platform {
  return useSyncExternalStore(subscribe, detectPlatform, () => "other");
}

/**
 * Props for an element that should move the window when dragged. Only macOS draws no title bar (titleBarStyle Overlay), so
 * only there does the app have to supply a drag area. Elsewhere the window manager owns moving, and an app-level drag region
 * can leave the pointer grabbed on tiling Wayland compositors.
 */
export function useDragRegion(): { "data-tauri-drag-region"?: "" } {
  return usePlatform() === "mac" ? { "data-tauri-drag-region": "" } : {};
}

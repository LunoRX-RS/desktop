import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type YouTubeConnectionStatus = "restoring" | "disconnected" | "connecting" | "connected" | "expired" | "storeError" | "unavailable";
export function youtubeConnectionStatus(): Promise<YouTubeConnectionStatus> { return invoke("youtube_connection_status"); }
export function youtubeConnect(): Promise<YouTubeConnectionStatus> { return invoke("youtube_connect"); }
export function youtubeConnectFinish(): Promise<YouTubeConnectionStatus> { return invoke("youtube_connect_finish"); }
export function youtubeDisconnect(): Promise<YouTubeConnectionStatus> { return invoke("youtube_disconnect"); }
export function subscribeYouTubeConnection(callback: (status: YouTubeConnectionStatus) => void): Promise<UnlistenFn> {
  return listen<YouTubeConnectionStatus>("core://youtube/connection", (event) => callback(event.payload));
}

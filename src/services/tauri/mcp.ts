import { invoke } from "./invoke";

/** `portInUse` and `keyStoreUnavailable` mean the switch is on but nothing listens. */
export type McpStatus = "off" | "listening" | "portInUse" | "keyStoreUnavailable";

export interface McpSettings {
  enabled: boolean;
  port: number;
  defaultPort: number;
  status: McpStatus;
  /** Endpoint and bearer token for the AI client's config; null when the OS key store is unavailable. */
  url: string | null;
  token: string | null;
}

export const mcpSettingsGet = () => invoke<McpSettings>("mcp_settings_get");
/** Rejects with "invalid port" for ports below 1024. */
export const mcpSettingsSet = (enabled: boolean, port: number) => invoke<void>("mcp_settings_set", { args: { enabled, port } });
export const mcpRotateToken = () => invoke<void>("mcp_rotate_token");

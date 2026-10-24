import { invoke as send, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { activityRecord } from "./reports";

// Shadowing wrapper over the raw invoke: every IPC call an app module makes is noted in the activity log with its
// duration, arguments and outcome. The re-throw is unchanged, so callers keep their typed errors.

/** Arguments whose value is a credential by name. The value never reaches the log, not even masked. */
const WITHHELD = new Set([
  "password",
  "token",
  "registerToken",
  "loginToken",
  "value",
  "cookie",
  "authorization",
  "secret",
  "key",
  "email",
]);

function slug(cmd: string): string {
  return cmd.toLowerCase().replace(/[^a-z0-9._-]+/g, "-");
}

function describeArgs(args: InvokeArgs | undefined): string {
  if (!args) return "";
  try {
    const shown: Record<string, unknown> = {};
    for (const [name, value] of Object.entries(args)) {
      shown[name] = WITHHELD.has(name) ? "<omitted>" : value;
    }
    return JSON.stringify(shown) ?? "";
  } catch {
    return "";
  }
}

function errorKind(error: unknown): string {
  const category = (error as { category?: unknown } | null)?.category;
  return typeof category === "string" ? category : "unknown";
}

function note(cmd: string, args: InvokeArgs | undefined, ok: boolean, startedMs: number, error?: unknown) {
  // The log must not note the call that writes to it: activityRecord targets Core directly, so nothing recurses.
  if (cmd === "activity_record") return;
  const name = `ipc.${slug(cmd)}`;
  const dur = `${Math.round(performance.now() - startedMs)}ms`;
  const detail = error === undefined ? `${dur} ${describeArgs(args)}` : `${dur} ${errorKind(error)} ${describeArgs(args)}`;
  void activityRecord({ kind: "ipc", name, detail, ok }).catch(() => {});
}

export function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  const started = performance.now();
  return send<T>(cmd, args, options).then(
    (value) => {
      note(cmd, args, true, started);
      return value;
    },
    (error) => {
      note(cmd, args, false, started, error);
      throw error;
    },
  );
}

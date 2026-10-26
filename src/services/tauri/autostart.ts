import { invoke } from "./invoke";

/** Whether the app is registered to open at login. The OS login item is the source of truth. */
export const autostartGet = () => invoke<boolean>("autostart_get");
/** Registers or unregisters the app as a login item. Rejects when the OS refuses. */
export const autostartSet = (enabled: boolean) => invoke<void>("autostart_set", { enabled });

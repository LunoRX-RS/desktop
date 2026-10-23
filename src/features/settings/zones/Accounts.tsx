"use client";

import { AccountPanel } from "../../accounts/AccountsScreen";
import SyncPanel from "../../sync/SyncPanel";
import { ProviderKeys } from "../ProviderKeys";
import { Inset, SettingRow } from "../rows";

/** Provider sign-in, the LunoRX account and Sync, managed right here: no trip to the Accounts or Sync page. */
export function Accounts() {
  return (
    <>
      <ProviderKeys />
      <section id="settings-lunor-account" tabIndex={-1} aria-label="LunoRX account" className="outline-none">
        <AccountPanel />
      </section>
      <SyncPanel embedded />
      <Inset id="settings-sync-server" title="Server">
        <SettingRow
          label="Server endpoint"
          hint="Read-only. No command exposes the configured endpoint or lets you change it. The core’s source default is http://127.0.0.1:8080."
        >
          Not reported
        </SettingRow>
      </Inset>
    </>
  );
}

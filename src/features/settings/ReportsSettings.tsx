"use client";
import { useEffect, useState } from "react";
import { Switch } from "../../components/Lyrics";
import { Button, ConfirmButton } from "../../components/ui";
import { reportsDelete, reportsFlush, reportsGet, reportsSetEnabled, type ErrorLogViewDto } from "../../services/tauri/reports";
import { Inset, NoteRow, SettingRow } from "./rows";

/** Settings > Privacy & data > Error reports. Off by default; shows exactly what would be sent. */
export function ReportsSettings() {
  const [view, setView] = useState<ErrorLogViewDto | null>(null);
  const [show, setShow] = useState(false);
  const [note, setNote] = useState<{ tone: "ok" | "bad"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const read = () => reportsGet().then(setView, () => setNote({ tone: "bad", text: "Could not read the error report setting." }));
  useEffect(() => { void read(); }, []);
  async function toggle(on: boolean) {
    setBusy(true);
    try { setView(await reportsSetEnabled(on)); setNote(null); } catch { setNote({ tone: "bad", text: "Could not change the setting." }); }
    finally { setBusy(false); }
  }
  async function sendNow() {
    setBusy(true);
    try { await reportsFlush(); setNote({ tone: "ok", text: "Sent." }); await read(); }
    catch { setNote({ tone: "bad", text: "Could not reach the server. The reports are kept and sent later." }); }
    finally { setBusy(false); }
  }
  async function remove() {
    setBusy(true);
    try { setView(await reportsDelete()); setNote({ tone: "ok", text: "Deleted from the server. Sharing is off and this log has a new anonymous ID." }); }
    catch { setNote({ tone: "bad", text: "Could not reach the server, so nothing was deleted. Try again." }); }
    finally { setBusy(false); }
  }
  if (!view) return null;
  return (
    <Inset id="settings-error-reports" title="Error reports">
      <SettingRow
        label="Share error reports"
        hint="Sends what failed and why: error messages, stack traces, the screen it happened on and the details around it — song titles and search terms included. Credentials and tokens are removed before anything is stored, and <masked> marks how many values were withheld. Kept on the server for 90 days."
      >
        <Switch on={view.enabled} onChange={(on) => void toggle(on)} name="Share error reports">{view.enabled ? "On" : "Off"}</Switch>
      </SettingRow>
      {view.enabled && (
        <SettingRow label={`Show what would be sent (${view.unsent} waiting)`} stack>
          <Button variant="ghost" onClick={() => setShow(!show)}>{show ? "Hide" : "Show"}</Button>
        </SettingRow>
      )}
      {show && view.records.length === 0 && <NoteRow tone="off">Nothing has failed on this device yet.</NoteRow>}
      {show &&
        view.records.slice(0, 20).map((r) => (
          <NoteRow key={r.uid} tone="off">
            {r.kind} · {r.view}
            {r.message && <> — {r.message}</>}
            {r.masked > 0 && <> ({r.masked} value{r.masked === 1 ? "" : "s"} withheld)</>}
          </NoteRow>
        ))}
      {view.enabled && (
        <SettingRow label="Upload and delete">
          <Button disabled={busy || view.unsent === 0} onClick={() => void sendNow()}>Send now</Button>
          <ConfirmButton disabled={busy} confirmLabel="Press again to delete" onConfirm={() => void remove()}>Delete my error reports</ConfirmButton>
        </SettingRow>
      )}
      {note && <NoteRow tone={note.tone} role={note.tone === "bad" ? "alert" : "status"}>{note.text}</NoteRow>}
    </Inset>
  );
}

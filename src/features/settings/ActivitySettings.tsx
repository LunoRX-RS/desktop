"use client";
import { useEffect, useState } from "react";
import { Switch } from "../../components/Lyrics";
import { Button, ConfirmButton } from "../../components/ui";
import { activityDelete, activityFlush, activityView, activitySetEnabled, type ActivityViewDto } from "../../services/tauri/reports";
import { Inset, NoteRow, SettingRow } from "./rows";

/** Settings > Privacy & data > Activity log. Off by default; shows exactly what would be sent. */
export function ActivitySettings() {
  const [view, setView] = useState<ActivityViewDto | null>(null);
  const [show, setShow] = useState(false);
  const [note, setNote] = useState<{ tone: "ok" | "bad"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const read = () => activityView().then(setView, () => setNote({ tone: "bad", text: "Could not read the activity setting." }));
  useEffect(() => { void read(); }, []);
  async function toggle(on: boolean) {
    setBusy(true);
    try { setView(await activitySetEnabled(on)); setNote(null); } catch { setNote({ tone: "bad", text: "Could not change the setting." }); }
    finally { setBusy(false); }
  }
  async function sendNow() {
    setBusy(true);
    try { await activityFlush(); setNote({ tone: "ok", text: "Sent." }); await read(); }
    catch { setNote({ tone: "bad", text: "Could not reach the server. The log is kept and sent later." }); }
    finally { setBusy(false); }
  }
  async function remove() {
    setBusy(true);
    try { setView(await activityDelete()); setNote({ tone: "ok", text: "Deleted from the server. Sharing is off and this log has a new anonymous ID." }); }
    catch { setNote({ tone: "bad", text: "Could not reach the server, so nothing was deleted. Try again." }); }
    finally { setBusy(false); }
  }
  if (!view) return null;
  return (
    <Inset id="settings-activity" title="Activity log">
      <SettingRow
        label="Share the activity log"
        hint={`Sends what you did and what the app did: every screen you opened, every command the app ran and every event it fired, with short details — titles, playlist names and search terms included. Credentials and tokens are removed, and <masked> marks how many values were withheld. The log holds ${view.capacity.toLocaleString()} entries on this device; the server keeps them 30 days.`}
      >
        <Switch on={view.enabled} onChange={(on) => void toggle(on)} name="Share the activity log">{view.enabled ? "On" : "Off"}</Switch>
      </SettingRow>
      {view.enabled && (
        <SettingRow label={`Show what would be sent (${view.unsent.toLocaleString()} waiting)`} stack>
          <Button variant="ghost" onClick={() => setShow(!show)}>{show ? "Hide" : "Show"}</Button>
        </SettingRow>
      )}
      {show && view.records.length === 0 && <NoteRow tone="off">Nothing is waiting to be sent.</NoteRow>}
      {show &&
        view.records.slice(0, 20).map((r) => (
          <NoteRow key={r.uid} tone="off">
            {new Date(r.atMs).toLocaleTimeString()} · {r.name} ×{r.count}
            {r.detail && <> — {r.detail}</>}
          </NoteRow>
        ))}
      {view.enabled && (
        <SettingRow label="Upload and delete">
          <Button disabled={busy || view.unsent === 0} onClick={() => void sendNow()}>Send now</Button>
          <ConfirmButton disabled={busy} confirmLabel="Press again to delete" onConfirm={() => void remove()}>Delete my activity log</ConfirmButton>
        </SettingRow>
      )}
      {note && <NoteRow tone={note.tone} role={note.tone === "bad" ? "alert" : "status"}>{note.text}</NoteRow>}
    </Inset>
  );
}

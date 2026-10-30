"use client";
import { useState } from "react";
import { ConfirmButton } from "../../components/ui";
import { tasteClear } from "../../services/tauri/stats";
import { Inset, NoteRow, SettingRow } from "./rows";

/** Settings > Privacy & data > Autoplay learning. The log stays on this device; this deletes it. */
export function TasteSettings() {
  const [note, setNote] = useState<{ tone: "ok" | "bad"; text: string } | null>(null);
  async function clear() {
    try { await tasteClear(); setNote({ tone: "ok", text: "Deleted. Autoplay uses its built-in taste until it hears new songs." }); }
    catch { setNote({ tone: "bad", text: "Could not delete the autoplay learning data. Try again." }); }
  }
  return (
    <Inset id="settings-taste" title="Autoplay learning">
      <SettingRow
        label="Delete autoplay learning"
        hint="Autoplay keeps how far it listened to its last 100 picks, on this device only, and uses that to choose what plays next. Nothing is uploaded. Deleting it resets autoplay to its built-in taste."
      >
        <ConfirmButton variant="ghost" confirmLabel="Press again to delete" onConfirm={() => void clear()}>Delete</ConfirmButton>
      </SettingRow>
      {note && <NoteRow tone={note.tone} role={note.tone === "bad" ? "alert" : "status"}>{note.text}</NoteRow>}
    </Inset>
  );
}

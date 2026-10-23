"use client";

import { Inset, NoteRow, SettingRow } from "../rows";
import { ShortcutSettings } from "../ShortcutSettings";

export function Shortcuts({ mac }: { mac: boolean }) {
  const groups: { id: string; title: string; hint?: string; rows: [string, string][] }[] = [
    {
      id: "settings-keys-fixed",
      title: "Always the same",
      rows: [
        ["Close Now Playing, the command list or a dialog", "Esc"],
        ["Move and choose in Search and the command list", "↑ ↓ Enter"],
      ],
    },
    {
      id: "settings-keys-maker",
      title: "Lyric Maker",
      hint: "Open it from the Make lyrics file button in Now Playing.",
      rows: [
        ["Type along: first letter stamps a line's start", "Any letter"],
        ["Type along: stamp again, or 0.2 s earlier", "Tab / Shift+Tab"],
        ["Type along: add the line, or stamp the next line now", "Enter / ⌘Enter"],
        ["Tap along: stamp the next line, or un-stamp the last", "Space / Backspace"],
        ["Nudge a selected line by 0.1 s, or delete it", "[ ] / Delete"],
        ["Undo and redo (field empty)", "⌘Z / ⇧⌘Z"],
        ["Back 5 s", "⇧⌘B"],
        ["Close the maker, then Now Playing", "Esc"],
      ],
    },
  ];
  return (
    <>
      <ShortcutSettings />
      {groups.map((g) => (
        <Inset key={g.id} id={g.id} title={g.title}>
          {g.rows.map(([action, keys]) => (
            <SettingRow key={action} label={action}>{keys}</SettingRow>
          ))}
        </Inset>
      ))}
      {mac && (
        <Inset id="settings-keys-mac" title="Playback menu (macOS)">
          <NoteRow tone="off">macOS only. These work from the Playback menu in the menu bar.</NoteRow>
          {(
            [
              ["Play or pause", "⌘P"],
              ["Stop", "⌘."],
              ["Seek back / forward", "⌘← / ⌘→"],
              ["Volume up / down", "⌘↑ / ⌘↓"],
              ["Mute", "⌥⌘↓"],
            ] as const
          ).map(([action, keys]) => (
            <SettingRow key={action} label={action}>{keys}</SettingRow>
          ))}
        </Inset>
      )}
    </>
  );
}

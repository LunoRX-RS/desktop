"use client";

import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { usePlatform, type Platform } from "../../lib/platform";
import {
	ACTIONS,
	bindingsFor,
	checkCombo,
	comboFromEvent,
	formatCombo,
	resetAllShortcuts,
	resetShortcut,
	setShortcut,
	useOverrides,
	type ActionDef,
	type Overrides,
} from "../../lib/shortcuts";
import { Inset, NoteRow, SettingRow } from "./rows";

const GROUPS: ActionDef["group"][] = ["Search", "Navigation", "View", "Playback"];

function Keys({ combos, platform }: { combos: string[]; platform: Platform }) {
	return (
		<span className="flex flex-wrap justify-end gap-1.5">
			{combos.map((c) => (
				<kbd key={c} className="rounded-[6px] bg-frost-200 px-2 py-0.5 font-mono text-[12px] text-primary-text">
					{formatCombo(c, platform)}
				</kbd>
			))}
		</span>
	);
}

function ShortcutRow({ def, platform, overrides }: { def: ActionDef; platform: Platform; overrides: Overrides }) {
	const [recording, setRecording] = useState(false);
	const [problem, setProblem] = useState("");
	const bindings = bindingsFor(platform, overrides);
	const changed = overrides[def.id] !== undefined;

	// While recording, this listener runs before every other key handler and swallows the press, so the key that is being
	// chosen does not also trigger its current action.
	useEffect(() => {
		if (!recording) return;
		function onKey(e: KeyboardEvent) {
			e.preventDefault();
			e.stopImmediatePropagation();
			if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
				setRecording(false);
				setProblem("");
				return;
			}
			const combo = comboFromEvent(e);
			if (!combo) return; // only modifiers so far: keep waiting for the key
			const check = checkCombo(combo, def.id, bindings);
			if (!check.ok) {
				setProblem(check.reason);
				return;
			}
			setShortcut(def.id, combo);
			setProblem("");
			setRecording(false);
		}
		window.addEventListener("keydown", onKey, true);
		return () => window.removeEventListener("keydown", onKey, true);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [recording, def.id, overrides, platform]);

	return (
		<SettingRow label={def.label} hint={recording ? (problem || "Press the new keys. Esc cancels.") : problem || undefined}>
			<span className="flex items-center gap-2">
				{recording ? (
					<span role="status" className="rounded-[6px] bg-frost-300 px-2 py-0.5 text-[12px] text-primary-text">Press keys…</span>
				) : (
					<Keys combos={bindings[def.id]} platform={platform} />
				)}
				<Button
					variant="ghost"
					onClick={() => {
						setProblem("");
						setRecording((r) => !r);
					}}
					aria-label={`${recording ? "Cancel changing" : "Change"} the shortcut for ${def.label}`}
				>
					{recording ? "Cancel" : "Change"}
				</Button>
				{changed && !recording && (
					<Button variant="ghost" onClick={() => resetShortcut(def.id)} aria-label={`Reset the shortcut for ${def.label}`}>
						Reset
					</Button>
				)}
			</span>
		</SettingRow>
	);
}

/** Shortcuts the user can record and change. Choices are kept on this device, so each computer follows its own system. */
export function ShortcutSettings() {
	const platform = usePlatform();
	const overrides = useOverrides();
	const system = platform === "mac" ? "macOS" : platform === "linux" ? "Linux" : "Windows";
	return (
		<>
			{GROUPS.map((group) => (
				<Inset key={group} id={`settings-keys-${group.toLowerCase()}`} title={group}>
					{ACTIONS.filter((a) => a.group === group).map((def) => (
						<ShortcutRow key={def.id} def={def} platform={platform} overrides={overrides} />
					))}
				</Inset>
			))}
			<Inset id="settings-keys-reset" title="Defaults">
				<NoteRow tone="off">
					Defaults follow {system}: {platform === "mac" ? "⌘ (Command)" : "Ctrl"} with the key. Changes are saved on this device only.
				</NoteRow>
				<SettingRow label="Put every shortcut back to its default">
					<Button onClick={() => resetAllShortcuts()} disabled={Object.keys(overrides).length === 0}>
						Reset all
					</Button>
				</SettingRow>
			</Inset>
		</>
	);
}

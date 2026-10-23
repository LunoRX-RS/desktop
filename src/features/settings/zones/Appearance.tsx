"use client";

import { Inset, SettingRow } from "../rows";
import { TextStyleSettings } from "../TextStyleSettings";

export function Appearance() {
  return (
    <>
      <Inset id="settings-appearance" title="Theme">
        <SettingRow label="Theme" hint="The one theme on all platforms.">LUNOR dark</SettingRow>
        <SettingRow label="Reduced motion" hint="Follows your operating system setting.">System</SettingRow>
        <SettingRow label="Reduced transparency" hint="Follows your operating system setting. The sidebar and player become solid.">System</SettingRow>
      </Inset>
      <TextStyleSettings />
    </>
  );
}

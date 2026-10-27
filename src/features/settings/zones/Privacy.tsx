"use client";

import { TelemetrySettings } from "../TelemetrySettings";
import { ReportsSettings } from "../ReportsSettings";
import { ActivitySettings } from "../ActivitySettings";
import { TasteSettings } from "../TasteSettings";

export function Privacy() {
  return (
    <>
      <TelemetrySettings />
      <ReportsSettings />
      <ActivitySettings />
      <TasteSettings />
    </>
  );
}

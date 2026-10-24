"use client";

import { TelemetrySettings } from "../TelemetrySettings";
import { ReportsSettings } from "../ReportsSettings";
import { ActivitySettings } from "../ActivitySettings";

export function Privacy() {
  return (
    <>
      <TelemetrySettings />
      <ReportsSettings />
      <ActivitySettings />
    </>
  );
}

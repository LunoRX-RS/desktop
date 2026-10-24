import { invoke } from "@tauri-apps/api/core";

// Mirrors commands/reports.rs. Records always stay on this device; nothing uploads until the user turned the
// matching switch on, and every free-text field is masked inside Core before it reaches disk.
export interface ErrorRecordDto {
  uid: string;
  atMs: number;
  source: string;
  kind: string;
  site: string;
  view: string;
  masked: number;
  message: string;
  stack: string;
  detail: string;
  sent: boolean;
}

export interface ErrorLogViewDto {
  enabled: boolean;
  asked: boolean;
  reportId: string;
  unsent: number;
  capacity: number;
  records: ErrorRecordDto[];
}

export interface ActivityRecordDto {
  uid: string;
  atMs: number;
  kind: string;
  name: string;
  detail: string;
  ok: boolean;
  count: number;
}

export interface ActivityViewDto {
  enabled: boolean;
  asked: boolean;
  logId: string;
  unsent: number;
  total: number;
  capacity: number;
  records: ActivityRecordDto[];
}

export interface NewErrorDto {
  source: string;
  kind: string;
  site: string;
  view: string;
  message?: string;
  stack?: string;
  detail?: string;
}

export interface NewActivityDto {
  kind: string;
  name: string;
  detail?: string;
  ok?: boolean;
}

export const reportsGet = () => invoke<ErrorLogViewDto>("reports_get");
export const reportsSetEnabled = (enabled: boolean) => invoke<ErrorLogViewDto>("reports_set_enabled", { enabled });
export const reportsRecord = (error: NewErrorDto) => invoke<void>("reports_record", { error });
export const reportsAsked = () => invoke<ErrorLogViewDto>("reports_asked");
export const reportsFlush = () => invoke<void>("reports_flush");
/** Deletes this device's reports on the server first, then clears the local log with a new anonymous id. */
export const reportsDelete = () => invoke<ErrorLogViewDto>("reports_delete");
export const activityView = () => invoke<ActivityViewDto>("activity_view");
export const activitySetEnabled = (enabled: boolean) => invoke<ActivityViewDto>("activity_set_enabled", { enabled });
export const activityRecord = (entry: NewActivityDto) => invoke<void>("activity_record", { entry });
export const activityFlush = () => invoke<void>("activity_flush");
export const activityDelete = () => invoke<ActivityViewDto>("activity_delete");

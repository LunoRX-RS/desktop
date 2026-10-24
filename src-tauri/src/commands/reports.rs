//! Consent-gated diagnostics: error reports and the action/event log. Thin commands over `core::errorlog` and
//! `core::activity`. Recording works with uploading off, so the local log and the Dev Mode screen are useful either way;
//! an upload refuses unless the store itself says it is enabled, which is the one place consent is decided.
use crate::core::activity::{ActivityLog, ActivityView, NewActivity};
use crate::core::errorlog::{ErrorLog, ErrorLogView, NewError};
use crate::core::library::LibraryError;
use crate::sync::api::{ActivityBatch, ActivityItem, ApiClient, ReportBatch, ReportItem};
use crate::sync::engine::{ErrorCategory, category};
use crate::sync::settings::default_base_url;
use tauri::{State, WebviewWindow};

pub struct ReportsState {
    log: std::sync::Mutex<Result<ErrorLog, LibraryError>>,
    path: Result<std::path::PathBuf, LibraryError>,
}

impl ReportsState {
    pub fn open(path: Result<std::path::PathBuf, LibraryError>) -> Self {
        let log = path
            .as_ref()
            .map_err(|e| *e)
            .and_then(|p| ErrorLog::open(p.clone()));
        if let Err(kind) = &log {
            eprintln!("error log unavailable: {kind:?}");
        }
        Self {
            log: std::sync::Mutex::new(log),
            path,
        }
    }

    pub(crate) fn with<T>(
        &self,
        action: impl FnOnce(&ErrorLog) -> Result<T, LibraryError>,
    ) -> Result<T, LibraryError> {
        let mut log = self.log.lock().map_err(|_| LibraryError::Storage)?;
        if log.is_err() {
            *log = self
                .path
                .as_ref()
                .map_err(|e| *e)
                .and_then(|p| ErrorLog::open(p.clone()));
        }
        action(log.as_ref().map_err(|e| *e)?)
    }

    /// For Rust-side failures such as a sync or playback error. Never fails the caller.
    pub fn record(&self, error: &NewError) {
        let _ = self.with(|log| log.record(error));
    }
}

pub struct ActivityState {
    log: std::sync::Mutex<Result<ActivityLog, LibraryError>>,
    path: Result<std::path::PathBuf, LibraryError>,
}

impl ActivityState {
    pub fn open(path: Result<std::path::PathBuf, LibraryError>) -> Self {
        let log = path
            .as_ref()
            .map_err(|e| *e)
            .and_then(|p| ActivityLog::open(p.clone()));
        if let Err(kind) = &log {
            eprintln!("activity log unavailable: {kind:?}");
        }
        Self {
            log: std::sync::Mutex::new(log),
            path,
        }
    }

    pub(crate) fn with<T>(
        &self,
        action: impl FnOnce(&ActivityLog) -> Result<T, LibraryError>,
    ) -> Result<T, LibraryError> {
        let mut log = self.log.lock().map_err(|_| LibraryError::Storage)?;
        if log.is_err() {
            *log = self
                .path
                .as_ref()
                .map_err(|e| *e)
                .and_then(|p| ActivityLog::open(p.clone()));
        }
        action(log.as_ref().map_err(|e| *e)?)
    }

    /// For Core-side events and command bookkeeping. Never fails the caller: an entry is dropped, the action is not.
    pub fn record(&self, entry: &NewActivity) {
        let _ = self.with(|log| log.record(entry));
    }
}

fn main(window: &WebviewWindow) -> Result<(), LibraryError> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err(LibraryError::Validation)
    }
}

fn os() -> &'static str {
    match std::env::consts::OS {
        o @ ("macos" | "windows" | "linux") => o,
        _ => "other",
    }
}

#[tauri::command]
pub fn reports_get(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
) -> Result<ErrorLogView, LibraryError> {
    main(&window)?;
    state.with(ErrorLog::view)
}

#[tauri::command]
pub fn reports_set_enabled(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
    enabled: bool,
) -> Result<ErrorLogView, LibraryError> {
    main(&window)?;
    state.with(|log| log.set_enabled(enabled))
}

/// Records one failure. The text is masked and length-bounded inside `core::errorlog` before it reaches disk.
#[tauri::command]
pub fn reports_record(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
    error: NewError,
) -> Result<(), LibraryError> {
    main(&window)?;
    state.with(|log| log.record(&error))
}

/// Notes that the first-run prompt has been answered, so it does not return on the next launch.
#[tauri::command]
pub fn reports_asked(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
) -> Result<ErrorLogView, LibraryError> {
    main(&window)?;
    state.with(ErrorLog::set_asked)
}

#[tauri::command]
pub fn activity_view(
    window: WebviewWindow,
    state: State<'_, ActivityState>,
) -> Result<ActivityView, LibraryError> {
    main(&window)?;
    state.with(ActivityLog::view)
}

#[tauri::command]
pub fn activity_set_enabled(
    window: WebviewWindow,
    state: State<'_, ActivityState>,
    enabled: bool,
) -> Result<ActivityView, LibraryError> {
    main(&window)?;
    state.with(|log| log.set_enabled(enabled))
}

/// Notes one action, IPC call or event. `detail` is masked inside `core::activity`.
#[tauri::command]
pub fn activity_record(
    window: WebviewWindow,
    state: State<'_, ActivityState>,
    entry: NewActivity,
) -> Result<(), LibraryError> {
    main(&window)?;
    state.with(|log| log.record(&entry)).map(|_| ())
}

/// Uploads the records error reports hold. Nothing is sent while consent is off or nothing is unsent. `once` is the
/// first-run prompt's "send these once": the click itself is the consent, the switch stays off afterwards.
#[tauri::command]
pub async fn reports_flush(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
    once: Option<bool>,
) -> Result<(), ErrorCategory> {
    main(&window).map_err(|_| ErrorCategory::Validation)?;
    if !state
        .with(ErrorLog::enabled)
        .map_err(|_| ErrorCategory::Storage)?
        && !once.unwrap_or(false)
    {
        return Ok(());
    }
    let reports = state
        .with(ErrorLog::pending)
        .map_err(|_| ErrorCategory::Storage)?;
    if reports.is_empty() {
        return Ok(());
    }
    let report_id = state
        .with(ErrorLog::report_id)
        .map_err(|_| ErrorCategory::Storage)?;
    let sent = async {
        let api = ApiClient::new(&default_base_url())?;
        api.post_reports(&ReportBatch {
            report_id: &report_id,
            app_version: env!("CARGO_PKG_VERSION"),
            os: os(),
            arch: std::env::consts::ARCH,
            reports: reports
                .iter()
                .map(|r| ReportItem {
                    uid: &r.uid,
                    at_ms: r.at_ms,
                    source: source_name(r.source),
                    kind: &r.kind,
                    site: &r.site,
                    view: &r.view,
                    message: &r.message,
                    stack: &r.stack,
                    detail: &r.detail,
                    masked: r.masked,
                })
                .collect(),
        })
        .await
    }
    .await;
    if let Err(e) = sent {
        // Nothing is marked: an unconfirmed upload stays pending, which is what rule 5 requires of user data.
        return Err(category(&e));
    }
    state
        .with(|log| log.mark_sent(&reports.iter().map(|r| r.uid.clone()).collect::<Vec<_>>()))
        .map_err(|_| ErrorCategory::Storage)
}

/// Uploads the entries the activity log holds, writing the throttled entries first so disk and upload agree.
#[tauri::command]
pub async fn activity_flush(
    window: WebviewWindow,
    state: State<'_, ActivityState>,
) -> Result<(), ErrorCategory> {
    main(&window).map_err(|_| ErrorCategory::Validation)?;
    state
        .with(ActivityLog::flush)
        .map_err(|_| ErrorCategory::Storage)?;
    if !state
        .with(ActivityLog::enabled)
        .map_err(|_| ErrorCategory::Storage)?
    {
        return Ok(());
    }
    let entries = state
        .with(ActivityLog::pending)
        .map_err(|_| ErrorCategory::Storage)?;
    if entries.is_empty() {
        return Ok(());
    }
    let log_id = state
        .with(ActivityLog::log_id)
        .map_err(|_| ErrorCategory::Storage)?;
    let sent = async {
        let api = ApiClient::new(&default_base_url())?;
        api.post_activity(&ActivityBatch {
            log_id: &log_id,
            app_version: env!("CARGO_PKG_VERSION"),
            os: os(),
            arch: std::env::consts::ARCH,
            entries: entries
                .iter()
                .map(|e| ActivityItem {
                    uid: &e.uid,
                    at_ms: e.at_ms,
                    kind: kind_name(e.kind),
                    name: &e.name,
                    detail: &e.detail,
                    ok: e.ok,
                    count: e.count,
                })
                .collect(),
        })
        .await
    }
    .await;
    if let Err(e) = sent {
        return Err(category(&e));
    }
    state
        .with(|log| log.mark_sent(&entries.iter().map(|e| e.uid.clone()).collect::<Vec<_>>()))
        .map_err(|_| ErrorCategory::Storage)
}

/// Asks the server to delete what it holds for this report id, then clears the local store with a new anonymous id.
/// Server first: a failed delete must never leave the user with nothing to retry from.
#[tauri::command]
pub async fn reports_delete(
    window: WebviewWindow,
    state: State<'_, ReportsState>,
) -> Result<ErrorLogView, ErrorCategory> {
    main(&window).map_err(|_| ErrorCategory::Validation)?;
    let report_id = state
        .with(ErrorLog::report_id)
        .map_err(|_| ErrorCategory::Storage)?;
    ApiClient::new(&default_base_url())
        .map_err(|e| category(&e))?
        .delete_reports(&report_id)
        .await
        .map_err(|e| category(&e))?;
    state
        .with(ErrorLog::reset)
        .map_err(|_| ErrorCategory::Storage)
}

#[tauri::command]
pub async fn activity_delete(
    window: WebviewWindow,
    state: State<'_, ActivityState>,
) -> Result<ActivityView, ErrorCategory> {
    main(&window).map_err(|_| ErrorCategory::Validation)?;
    let log_id = state
        .with(ActivityLog::log_id)
        .map_err(|_| ErrorCategory::Storage)?;
    ApiClient::new(&default_base_url())
        .map_err(|e| category(&e))?
        .delete_activity(&log_id)
        .await
        .map_err(|e| category(&e))?;
    state
        .with(ActivityLog::reset)
        .map_err(|_| ErrorCategory::Storage)
}

fn source_name(source: crate::core::errorlog::ErrorSource) -> &'static str {
    use crate::core::errorlog::ErrorSource::*;
    match source {
        Frontend => "frontend",
        Ipc => "ipc",
        Core => "core",
        Sync => "sync",
        Player => "player",
        Native => "native",
    }
}

fn kind_name(kind: crate::core::activity::ActivityKind) -> &'static str {
    use crate::core::activity::ActivityKind::*;
    match kind {
        Action => "action",
        Ipc => "ipc",
        Event => "event",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_names_match_what_the_stores_parse() {
        use crate::core::errorlog::ErrorSource;
        use crate::core::errorlog::parse_source;
        for s in [
            ErrorSource::Frontend,
            ErrorSource::Ipc,
            ErrorSource::Core,
            ErrorSource::Sync,
            ErrorSource::Player,
            ErrorSource::Native,
        ] {
            assert_eq!(parse_source(source_name(s)), Some(s));
        }
        use crate::core::activity::{ActivityKind, parse_kind};
        for k in [ActivityKind::Action, ActivityKind::Ipc, ActivityKind::Event] {
            assert_eq!(parse_kind(kind_name(k)), Some(k));
        }
    }
}

//! Every user action, every Core command and every app event, kept on this device. Uploading it needs its own consent,
//! separate from error reports and from usage counts. `detail` is scrubbed by `core::secrets` before it is stored, so a
//! password, token or session cookie cannot reach the file even though titles, artists and search terms can.
use super::library::LibraryError;
use super::secrets::scrub;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_RECORDS: usize = 20_000;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
/// What is taken for one upload; the server refuses a bigger batch.
const MAX_BATCH: usize = 200;
/// What the Dev Mode screen may ask for in one IPC response.
const MAX_VIEW: usize = 200;
const DETAIL_CHARS: usize = 400;
/// Repeats of the same name inside this window count instead of growing the log. `core://player/progress` has seven
/// emit sites at about one per second, which would otherwise dominate the file.
const COALESCE_MS: u64 = 5_000;
/// The file is rewritten at most this often. A crash inside the window loses those entries, which the settings copy says.
const COMMIT_INTERVAL: Duration = Duration::from_secs(2);
/// Entries older than this are dropped at commit: a usage history is not worth keeping forever.
const MAX_AGE_MS: u64 = 90 * 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityKind {
    Action,
    Ipc,
    Event,
}

/// `None` for anything off this list, so an IPC caller cannot invent a category.
pub fn parse_kind(value: &str) -> Option<ActivityKind> {
    match value {
        "action" => Some(ActivityKind::Action),
        "ipc" => Some(ActivityKind::Ipc),
        "event" => Some(ActivityKind::Event),
        _ => None,
    }
}

impl ActivityKind {
    fn prefix(self) -> &'static str {
        match self {
            ActivityKind::Action => "action.",
            ActivityKind::Ipc => "ipc.",
            ActivityKind::Event => "event.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub uid: String,
    pub at_ms: u64,
    pub kind: ActivityKind,
    /// A slug such as `action.navigate.home`, `ipc.sync_status` or `event.player-progress`.
    pub name: String,
    /// Scrubbed context: a track title, a playlist name, a search query, masked command arguments.
    pub detail: String,
    /// False when the action or call failed.
    pub ok: bool,
    /// How many repeats this entry absorbed.
    pub count: u32,
    pub sent: bool,
}

/// One entry as offered by the caller. Not `Debug`: `detail` is unscrubbed until `record` runs.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewActivity {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default = "always_ok")]
    pub ok: bool,
}

fn always_ok() -> bool {
    true
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityView {
    pub enabled: bool,
    pub asked: bool,
    pub log_id: String,
    pub unsent: usize,
    pub total: usize,
    pub capacity: usize,
    pub records: Vec<ActivityRecord>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    asked: bool,
    #[serde(default)]
    log_id: String,
    #[serde(default)]
    records: Vec<ActivityRecord>,
}

struct Inner {
    saved: Saved,
    /// When the file was last rewritten, so a busy log is not written on every entry.
    committed: Instant,
}

pub struct ActivityLog {
    path: PathBuf,
    inner: Mutex<Inner>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn new_uid() -> String {
    format!("{:012x}", rand::random::<u64>() >> 16)
}

/// A UUID v4 string, its own value so the activity log cannot be joined to the usage counters or the error reports
/// unless the user turns each one on.
fn new_log_id() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Lower-case slug segments joined by dots, no empty segment, at most `max` bytes. The same rule `core::errorlog`
/// applies to its own slugs, and the same one the backend's CHECK constraints enforce.
fn slug_ok(name: &str, max: usize) -> bool {
    !name.is_empty()
        && name.len() <= max
        && name.split('.').all(|s| !s.is_empty())
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// A name the server accepts: this kind's prefix, then `[a-z0-9._-]`, at most 64 characters in total.
pub fn valid_name(kind: ActivityKind, name: &str) -> bool {
    name.strip_prefix(kind.prefix())
        .is_some_and(|tail| slug_ok(tail, 58))
}

impl ActivityLog {
    pub fn open(path: PathBuf) -> Result<Self, LibraryError> {
        let mut saved = if path.exists() {
            let meta = fs::symlink_metadata(&path).map_err(|_| LibraryError::Storage)?;
            if !meta.file_type().is_file() || meta.len() > 2 * MAX_BYTES {
                return Err(LibraryError::Storage);
            }
            serde_json::from_slice::<Saved>(&fs::read(&path).map_err(|_| LibraryError::Storage)?)
                .map_err(|_| LibraryError::Storage)?
        } else {
            Saved::default()
        };
        // A hand-edited file is not partially trusted: one bad name empties the record set, as in `core::telemetry`.
        if saved.records.len() > MAX_RECORDS
            || saved.records.iter().any(|r| !valid_name(r.kind, &r.name))
        {
            saved.records.clear();
        }
        // A credential learned after an entry was written is masked on the way out, in `view_of` and `pending`, so the
        // work is bounded by the entries read rather than by the whole file at launch.
        if saved.log_id.is_empty() {
            saved.log_id = new_log_id();
        }
        Ok(Self {
            path,
            inner: Mutex::new(Inner {
                saved,
                committed: Instant::now(),
            }),
        })
    }

    pub fn view(&self) -> Result<ActivityView, LibraryError> {
        let g = self.inner.lock().map_err(|_| LibraryError::Storage)?;
        Ok(view_of(&g.saved))
    }

    pub fn enabled(&self) -> Result<bool, LibraryError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| LibraryError::Storage)?
            .saved
            .enabled)
    }

    pub fn log_id(&self) -> Result<String, LibraryError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| LibraryError::Storage)?
            .saved
            .log_id
            .clone())
    }

    /// Turns uploading on or off. The local log is kept either way: recording activity is what makes the Dev Mode screen
    /// useful, and consent governs only what leaves the device.
    pub fn set_enabled(&self, on: bool) -> Result<ActivityView, LibraryError> {
        self.update(|s| s.enabled = on)
    }

    /// Notes that the user has been asked, so the prompt does not return on every launch.
    pub fn set_asked(&self) -> Result<ActivityView, LibraryError> {
        self.update(|s| {
            s.asked = true;
        })
    }

    /// Appends one entry, absorbing a repeat of the trailing entry when it is the same thing happening again inside the
    /// coalescing window. Returns what is now stored.
    pub fn record(&self, input: &NewActivity) -> Result<ActivityRecord, LibraryError> {
        let Some(kind) = parse_kind(&input.kind) else {
            return Err(LibraryError::Validation);
        };
        if !valid_name(kind, &input.name) {
            return Err(LibraryError::Validation);
        }
        let detail = scrub(&input.detail, DETAIL_CHARS).text;
        let at = now_ms();
        let record = ActivityRecord {
            uid: new_uid(),
            at_ms: at,
            kind,
            name: input.name.clone(),
            detail,
            ok: input.ok,
            count: 1,
            sent: false,
        };
        let mut g = self.inner.lock().map_err(|_| LibraryError::Storage)?;
        let merged = match g.saved.records.last_mut() {
            Some(last)
                if !last.sent
                    && last.kind == record.kind
                    && last.name == record.name
                    && last.detail == record.detail
                    && last.ok == record.ok
                    && at.saturating_sub(last.at_ms) <= COALESCE_MS =>
            {
                last.count = last.count.saturating_add(1);
                Some((*last).clone())
            }
            _ => None,
        };
        let stored = match merged {
            Some(out) => out,
            None => {
                if g.saved.records.len() >= MAX_RECORDS {
                    // A ring: the oldest goes. Unlike the error store this one is a rolling window by design, and the
                    // settings screen states the cap.
                    g.saved.records.remove(0);
                }
                g.saved.records.push(record.clone());
                record
            }
        };
        if g.committed.elapsed() >= COMMIT_INTERVAL {
            self.write_locked(&mut g)?;
        }
        Ok(stored)
    }

    /// What has never been uploaded, oldest first, at most one batch. Reading does not clear anything.
    pub fn pending(&self) -> Result<Vec<ActivityRecord>, LibraryError> {
        let g = self.inner.lock().map_err(|_| LibraryError::Storage)?;
        Ok(g.saved
            .records
            .iter()
            .filter(|r| !r.sent)
            .take(MAX_BATCH)
            .map(scrubbed)
            .collect())
    }

    /// Marks an uploaded batch and re-masks what is stored, so a credential learned since an entry was written leaves the
    /// file as well as the preview.
    pub fn mark_sent(&self, uids: &[String]) -> Result<(), LibraryError> {
        self.update(|s| {
            for r in &mut s.records {
                let clean = scrubbed(r);
                *r = clean;
                if uids.iter().any(|u| u == &r.uid) {
                    r.sent = true;
                }
            }
        })
        .map(|_| ())
    }

    pub fn clear(&self) -> Result<ActivityView, LibraryError> {
        self.update(|s| {
            s.records.clear();
        })
    }

    /// After the server deleted this device's rows: a new anonymous id, uploading off, nothing kept.
    pub fn reset(&self) -> Result<ActivityView, LibraryError> {
        self.update(|s| {
            s.enabled = false;
            s.records.clear();
            s.log_id = new_log_id();
        })
    }

    /// Writes what is held in memory whether or not the throttle window has passed. The upload path calls it first, so
    /// what the user reviews is what is on disk.
    pub fn flush(&self) -> Result<ActivityView, LibraryError> {
        let mut g = self.inner.lock().map_err(|_| LibraryError::Storage)?;
        self.write_locked(&mut g)
    }

    fn update(&self, change: impl FnOnce(&mut Saved)) -> Result<ActivityView, LibraryError> {
        let mut g = self.inner.lock().map_err(|_| LibraryError::Storage)?;
        change(&mut g.saved);
        self.write_locked(&mut g)
    }

    /// Rewrites the whole file after dropping entries past the age limit. The caller holds the lock, and a torn file
    /// cannot exist because the rename replaces the whole document.
    fn write_locked(&self, g: &mut Inner) -> Result<ActivityView, LibraryError> {
        let cutoff = now_ms().saturating_sub(MAX_AGE_MS);
        let stale = g
            .saved
            .records
            .iter()
            .position(|r| r.at_ms >= cutoff)
            .unwrap_or(g.saved.records.len());
        if stale > 0 {
            g.saved.records.drain(..stale);
        }
        let bytes = shrink_to_budget(&mut g.saved, MAX_BYTES)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| LibraryError::Storage)?;
        }
        let part = self
            .path
            .with_extension(format!("{:032x}.part", rand::random::<u128>()));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&part).map_err(|_| LibraryError::Storage)?;
            file.write_all(&bytes).map_err(|_| LibraryError::Storage)?;
            file.sync_all().map_err(|_| LibraryError::Storage)?;
            fs::rename(&part, &self.path).map_err(|_| LibraryError::Storage)
        })();
        let _ = fs::remove_file(part);
        result?;
        g.committed = Instant::now();
        Ok(view_of(&g.saved))
    }
}

/// Serialises the store, dropping the oldest entries in halves until it fits `max_bytes`. A few hundred bytes of detail
/// times twenty thousand entries passes the file budget before the record cap does, and this is a rolling window by
/// design: failing the write instead would leave the log never reaching disk again.
fn shrink_to_budget(saved: &mut Saved, max_bytes: u64) -> Result<Vec<u8>, LibraryError> {
    let mut bytes = serde_json::to_vec(saved).map_err(|_| LibraryError::Storage)?;
    while bytes.len() as u64 > max_bytes && saved.records.len() > 1 {
        let half = saved.records.len() / 2;
        saved.records.drain(..half);
        bytes = serde_json::to_vec(saved).map_err(|_| LibraryError::Storage)?;
    }
    if bytes.len() as u64 > max_bytes {
        return Err(LibraryError::Full);
    }
    Ok(bytes)
}

/// Re-masks a stored entry on the way out, for a credential created or read since it was written.
fn scrubbed(record: &ActivityRecord) -> ActivityRecord {
    ActivityRecord {
        detail: scrub(&record.detail, DETAIL_CHARS).text,
        ..record.clone()
    }
}

fn view_of(s: &Saved) -> ActivityView {
    let records: Vec<ActivityRecord> = s
        .records
        .iter()
        .rev()
        .take(MAX_VIEW)
        .map(scrubbed)
        .collect();
    ActivityView {
        enabled: s.enabled,
        asked: s.asked,
        log_id: s.log_id.clone(),
        unsent: s.records.iter().filter(|r| !r.sent).count(),
        total: s.records.len(),
        capacity: MAX_RECORDS,
        records,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::secrets::registry;

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "lunorx-activity-{:032x}.json",
            rand::random::<u128>()
        ))
    }

    fn open() -> (ActivityLog, PathBuf) {
        let p = path();
        (ActivityLog::open(p.clone()).unwrap(), p)
    }

    fn entry(kind: &str, name: &str, detail: &str) -> NewActivity {
        NewActivity {
            kind: kind.into(),
            name: name.into(),
            detail: detail.into(),
            ok: true,
        }
    }

    #[test]
    fn a_name_must_carry_its_own_kind_prefix() {
        let (log, p) = open();
        assert!(valid_name(ActivityKind::Action, "action.navigate.home"));
        assert!(valid_name(ActivityKind::Ipc, "ipc.sync_status"));
        assert!(valid_name(ActivityKind::Event, "event.player-progress"));
        assert!(!valid_name(ActivityKind::Action, "event.navigate"));
        assert!(!valid_name(ActivityKind::Ipc, "sync_status"));
        assert!(!valid_name(ActivityKind::Action, "action."));
        assert!(!valid_name(ActivityKind::Action, "action.Search"));
        assert!(!valid_name(
            ActivityKind::Action,
            &format!("action.{}", "a".repeat(60))
        ));
        assert!(matches!(
            log.record(&entry("action", "event.player-progress", "")),
            Err(LibraryError::Validation)
        ));
        assert!(matches!(
            log.record(&entry("robot", "action.navigate.home", "")),
            Err(LibraryError::Validation)
        ));
        assert!(log.view().unwrap().records.is_empty());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_user_action_keeps_its_context_and_a_search_keeps_its_query() {
        let (log, p) = open();
        let kept = log
            .record(&entry(
                "action",
                "action.playback.start",
                "Bohemian Rhapsody — Queen",
            ))
            .unwrap();
        assert_eq!(kept.count, 1);
        assert!(kept.ok);
        log.record(&entry("action", "action.navigate.search", "lovatic"))
            .unwrap();
        let v = log.view().unwrap();
        assert_eq!(v.total, 2);
        assert_eq!(v.records[0].detail, "lovatic");
        assert_eq!(v.records[1].detail, "Bohemian Rhapsody — Queen");
        let _ = fs::remove_file(p);
    }

    #[test]
    fn repeated_events_inside_the_window_count_instead_of_growing_the_log() {
        let (log, p) = open();
        for _ in 0..25 {
            log.record(&entry("event", "event.player-progress", ""))
                .unwrap();
        }
        let v = log.view().unwrap();
        assert_eq!(v.total, 1);
        assert_eq!(v.records[0].count, 25);
        // A different detail is a different thing that happened, so it does not merge.
        log.record(&entry("event", "event.player-progress", "next"))
            .unwrap();
        assert_eq!(log.view().unwrap().total, 2);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_credential_in_the_detail_is_masked_before_it_is_written() {
        let (log, p) = open();
        let secret = "ya29.a0AfH6SMBx7KqLmZABCDEFGHIJKLMNOPQRSTUV";
        registry().register(secret);
        log.record(&entry(
            "ipc",
            "ipc.sync_sign_in",
            &format!("failed for {secret}"),
        ))
        .unwrap();
        let v = log.view().unwrap();
        assert!(!v.records[0].detail.contains(secret));
        assert!(v.records[0].detail.contains("<masked:"));
        log.flush().unwrap();
        assert!(!fs::read_to_string(&p).unwrap().contains(secret));
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_failed_entry_is_marked_and_still_pending() {
        let (log, p) = open();
        let mut e = entry("ipc", "ipc.player_load", "mpv rejected the URI");
        e.ok = false;
        log.record(&e).unwrap();
        let v = log.view().unwrap();
        assert!(!v.records[0].ok);
        assert_eq!(v.unsent, 1);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn the_throttle_defers_the_write_and_flush_forces_it() {
        let (log, p) = open();
        log.record(&entry("action", "action.library.like", "a track"))
            .unwrap();
        assert!(!p.exists(), "the two second window has not passed");
        let v = log.flush().unwrap();
        assert_eq!(v.total, 1);
        assert!(p.exists());
        let again = ActivityLog::open(p.clone()).unwrap();
        assert_eq!(again.view().unwrap().records[0].name, "action.library.like");
        assert_eq!(again.log_id().unwrap(), v.log_id);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn reading_pending_does_not_destroy_it_and_mark_sent_is_by_uid() {
        let (log, p) = open();
        log.set_enabled(true).unwrap();
        log.record(&entry("action", "action.a", "")).unwrap();
        log.record(&entry("action", "action.b", "")).unwrap();
        log.record(&entry("action", "action.c", "")).unwrap();
        let first = log.pending().unwrap();
        assert_eq!(first.len(), 3);
        assert_eq!(log.pending().unwrap().len(), 3);
        log.mark_sent(&[first[0].uid.clone()]).unwrap();
        let rest = log.pending().unwrap();
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[0].name, "action.b");
        let _ = fs::remove_file(p);
    }

    #[test]
    fn turning_uploading_off_keeps_the_log_and_reset_starts_a_new_anonymous_id() {
        let (log, p) = open();
        log.record(&entry("action", "action.a", "")).unwrap();
        let id = log.log_id().unwrap();
        assert_eq!(id.len(), 36);
        log.set_enabled(true).unwrap();
        log.set_asked().unwrap();
        log.set_enabled(false).unwrap();
        assert_eq!(
            log.view().unwrap().total,
            1,
            "the local log is not consent-scoped"
        );
        let after = log.reset().unwrap();
        assert!(!after.enabled && after.records.is_empty() && after.log_id != id);
        assert!(after.asked);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn the_store_rotates_at_its_cap_and_keeps_the_newest() {
        let p = path();
        let at = now_ms();
        let records = (0..MAX_RECORDS)
            .map(|i| ActivityRecord {
                uid: format!("{i:012x}"),
                at_ms: at,
                kind: ActivityKind::Action,
                name: "action.old".into(),
                detail: String::new(),
                ok: true,
                count: 1,
                sent: true,
            })
            .collect();
        let saved = Saved {
            enabled: true,
            asked: true,
            log_id: "00000000-0000-4000-8000-000000000000".into(),
            records,
        };
        fs::write(&p, serde_json::to_vec(&saved).unwrap()).unwrap();
        let log = ActivityLog::open(p.clone()).unwrap();
        log.record(&entry("action", "action.newest", "")).unwrap();
        let v = log.view().unwrap();
        assert_eq!(v.total, MAX_RECORDS);
        assert_eq!(v.records[0].name, "action.newest");
        assert_eq!(log.pending().unwrap().len(), 1);
        let _ = fs::remove_file(p);
    }

    fn budget_store(count: usize, detail: &str) -> Saved {
        let at = now_ms();
        Saved {
            enabled: true,
            asked: true,
            log_id: "00000000-0000-4000-8000-000000000000".into(),
            records: (0..count)
                .map(|i| ActivityRecord {
                    uid: format!("{i:012x}"),
                    at_ms: at,
                    kind: ActivityKind::Ipc,
                    name: "ipc.search_query".into(),
                    detail: detail.into(),
                    ok: true,
                    count: 1,
                    sent: true,
                })
                .collect(),
        }
    }

    #[test]
    fn a_store_over_the_file_budget_loses_the_oldest_not_the_write() {
        let mut saved = budget_store(200, &"search term ".repeat(40));
        let whole = serde_json::to_vec(&saved).unwrap().len() as u64;
        let bytes = shrink_to_budget(&mut saved, whole / 4).unwrap();
        assert!((bytes.len() as u64) <= whole / 4);
        assert!(saved.records.len() < 200);
        assert_eq!(saved.records.last().unwrap().uid, "0000000000c7");
        assert_ne!(saved.records.first().unwrap().uid, "000000000000");
    }

    #[test]
    fn one_record_bigger_than_the_budget_is_an_error_not_a_silent_emptying() {
        let mut saved = budget_store(1, &"search term ".repeat(40));
        assert!(matches!(
            shrink_to_budget(&mut saved, 100),
            Err(LibraryError::Full)
        ));
        assert_eq!(saved.records.len(), 1);
    }

    #[test]
    fn entries_past_the_age_limit_are_dropped_at_the_next_write() {
        let p = path();
        let old = now_ms().saturating_sub(MAX_AGE_MS + 60_000);
        let saved = Saved {
            enabled: true,
            asked: true,
            log_id: "00000000-0000-4000-8000-000000000000".into(),
            records: vec![
                ActivityRecord {
                    uid: "1".into(),
                    at_ms: old,
                    kind: ActivityKind::Action,
                    name: "action.ancient".into(),
                    detail: String::new(),
                    ok: true,
                    count: 40,
                    sent: true,
                },
                ActivityRecord {
                    uid: "2".into(),
                    at_ms: now_ms(),
                    kind: ActivityKind::Action,
                    name: "action.recent".into(),
                    detail: String::new(),
                    ok: true,
                    count: 1,
                    sent: true,
                },
            ],
        };
        fs::write(&p, serde_json::to_vec(&saved).unwrap()).unwrap();
        let log = ActivityLog::open(p.clone()).unwrap();
        let v = log.flush().unwrap();
        assert_eq!(v.total, 1);
        assert_eq!(v.records[0].name, "action.recent");
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_file_with_a_forged_name_is_emptied_on_open() {
        let p = path();
        let saved = Saved {
            enabled: true,
            asked: true,
            log_id: "a".into(),
            records: vec![ActivityRecord {
                uid: "1".into(),
                at_ms: now_ms(),
                kind: ActivityKind::Action,
                name: "action.ok sk-provision-secret".into(),
                detail: String::new(),
                ok: true,
                count: 1,
                sent: false,
            }],
        };
        fs::write(&p, serde_json::to_vec(&saved).unwrap()).unwrap();
        let log = ActivityLog::open(p.clone()).unwrap();
        assert!(log.view().unwrap().records.is_empty());
        assert!(log.pending().unwrap().is_empty());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn long_detail_is_clamped() {
        let (log, p) = open();
        // Words separated by spaces: an unbroken run of 600 token-shaped characters would be masked as one credential
        // instead of reaching the clamp, which a separate test in `core::secrets` covers.
        log.record(&entry(
            "ipc",
            "ipc.search_query",
            &"search term ".repeat(60),
        ))
        .unwrap();
        let v = log.view().unwrap();
        assert_eq!(v.records[0].detail.chars().count(), DETAIL_CHARS);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn an_uploaded_entry_is_not_merged_into_again() {
        let (log, p) = open();
        let first = log
            .record(&entry("event", "event.app-foreground", ""))
            .unwrap();
        log.mark_sent(std::slice::from_ref(&first.uid)).unwrap();
        let second = log
            .record(&entry("event", "event.app-foreground", ""))
            .unwrap();
        assert_ne!(second.uid, first.uid, "a sent entry is never absorbed");
        assert_eq!(second.count, 1);
        assert_eq!(log.view().unwrap().total, 2);
        let _ = fs::remove_file(p);
    }
}

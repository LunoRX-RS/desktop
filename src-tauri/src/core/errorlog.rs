//! Opt-in error reports. Records are kept on this device; nothing is uploaded until the user says so, and every free-text
//! field has passed `core::secrets::scrub` before it reaches disk. Deliberately separate from `core::telemetry`: usage
//! counts and incident reports have different consent, different content and different retention.
use super::library::LibraryError;
use super::secrets::scrub;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, sync::Mutex};

const MAX_RECORDS: usize = 500;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
/// What is taken for one upload. The server caps a batch at 50 too, so a bigger batch is refused as invalid.
const MAX_BATCH: usize = 50;
/// What the settings screen may ask for in one IPC response.
const MAX_VIEW: usize = 50;
const MESSAGE_CHARS: usize = 2_000;
const STACK_CHARS: usize = 6_000;
const DETAIL_CHARS: usize = 1_000;

/// The screens the app can be on, from `src/lib/store.ts`. An unknown name is a validation failure, not a text field.
const VIEWS: [&str; 16] = [
    "home",
    "explore",
    "search",
    "library",
    "playlists",
    "player",
    "connections",
    "accounts",
    "login",
    "settings",
    "sync",
    "replay",
    "shared",
    "artist",
    "album",
    "none",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSource {
    Frontend,
    Ipc,
    Core,
    Sync,
    Player,
    Native,
}

/// `None` for anything off this list, so an IPC caller cannot invent a category.
pub fn parse_source(value: &str) -> Option<ErrorSource> {
    match value {
        "frontend" => Some(ErrorSource::Frontend),
        "ipc" => Some(ErrorSource::Ipc),
        "core" => Some(ErrorSource::Core),
        "sync" => Some(ErrorSource::Sync),
        "player" => Some(ErrorSource::Player),
        "native" => Some(ErrorSource::Native),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorRecord {
    pub uid: String,
    pub at_ms: u64,
    pub source: ErrorSource,
    /// A grouped name such as `error.player.timeout`. Never the message.
    pub kind: String,
    /// Where it happened: a component, command or module name in `[a-z0-9._-]`, never a path or URL.
    pub site: String,
    pub view: String,
    pub message: String,
    pub stack: String,
    pub detail: String,
    /// How many credential-shaped values were withheld from these fields. Shown in the review screen.
    pub masked: usize,
    pub sent: bool,
}

/// One failure as observed by the caller. Deliberately not `Debug`: these strings are unscrubbed until `record` runs.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewError {
    pub source: String,
    pub kind: String,
    pub site: String,
    pub view: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLogView {
    pub enabled: bool,
    pub asked: bool,
    pub report_id: String,
    pub unsent: usize,
    pub capacity: usize,
    pub records: Vec<ErrorRecord>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    asked: bool,
    #[serde(default)]
    report_id: String,
    #[serde(default)]
    records: Vec<ErrorRecord>,
}

pub struct ErrorLog {
    path: PathBuf,
    saved: Mutex<Saved>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn new_uid() -> String {
    format!("{:012x}", rand::random::<u64>() >> 16)
}

/// A UUID v4 string. Its own value, never `telemetry`'s install id, so the two datasets are unjoinable unless the user
/// turns both on.
fn new_report_id() -> String {
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

/// Lower-case slug segments joined by dots: `[a-z0-9._-]`, no empty segment, at most `max` bytes. `a`, `a.b-c` pass;
/// `A`, `a b`, `a..b`, `.a` and `a.` do not, so the server groups on a shape it never has to guess about.
fn slug_ok(name: &str, max: usize) -> bool {
    !name.is_empty()
        && name.len() <= max
        && name.split('.').all(|s| !s.is_empty())
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// `error.` plus a slug the server will accept.
pub fn valid_kind(kind: &str) -> bool {
    kind.strip_prefix("error.")
        .is_some_and(|tail| slug_ok(tail, 58))
}

pub fn valid_site(site: &str) -> bool {
    slug_ok(site, 64)
}

pub fn valid_view(view: &str) -> bool {
    VIEWS.contains(&view)
}

impl ErrorLog {
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
        // A hand-edited or half-written file never reaches the upload path: slugs are re-checked and the whole record set
        // is dropped rather than partially trusted. Same posture as `core::telemetry`.
        if saved.records.len() > MAX_RECORDS
            || saved
                .records
                .iter()
                .any(|r| !valid_kind(&r.kind) || !valid_site(&r.site) || !valid_view(&r.view))
        {
            saved.records.clear();
        }
        // A secret that became known after a record was written is masked on the way out, in `view_of` and `pending`,
        // which bounds the work to the records actually read instead of the whole file at launch.
        if saved.report_id.is_empty() {
            saved.report_id = new_report_id();
        }
        Ok(Self {
            path,
            saved: Mutex::new(saved),
        })
    }

    pub fn view(&self) -> Result<ErrorLogView, LibraryError> {
        let s = self.saved.lock().map_err(|_| LibraryError::Storage)?;
        Ok(view_of(&s))
    }

    /// Turning sharing off stops uploads. Records stay on the device, unlike telemetry counters, which are dropped: the
    /// local log is the product as much as the upload is.
    pub fn set_enabled(&self, on: bool) -> Result<ErrorLogView, LibraryError> {
        self.update(|s| s.enabled = on)
    }

    /// Notes that the user has been asked, so the prompt does not return on every launch.
    pub fn set_asked(&self) -> Result<ErrorLogView, LibraryError> {
        self.update(|s| {
            s.asked = true;
        })
    }

    /// Stores one failure. A full store refuses the newest instead of evicting an unsent one, which is what
    /// `sync::queue` does for data the user has not lost yet.
    pub fn record(&self, input: &NewError) -> Result<(), LibraryError> {
        let Some(source) = parse_source(&input.source) else {
            return Err(LibraryError::Validation);
        };
        if !valid_kind(&input.kind) || !valid_site(&input.site) || !valid_view(&input.view) {
            return Err(LibraryError::Validation);
        }
        let message = scrub(&input.message, MESSAGE_CHARS);
        let stack = scrub(&input.stack, STACK_CHARS);
        let detail = scrub(&input.detail, DETAIL_CHARS);
        let record = ErrorRecord {
            uid: new_uid(),
            at_ms: now_ms(),
            source,
            kind: input.kind.clone(),
            site: input.site.clone(),
            view: input.view.clone(),
            masked: message.masked + stack.masked + detail.masked,
            message: message.text,
            stack: stack.text,
            detail: detail.text,
            sent: false,
        };
        let mut refused = false;
        self.update(|s| {
            if s.records.len() >= MAX_RECORDS {
                if s.records.iter().all(|r| r.sent) {
                    s.records.remove(0);
                } else {
                    refused = true;
                    return;
                }
            }
            s.records.push(record);
        })?;
        if refused {
            return Err(LibraryError::Full);
        }
        Ok(())
    }

    /// What has never been uploaded, oldest first, at most one batch. Reading does not clear anything.
    pub fn pending(&self) -> Result<Vec<ErrorRecord>, LibraryError> {
        let s = self.saved.lock().map_err(|_| LibraryError::Storage)?;
        Ok(s.records
            .iter()
            .filter(|r| !r.sent)
            .take(MAX_BATCH)
            .map(scrubbed)
            .collect())
    }

    pub fn enabled(&self) -> Result<bool, LibraryError> {
        Ok(self
            .saved
            .lock()
            .map_err(|_| LibraryError::Storage)?
            .enabled)
    }

    pub fn report_id(&self) -> Result<String, LibraryError> {
        Ok(self
            .saved
            .lock()
            .map_err(|_| LibraryError::Storage)?
            .report_id
            .clone())
    }

    /// Marks an uploaded batch. The upload path only ever marks, never deletes, so a crash between the server accepting
    /// a record and this write leaves it in the file to be sent again. Rewrites every record re-masked, which is when a
    /// credential learned since a record was written leaves the disk as well as the preview.
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

    /// Removes every record on this device. The report id is kept, so what the user already uploaded stays deletable.
    pub fn clear(&self) -> Result<ErrorLogView, LibraryError> {
        self.update(|s| {
            s.records.clear();
        })
    }

    /// After the server deleted this install's rows: a new anonymous id, sharing off, nothing kept.
    pub fn reset(&self) -> Result<ErrorLogView, LibraryError> {
        self.update(|s| {
            s.enabled = false;
            s.records.clear();
            s.report_id = new_report_id();
        })
    }

    fn update(&self, change: impl FnOnce(&mut Saved)) -> Result<ErrorLogView, LibraryError> {
        let mut s = self.saved.lock().map_err(|_| LibraryError::Storage)?;
        let mut next = s.clone();
        change(&mut next);
        let bytes = serde_json::to_vec(&next).map_err(|_| LibraryError::Storage)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(LibraryError::Full);
        }
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
        *s = next;
        Ok(view_of(&s))
    }
}

/// Re-masks a stored record on the way out. `record` already masked it against what was known then; a credential created
/// or read since then is caught here, so neither the settings preview nor an upload can carry it.
fn scrubbed(record: &ErrorRecord) -> ErrorRecord {
    let message = scrub(&record.message, MESSAGE_CHARS);
    let stack = scrub(&record.stack, STACK_CHARS);
    let detail = scrub(&record.detail, DETAIL_CHARS);
    ErrorRecord {
        masked: record
            .masked
            .max(message.masked + stack.masked + detail.masked),
        message: message.text,
        stack: stack.text,
        detail: detail.text,
        ..record.clone()
    }
}

fn view_of(s: &Saved) -> ErrorLogView {
    let records: Vec<ErrorRecord> = s
        .records
        .iter()
        .rev()
        .take(MAX_VIEW)
        .map(scrubbed)
        .collect();
    ErrorLogView {
        enabled: s.enabled,
        asked: s.asked,
        report_id: s.report_id.clone(),
        unsent: s.records.iter().filter(|r| !r.sent).count(),
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
            "lunorx-errorlog-{:032x}.json",
            rand::random::<u128>()
        ))
    }

    fn open() -> (ErrorLog, PathBuf) {
        let p = path();
        (ErrorLog::open(p.clone()).unwrap(), p)
    }

    fn input(message: &str) -> NewError {
        NewError {
            source: "frontend".into(),
            kind: "error.render.type-error".into(),
            site: "player-bar".into(),
            view: "home".into(),
            message: message.into(),
            stack: String::new(),
            detail: String::new(),
        }
    }

    fn stored(uid: &str, message: &str, sent: bool) -> ErrorRecord {
        ErrorRecord {
            uid: uid.into(),
            at_ms: 1,
            source: ErrorSource::Core,
            kind: "error.core.test".into(),
            site: "test".into(),
            view: "home".into(),
            message: message.into(),
            stack: String::new(),
            detail: String::new(),
            masked: 0,
            sent,
        }
    }

    fn filled(records: Vec<ErrorRecord>) -> Saved {
        Saved {
            enabled: true,
            asked: true,
            report_id: "00000000-0000-4000-8000-000000000000".into(),
            records,
        }
    }

    #[test]
    fn slugs_follow_the_server_rules_and_views_are_an_allowlist() {
        assert!(valid_kind("error.player.timeout"));
        assert!(!valid_kind("player.timeout"));
        assert!(!valid_kind("error."));
        assert!(!valid_kind("error.A"));
        assert!(!valid_kind("error.a b"));
        assert!(!valid_kind(&format!("error.{}", "a".repeat(60))));
        assert!(valid_site("sync.sign-in"));
        assert!(!valid_site("https://x.dev/a"));
        assert!(valid_view("playlists"));
        assert!(!valid_view("Settings"));
        assert!(!valid_view(""));
    }

    #[test]
    fn an_argument_outside_the_allowlists_is_refused_and_nothing_is_stored() {
        let (log, p) = open();
        let mut bad = input("boom");
        bad.source = "Bearer eyJhbGciOi".into();
        assert!(matches!(log.record(&bad), Err(LibraryError::Validation)));
        bad.source = "frontend".into();
        bad.kind = "a song title".into();
        assert!(matches!(log.record(&bad), Err(LibraryError::Validation)));
        bad.kind = "error.render.type-error".into();
        bad.site = "https://music.youtube.com/watch?v=x".into();
        assert!(matches!(log.record(&bad), Err(LibraryError::Validation)));
        bad.site = "player-bar".into();
        bad.view = "Settings".into();
        assert!(matches!(log.record(&bad), Err(LibraryError::Validation)));
        assert!(log.view().unwrap().records.is_empty());
        assert!(log.record(&input("x")).is_ok());
        assert_eq!(log.view().unwrap().records.len(), 1);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn records_are_kept_while_sharing_is_off_and_nothing_is_claimed_sent() {
        let (log, p) = open();
        log.record(&input("offline")).unwrap();
        assert!(!log.enabled().unwrap());
        assert_eq!(log.pending().unwrap().len(), 1);
        assert_eq!(log.view().unwrap().unsent, 1);
        log.set_enabled(true).unwrap();
        assert!(log.view().unwrap().enabled);
        log.set_enabled(false).unwrap();
        assert_eq!(log.view().unwrap().records.len(), 1);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn reading_pending_does_not_destroy_it_and_mark_sent_is_by_uid() {
        let (log, p) = open();
        log.set_enabled(true).unwrap();
        log.record(&input("one")).unwrap();
        log.record(&input("two")).unwrap();
        let first = log.pending().unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(log.pending().unwrap().len(), 2);
        let uids: Vec<String> = first.iter().take(1).map(|r| r.uid.clone()).collect();
        log.mark_sent(&uids).unwrap();
        let rest = log.pending().unwrap();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].message, "two");
        assert_eq!(log.view().unwrap().unsent, 1);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_record_with_a_credential_in_it_is_stored_masked() {
        let (log, p) = open();
        let secret = "z9X8w7V6u5T4s3R2q1P0o7I6n5M4l3K2";
        registry().register(secret);
        log.record(&input(&format!("session value {secret} rejected")))
            .unwrap();
        let record = &log.view().unwrap().records[0];
        assert!(!record.message.contains(secret));
        assert!(record.message.contains("<masked:"));
        assert_eq!(record.masked, 1);
        assert!(!fs::read_to_string(&p).unwrap().contains(secret));
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_stored_record_is_masked_again_when_a_secret_becomes_known_later() {
        let p = path();
        let secret = "q1W2e3R4t5Y6u7I8o9P0aSdFgHjKlz";
        // A file written by an older build, before this value was ever registered.
        let saved = filled(vec![stored(
            "1",
            &format!("stream failed with {secret}"),
            false,
        )]);
        fs::write(&p, serde_json::to_vec(&saved).unwrap()).unwrap();
        registry().register(secret);
        let log = ErrorLog::open(p.clone()).unwrap();
        let record = &log.view().unwrap().records[0];
        assert!(!record.message.contains(secret));
        assert_eq!(record.masked, 1);
        assert!(!log.pending().unwrap()[0].message.contains(secret));
        // The rewrite that follows an upload takes it off the disk too, not only out of what is shown.
        log.mark_sent(&["1".into()]).unwrap();
        assert!(!fs::read_to_string(&p).unwrap().contains(secret));
        let _ = fs::remove_file(p);
    }

    #[test]
    fn the_newest_record_is_refused_rather_than_an_unsent_one_evicted() {
        let p = path();
        let records = (0..MAX_RECORDS)
            .map(|i| stored(&format!("{i:012x}"), "older", false))
            .collect();
        fs::write(&p, serde_json::to_vec(&filled(records)).unwrap()).unwrap();
        let log = ErrorLog::open(p.clone()).unwrap();
        assert!(matches!(
            log.record(&input("one too many")),
            Err(LibraryError::Full)
        ));
        let v = log.view().unwrap();
        assert_eq!(v.unsent, MAX_RECORDS);
        assert!(!v.records.iter().any(|r| r.message == "one too many"));
        assert_eq!(log.pending().unwrap().len(), MAX_BATCH);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn one_uploaded_record_does_not_make_room_when_unsent_ones_are_still_held() {
        let p = path();
        let mut records: Vec<ErrorRecord> = (0..MAX_RECORDS - 1)
            .map(|i| stored(&format!("{i:012x}"), "older", false))
            .collect();
        records.push(stored("ffffffffffff", "already uploaded", true));
        fs::write(&p, serde_json::to_vec(&filled(records)).unwrap()).unwrap();
        let log = ErrorLog::open(p.clone()).unwrap();
        assert!(matches!(
            log.record(&input("newest")),
            Err(LibraryError::Full)
        ));
        let v = log.view().unwrap();
        assert_eq!(v.unsent, MAX_RECORDS - 1);
        assert!(!v.records.iter().any(|r| r.message == "newest"));
        assert!(v.records.iter().any(|r| r.message == "already uploaded"));
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_fully_uploaded_store_makes_room_for_a_new_failure() {
        let p = path();
        let records = (1..=MAX_RECORDS)
            .map(|i| stored(&format!("{i:012x}"), "uploaded", true))
            .collect();
        fs::write(&p, serde_json::to_vec(&filled(records)).unwrap()).unwrap();
        let log = ErrorLog::open(p.clone()).unwrap();
        log.record(&input("newest")).unwrap();
        let v = log.view().unwrap();
        assert_eq!(
            v.unsent, 1,
            "the oldest uploaded record left, the newest joined"
        );
        assert!(v.records.iter().any(|r| r.message == "newest"));
        let _ = fs::remove_file(p);
    }

    #[test]
    fn records_survive_a_reopen_and_reset_makes_a_new_anonymous_id() {
        let (log, p) = open();
        log.record(&input("persisted")).unwrap();
        let id = log.report_id().unwrap();
        assert_eq!(id.len(), 36);
        let again = ErrorLog::open(p.clone()).unwrap();
        assert_eq!(again.report_id().unwrap(), id);
        assert_eq!(again.view().unwrap().records[0].message, "persisted");
        assert!(!again.view().unwrap().asked);
        assert!(!again.view().unwrap().enabled);
        again.set_asked().unwrap();
        let after = again.reset().unwrap();
        assert!(!after.enabled && after.records.is_empty() && after.report_id != id);
        assert!(
            after.asked,
            "the user's answer outlives the data it was about"
        );
        let _ = fs::remove_file(p);
    }

    #[test]
    fn a_file_with_a_forged_slug_is_emptied_on_open() {
        let p = path();
        let mut forged = stored("1", "m", false);
        forged.kind = "error.ok sk-provision-secret".into();
        fs::write(&p, serde_json::to_vec(&filled(vec![forged])).unwrap()).unwrap();
        let log = ErrorLog::open(p.clone()).unwrap();
        assert!(log.view().unwrap().records.is_empty());
        assert!(log.pending().unwrap().is_empty());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn long_text_is_clamped_so_one_record_cannot_fill_the_file() {
        let (log, p) = open();
        // Prose with spaces: an unbroken run of 40+ token-shaped characters is masked as a credential instead of
        // reaching the clamp, which `core::secrets` tests cover.
        let mut input = input(&"cannot read property of undefined ".repeat(100));
        input.stack = "at onClick (src/lib/playback.ts:91:12) ".repeat(200);
        input.detail = "queue length 3 provider youtube ".repeat(50);
        log.record(&input).unwrap();
        let record = &log.view().unwrap().records[0];
        assert_eq!(record.message.chars().count(), MESSAGE_CHARS);
        assert_eq!(record.stack.chars().count(), STACK_CHARS);
        assert_eq!(record.detail.chars().count(), DETAIL_CHARS);
        assert_eq!(record.source, ErrorSource::Frontend);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn an_unbroken_token_shaped_run_is_masked_rather_than_clamped() {
        let (log, p) = open();
        let mut input = input(&"a".repeat(MESSAGE_CHARS + 500));
        input.stack = "b".repeat(STACK_CHARS + 500);
        input.detail = "c".repeat(DETAIL_CHARS + 500);
        log.record(&input).unwrap();
        let record = &log.view().unwrap().records[0];
        assert_eq!(record.message, "<masked:2500>");
        assert_eq!(record.stack, "<masked:6500>");
        assert_eq!(record.detail, "<masked:1500>");
        assert!(record.masked >= 3);
        let _ = fs::remove_file(p);
    }
}

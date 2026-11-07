//! One radio session. The host's confirmed EOF (or a failed track) is the only advance trigger. A session does not give up
//! on a failure that can pass: a network error, a refused stream, a track that cannot be loaded. It waits, tries the
//! next candidate and asks again, so music keeps playing for as long as the listener lets it.
use super::{
    audio::AudioControl,
    cache::{DiskCache, OfflineTrack},
    command::PlaybackCommand,
    engine::PlaybackEngine,
    extract::valid_id,
    learn::Taste,
    state::PlaybackState,
    taste::{Candidate, Features, Profile, Vibe, features, sequence},
};
use crate::core::{
    domain::{ProviderId, TrackReference},
    yt::Yt,
};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, watch};

/// How the end of the current track is handled. `repeat` is "off", "all" (play the list again once it is used up)
/// or "one" (play this track again). `radio` adds related tracks when the list runs out.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoplayStatus {
    pub state: &'static str,
    pub next_title: Option<String>,
    pub queue: Vec<OfflineTrack>,
    pub repeat: &'static str,
    pub radio: bool,
    /// How closely related tracks stick to what plays: "same", "balanced" or "explore".
    pub vibe: &'static str,
    #[serde(skip)]
    history: VecDeque<String>,
    /// Tracks already left, newest last; backs the Previous command.
    #[serde(skip)]
    back: Vec<TrackReference>,
    /// The list that was started (first track, then the tracks queued behind it); repeat-all plays it again.
    #[serde(skip)]
    context: Vec<OfflineTrack>,
    /// True when that list came from a playlist or album, so related tracks wait until it is used up.
    #[serde(skip)]
    list: bool,
    /// Bumped on every manual load, which replaces the queue; a slow `load_more` that started before it is dropped.
    #[serde(skip)]
    generation: u64,
}
impl Default for AutoplayStatus {
    fn default() -> Self {
        Self {
            state: "",
            next_title: None,
            queue: Vec::new(),
            repeat: "off",
            radio: true,
            vibe: "balanced",
            history: VecDeque::new(),
            back: Vec::new(),
            context: Vec::new(),
            list: false,
            generation: 0,
        }
    }
}
impl AutoplayStatus {
    /// Like `Default`, but keeps the Previous stack and the user's repeat and autoplay choices across a manual load.
    pub fn reset(&mut self) {
        *self = Self {
            back: std::mem::take(&mut self.back),
            repeat: self.repeat,
            radio: self.radio,
            vibe: self.vibe,
            generation: self.generation.wrapping_add(1),
            ..Default::default()
        };
    }
}
/// Sets repeat ("off" | "all" | "one"), autoplay and, when given, the vibe ("same" | "balanced" | "explore"). Anything else is refused.
pub fn set_mode(state: &Status, repeat: &str, radio: bool, vibe: Option<&str>) -> bool {
    let repeat: &'static str = match repeat {
        "off" => "off",
        "all" => "all",
        "one" => "one",
        _ => return false,
    };
    let vibe = match vibe.map(Vibe::parse) {
        Some(None) => return false,
        Some(Some(v)) => Some(v.as_str()),
        None => None,
    };
    if let Ok(mut s) = state.lock() {
        s.repeat = repeat;
        s.radio = radio;
        if let Some(v) = vibe {
            s.vibe = v;
        }
        return true;
    }
    false
}
fn offline_of(t: &TrackReference) -> OfflineTrack {
    OfflineTrack {
        provider: "YouTube".into(),
        id: t.provider_track_id.clone(),
        title: t.title.clone(),
        artists: t.artists.clone(),
        artwork_url: t.artwork_url.clone(),
        duration_ms: t.duration_ms.and_then(|d| i64::try_from(d).ok()),
    }
}
/// Remembers the list a manual load started, for repeat-all.
pub fn set_context(state: &Status, first: &TrackReference, following: &[OfflineTrack]) {
    if let Ok(mut s) = state.lock() {
        let mut list = vec![offline_of(first)];
        list.extend(following.iter().cloned());
        s.context = list;
        s.list = !following.is_empty();
    }
}
/// Next with nothing queued and repeat-all on: starts the remembered list again.
pub fn restart_context(state: &Status) -> Option<TrackReference> {
    let mut s = state.lock().ok()?;
    if s.repeat != "all" || s.context.is_empty() {
        return None;
    }
    let first = reference(&s.context[0]).ok()?;
    s.queue = s.context.iter().skip(1).cloned().collect();
    s.next_title = None;
    s.state = "finding";
    Some(first)
}
pub fn push_back(state: &Status, track: TrackReference) {
    if let Ok(mut s) = state.lock() {
        if s.back
            .last()
            .is_some_and(|t| t.provider_track_id == track.provider_track_id)
        {
            return;
        }
        s.back.push(track);
        if s.back.len() > 50 {
            s.back.remove(0);
        }
    }
}
pub fn pop_back(state: &Status) -> Option<TrackReference> {
    state.lock().ok()?.back.pop()
}
/// How the user changes the up-next list by hand.
pub enum QueueEdit {
    /// Puts the track at the head, moving it up if it was already queued.
    Next(OfflineTrack),
    Remove(String),
    Move {
        from: usize,
        to: usize,
    },
}
const QUEUE_MAX: usize = 500;
/// Returns false when the edit does not apply (unknown id, index out of range).
pub fn edit_queue(state: &Status, edit: QueueEdit) -> bool {
    let Ok(mut s) = state.lock() else {
        return false;
    };
    let head = s.queue.first().map(|t| t.id.clone());
    let ok = match edit {
        QueueEdit::Next(track) => {
            s.queue.retain(|t| t.id != track.id);
            s.queue.insert(0, track);
            s.queue.truncate(QUEUE_MAX);
            true
        }
        QueueEdit::Remove(id) => match s.queue.iter().position(|t| t.id == id) {
            Some(i) => {
                s.queue.remove(i);
                true
            }
            None => false,
        },
        QueueEdit::Move { from, to } => {
            if from < s.queue.len() && to < s.queue.len() {
                let t = s.queue.remove(from);
                s.queue.insert(to, t);
                true
            } else {
                false
            }
        }
    };
    if ok && s.queue.first().map(|t| &t.id) != head.as_ref() {
        s.next_title = None;
    }
    ok
}
/// Head of the up-next list, consumed like a manual queue selection.
pub fn select_first(state: &Status) -> Option<TrackReference> {
    let id = state.lock().ok()?.queue.first()?.id.clone();
    select(state, &id)
}
pub type Status = Arc<std::sync::Mutex<AutoplayStatus>>;
fn status(target: &Status, state: &'static str, title: Option<String>) {
    if let Ok(mut value) = target.lock() {
        value.state = state;
        value.next_title = title;
    }
}

fn remember(history: &mut VecDeque<String>, id: String) {
    history.push_back(id);
    if history.len() > 100 {
        history.pop_front();
    }
}
fn eligible(id: &str, history: &VecDeque<String>) -> bool {
    valid_id(id) && !history.iter().any(|old| old == id)
}
/// Preparing the next song (downloading it ahead) is only a head start: the player can still load a song that was not
/// prepared, through the page player when the direct stream is refused. So preparing never takes longer than this in all.
const PREPARE_BUDGET: Duration = Duration::from_secs(60);
const PREPARE_TIMEOUT: Duration = Duration::from_secs(45);
/// The longest wait between two attempts after failures in a row.
const RETRY_CAP_SECS: u64 = 30;

/// 1 s after the first failure in a row, then 2, 4, 8, 16 and at most 30 s.
fn backoff(failures: u32) -> Duration {
    Duration::from_secs((1u64 << failures.saturating_sub(1).min(5)).min(RETRY_CAP_SECS))
}

/// Waits before the next attempt after `failures` in a row. While the song is still playing there is time to spare, so the
/// wait grows to the cap. If the song ends meanwhile the listener is waiting for sound, so what is left of the wait is
/// cut to [`OVER_RETRY_CAP`] from that moment. A change the caller must plan again for (`interrupted`) ends the wait
/// at once. The state is looked at every [`RETRY_POLL`], so progress reports cannot shorten the pacing.
async fn retry_pause(
    engine: &Mutex<PlaybackEngine>,
    failures: u32,
    interrupted: impl Fn() -> bool,
) {
    let mut until = Instant::now() + backoff(failures);
    let mut over_seen = false;
    loop {
        let now = Instant::now();
        if now >= until || interrupted() {
            return;
        }
        if !over_seen
            && matches!(
                engine.lock().await.state(),
                PlaybackState::Ended | PlaybackState::Error
            )
        {
            over_seen = true;
            until = until.min(now + OVER_RETRY_CAP);
        }
        tokio::time::sleep(RETRY_POLL.min(until.saturating_duration_since(now))).await;
    }
}
const RETRY_POLL: Duration = Duration::from_millis(250);
#[cfg(not(test))]
const OVER_RETRY_CAP: Duration = Duration::from_secs(4);
#[cfg(test)]
const OVER_RETRY_CAP: Duration = Duration::from_millis(400);

/// How a session ended. Neither is a failure of autoplay: the listener stopped, changed or ended the list.
#[derive(Debug, PartialEq, Eq)]
enum Ended {
    /// What was playing is gone (stopped, replaced, blocked) or the player went away.
    Gone,
    /// Autoplay is off and the list is used up: the normal end.
    Finished,
}

async fn session(
    engine: Arc<Mutex<PlaybackEngine>>,
    yt: Yt,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    taste: Taste,
    mut reports: watch::Receiver<u64>,
    state: Status,
) -> Ended {
    let mut history = state.lock().map(|s| s.history.clone()).unwrap_or_default();
    let mut profile = Profile::default();
    let mut observed: Option<String> = None;
    // Failures in a row (a candidate that would not load, nothing found to play); it paces the retries.
    let mut failures: u32 = 0;
    let mut planned_vibe: Option<&'static str> = None;
    let vibe_of = |state: &Status| state.lock().map(|s| s.vibe).unwrap_or("balanced");
    let mode = |state: &Status| {
        state
            .lock()
            .map(|s| (s.repeat, s.radio))
            .unwrap_or(("off", true))
    };
    'outer: loop {
        let current = engine.lock().await.active_track().cloned();
        let Some(current) = current else {
            return Ended::Gone;
        };
        let (repeat, radio) = mode(&state);
        let seed = current.provider_track_id.clone();
        // What is playing joins the session's taste (once, even if the ending is planned again after a mode change).
        if observed.as_deref() != Some(seed.as_str()) {
            profile.fade(PROFILE_KEEP);
            profile.observe(&offline_of(&current), 1.0);
            observed = Some(seed.clone());
        }
        remember(&mut history, seed.clone());
        if let Ok(mut status) = state.lock() {
            status.history = history.clone();
        }
        status(&state, "finding", None);
        let mut queue = state.lock().map(|s| s.queue.clone()).unwrap_or_default();
        // What the copy above was made from: asking for related music takes seconds, and an edit made meanwhile must
        // not be overwritten when this pass publishes its list.
        let cloned: Vec<OfflineTrack> = queue.clone();
        let (list, context) = state
            .lock()
            .map(|s| (s.list, s.context.clone()))
            .unwrap_or_default();
        // A new vibe applies to the songs after this one: the related part of the queue is asked for again. A list the user
        // started (a playlist or album) is left as it is.
        let vibe = vibe_of(&state);
        if planned_vibe.is_some_and(|v| v != vibe) && !list {
            queue.clear();
            if let Ok(mut log) = taste.lock() {
                log.forget_pending_except(Some(&seed));
            }
        }
        planned_vibe = Some(vibe);
        // A playlist or album plays through first; related tracks come in only once it is used up.
        if radio && repeat != "one" && (queue.is_empty() || (!list && queue.len() < 10)) {
            let recent = state
                .lock()
                .map(|s| {
                    s.back
                        .iter()
                        .rev()
                        .take(RECENT_RADIOS)
                        .map(|t| t.provider_track_id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            // The weights follow what the device has learned so far; a session that is still young keeps the built-in ones.
            if let Ok(log) = taste.lock() {
                profile.learn(log.learned());
            }
            let ctx = RadioContext {
                profile: &profile,
                vibe: Vibe::parse(vibe).unwrap_or(Vibe::Balanced),
                recent,
                current: offline_of(&current),
            };
            let (refilled, picks) = related(&yt, &seed, &history, queue, &ctx).await;
            if queue_edited(&state, &cloned) {
                continue 'outer;
            }
            queue = refilled;
            if let Ok(mut log) = taste.lock() {
                for (id, x) in picks {
                    log.expect(id, ctx.vibe, x);
                }
            }
        }
        if queue.is_empty() && repeat == "all" && !context.is_empty() {
            queue = context.iter().skip(1).cloned().collect();
            queue.insert(0, context[0].clone());
        }
        let mut published: Vec<OfflineTrack> = Vec::new();
        publish(&state, &queue, &mut published);
        let repeat_one = repeat == "one";
        if repeat_one {
            // The same track again: it is already cached, so this only keeps the file alive until Load takes it.
            queue = vec![offline_of(&current)];
        }
        // Prefer a song that could be prepared among the first few; if none could, the first one is tried as it is.
        let deadline = Instant::now() + PREPARE_BUDGET;
        let mut prepared = None;
        for (index, track) in queue.iter().take(3).cloned().enumerate() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            status(&state, "preparing", Some(track.title.clone()));
            // Keep the file alive until Load acquires it, including when cache quota is tight.
            if let Ok(Ok(file)) = tokio::time::timeout(
                left.min(PREPARE_TIMEOUT),
                cache.ensure(&yt, track.clone(), audio.settings().quality),
            )
            .await
            {
                prepared = Some((index, track, Some(file)));
                break;
            }
        }
        // The list may have been edited while songs were being prepared (an edit wakes the session): plan again from it.
        if queue_edited(&state, &published) {
            continue 'outer;
        }
        let (next, _file) = match prepared {
            Some((skipped, track, file)) => {
                // The ones passed over could not be prepared and are dropped from the list.
                queue.drain(..skipped);
                if skipped > 0 {
                    publish(&state, &queue, &mut published);
                }
                (track, file)
            }
            None => match queue.first().cloned() {
                Some(track) => (track, None),
                None if !radio => return Ended::Finished,
                None => {
                    // Related music is on but nothing came back (the service or the network failed): ask again.
                    failures = failures.saturating_add(1);
                    status(&state, "finding", None);
                    retry_pause(&engine, failures, || {
                        mode(&state) != (repeat, radio)
                            || vibe_of(&state) != vibe
                            || queue_edited(&state, &published)
                    })
                    .await;
                    continue 'outer;
                }
            },
        };
        status(&state, "ready", Some(next.title.clone()));
        // Best effort, after readiness is published; never awaited.
        super::track_info::prefetch_lyrics(&next.id, &next.title, &next.artists, next.duration_ms);
        loop {
            reports.borrow_and_update();
            // The user changed repeat or autoplay while this track plays: plan the ending again.
            if mode(&state) != (repeat, radio) || vibe_of(&state) != vibe {
                continue 'outer;
            }
            // The listener edited the up-next list: the song prepared for the end of this one may no longer be next.
            if queue_edited(&state, &published) {
                continue 'outer;
            }
            let mut player = engine.lock().await;
            match advance(&mut player, &seed, &next).await {
                Advance::Played => {
                    failures = 0;
                    // The song ended, so it was listened to in full.
                    if let Ok(mut log) = taste.lock() {
                        log.finish(&seed, 1.0);
                    }
                    push_back(&state, current.clone());
                    if !repeat_one {
                        queue.remove(0);
                    }
                    publish(&state, &queue, &mut published);
                    break;
                }
                Advance::Stalled => {
                    drop(player);
                    push_back(&state, current.clone());
                    if !repeat_one {
                        queue.remove(0);
                    }
                    publish(&state, &queue, &mut published);
                    failures = failures.saturating_add(1);
                    retry_pause(&engine, failures, || {
                        mode(&state) != (repeat, radio)
                            || vibe_of(&state) != vibe
                            || queue_edited(&state, &published)
                    })
                    .await;
                    continue 'outer;
                }
                Advance::Waiting => {}
                Advance::Gone => return Ended::Gone,
                Advance::Failed => {
                    drop(player);
                    // This one cannot be loaded (even through the page player): it leaves the list and the next one
                    // gets a turn, after a pause that grows while failures keep coming.
                    failures = failures.saturating_add(1);
                    queue.retain(|t| t.id != next.id);
                    publish(&state, &queue, &mut published);
                    status(&state, "finding", None);
                    retry_pause(&engine, failures, || {
                        mode(&state) != (repeat, radio)
                            || vibe_of(&state) != vibe
                            || queue_edited(&state, &published)
                    })
                    .await;
                    continue 'outer;
                }
            }
            drop(player);
            if reports.changed().await.is_err() {
                return Ended::Gone;
            }
        }
    }
}
const QUEUE_SIZE: usize = 50;
fn publish_queue(state: &Status, queue: &[OfflineTrack]) {
    if let Ok(mut status) = state.lock() {
        status.queue = queue.to_vec();
    }
}
/// Publishes `queue` and remembers what was published, so a later edit by the listener can be told apart.
fn publish(state: &Status, queue: &[OfflineTrack], published: &mut Vec<OfflineTrack>) {
    publish_queue(state, queue);
    *published = queue.to_vec();
}
/// True when the up-next list is no longer what the session last published: the listener edited it (or more songs
/// were added) meanwhile. The session then plans again from the list as it is, instead of overwriting it with its own
/// older copy.
///
/// Whole tracks are compared, not only their ids: an edit can put a song that is already queued back at the head with
/// newer details (its title or artwork), and that is an edit too.
fn queue_edited(state: &Status, published: &[OfflineTrack]) -> bool {
    state
        .lock()
        .map(|s| s.queue.as_slice() != published)
        .unwrap_or(false)
}
fn append(
    queue: &mut Vec<OfflineTrack>,
    tracks: impl IntoIterator<Item = OfflineTrack>,
    history: &VecDeque<String>,
) {
    for track in tracks {
        if queue.len() >= QUEUE_SIZE {
            break;
        }
        if eligible(&track.id, history)
            && !track.title.trim().is_empty()
            && track.title.len() <= 512
            && track.artists.len() <= 30
            && track.artists.iter().all(|a| a.len() <= 512)
            && !queue.iter().any(|old| old.id == track.id)
        {
            queue.push(track);
        }
    }
}
/// How much of the session's taste remains after each new song (0.8 keeps about five songs in view).
const PROFILE_KEEP: f32 = 0.8;
/// How many of the songs played just before this one also get their radio asked, to see which songs sit in the same neighbourhood.
const RECENT_RADIOS: usize = 2;
/// A song that came only from another song's radio is placed after this many of the playing song's own.
const OTHER_RADIO_RANK: usize = 30;
/// When the radios of the playing song and its predecessors give fewer songs than this, the best of them are asked too.
const ENOUGH_CANDIDATES: usize = 60;

/// What the ranking knows about the session.
pub struct RadioContext<'a> {
    pub profile: &'a Profile,
    pub vibe: Vibe,
    /// Ids of the songs played just before, newest first.
    pub recent: Vec<String>,
    pub current: OfflineTrack,
}

/// A radio's songs in order, each with whether it is a video (an upload, a lyric video) rather than an official audio track.
type RadioPage = Vec<(OfflineTrack, bool)>;

async fn radio(yt: &Yt, id: &str, timeout: Duration) -> Option<RadioPage> {
    let music = yt.music();
    let Ok(Ok(page)) = tokio::time::timeout(timeout, music.radio(id)).await else {
        return None;
    };
    Some(
        page.queue
            .into_iter()
            .map(|t| {
                let video = t.kind == innerlight::music::TrackKind::Video;
                let track = OfflineTrack {
                    provider: "YouTube".into(),
                    id: t.id,
                    title: t.title,
                    artists: t.artists.into_iter().map(|a| a.name).collect(),
                    artwork_url: t
                        .thumbnails
                        .into_iter()
                        .max_by_key(|c| c.width)
                        .and_then(|c| super::track_info::artwork_url(&c.url)),
                    duration_ms: t.duration_ms.and_then(|d| i64::try_from(d).ok()),
                };
                (track, video)
            })
            .collect(),
    )
}

/// Merges the radios into candidates: the playing song's radio gives the order, the others give votes.
fn candidates_from(
    primary: RadioPage,
    others: &[RadioPage],
    skip: impl Fn(&OfflineTrack) -> bool,
) -> Vec<Candidate> {
    let mut by_id: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out: Vec<Candidate> = Vec::new();
    for (rank, (track, video)) in primary.into_iter().enumerate() {
        if skip(&track) || by_id.contains_key(&track.id) {
            continue;
        }
        by_id.insert(track.id.clone(), out.len());
        out.push(Candidate {
            track,
            rank,
            votes: 1,
            video,
        });
    }
    for page in others {
        let mut seen_here = std::collections::HashSet::new();
        for (i, (track, video)) in page.iter().enumerate() {
            if skip(track) || !seen_here.insert(track.id.clone()) {
                continue;
            }
            match by_id.get(&track.id) {
                Some(&at) => out[at].votes += 1,
                None => {
                    by_id.insert(track.id.clone(), out.len());
                    out.push(Candidate {
                        track: track.clone(),
                        rank: OTHER_RADIO_RANK + i,
                        votes: 1,
                        video: *video,
                    });
                }
            }
        }
    }
    out
}

/// Adds related songs to the queue, in an order that keeps the session's mood. Returns what was queued when the source is
/// unavailable (nothing new is better than a wrong turn). The songs added come with the terms they scored on, for the
/// learning log.
///
/// The radio answer has no next-page handle. Instead of walking on from the last song queued (each step drifts a little
/// further), the playing song's radio and the radios of the songs just before it are read together, and a song that
/// several of them list counts for more. Only if that is too few, the radios of the best-fitting songs are read as well.
pub async fn related(
    yt: &Yt,
    seed: &str,
    history: &VecDeque<String>,
    mut queue: Vec<OfflineTrack>,
    ctx: &RadioContext<'_>,
) -> (Vec<OfflineTrack>, Vec<(String, Features)>) {
    queue.truncate(QUEUE_SIZE);
    let started = tokio::time::Instant::now();
    let recent: Vec<&String> = ctx
        .recent
        .iter()
        .filter(|id| valid_id(id) && id.as_str() != seed)
        .collect();
    let (primary, a, b) = tokio::join!(
        radio(yt, seed, Duration::from_secs(14)),
        async {
            match recent.first() {
                Some(id) => radio(yt, id, Duration::from_secs(14)).await,
                None => None,
            }
        },
        async {
            match recent.get(1) {
                Some(id) => radio(yt, id, Duration::from_secs(14)).await,
                None => None,
            }
        },
    );
    let Some(primary) = primary else {
        return (queue, Vec::new());
    };
    let mut others: Vec<RadioPage> = [a, b].into_iter().flatten().collect();
    let queued: std::collections::HashSet<String> = queue.iter().map(|t| t.id.clone()).collect();
    let skip =
        |t: &OfflineTrack| t.id == seed || queued.contains(&t.id) || !eligible(&t.id, history);
    let mut candidates = candidates_from(primary.clone(), &others, skip);
    let room = QUEUE_SIZE.saturating_sub(queue.len());
    if candidates.len() < ENOUGH_CANDIDATES.min(room) && started.elapsed() < Duration::from_secs(10)
    {
        // Too few: ask the radios of the songs that fit best so far, once, and vote again.
        let best = sequence(
            ctx.profile,
            ctx.vibe,
            candidates.clone(),
            1 + others.len(),
            Some(&ctx.current),
            2,
        );
        for track in best {
            if let Some(page) = radio(yt, &track.id, Duration::from_secs(10)).await {
                others.push(page);
            }
        }
        candidates = candidates_from(primary, &others, skip);
    }
    let radios = 1 + others.len();
    let scored: Vec<Candidate> = candidates.clone();
    let picked = sequence(
        ctx.profile,
        ctx.vibe,
        candidates,
        radios,
        queue.last().or(Some(&ctx.current)),
        room,
    );
    let terms: Vec<(String, Features)> = picked
        .iter()
        .filter_map(|t| {
            scored
                .iter()
                .find(|c| c.track.id == t.id)
                .map(|c| (t.id.clone(), features(ctx.profile, c, radios)))
        })
        .collect();
    append(&mut queue, picked, history);
    // A pick `append` turned away (a repeat, an unusable title) never plays, so it is not logged.
    let terms = terms
        .into_iter()
        .filter(|(id, _)| queue.iter().any(|t| t.id == *id))
        .collect();
    (queue, terms)
}
/// Appends the radio of a recently queued song to the up-next list, so the list keeps growing as the user scrolls.
/// The newest songs are asked together and the first answer with something new wins; uploads and lyric videos are
/// left out. Returns how many songs were added: 0 when the list is empty or full, or the service gave nothing new.
pub async fn load_more(state: &Status, yt: &Yt) -> usize {
    let (seeds, history, known, generation) = {
        let Ok(s) = state.lock() else {
            return 0;
        };
        if s.queue.is_empty() || s.queue.len() >= QUEUE_MAX {
            return 0;
        }
        (
            s.queue
                .iter()
                .rev()
                .take(3)
                .map(|t| t.id.clone())
                .collect::<Vec<_>>(),
            s.history.clone(),
            s.queue.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            s.generation,
        )
    };
    let mut asking = tokio::task::JoinSet::new();
    for seed in seeds {
        let yt = yt.clone();
        asking.spawn(async move { radio(&yt, &seed, Duration::from_secs(8)).await });
    }
    let mut fresh = Vec::new();
    while let Some(done) = asking.join_next().await {
        let Ok(Some(page)) = done else {
            continue;
        };
        append(
            &mut fresh,
            page.into_iter()
                .filter(|(_, video)| !video)
                .map(|(track, _)| track)
                .filter(|t| !known.contains(&t.id)),
            &history,
        );
        if !fresh.is_empty() {
            break;
        }
    }
    let Ok(mut s) = state.lock() else {
        return 0;
    };
    // A manual load replaced the queue while the radios were being asked: these songs belong to the old one.
    if s.generation != generation {
        return 0;
    }
    let before = s.queue.len();
    for track in fresh {
        if s.queue.len() >= QUEUE_MAX {
            break;
        }
        if !s.queue.iter().any(|old| old.id == track.id) {
            s.queue.push(track);
        }
    }
    s.queue.len() - before
}
fn reference(next: &OfflineTrack) -> Result<TrackReference, ()> {
    let mut track = TrackReference::new(
        ProviderId::YouTube,
        next.id.clone(),
        format!("https://www.youtube.com/watch?v={}", next.id),
        next.title.clone(),
    )
    .map_err(|_| ())?;
    track.artists = next.artists.clone();
    track.artwork_url = next.artwork_url.clone();
    Ok(track)
}
pub fn select(state: &Status, id: &str) -> Option<TrackReference> {
    let mut state = state.lock().ok()?;
    let index = state.queue.iter().position(|t| t.id == id)?;
    let track = reference(&state.queue[index]).ok()?;
    state.queue.drain(..=index);
    state.next_title = None;
    state.state = "finding";
    Some(track)
}
/// What [`advance`] did.
#[derive(Debug, PartialEq, Eq)]
enum Advance {
    /// The track is still playing (or paused): nothing to do yet.
    Waiting,
    /// The next track is loaded and playing.
    Played,
    /// The next track is loaded but would not start. It is the active one now; being errored it is over like any
    /// other track, so the session goes on from it, at the pace of a failure.
    Stalled,
    /// The listener stopped, replaced or was blocked on the track: this session has no business going on.
    Gone,
    /// The track ended (or failed) and the next one could not be loaded.
    Failed,
}

/// Moves to `next` once `seed` is over: it ended, or it failed to play. Anything else is left alone, and a listener's
/// own stop or change ends the session.
async fn advance(player: &mut PlaybackEngine, seed: &str, next: &OfflineTrack) -> Advance {
    let state = player.state();
    if player
        .active_track()
        .is_none_or(|t| t.provider_track_id != seed)
        || matches!(state, PlaybackState::Idle | PlaybackState::Blocked)
    {
        return Advance::Gone;
    }
    // A track that fails mid-way is as over as one that ended: the music goes on instead of stopping dead.
    if !matches!(state, PlaybackState::Ended | PlaybackState::Error) {
        return Advance::Waiting;
    }
    let Ok(track) = reference(next) else {
        return Advance::Failed;
    };
    if player.dispatch(PlaybackCommand::Load(track)).await.is_err() {
        return Advance::Failed;
    }
    if player.dispatch(PlaybackCommand::Play).await.is_err() {
        return Advance::Stalled;
    }
    Advance::Played
}
pub async fn run(
    engine: Arc<Mutex<PlaybackEngine>>,
    yt: Yt,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    taste: Taste,
    reports: watch::Receiver<u64>,
    state: Status,
) {
    // A session ends only when the listener's choice ends it (stop, a new track, autoplay off with the list used up).
    // Failures that can pass never end it, so there is no "unavailable" outcome any more.
    let _ended = session(engine, yt, cache, audio, taste, reports, state.clone()).await;
    status(&state, "idle", None);
}
#[cfg(test)]
mod tests {
    use super::super::host::{HostFuture, HostOutcome, HostRequest, PlayerEvent, PlayerHost};
    use super::*;
    use crate::core::{
        capabilities::{CapabilityStatus, ProviderCapabilities},
        registry::ProviderRegistry,
    };
    struct Host;
    impl PlayerHost for Host {
        fn provider(&self) -> ProviderId {
            ProviderId::YouTube
        }
        fn prepare<'a>(&'a self, r: HostRequest) -> HostFuture<'a> {
            Box::pin(async move {
                HostOutcome::Confirmed {
                    request_id: r.request_id,
                    revision: r.revision,
                }
            })
        }
    }
    fn engine() -> PlaybackEngine {
        engine_with(Host)
    }
    fn engine_with(host: impl PlayerHost + 'static) -> PlaybackEngine {
        let mut registry = ProviderRegistry::new();
        registry
            .register(
                ProviderId::YouTube,
                ProviderCapabilities {
                    playback: CapabilityStatus::Supported,
                    ..ProviderCapabilities::unverified()
                },
            )
            .unwrap();
        PlaybackEngine::new(
            registry,
            std::collections::HashMap::from([(
                ProviderId::YouTube,
                Box::new(host) as Box<dyn PlayerHost>,
            )]),
            Duration::from_secs(1),
        )
    }
    fn track(id: &str) -> OfflineTrack {
        OfflineTrack {
            provider: "YouTube".into(),
            id: id.into(),
            title: id.into(),
            artists: vec![],
            artwork_url: None,
            duration_ms: None,
        }
    }
    #[test]
    fn mode_is_validated_and_survives_a_manual_load() {
        let state: Status = Default::default();
        assert!(!set_mode(&state, "forever", true, None));
        assert!(set_mode(&state, "all", false, None));
        state.lock().unwrap().reset();
        let s = state.lock().unwrap();
        assert_eq!((s.repeat, s.radio), ("all", false));
    }
    #[test]
    fn repeat_all_restarts_the_remembered_list_only_when_asked() {
        let state: Status = Default::default();
        let first = reference(&track("aqz-KE-bpKQ")).unwrap();
        set_context(&state, &first, &[track("r-G2pKr2_n8")]);
        assert!(restart_context(&state).is_none(), "repeat is off");
        assert!(set_mode(&state, "all", true, None));
        let again = restart_context(&state).unwrap();
        assert_eq!(again.provider_track_id, "aqz-KE-bpKQ");
        let queue = state.lock().unwrap().queue.clone();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].id, "r-G2pKr2_n8");
    }
    #[test]
    fn a_manual_load_starts_a_new_queue_generation() {
        let state: Status = Default::default();
        let before = state.lock().unwrap().generation;
        state.lock().unwrap().reset();
        assert_ne!(state.lock().unwrap().generation, before);
    }
    #[test]
    fn the_up_next_list_can_be_edited_by_hand() {
        let state: Status = Default::default();
        state.lock().unwrap().queue = vec![
            track("aaaaaaaaaaa"),
            track("bbbbbbbbbbb"),
            track("ccccccccccc"),
        ];
        let ids = |s: &Status| {
            s.lock()
                .unwrap()
                .queue
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
        };
        assert!(edit_queue(&state, QueueEdit::Next(track("ccccccccccc"))));
        assert_eq!(ids(&state), ["ccccccccccc", "aaaaaaaaaaa", "bbbbbbbbbbb"]);
        assert!(edit_queue(&state, QueueEdit::Move { from: 2, to: 0 }));
        assert_eq!(ids(&state), ["bbbbbbbbbbb", "ccccccccccc", "aaaaaaaaaaa"]);
        assert!(!edit_queue(&state, QueueEdit::Move { from: 0, to: 3 }));
        assert!(edit_queue(&state, QueueEdit::Remove("ccccccccccc".into())));
        assert!(!edit_queue(&state, QueueEdit::Remove("zzzzzzzzzzz".into())));
        assert_eq!(ids(&state), ["bbbbbbbbbbb", "aaaaaaaaaaa"]);
    }
    fn next() -> OfflineTrack {
        OfflineTrack {
            provider: "YouTube".into(),
            id: "aqz-KE-bpKQ".into(),
            title: "Next".into(),
            artists: vec![],
            artwork_url: None,
            duration_ms: None,
        }
    }
    async fn playing() -> PlaybackEngine {
        let mut player = engine();
        let track = TrackReference::new(
            ProviderId::YouTube,
            "r-G2pKr2_n8".into(),
            "https://www.youtube.com/watch?v=r-G2pKr2_n8".into(),
            "Seed".into(),
        )
        .unwrap();
        player.dispatch(PlaybackCommand::Load(track)).await.unwrap();
        player.dispatch(PlaybackCommand::Play).await.unwrap();
        player
    }
    #[tokio::test]
    async fn advances_only_after_confirmed_eof_and_once() {
        let mut player = playing().await;
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Waiting
        );
        player.dispatch(PlaybackCommand::Pause).await.unwrap();
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Waiting
        );
        player.dispatch(PlaybackCommand::Resume).await.unwrap();
        player
            .observe(player.revision(), PlayerEvent::Ended)
            .unwrap();
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Played
        );
        assert_eq!(player.state(), PlaybackState::Playing);
        assert_eq!(
            player.active_track().unwrap().provider_track_id,
            "aqz-KE-bpKQ"
        );
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Gone
        );
    }
    #[tokio::test]
    async fn stopped_and_stale_sessions_cannot_restart_playback() {
        let mut player = playing().await;
        assert_eq!(
            advance(&mut player, "other-seed", &next()).await,
            Advance::Gone
        );
        player.dispatch(PlaybackCommand::Stop).await.unwrap();
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Gone
        );
    }
    #[tokio::test]
    async fn a_track_that_fails_midway_is_over_and_the_music_goes_on() {
        let mut player = playing().await;
        player
            .observe(player.revision(), PlayerEvent::Failed)
            .unwrap();
        assert_eq!(player.state(), PlaybackState::Error);
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Played
        );
        assert_eq!(player.state(), PlaybackState::Playing);
    }
    /// Refuses to load one track, and to start another, the way a host does when a stream is refused.
    struct Picky {
        unloadable: &'static str,
        unstartable: bool,
        loaded: std::sync::Mutex<String>,
    }
    impl PlayerHost for Picky {
        fn provider(&self) -> ProviderId {
            ProviderId::YouTube
        }
        fn prepare<'a>(&'a self, r: HostRequest) -> HostFuture<'a> {
            Box::pin(async move {
                let refuse = match &r.command {
                    PlaybackCommand::Load(t) => {
                        *self.loaded.lock().unwrap() = t.provider_track_id.clone();
                        t.provider_track_id == self.unloadable
                    }
                    PlaybackCommand::Play => {
                        self.unstartable && *self.loaded.lock().unwrap() == "aqz-KE-bpKQ"
                    }
                    _ => false,
                };
                if refuse {
                    HostOutcome::Rejected {
                        request_id: r.request_id,
                        revision: r.revision,
                        kind: crate::core::errors::PlaybackErrorKind::HostRejected(
                            crate::core::errors::HostRejectKind::Failed,
                        ),
                    }
                } else {
                    HostOutcome::Confirmed {
                        request_id: r.request_id,
                        revision: r.revision,
                    }
                }
            })
        }
    }
    fn picky(unloadable: &'static str, unstartable: bool) -> Picky {
        Picky {
            unloadable,
            unstartable,
            loaded: std::sync::Mutex::new(String::new()),
        }
    }
    async fn ended_with(host: Picky) -> PlaybackEngine {
        let mut player = engine_with(host);
        let seed = TrackReference::new(
            ProviderId::YouTube,
            "r-G2pKr2_n8".into(),
            "https://www.youtube.com/watch?v=r-G2pKr2_n8".into(),
            "Seed".into(),
        )
        .unwrap();
        player.dispatch(PlaybackCommand::Load(seed)).await.unwrap();
        player.dispatch(PlaybackCommand::Play).await.unwrap();
        player
            .observe(player.revision(), PlayerEvent::Ended)
            .unwrap();
        player
    }
    #[tokio::test]
    async fn a_next_track_that_cannot_load_fails_and_another_can_still_follow() {
        let mut player = ended_with(picky("aqz-KE-bpKQ", false)).await;
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Failed
        );
        // The failed load left the player in Error with the seed still active: the session tries the next candidate.
        assert_eq!(player.state(), PlaybackState::Error);
        let other = OfflineTrack {
            id: "abcdefghijk".into(),
            ..next()
        };
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &other).await,
            Advance::Played
        );
        assert_eq!(player.state(), PlaybackState::Playing);
    }
    #[tokio::test]
    async fn a_track_that_loads_but_will_not_start_is_reported_as_stalled() {
        let mut player = ended_with(picky("none", true)).await;
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Advance::Stalled
        );
        assert_eq!(player.state(), PlaybackState::Error);
        assert_eq!(
            player.active_track().unwrap().provider_track_id,
            "aqz-KE-bpKQ"
        );
        // From there the session carries on with the next candidate.
        let other = OfflineTrack {
            id: "abcdefghijk".into(),
            ..next()
        };
        assert_eq!(
            advance(&mut player, "aqz-KE-bpKQ", &other).await,
            Advance::Played
        );
    }
    #[tokio::test]
    async fn a_retry_wait_ends_at_once_when_the_plan_changes() {
        let engine = Mutex::new(playing().await);
        let started = Instant::now();
        retry_pause(&engine, 6, || true).await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }
    #[tokio::test]
    async fn a_retry_wait_is_cut_short_when_the_song_ends_during_it() {
        let engine = Mutex::new(playing().await);
        let ending = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let mut player = engine.lock().await;
            let revision = player.revision();
            player.observe(revision, PlayerEvent::Ended).unwrap();
        };
        let started = Instant::now();
        // A 30 s wait that would otherwise leave silence after the song ends.
        tokio::join!(retry_pause(&engine, 6, || false), ending);
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(600), "{waited:?}");
        assert!(waited < Duration::from_secs(3), "{waited:?}");
    }
    #[tokio::test]
    async fn a_retry_wait_keeps_its_pacing_while_the_song_plays() {
        let engine = Mutex::new(playing().await);
        let started = Instant::now();
        retry_pause(&engine, 1, || false).await;
        assert!(started.elapsed() >= Duration::from_millis(950));
    }
    #[test]
    fn an_edit_of_the_up_next_list_is_told_from_what_the_session_published() {
        let state: Status = Default::default();
        let mut published = Vec::new();
        let queue = vec![track("aaaaaaaaaaa"), track("bbbbbbbbbbb")];
        publish(&state, &queue, &mut published);
        assert!(!queue_edited(&state, &published));
        assert!(edit_queue(&state, QueueEdit::Next(track("ccccccccccc"))));
        assert!(queue_edited(&state, &published));
        // Planning again publishes the list as it is, and that is the new baseline.
        let now = state.lock().unwrap().queue.clone();
        publish(&state, &now, &mut published);
        assert!(!queue_edited(&state, &published));
        assert!(edit_queue(&state, QueueEdit::Remove("ccccccccccc".into())));
        assert!(queue_edited(&state, &published));
    }
    #[test]
    fn a_song_put_back_at_the_head_with_new_details_counts_as_an_edit() {
        let state: Status = Default::default();
        let mut published = Vec::new();
        publish(
            &state,
            &[track("aaaaaaaaaaa"), track("bbbbbbbbbbb")],
            &mut published,
        );
        let renamed = OfflineTrack {
            title: "Renamed".into(),
            ..track("aaaaaaaaaaa")
        };
        // Same id, same place, new details: the ids alone would not show it.
        assert!(edit_queue(&state, QueueEdit::Next(renamed)));
        assert!(queue_edited(&state, &published));
    }
    #[test]
    fn retries_slow_down_and_stop_at_the_cap() {
        let waits: Vec<u64> = (1..=8).map(|n| backoff(n).as_secs()).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff(0).as_secs(), 1);
        assert_eq!(backoff(u32::MAX).as_secs(), 30);
    }
    #[test]
    fn queue_is_unique_bounded_and_excludes_history() {
        let history = VecDeque::from(["00000000000".into()]);
        let tracks = (0..100).flat_map(|i| {
            let mut t = next();
            t.id = format!("{i:011}");
            [t.clone(), t]
        });
        let mut queue = vec![];
        append(&mut queue, tracks, &history);
        assert_eq!(queue.len(), 50);
        assert_eq!(queue[0].id, "00000000001");
        assert_eq!(
            queue
                .iter()
                .map(|t| &t.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            50
        );
    }
    #[test]
    fn selecting_a_queue_song_keeps_the_following_order() {
        let state = Status::default();
        let mut queue = vec![];
        for i in 0..5 {
            let mut t = next();
            t.id = format!("{i:011}");
            queue.push(t);
        }
        publish_queue(&state, &queue);
        status(&state, "ready", Some("Next".into()));
        assert_eq!(state.lock().unwrap().queue.len(), 5);
        assert!(select(&state, "not-in-queue").is_none());
        assert_eq!(state.lock().unwrap().queue.len(), 5);
        let selected = select(&state, "00000000002").unwrap();
        assert_eq!(selected.provider_track_id, "00000000002");
        let queue = &state.lock().unwrap().queue;
        assert_eq!(
            queue.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["00000000003", "00000000004"]
        );
    }
    #[test]
    fn excludes_recent_tracks_and_bounds_history() {
        let mut history = VecDeque::new();
        remember(&mut history, "r-G2pKr2_n8".into());
        assert!(!eligible("r-G2pKr2_n8", &history));
        assert!(!eligible("invalid", &history));
        assert!(eligible("aqz-KE-bpKQ", &history));
        for i in 0..120 {
            remember(&mut history, format!("{i:011}"));
        }
        assert_eq!(history.len(), 100);
        assert!(eligible("r-G2pKr2_n8", &history));
    }
    #[test]
    fn candidates_keep_the_playing_songs_order_and_count_votes() {
        let page = |ids: &[&str]| -> RadioPage {
            ids.iter()
                .map(|id| {
                    (
                        {
                            let mut t = next();
                            t.id = format!("{id:0>11}");
                            t
                        },
                        false,
                    )
                })
                .collect()
        };
        let id = |short: &str| format!("{short:0>11}");
        let primary = page(&["a", "b", "c", "seed"]);
        let others = [page(&["c", "d"]), page(&["c", "a", "c"])];
        let skip = |t: &OfflineTrack| t.id == id("seed");
        let found = candidates_from(primary, &others, skip);
        let by = |short: &str| found.iter().find(|c| c.track.id == id(short)).unwrap();
        assert_eq!(found.len(), 4, "the playing song itself is left out");
        assert_eq!((by("a").rank, by("a").votes), (0, 2));
        assert_eq!((by("b").rank, by("b").votes), (1, 1));
        assert_eq!(
            (by("c").rank, by("c").votes),
            (2, 3),
            "listed twice in one radio still counts once"
        );
        assert_eq!(
            (by("d").rank, by("d").votes),
            (OTHER_RADIO_RANK + 1, 1),
            "only seen in another radio: long tail"
        );
    }
    #[test]
    fn the_vibe_is_validated_and_kept_across_a_manual_load() {
        let state = Status::default();
        assert_eq!(state.lock().unwrap().vibe, "balanced");
        assert!(
            !set_mode(&state, "off", true, Some("wild")),
            "unknown vibe is refused"
        );
        assert_eq!(state.lock().unwrap().vibe, "balanced");
        assert!(set_mode(&state, "off", true, Some("same")));
        assert!(
            set_mode(&state, "all", true, None),
            "no vibe leaves it as it is"
        );
        assert_eq!(state.lock().unwrap().vibe, "same");
        state.lock().unwrap().reset();
        assert_eq!(
            state.lock().unwrap().vibe,
            "same",
            "a manual load keeps the user's choice"
        );
    }
}

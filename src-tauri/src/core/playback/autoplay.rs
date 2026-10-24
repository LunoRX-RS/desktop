//! One bounded radio session. The host's confirmed EOF is the only advance trigger.
use super::{
    audio::AudioControl,
    cache::{DiskCache, OfflineTrack},
    command::PlaybackCommand,
    engine::PlaybackEngine,
    extract::valid_id,
    state::PlaybackState,
    taste::{Candidate, Profile, Vibe, sequence},
};
use crate::core::{
    domain::{ProviderId, TrackReference},
    yt::Yt,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};
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
async fn session(
    engine: Arc<Mutex<PlaybackEngine>>,
    yt: Yt,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    mut reports: watch::Receiver<u64>,
    state: Status,
) {
    let mut history = state.lock().map(|s| s.history.clone()).unwrap_or_default();
    let mut profile = Profile::default();
    let mut observed: Option<String> = None;
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
        let Some(current) = current else { return };
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
        let (list, context) = state
            .lock()
            .map(|s| (s.list, s.context.clone()))
            .unwrap_or_default();
        // A new vibe applies to the songs after this one: the related part of the queue is asked for again. A list the user
        // started (a playlist or album) is left as it is.
        let vibe = vibe_of(&state);
        if planned_vibe.is_some_and(|v| v != vibe) && !list {
            queue.clear();
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
            let ctx = RadioContext {
                profile: &profile,
                vibe: Vibe::parse(vibe).unwrap_or(Vibe::Balanced),
                recent,
                current: offline_of(&current),
            };
            queue = related(&yt, &seed, &history, queue, &ctx).await;
        }
        if queue.is_empty() && repeat == "all" && !context.is_empty() {
            queue = context.iter().skip(1).cloned().collect();
            queue.insert(0, context[0].clone());
        }
        publish_queue(&state, &queue);
        let repeat_one = repeat == "one";
        if repeat_one {
            // The same track again: it is already cached, so this only keeps the file alive until Load takes it.
            queue = vec![offline_of(&current)];
        }
        let mut next = None;
        for _ in 0..3 {
            let Some(track) = queue.first().cloned() else {
                break;
            };
            status(&state, "preparing", Some(track.title.clone()));
            // Keep the file alive until Load acquires it, including when cache quota is tight.
            if let Ok(Ok(file)) = tokio::time::timeout(
                Duration::from_secs(90),
                cache.ensure(&yt, track.clone(), audio.settings().quality),
            )
            .await
            {
                next = Some((track, file));
                break;
            }
            queue.remove(0);
            publish_queue(&state, &queue);
        }
        let Some((next, _file)) = next else { return };
        status(&state, "ready", Some(next.title.clone()));
        // Best effort, after readiness is published; never awaited.
        super::track_info::prefetch_lyrics(&next.id, &next.title, &next.artists, next.duration_ms);
        loop {
            reports.borrow_and_update();
            // The user changed repeat or autoplay while this track plays: plan the ending again.
            if mode(&state) != (repeat, radio) || vibe_of(&state) != vibe {
                continue 'outer;
            }
            let mut player = engine.lock().await;
            match advance(&mut player, &seed, &next).await {
                Ok(true) => {
                    push_back(&state, current.clone());
                    if !repeat_one {
                        queue.remove(0);
                    }
                    publish_queue(&state, &queue);
                    break;
                }
                Ok(false) => {}
                Err(()) => return,
            }
            drop(player);
            if reports.changed().await.is_err() {
                return;
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
/// unavailable (nothing new is better than a wrong turn).
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
) -> Vec<OfflineTrack> {
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
    let Some(primary) = primary else { return queue };
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
    let picked = sequence(
        ctx.profile,
        ctx.vibe,
        candidates,
        1 + others.len(),
        queue.last().or(Some(&ctx.current)),
        room,
    );
    append(&mut queue, picked, history);
    queue
}
/// Appends the radio of a recently queued song to the up-next list, so the list keeps growing as the user scrolls.
/// The newest songs are asked together and the first answer with something new wins; uploads and lyric videos are
/// left out. Returns how many songs were added: 0 when the list is empty or full, or the service gave nothing new.
pub async fn load_more(state: &Status, yt: &Yt) -> usize {
    let (seeds, history, known) = {
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
async fn advance(player: &mut PlaybackEngine, seed: &str, next: &OfflineTrack) -> Result<bool, ()> {
    if player
        .active_track()
        .is_none_or(|t| t.provider_track_id != seed)
        || matches!(
            player.state(),
            PlaybackState::Idle | PlaybackState::Error | PlaybackState::Blocked
        )
    {
        return Err(());
    }
    if player.state() != PlaybackState::Ended {
        return Ok(false);
    }
    let track = reference(next)?;
    player
        .dispatch(PlaybackCommand::Load(track))
        .await
        .map_err(|_| ())?;
    player
        .dispatch(PlaybackCommand::Play)
        .await
        .map_err(|_| ())?;
    Ok(true)
}
pub async fn run(
    engine: Arc<Mutex<PlaybackEngine>>,
    yt: Yt,
    cache: Arc<DiskCache>,
    audio: AudioControl,
    reports: watch::Receiver<u64>,
    state: Status,
) {
    session(engine, yt, cache, audio, reports, state.clone()).await;
    let radio = state.lock().map(|s| s.radio).unwrap_or(true);
    // With autoplay off, running out of tracks is the normal end, not a failure.
    status(&state, if radio { "unavailable" } else { "idle" }, None);
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
                Box::new(Host) as Box<dyn PlayerHost>,
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
            Ok(false)
        );
        player.dispatch(PlaybackCommand::Pause).await.unwrap();
        assert_eq!(
            advance(&mut player, "r-G2pKr2_n8", &next()).await,
            Ok(false)
        );
        player.dispatch(PlaybackCommand::Resume).await.unwrap();
        player
            .observe(player.revision(), PlayerEvent::Ended)
            .unwrap();
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Ok(true));
        assert_eq!(player.state(), PlaybackState::Playing);
        assert_eq!(
            player.active_track().unwrap().provider_track_id,
            "aqz-KE-bpKQ"
        );
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Err(()));
    }
    #[tokio::test]
    async fn stopped_and_stale_sessions_cannot_restart_playback() {
        let mut player = playing().await;
        assert_eq!(advance(&mut player, "other-seed", &next()).await, Err(()));
        player.dispatch(PlaybackCommand::Stop).await.unwrap();
        assert_eq!(advance(&mut player, "r-G2pKr2_n8", &next()).await, Err(()));
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

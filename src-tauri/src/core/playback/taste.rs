//! Keeps autoplay in one mood. YouTube Music's radio is a good list of songs near the one that plays, but it wanders: later pages
//! are broader, covers and long compilations come with it, and starting each new batch from the last song queued makes the
//! mood slide a little further every time (sad, then middling, then a party song). This module scores every candidate against a
//! profile of what has been playing this session and puts them in an order that holds the mood, the language and the genre.
//!
//! It is pure: no network and no clock, so every rule here is covered by a unit test. The signals, from strongest to weakest:
//! votes (the song shows up in the radios of several recent songs, so it sits in the same neighbourhood), the artists and mood
//! words of the songs that played, the script of the title (Thai, Latin, Japanese ...), and finally YouTube's own order.

use super::cache::OfflineTrack;
use std::collections::HashMap;

/// How closely autoplay sticks to what is playing. Chosen by the user.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vibe {
    /// Same mood, language and genre; songs that break it are left out.
    Same,
    /// Mostly the same, with room for a surprise. The default.
    Balanced,
    /// YouTube's own radio order, only cleaned of covers and mixes.
    Explore,
}

impl Vibe {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "same" => Some(Self::Same),
            "balanced" => Some(Self::Balanced),
            "explore" => Some(Self::Explore),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Balanced => "balanced",
            Self::Explore => "explore",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Same => 0,
            Self::Balanced => 1,
            Self::Explore => 2,
        }
    }
    /// The built-in weights of the learnable terms, where fitting starts from.
    pub fn prior(self) -> Features {
        self.weights().to_array()
    }
    fn weights(self) -> Weights {
        match self {
            Self::Same => Weights {
                rank: 0.8,
                votes: 0.9,
                mood: 2.0,
                folk: 1.6,
                script: 1.4,
                artist: 0.9,
                repeat_artist: 0.8,
            },
            Self::Balanced => Weights {
                rank: 1.3,
                votes: 0.6,
                mood: 1.2,
                folk: 1.0,
                script: 0.8,
                artist: 0.5,
                repeat_artist: 0.8,
            },
            Self::Explore => Weights {
                rank: 2.0,
                votes: 0.2,
                mood: 0.2,
                folk: 0.2,
                script: 0.2,
                artist: 0.1,
                repeat_artist: 0.5,
            },
        }
    }
}

/// The terms the score weighs, in the order of `Weights::to_array`. What the learning in `learn.rs` fits.
pub const FEATURES: usize = 6;
pub type Features = [f32; FEATURES];

#[derive(Clone, Copy)]
struct Weights {
    rank: f32,
    votes: f32,
    mood: f32,
    folk: f32,
    script: f32,
    artist: f32,
    repeat_artist: f32,
}

impl Weights {
    fn to_array(self) -> Features {
        [
            self.rank,
            self.votes,
            self.mood,
            self.folk,
            self.script,
            self.artist,
        ]
    }
    /// The weights a fit gives for `vibe`; `repeat_artist` is not learned and keeps its built-in value.
    fn learned(vibe: Vibe, x: Features) -> Self {
        Self {
            rank: x[0],
            votes: x[1],
            mood: x[2],
            folk: x[3],
            script: x[4],
            artist: x[5],
            repeat_artist: vibe.weights().repeat_artist,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Script {
    Thai,
    Latin,
    Japanese,
    Korean,
    Chinese,
    Other,
}
const SCRIPTS: usize = 6;

impl Script {
    fn index(self) -> usize {
        self as usize
    }
}

/// The script most of the letters in `text` are written in.
pub fn script_of(text: &str) -> Script {
    let mut counts = [0usize; SCRIPTS];
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        let script = match c as u32 {
            0x0E00..=0x0E7F => Script::Thai,
            0x3040..=0x30FF => Script::Japanese,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => Script::Korean,
            0x4E00..=0x9FFF => Script::Chinese,
            _ if c.is_ascii_alphabetic() || ('\u{00C0}'..='\u{024F}').contains(&c) => Script::Latin,
            _ => Script::Other,
        };
        counts[script.index()] += 1;
    }
    // Kanji next to kana is Japanese, not Chinese.
    if counts[Script::Japanese.index()] > 0 {
        counts[Script::Japanese.index()] += counts[Script::Chinese.index()];
        counts[Script::Chinese.index()] = 0;
    }
    let all = [
        Script::Thai,
        Script::Latin,
        Script::Japanese,
        Script::Korean,
        Script::Chinese,
        Script::Other,
    ];
    all.into_iter()
        .max_by_key(|s| counts[s.index()])
        .filter(|s| counts[s.index()] > 0)
        .unwrap_or(Script::Other)
}

/// The language of a song: its title's script. Artist names are often Latin even for a Thai song ("Saran"), so they only count
/// when the title has no letters at all.
pub fn song_script(title: &str, artists: &[String]) -> Script {
    match script_of(title) {
        Script::Other => script_of(&artists.join(" ")),
        s => s,
    }
}

/// What a title says about the song. The mood tags exclude each other; `Folk` is a genre (luk thung, mor lam, Isan).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Sad,
    Party,
    Chill,
    Folk,
    Love,
}
const TAGS: [Tag; 5] = [Tag::Sad, Tag::Party, Tag::Chill, Tag::Folk, Tag::Love];

impl Tag {
    fn index(self) -> usize {
        self as usize
    }
    fn is_mood(self) -> bool {
        matches!(self, Tag::Sad | Tag::Party | Tag::Chill)
    }
    /// Words that mark it, matched inside the lower-cased title and artist names.
    fn words(self) -> &'static [&'static str] {
        match self {
            Tag::Sad => &[
                "เสียใจ",
                "เศร้า",
                "น้ำตา",
                "ร้องไห้",
                "เจ็บ",
                "คิดถึง",
                "เหงา",
                "ลืม",
                "ขอโทษ",
                "อกหัก",
                "จากลา",
                "ขอลา",
                "ลาก่อน",
                "จากกัน",
                "เลิก",
                "ทิ้ง",
                "หมดแรง",
                "ไม่ไหว",
                "ตัดใจ",
                "ขาดใจ",
                "ร้าว",
                "ใจพัง",
                "ส่งท้าย",
                "หายไป",
                "ไม่เหลือ",
                "สิ้นสุด",
                "เดินมาส่ง",
                "ปล่อยมือ",
                "sad",
                "cry",
                "tears",
                "broken",
                "goodbye",
                "alone",
                "lonely",
                "miss you",
                "hurt",
                "heartbreak",
                "pain",
                "without you",
                "lost",
                "涙",
                "悲し",
                "さよなら",
                "切な",
                "슬",
                "눈물",
            ],
            Tag::Party => &[
                "สนุก",
                "เต้น",
                "ปาร์ตี้",
                "แดนซ์",
                "ซิ่ง",
                "เมามัน",
                "ฟินเวอร์",
                "party",
                "dance",
                "club",
                "turn up",
                "banger",
                "ไนท์",
                "ควบ",
            ],
            Tag::Chill => &[
                "chill",
                "acoustic",
                "lofi",
                "lo-fi",
                "piano",
                "ambient",
                "sleep",
                "relax",
                "คลอ",
                "ผ่อนคลาย",
            ],
            Tag::Folk => &[
                "ลูกทุ่ง",
                "หมอลำ",
                "ลำซิ่ง",
                "ลำเพลิน",
                "อีสาน",
                "ลายพิณ",
                "โปงลาง",
                "ฮัก",
                "คึดฮอด",
                "เด้อ",
                "อ้าย",
                "ผู้สาว",
                "ผู้บ่าว",
                "สาวนา",
                "ม่วน",
                "ฮอย",
                "แพ้ฮัก",
                "เอื้อย",
                "อีหลี",
                "บักหล่า",
                "ลำ ",
            ],
            Tag::Love => &["รัก", "หัวใจ", "กอด", "love", "heart", "ชอบ", "คู่"],
        }
    }
}

fn contains_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| text.contains(w))
}

/// The tags a title and its artists carry.
pub fn tags_of(title: &str, artists: &[String]) -> [bool; 5] {
    let text = format!(
        "{} {}",
        title.to_lowercase(),
        artists.join(" ").to_lowercase()
    );
    let mut out = [false; 5];
    for tag in TAGS {
        out[tag.index()] = contains_any(&text, tag.words());
    }
    // A song that says both sad and party words is a sad song with a beat, not a party song.
    if out[Tag::Sad.index()] {
        out[Tag::Party.index()] = false;
    }
    out
}

const VARIANT_WORDS: [&str; 20] = [
    "cover",
    "คัฟเวอร์",
    "remix",
    "live",
    "karaoke",
    "คาราโอเกะ",
    "instrumental",
    "nightcore",
    "slowed",
    "sped up",
    "8d",
    "lyric video",
    "lyrics version",
    "cover version",
    "reaction",
    "tutorial",
    "ver.",
    "session",
    "acoustic ver",
    "รีมิกซ์",
];
const MIX_WORDS: [&str; 10] = [
    "รวมเพลง",
    "รวมฮิต",
    "รวมกลอนลำ",
    "non-stop",
    "nonstop",
    "1 hour",
    "playlist",
    "mix 20",
    "megamix",
    "full album",
];
/// Longer than this is a compilation, not a song.
const LONGEST_SONG_MS: i64 = 12 * 60 * 1000;

fn is_variant(title: &str) -> bool {
    let t = title.to_lowercase();
    contains_any(&t, &VARIANT_WORDS)
}

fn is_mix(title: &str, duration_ms: Option<i64>) -> bool {
    let t = title.to_lowercase();
    contains_any(&t, &MIX_WORDS) || duration_ms.is_some_and(|d| d > LONGEST_SONG_MS)
}

/// What has been playing this session: how much of each tag, script and artist, with recent songs counting more.
#[derive(Clone, Default, Debug)]
pub struct Profile {
    tags: [f32; 5],
    scripts: [f32; SCRIPTS],
    artists: HashMap<String, f32>,
    variants: f32,
    total: f32,
    /// Fitted weights per vibe (see `Vibe::index`); `None` keeps the built-in ones for that vibe.
    learned: [Option<Features>; 3],
}

impl Profile {
    /// Replaces the fitted weights. Taken from the listening log, so the same songs always score the same way.
    pub fn learn(&mut self, learned: [Option<Features>; 3]) {
        self.learned = learned;
    }

    fn weights(&self, vibe: Vibe) -> Weights {
        match self.learned[vibe.index()] {
            Some(x) => Weights::learned(vibe, x),
            None => vibe.weights(),
        }
    }

    /// Older songs fade by `keep` (0..1) before a new one is added, so a change of taste takes over in a few songs.
    pub fn fade(&mut self, keep: f32) {
        for v in self.tags.iter_mut().chain(self.scripts.iter_mut()) {
            *v *= keep;
        }
        self.variants *= keep;
        self.total *= keep;
        self.artists.retain(|_, v| {
            *v *= keep;
            *v > 0.01
        });
    }

    pub fn observe(&mut self, track: &OfflineTrack, weight: f32) {
        let tags = tags_of(&track.title, &track.artists);
        for tag in TAGS {
            if tags[tag.index()] {
                self.tags[tag.index()] += weight;
            }
        }
        self.scripts[song_script(&track.title, &track.artists).index()] += weight;
        for artist in &track.artists {
            *self.artists.entry(artist.to_lowercase()).or_default() += weight;
        }
        if is_variant(&track.title) {
            self.variants += weight;
        }
        self.total += weight;
    }

    pub fn is_empty(&self) -> bool {
        self.total <= 0.0
    }

    /// 0..1: the share of recent songs that carry the tag.
    fn tag_share(&self, tag: Tag) -> f32 {
        if self.total > 0.0 {
            (self.tags[tag.index()] / self.total).min(1.0)
        } else {
            0.0
        }
    }

    fn script_share(&self, script: Script) -> f32 {
        if self.total > 0.0 {
            (self.scripts[script.index()] / self.total).min(1.0)
        } else {
            0.0
        }
    }

    fn artist_share(&self, artists: &[String]) -> f32 {
        if self.total <= 0.0 {
            return 0.0;
        }
        let best = artists
            .iter()
            .filter_map(|a| self.artists.get(&a.to_lowercase()))
            .fold(0.0f32, |m, v| m.max(*v));
        (best / self.total).min(1.0)
    }
}

/// One song that could play next, with what is known about where it came from.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub track: OfflineTrack,
    /// Place in the radio of the song that is playing (0 is first); a large number when it came only from another radio.
    pub rank: usize,
    /// In how many of the radios consulted it appears (at least 1).
    pub votes: usize,
    /// A user upload (lyric video, cover) rather than an official YouTube Music track.
    pub video: bool,
}

/// Mood tags a candidate shows that the profile does not (a party song in a sad session); the profile's strongest mood is used.
fn mood_fit(profile: &Profile, tags: &[bool; 5]) -> f32 {
    let mut fit = 0.0;
    for tag in TAGS.into_iter().filter(|t| t.is_mood() && tags[t.index()]) {
        // Centred on a half: a mood most of the session has is a match, one it has hardly seen is a break.
        fit += profile.tag_share(tag) - 0.5;
    }
    fit
}

fn folk_fit(profile: &Profile, tags: &[bool; 5]) -> f32 {
    let session = profile.tag_share(Tag::Folk);
    let song = tags[Tag::Folk.index()];
    match (session >= 0.4, song) {
        (true, true) => 0.6,
        (true, false) => -0.25, // the lexicon is not complete, so only a light push away
        (false, true) if session < 0.1 => -0.6,
        _ => 0.0,
    }
}

fn script_fit(profile: &Profile, script: Script) -> f32 {
    let share = profile.script_share(script);
    let dominant = [
        Script::Thai,
        Script::Latin,
        Script::Japanese,
        Script::Korean,
        Script::Chinese,
    ]
    .into_iter()
    .map(|s| profile.script_share(s))
    .fold(0.0f32, f32::max);
    // A session that is nearly all one language and a song in a language it has hardly heard: a clear break, not a slight one.
    if script != Script::Other && share < 0.1 && dominant >= 0.7 {
        -1.0
    } else {
        share - 0.5
    }
}

/// What a candidate looks like to the score: the learnable terms, in `Weights::to_array` order. The profile terms are zero
/// before the session has any songs.
pub fn features(profile: &Profile, c: &Candidate, radios: usize) -> Features {
    let tags = tags_of(&c.track.title, &c.track.artists);
    let rank = 1.0 / (1.0 + c.rank as f32 / 8.0); // 1.0 first, 0.5 at the ninth, long tail below
    let votes = if radios > 1 {
        (c.votes.saturating_sub(1)) as f32 / (radios - 1) as f32
    } else {
        0.0
    };
    if profile.is_empty() {
        return [rank, votes, 0.0, 0.0, 0.0, 0.0];
    }
    [
        rank,
        votes,
        mood_fit(profile, &tags),
        folk_fit(profile, &tags),
        script_fit(profile, song_script(&c.track.title, &c.track.artists)),
        profile.artist_share(&c.track.artists),
    ]
}

/// How well a candidate fits the profile, before its place in the order is known. Higher is better.
fn fit(profile: &Profile, w: &Weights, c: &Candidate, x: &Features) -> f32 {
    let mut score: f32 = w.to_array().iter().zip(x).map(|(a, b)| a * b).sum();
    if is_variant(&c.track.title) && profile.variants < 0.5 {
        score -= 1.2;
    }
    if is_mix(&c.track.title, c.track.duration_ms) {
        score -= 3.0;
    }
    if c.video {
        score -= 0.4;
    }
    score
}

/// How well `next` follows `previous`: the same mood and genre, a different artist.
fn transition(previous: &OfflineTrack, next: &OfflineTrack, w: &Weights) -> f32 {
    let a = tags_of(&previous.title, &previous.artists);
    let b = tags_of(&next.title, &next.artists);
    let mut score = 0.0;
    for tag in TAGS {
        if a[tag.index()] && b[tag.index()] {
            score += 0.3;
        } else if tag.is_mood()
            && (a[tag.index()] != b[tag.index()])
            && (a[tag.index()] || b[tag.index()])
        {
            score -= 0.4; // sad straight into party, or back
        }
    }
    if previous
        .artists
        .iter()
        .any(|x| next.artists.iter().any(|y| x.eq_ignore_ascii_case(y)))
    {
        score -= w.repeat_artist; // not the same artist twice in a row
    }
    score
}

/// Songs to take out altogether for "Same vibe": a clear break of mood or language from a session that has a clear one.
fn breaks_vibe(profile: &Profile, c: &Candidate) -> bool {
    if profile.is_empty() {
        return false;
    }
    let tags = tags_of(&c.track.title, &c.track.artists);
    let wrong_mood = TAGS
        .into_iter()
        .any(|t| t.is_mood() && tags[t.index()] && profile.tag_share(t) < 0.1)
        && TAGS
            .into_iter()
            .any(|t| t.is_mood() && profile.tag_share(t) >= 0.5);
    let wrong_script = {
        let script = song_script(&c.track.title, &c.track.artists);
        script != Script::Other
            && profile.script_share(script) < 0.1
            && [
                Script::Thai,
                Script::Latin,
                Script::Japanese,
                Script::Korean,
                Script::Chinese,
            ]
            .into_iter()
            .any(|s| profile.script_share(s) >= 0.7)
    };
    wrong_mood || wrong_script
}

/// Orders `candidates` for the queue: each pick is the best fit to the session, nudged by how well it follows the previous pick.
/// `radios` is how many radios were consulted (for the votes), `previous` the song that is playing.
pub fn sequence(
    profile: &Profile,
    vibe: Vibe,
    candidates: Vec<Candidate>,
    radios: usize,
    previous: Option<&OfflineTrack>,
    limit: usize,
) -> Vec<OfflineTrack> {
    let w = profile.weights(vibe);
    let mut pool: Vec<(f32, Candidate)> = candidates
        .into_iter()
        .filter(|c| vibe != Vibe::Same || !breaks_vibe(profile, c))
        .map(|c| (fit(profile, &w, &c, &features(profile, &c, radios)), c))
        .collect();
    let mut out: Vec<OfflineTrack> = Vec::new();
    let mut last = previous.cloned();
    while out.len() < limit && !pool.is_empty() {
        let best = pool
            .iter()
            .enumerate()
            .map(|(i, (score, c))| {
                (
                    i,
                    score + last.as_ref().map_or(0.0, |p| transition(p, &c.track, &w)),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        let Some(i) = best else { break };
        let (_, picked) = pool.swap_remove(i);
        last = Some(picked.track.clone());
        out.push(picked.track);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str, title: &str, artist: &str) -> OfflineTrack {
        OfflineTrack {
            provider: "YouTube".into(),
            id: id.into(),
            title: title.into(),
            artists: vec![artist.into()],
            artwork_url: None,
            duration_ms: Some(200_000),
        }
    }

    fn cand(rank: usize, votes: usize, t: OfflineTrack) -> Candidate {
        Candidate {
            track: t,
            rank,
            votes,
            video: false,
        }
    }

    fn profile_of(songs: &[OfflineTrack]) -> Profile {
        let mut p = Profile::default();
        for s in songs {
            p.fade(0.8);
            p.observe(s, 1.0);
        }
        p
    }

    fn ids(v: &[OfflineTrack]) -> Vec<&str> {
        v.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn scripts_are_told_apart() {
        assert_eq!(script_of("กลัวความเสียใจ Saran"), Script::Thai);
        assert_eq!(script_of("Bad Guy"), Script::Latin);
        assert_eq!(script_of("オトノケ"), Script::Japanese);
        assert_eq!(
            script_of("夜に駆ける 夜"),
            Script::Japanese,
            "kanji with kana is Japanese"
        );
        assert_eq!(script_of("눈물"), Script::Korean);
        assert_eq!(script_of("123 !!"), Script::Other);
    }

    #[test]
    fn titles_carry_mood_and_genre_tags() {
        let sad = tags_of("กลัวความเสียใจ", &["Saran".into()]);
        assert!(sad[Tag::Sad.index()]);
        let folk = tags_of("รักบ่าวอีสานใต้", &["จินตหรา พูนลาภ".into()]);
        assert!(folk[Tag::Folk.index()]);
        let sad_beat = tags_of("เสียใจ (Dance Remix)", &[]);
        assert!(
            sad_beat[Tag::Sad.index()] && !sad_beat[Tag::Party.index()],
            "sad wins over party"
        );
        assert!(!tags_of("April", &["Only Monday".into()]).iter().any(|t| *t));
    }

    #[test]
    fn covers_and_long_mixes_are_recognised() {
        assert!(is_variant("เอาที่สบายใจ (COVER VERSION)"));
        assert!(is_variant("Song [Lyric Video]") || is_variant("Song lyric video"));
        assert!(!is_variant("Fame is a Gun"));
        assert!(is_mix("รวมกลอนลำ แทนความคิดถึง", Some(200_000)));
        assert!(is_mix("A normal title", Some(3_600_000)));
        assert!(!is_mix("A normal title", Some(240_000)));
    }

    #[test]
    fn a_sad_session_does_not_slide_into_a_party_song() {
        let played = [
            song("a", "กลัวความเสียใจ", "Saran"),
            song("b", "ขอลา", "Pearpilincys"),
            song("c", "จากกันโดยสมบูรณ์", "guncharlie"),
        ];
        let profile = profile_of(&played);
        // YouTube lists the fun one first; the sad ones follow.
        let candidates = vec![
            cand(0, 1, song("party", "ปาร์ตี้ไนท์ สนุกสุดเหวี่ยง", "DJ X")),
            cand(1, 2, song("s1", "ใจพัง", "Saran")),
            cand(2, 2, song("s2", "ฝากให้เขารัก", "Yes'sir days")),
            cand(3, 1, song("s3", "เดินมาส่ง (BYE)", "First Anuwat")),
        ];
        let order = sequence(
            &profile,
            Vibe::Balanced,
            candidates.clone(),
            3,
            Some(&played[2]),
            10,
        );
        let party_at = order.iter().position(|t| t.id == "party");
        assert!(
            party_at.is_none_or(|p| p >= 3),
            "party song must come last, got {:?}",
            ids(&order)
        );
        let same = sequence(&profile, Vibe::Same, candidates, 3, Some(&played[2]), 10);
        assert!(
            !ids(&same).contains(&"party"),
            "Same vibe leaves it out, got {:?}",
            ids(&same)
        );
    }

    #[test]
    fn an_isan_session_stays_isan_and_thai() {
        let played = [
            song("a", "รักบ่าวอีสานใต้", "จินตหรา"),
            song("b", "ลำเพลิน ฮักอ้ายเด้อ", "ไผ่ พงศธร"),
        ];
        let profile = profile_of(&played);
        let candidates = vec![
            cand(0, 1, song("en", "Blinding Lights", "The Weeknd")),
            cand(1, 1, song("pop", "ใจพัง", "Saran")),
            cand(2, 2, song("f1", "ผู้สาวขาเลาะ", "ลำไย ไหทองคำ")),
            cand(3, 2, song("f2", "คึดฮอดบ้านนา", "ก้อง ห้วยไร่")),
        ];
        let order = sequence(
            &profile,
            Vibe::Balanced,
            candidates,
            2,
            Some(&played[1]),
            10,
        );
        assert_eq!(&ids(&order)[..2], &["f1", "f2"], "got {:?}", ids(&order));
        assert_eq!(
            *ids(&order).last().unwrap(),
            "en",
            "English comes last in a Thai session, got {:?}",
            ids(&order)
        );
    }

    #[test]
    fn explore_follows_youtubes_order_but_still_drops_mixes() {
        let profile = profile_of(&[song("a", "กลัวความเสียใจ", "Saran")]);
        let mut mix = song("mix", "รวมเพลงเศร้า 1 hour", "Someone");
        mix.duration_ms = Some(3_700_000);
        let candidates = vec![
            cand(0, 1, song("party", "ปาร์ตี้ สนุก", "DJ X")),
            cand(1, 1, mix),
            cand(2, 1, song("s1", "ใจพัง", "Saran")),
        ];
        let order = sequence(&profile, Vibe::Explore, candidates, 1, None, 10);
        assert_eq!(
            ids(&order)[0],
            "party",
            "Explore keeps YouTube's order, got {:?}",
            ids(&order)
        );
        assert_eq!(*ids(&order).last().unwrap(), "mix");
    }

    #[test]
    fn covers_are_pushed_down_unless_the_session_is_covers() {
        let profile = profile_of(&[song("a", "April", "Only Monday")]);
        let candidates = vec![
            cand(0, 1, song("c", "April (Cover Version)", "Someone")),
            cand(1, 1, song("o", "Ghost", "Only Monday")),
        ];
        assert_eq!(
            ids(&sequence(
                &profile,
                Vibe::Balanced,
                candidates.clone(),
                1,
                None,
                10
            ))[0],
            "o"
        );
        let cover_profile = profile_of(&[
            song("a", "Song (Cover Version)", "X"),
            song("b", "Other (Cover)", "Y"),
        ]);
        assert_eq!(
            ids(&sequence(
                &cover_profile,
                Vibe::Balanced,
                candidates,
                1,
                None,
                10
            ))[0],
            "c"
        );
    }

    #[test]
    fn the_same_artist_is_not_played_twice_in_a_row() {
        let first = song("a", "April", "Saran");
        let profile = profile_of(std::slice::from_ref(&first));
        // Neutral titles, so only the artist rule is in play.
        let candidates = vec![
            cand(0, 1, song("s1", "Ghost", "SARAN")),
            cand(1, 1, song("s2", "Mirror", "SARAN")),
            cand(2, 1, song("o1", "River", "Pearpilincys")),
            cand(3, 1, song("o2", "Window", "guncharlie")),
        ];
        let order = sequence(&profile, Vibe::Balanced, candidates, 1, Some(&first), 10);
        assert_eq!(order.len(), 4);
        for pair in order.windows(2) {
            assert!(
                !pair[0].artists[0].eq_ignore_ascii_case(&pair[1].artists[0]),
                "same artist back to back in {:?}",
                ids(&order)
            );
        }
        assert!(
            !order[0].artists[0].eq_ignore_ascii_case("saran"),
            "the song that is playing is by Saran, got {:?}",
            ids(&order)
        );
    }

    #[test]
    fn votes_bring_a_neighbour_up() {
        let profile = Profile::default();
        let candidates = vec![
            cand(0, 1, song("lone", "A", "X")),
            cand(4, 3, song("shared", "B", "Y")),
        ];
        let order = sequence(&profile, Vibe::Same, candidates, 3, None, 10);
        assert_eq!(ids(&order)[0], "shared");
    }

    #[test]
    fn a_change_of_taste_takes_over_after_a_few_songs() {
        let mut p = Profile::default();
        for i in 0..4 {
            p.fade(0.8);
            p.observe(&song(&format!("s{i}"), "เสียใจ", "A"), 1.0);
        }
        assert!(p.tag_share(Tag::Sad) > 0.9);
        for i in 0..4 {
            p.fade(0.8);
            p.observe(&song(&format!("p{i}"), "ปาร์ตี้ สนุก", "B"), 1.0);
        }
        assert!(
            p.tag_share(Tag::Party) > p.tag_share(Tag::Sad),
            "party {} sad {}",
            p.tag_share(Tag::Party),
            p.tag_share(Tag::Sad)
        );
    }

    #[test]
    fn an_empty_profile_just_uses_youtubes_order() {
        let order = sequence(
            &Profile::default(),
            Vibe::Balanced,
            vec![
                cand(2, 1, song("c", "C", "z")),
                cand(0, 1, song("a", "A", "x")),
                cand(1, 1, song("b", "B", "y")),
            ],
            1,
            None,
            10,
        );
        assert_eq!(ids(&order), ["a", "b", "c"]);
    }

    #[test]
    fn limit_and_vibe_names() {
        let many: Vec<Candidate> = (0..30)
            .map(|i| cand(i, 1, song(&format!("{i:03}"), "T", &format!("artist{i}"))))
            .collect();
        assert_eq!(
            sequence(&Profile::default(), Vibe::Balanced, many, 1, None, 7).len(),
            7
        );
        for v in [Vibe::Same, Vibe::Balanced, Vibe::Explore] {
            assert_eq!(Vibe::parse(v.as_str()), Some(v));
        }
        assert_eq!(Vibe::parse("wild"), None);
    }
}

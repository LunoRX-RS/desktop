//! Orders search hits so the real thing comes first: official releases and verified artists' own uploads, then the other
//! YouTube Music tracks, then everything else. Pure and testable; the adapter only gathers the signals.

use super::hit::SearchHit;

/// Where a hit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An official YouTube Music release (audio track).
    Release,
    /// A YouTube Music video result.
    MusicVideo,
    /// An ordinary YouTube video.
    Video,
}

/// The uploader's verification mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Unknown,
    /// Verified channel (checkmark).
    Verified,
    /// Verified music artist (note symbol).
    Artist,
}

pub struct Candidate {
    pub hit: SearchHit,
    pub kind: Kind,
    pub channel: Channel,
}

// A result that is a cover, remix or alternate version of what was asked for is not the original.
const ALTERNATE_WORDS: [&str; 12] = [
    "cover",
    "remix",
    "live",
    "karaoke",
    "instrumental",
    "nightcore",
    "slowed",
    "reverb",
    "sped",
    "8d",
    "reaction",
    "คัฟเวอร์",
];

// Share of the query words that are not the artist's name (percent) the title must contain before a hit can rank as official.
const MIN_TITLE_MATCH: usize = 80;

fn tokens(text: &str) -> Vec<String> {
    // Only whitespace and ASCII punctuation separate words. Thai vowel and tone marks are not alphanumeric, so splitting on
    // "not alphanumeric" would cut Thai words apart.
    text.to_lowercase()
        .split(|c: char| {
            c.is_whitespace() || c.is_ascii_punctuation() || "【】「」・–—".contains(c)
        })
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

struct Scored {
    index: usize,
    tier: u8,
    artist_match: bool,
    relevance: usize,
    official: bool,
    /// Only the first of these may keep `official` when the query names no artist (see `order`).
    needs_trust: bool,
    hit: SearchHit,
}

fn relevance(query: &[String], text: &[String]) -> usize {
    if query.is_empty() {
        return 0;
    }
    // The last word may be half typed, so it also matches as a prefix.
    let last = query.len() - 1;
    let found = query
        .iter()
        .enumerate()
        .filter(|(i, q)| {
            text.iter()
                .any(|t| t == *q || (*i == last && t.starts_with(q.as_str())))
        })
        .count();
    found * 100 / query.len()
}

fn score(index: usize, query: &[String], c: Candidate) -> Scored {
    let title = tokens(c.hit.title());
    let artist_words: Vec<String> = c.hit.artists().iter().flat_map(|a| tokens(a)).collect();
    let mut text = title.clone();
    text.extend(artist_words.iter().cloned());
    let relevance = relevance(query, &text);
    // What is left of the query once the artist's name is taken out must be in the title: "creepy nuts otonoke" is about
    // Otonoke, so another Creepy Nuts song is not the official answer.
    let rest: Vec<String> = query
        .iter()
        .filter(|q| !artist_words.contains(q))
        .cloned()
        .collect();
    let title_match = if rest.is_empty() {
        100
    } else {
        self::relevance(&rest, &title)
    };
    let alternate = ALTERNATE_WORDS
        .iter()
        .any(|w| title.iter().any(|t| t == w) && !query.iter().any(|q| q == w));
    let artist_match = c.hit.artists().iter().any(|artist| {
        let words = tokens(artist);
        !words.is_empty() && words.iter().all(|w| query.contains(w))
    });
    let says_official = title.iter().any(|t| t == "official");
    // A verified artist channel counts when the song is credited to that artist: named in the query or in the title. A dance
    // studio or reaction channel with the same mark does not.
    let credited = artist_match
        || (!artist_words.is_empty() && artist_words.iter().all(|w| title.contains(w)));
    let genuine = c.kind == Kind::Release
        || (c.channel == Channel::Artist && credited)
        || (says_official && c.channel != Channel::Unknown);
    // Someone asking for covers, remixes or live versions wants those, not the original.
    let asks_alternate = ALTERNATE_WORDS.iter().any(|w| query.iter().any(|q| q == w));
    let official = genuine && !alternate && !asks_alternate && title_match >= MIN_TITLE_MATCH;
    let base = if official {
        0
    } else if c.kind == Kind::Video {
        2
    } else {
        1
    };
    // An alternate version drops one tier but never below the plain videos.
    let tier = if alternate && base < 2 {
        base + 1
    } else {
        base
    };
    Scored {
        index,
        tier,
        artist_match,
        relevance,
        official,
        needs_trust: official && !artist_match && c.channel != Channel::Artist,
        hit: c.hit,
    }
}

/// Official first, then YouTube Music, then videos. Inside a tier: hits by an artist named in the query, then the best
/// word match, then the provider's own order. Duplicate videos keep their first (best placed) copy.
pub fn order(query: &str, candidates: Vec<Candidate>, limit: usize) -> Vec<SearchHit> {
    let query = tokens(query);
    let mut seen = std::collections::HashSet::new();
    let mut scored: Vec<Scored> = candidates
        .into_iter()
        .filter(|c| seen.insert(c.hit.provider_track_id().to_owned()))
        .enumerate()
        .map(|(i, c)| score(i, &query, c))
        .collect();
    // Many artists release a song with the same title. When the query names no artist, only the provider's top match is
    // called official; the rest are still shown, as music.
    // A hit by an artist the query names always outranks that guess.
    let named_exists = scored.iter().any(|s| s.official && s.artist_match);
    let mut trusted = false;
    for s in scored.iter_mut().filter(|s| s.needs_trust) {
        if trusted || named_exists {
            s.official = false;
            s.tier = 1;
        }
        trusted = true;
    }
    scored.sort_by_key(|s| {
        (
            s.tier,
            !s.artist_match,
            std::cmp::Reverse(s.relevance),
            s.index,
        )
    });
    scored.truncate(limit);
    scored
        .into_iter()
        .map(|mut s| {
            s.hit.official = s.official;
            s.hit
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain::ProviderId;

    fn cand(id: &str, title: &str, artist: &str, kind: Kind, channel: Channel) -> Candidate {
        let hit = SearchHit::new(
            ProviderId::YouTube,
            id.into(),
            format!("https://www.youtube.com/watch?v={id}"),
            title.into(),
            vec![artist.into()],
        )
        .unwrap();
        Candidate { hit, kind, channel }
    }

    fn ids(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(SearchHit::provider_track_id).collect()
    }

    #[test]
    fn official_then_music_then_videos() {
        let hits = order(
            "addison rae fame is a gun",
            vec![
                cand(
                    "v1",
                    "Addison Rae - Fame is a Gun (fan edit)",
                    "Some Fan",
                    Kind::Video,
                    Channel::Unknown,
                ),
                cand(
                    "m1",
                    "Fame is a Gun (acoustic)",
                    "Other Band",
                    Kind::MusicVideo,
                    Channel::Unknown,
                ),
                cand(
                    "o1",
                    "Fame is a Gun",
                    "Addison Rae",
                    Kind::Release,
                    Channel::Unknown,
                ),
            ],
            10,
        );
        assert_eq!(ids(&hits), ["o1", "m1", "v1"]);
        assert!(hits[0].official());
        assert!(!hits[1].official());
    }

    #[test]
    fn verified_artist_upload_is_official() {
        let hits = order(
            "fame is a gun official",
            vec![
                cand(
                    "m1",
                    "Fame is a Gun",
                    "Cover Girl",
                    Kind::MusicVideo,
                    Channel::Unknown,
                ),
                cand(
                    "a1",
                    "Addison Rae - Fame is a Gun (Official Video)",
                    "Addison Rae",
                    Kind::Video,
                    Channel::Artist,
                ),
            ],
            10,
        );
        assert_eq!(ids(&hits), ["a1", "m1"]);
        assert!(hits[0].official());
    }

    #[test]
    fn unverified_upload_saying_official_is_not_official() {
        let hits = order(
            "fame is a gun",
            vec![cand(
                "v1",
                "Fame is a Gun (Official)",
                "Random",
                Kind::Video,
                Channel::Unknown,
            )],
            10,
        );
        assert!(!hits[0].official());
    }

    #[test]
    fn covers_and_remixes_drop_unless_asked_for() {
        let list = || {
            vec![
                cand(
                    "c1",
                    "Fame is a Gun (Cover)",
                    "Cover Girl",
                    Kind::Release,
                    Channel::Unknown,
                ),
                cand(
                    "o1",
                    "Fame is a Gun",
                    "Addison Rae",
                    Kind::Release,
                    Channel::Unknown,
                ),
            ]
        };
        assert_eq!(ids(&order("fame is a gun", list(), 10)), ["o1", "c1"]);
        assert!(!order("fame is a gun", list(), 10)[1].official());
        // Asking for covers means no hit is labelled official, but the cover still comes first.
        let asked = order("fame is a gun cover", list(), 10);
        assert_eq!(ids(&asked)[0], "c1");
        assert!(asked.iter().all(|h| !h.official()));
    }

    #[test]
    fn another_song_by_the_named_artist_is_not_official() {
        let hits = order(
            "creepy nuts otonoke",
            vec![
                cand(
                    "n",
                    "Nemure",
                    "Creepy Nuts",
                    Kind::Release,
                    Channel::Unknown,
                ),
                cand(
                    "o",
                    "Otonoke",
                    "Creepy Nuts",
                    Kind::Release,
                    Channel::Unknown,
                ),
            ],
            10,
        );
        assert_eq!(ids(&hits), ["o", "n"]);
        assert!(hits[0].official());
        assert!(!hits[1].official());
        // Asking for the artist alone makes all of their releases official.
        let all = order(
            "creepy nuts",
            vec![
                cand(
                    "n",
                    "Nemure",
                    "Creepy Nuts",
                    Kind::Release,
                    Channel::Unknown,
                ),
                cand(
                    "o",
                    "Otonoke",
                    "Creepy Nuts",
                    Kind::Release,
                    Channel::Unknown,
                ),
            ],
            10,
        );
        assert!(all.iter().all(SearchHit::official));
    }

    #[test]
    fn without_an_artist_in_the_query_only_the_top_release_is_official() {
        let hits = order(
            "fame is a gun",
            vec![
                cand(
                    "a",
                    "Fame is a Gun",
                    "Addison Rae",
                    Kind::Release,
                    Channel::Unknown,
                ),
                cand("b", "Fame is a Gun", "EVE", Kind::Release, Channel::Unknown),
                cand(
                    "m",
                    "Fame is a Gun (Official Video)",
                    "Addison Rae",
                    Kind::Video,
                    Channel::Artist,
                ),
            ],
            10,
        );
        let official: Vec<&str> = hits
            .iter()
            .filter(|h| h.official())
            .map(|h| h.provider_track_id())
            .collect();
        assert_eq!(
            official,
            ["a", "m"],
            "the verified artist's own video stays official"
        );
        assert_eq!(hits[2].provider_track_id(), "b");
    }

    #[test]
    fn a_hit_that_barely_matches_cannot_be_official() {
        let hits = order(
            "fame is a gun",
            vec![cand(
                "o1",
                "Different Song",
                "Addison Rae",
                Kind::Release,
                Channel::Artist,
            )],
            10,
        );
        assert!(!hits[0].official());
    }

    #[test]
    fn artist_named_in_the_query_wins_inside_a_tier() {
        let hits = order(
            "billie eilish bad guy",
            vec![
                cand(
                    "a",
                    "bad guy",
                    "Someone Else",
                    Kind::Release,
                    Channel::Unknown,
                ),
                cand(
                    "b",
                    "bad guy",
                    "Billie Eilish",
                    Kind::Release,
                    Channel::Unknown,
                ),
            ],
            10,
        );
        assert_eq!(ids(&hits), ["b", "a"]);
    }

    #[test]
    fn half_typed_last_word_still_matches() {
        let hits = order(
            "addison rae fame is a g",
            vec![cand(
                "o1",
                "Fame is a Gun",
                "Addison Rae",
                Kind::Release,
                Channel::Unknown,
            )],
            10,
        );
        assert!(hits[0].official());
    }

    #[test]
    fn thai_words_are_not_split() {
        assert_eq!(tokens("เพลงใหม่ - ล่าสุด"), ["เพลงใหม่", "ล่าสุด"]);
        let hits = order(
            "ก้าวกระโดด",
            vec![cand(
                "t1",
                "ก้าวกระโดด",
                "ศิลปิน",
                Kind::Release,
                Channel::Unknown,
            )],
            10,
        );
        assert!(hits[0].official());
    }

    #[test]
    fn duplicates_collapse_and_limit_applies() {
        let hits = order(
            "song",
            vec![
                cand("same", "Song", "A", Kind::Release, Channel::Unknown),
                cand("same", "Song", "A", Kind::Video, Channel::Unknown),
                cand("two", "Song", "B", Kind::Release, Channel::Unknown),
                cand("three", "Song", "C", Kind::Release, Channel::Unknown),
            ],
            2,
        );
        assert_eq!(ids(&hits), ["same", "two"]);
    }
}

//! Which YouTube video is the music video of a song. Pure: the candidates come from the search, this only ranks them.
//! A candidate must carry the song's title; then the official-video wording, the artist as channel, and a length that
//! matches the song decide. Covers, remixes, live takes and sped-up edits are pushed down, never shown.

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration_ms: Option<u64>,
}

/// Letters, marks and digits in lower case, so "BU (Official MV)" and "bu official mv" compare equal.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || is_mark(c) {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_mark(c: char) -> bool {
    // Thai and other scripts keep vowels and tone marks as combining characters.
    matches!(c as u32, 0x0300..=0x036F | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E)
}

fn has_word(hay: &str, word: &str) -> bool {
    hay.split(' ').any(|w| w == word)
}

fn score(c: &Candidate, title: &str, artist: &str, duration_ms: Option<u64>) -> Option<i32> {
    let t = norm(&c.title);
    let want = norm(title);
    if want.is_empty() || !t.contains(&want) {
        return None;
    }
    let mut s = 0;
    if t.contains("official")
        && (t.contains("music video") || has_word(&t, "mv") || has_word(&t, "video"))
    {
        s += 6;
    } else if has_word(&t, "mv") || t.contains("music video") {
        s += 4;
    }
    let ch = norm(&c.channel);
    let artist = norm(artist);
    if !artist.is_empty() && (ch.contains(&artist) || t.contains(&artist)) {
        s += 3;
    }
    for bad in [
        "cover",
        "remix",
        "reaction",
        "karaoke",
        "instrumental",
        "nightcore",
        "slowed",
        "sped up",
        "8d",
        "4d",
        "live",
        "acoustic",
        "lyrics",
        "1 hour",
        "loop",
    ] {
        if t.contains(bad) {
            s -= 8;
        }
    }
    if let (Some(want), Some(got)) = (duration_ms, c.duration_ms) {
        let diff = want.abs_diff(got);
        if diff <= 3_000 {
            s += 4;
        } else if diff > 20_000 {
            s -= 6;
        }
    }
    Some(s)
}

/// The best candidate id, or None when no candidate carries the title. Ties keep the search order.
pub fn best(
    title: &str,
    artist: &str,
    duration_ms: Option<u64>,
    candidates: &[Candidate],
) -> Option<String> {
    candidates
        .iter()
        .filter_map(|c| score(c, title, artist, duration_ms).map(|s| (s, c)))
        .fold(None::<(i32, &Candidate)>, |acc, (s, c)| match acc {
            Some((best, _)) if best >= s => acc,
            _ => Some((s, c)),
        })
        .map(|(_, c)| c.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str, title: &str, channel: &str, duration_ms: Option<u64>) -> Candidate {
        Candidate {
            id: id.into(),
            title: title.into(),
            channel: channel.into(),
            duration_ms,
        }
    }

    #[test]
    fn prefers_the_official_video_over_a_cover_and_the_audio_upload() {
        let list = [
            c("cover1", "BU cover by someone", "someone", Some(200_000)),
            c("audio1", "BU", "SURIYA MQT - Topic", Some(206_000)),
            c("mv1", "BU (Official MV)", "SURIYA MQT", Some(206_000)),
        ];
        assert_eq!(
            best("BU", "SURIYA MQT", Some(206_000), &list).as_deref(),
            Some("mv1")
        );
    }

    #[test]
    fn thai_titles_and_vowel_marks_match() {
        let list = [c(
            "mv2",
            "พอตเตอร์ (POTTER) Official MV",
            "Anatomy Rabbit",
            None,
        )];
        assert_eq!(
            best("พอตเตอร์", "Anatomy Rabbit", None, &list).as_deref(),
            Some("mv2")
        );
    }

    #[test]
    fn a_title_that_does_not_contain_the_song_is_never_picked() {
        let list = [c(
            "x",
            "Something else official music video",
            "Artist",
            None,
        )];
        assert_eq!(best("BU", "Artist", None, &list), None);
    }

    #[test]
    fn length_breaks_ties_between_official_videos() {
        let list = [
            c(
                "long",
                "Song Official Video (Extended)",
                "Artist",
                Some(600_000),
            ),
            c("right", "Song Official Video", "Artist", Some(210_000)),
        ];
        assert_eq!(
            best("Song", "Artist", Some(210_000), &list).as_deref(),
            Some("right")
        );
    }
}

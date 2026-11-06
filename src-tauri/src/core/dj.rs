//! DJ deck rules that need no window: which deck is which, how the crossfader splits volume, and which loops and
//! speeds are acceptable. The decks themselves are YouTube pages driven from `commands::dj`.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Deck {
    A,
    B,
}

impl Deck {
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "youtube-dj-a",
            Self::B => "youtube-dj-b",
        }
    }
}

/// Volume (0 to 100) for each deck. The crossfader runs 0 (all A) to 100 (all B); the curve is equal-power so the
/// middle is not quieter than the ends. `master` is 0 to 100.
pub fn deck_volumes(crossfader: u8, master: u8) -> (u8, u8) {
    let x = f64::from(crossfader.min(100)) / 100.0;
    let m = f64::from(master.min(100));
    let a = (x * std::f64::consts::FRAC_PI_2).cos();
    let b = (x * std::f64::consts::FRAC_PI_2).sin();
    ((a * m).round() as u8, (b * m).round() as u8)
}

/// A loop between two points in the track, in milliseconds.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Loop {
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Loop {
    pub const MIN_MS: u64 = 100;
    pub const MAX_MS: u64 = 10 * 60 * 1000;
    /// True when the end is after the start by a playable amount and the loop is not absurdly long.
    pub fn valid(&self) -> bool {
        self.end_ms > self.start_ms
            && (Self::MIN_MS..=Self::MAX_MS).contains(&(self.end_ms - self.start_ms))
    }
}

/// A live control on one deck. Every value is range-checked before it reaches the page.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DeckControl {
    /// -100 (low-pass closed) to 100 (high-pass open); 0 is off.
    Filter { value: i8 },
    /// Input gain in percent, 0 to 200.
    Trim { value: u8 },
    /// Channel fader, 0 to 100.
    Fader { value: u8 },
    /// Temporary speed nudge in percent, -20 to 20; 0 releases it.
    Bend { value: i8 },
    /// Echo every `delay_ms` (0 is off, up to 4000) with `feedback` 0 to 90 percent.
    Echo { delay_ms: u16, feedback: u8 },
    /// Slows the deck to a stop, like a turntable losing power.
    Brake,
}

impl DeckControl {
    pub fn valid(&self) -> bool {
        match *self {
            Self::Filter { value } => (-100..=100).contains(&value),
            Self::Trim { value } => value <= 200,
            Self::Fader { value } => value <= 100,
            Self::Bend { value } => (-20..=20).contains(&value),
            Self::Echo { delay_ms, feedback } => delay_ms <= 4000 && feedback <= 90,
            Self::Brake => true,
        }
    }
}

/// One cue point can be jumped to; the track is never longer than a day.
pub const MAX_POSITION_MS: u64 = 24 * 60 * 60 * 1000;

pub fn position_ok(ms: u64) -> bool {
    ms <= MAX_POSITION_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_ids_are_eleven_url_safe_characters() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(!valid_video_id("short"));
        assert!(!valid_video_id("dQw4w9WgXc!"));
        assert!(!valid_video_id("dQw4w9WgXcQ0"));
    }

    #[test]
    fn wave_batches_parse_and_reject_junk() {
        let w = parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|10.20.30,255.0.7").unwrap();
        assert_eq!(
            (w.at_ms, w.bands.clone()),
            (1500, vec![[10, 20, 30], [255, 0, 7]])
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|10.20").is_none(),
            "missing band"
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|dQw4w9WgXcQ|1500|300.0.0").is_none(),
            "out of range"
        );
        assert!(
            parse_wave(Deck::A, "LUNORX_WF|bad|1500|1.2.3").is_none(),
            "bad id"
        );
        assert!(
            parse_wave(
                Deck::A,
                &format!("LUNORX_WF|dQw4w9WgXcQ|0|{}", vec!["1.2.3"; 17].join(","))
            )
            .is_none(),
            "too many"
        );
        assert!(parse_wave(Deck::A, "LUNORX_ST|dQw4w9WgXcQ|0|0|playing").is_none());
    }

    #[test]
    fn crossfader_ends_and_middle() {
        assert_eq!(deck_volumes(0, 100), (100, 0));
        assert_eq!(deck_volumes(100, 100), (0, 100));
        let (a, b) = deck_volumes(50, 100);
        assert_eq!((a, b), (71, 71), "equal power: about -3 dB each, not half");
    }

    #[test]
    fn master_scales_and_inputs_are_clamped() {
        assert_eq!(deck_volumes(0, 50), (50, 0));
        assert_eq!(deck_volumes(0, 0), (0, 0));
        assert_eq!(
            deck_volumes(250, 250),
            (0, 100),
            "out of range is clamped, never panics"
        );
    }

    #[test]
    fn loops_must_be_forward_and_sane() {
        assert!(
            Loop {
                start_ms: 1_000,
                end_ms: 5_000
            }
            .valid()
        );
        assert!(
            !Loop {
                start_ms: 5_000,
                end_ms: 1_000
            }
            .valid(),
            "backwards"
        );
        assert!(
            !Loop {
                start_ms: 1_000,
                end_ms: 1_050
            }
            .valid(),
            "too short to hear"
        );
        assert!(
            !Loop {
                start_ms: 0,
                end_ms: Loop::MAX_MS + 1
            }
            .valid(),
            "too long"
        );
        assert!(
            !Loop {
                start_ms: u64::MAX,
                end_ms: 0
            }
            .valid()
        );
    }

    #[test]
    fn deck_labels_are_distinct_and_serialise_lowercase() {
        assert_ne!(Deck::A.label(), Deck::B.label());
        assert_eq!(serde_json::to_string(&Deck::A).unwrap(), "\"a\"");
        assert_eq!(serde_json::from_str::<Deck>("\"b\"").unwrap(), Deck::B);
    }

    #[test]
    fn controls_are_range_checked_and_keep_their_wire_shape() {
        assert!(DeckControl::Filter { value: -100 }.valid());
        assert!(!DeckControl::Filter { value: 101 }.valid());
        assert!(!DeckControl::Trim { value: 201 }.valid());
        assert!(!DeckControl::Bend { value: -21 }.valid());
        assert!(
            !DeckControl::Echo {
                delay_ms: 4001,
                feedback: 10
            }
            .valid()
        );
        assert!(
            !DeckControl::Echo {
                delay_ms: 500,
                feedback: 91
            }
            .valid()
        );
        assert!(DeckControl::Brake.valid());
        let c: DeckControl =
            serde_json::from_str(r#"{"kind":"echo","delayMs":500,"feedback":40}"#).unwrap();
        assert_eq!(
            c,
            DeckControl::Echo {
                delay_ms: 500,
                feedback: 40
            }
        );
        assert_eq!(
            serde_json::to_string(&DeckControl::Brake).unwrap(),
            r#"{"kind":"brake"}"#
        );
    }

    #[test]
    fn positions_are_bounded() {
        assert!(position_ok(0));
        assert!(!position_ok(MAX_POSITION_MS + 1));
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wave {
    pub deck: Deck,
    pub video_id: String,
    pub at_ms: u64,
    pub bands: Vec<[u8; 3]>,
}

/// `LUNORX_WF|<id>|<at ms>|l.m.h,l.m.h,...`. At most 16 samples; anything malformed is dropped whole.
pub fn parse_wave(deck: Deck, title: &str) -> Option<Wave> {
    let mut p = title.strip_prefix("LUNORX_WF|")?.splitn(3, '|');
    let video_id = p.next()?.to_owned();
    let at_ms = p.next()?.parse().ok()?;
    let bands = p
        .next()?
        .split(',')
        .map(|s| {
            let mut v = s.split('.').map(|x| x.parse::<u8>().ok());
            Some([v.next()??, v.next()??, v.next()??])
        })
        .collect::<Option<Vec<_>>>()?;
    (!bands.is_empty() && bands.len() <= 16 && valid_video_id(&video_id)).then_some(Wave {
        deck,
        video_id,
        at_ms,
        bands,
    })
}

pub fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

//! Device-local taste data for autoplay. Each song autoplay picked is logged with the terms it scored on and how much of it was
//! listened to. The last `MAX_ROWS` songs are kept, and the weights of the score are fitted from them, pulled back to the
//! built-in weights so a few skips cannot swing the order. Nothing leaves the device; the user can clear it in Settings.
use super::taste::{FEATURES, Features, Vibe};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// The most songs kept. Older ones are dropped as new ones are logged.
pub const MAX_ROWS: usize = 100;
/// Fewer songs than this for a vibe and the built-in weights stay for it.
const MIN_ROWS: usize = 20;
/// How strongly the fit is pulled back to the built-in weights (ridge penalty). Larger means the songs must agree more.
const PULL: f64 = 4.0;

/// The shared log: the autoplay session and the player both write to it.
pub type Taste = Arc<Mutex<TasteLog>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub vibe: String,
    pub x: Features,
    /// How much of the song was listened to, 0..1.
    pub y: f32,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    rows: Vec<Row>,
}

pub struct TasteLog {
    path: Option<PathBuf>,
    rows: Vec<Row>,
    /// Picks still playing or queued: their terms, waiting for the listened share.
    pending: HashMap<String, (Vibe, Features)>,
}

/// The share of a song listened to, from the player's position and the song's length.
pub fn listened_share(position_ms: u64, duration_ms: u64) -> f32 {
    if duration_ms == 0 {
        return 0.0;
    }
    (position_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
}

impl TasteLog {
    /// Loads the saved rows. A missing or unreadable file starts an empty log: learning is optional, never an error.
    pub fn open(path: Option<PathBuf>) -> Self {
        let rows = path
            .as_ref()
            .and_then(|p| fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
            .map(|saved| saved.rows)
            .unwrap_or_default();
        let mut log = Self {
            path,
            rows,
            pending: HashMap::new(),
        };
        log.trim();
        log
    }

    /// A pick autoplay queued. Its listened share is logged when it ends or is left.
    pub fn expect(&mut self, id: String, vibe: Vibe, x: Features) {
        self.pending.insert(id, (vibe, x));
    }

    /// The song `id` is done (or left) after `listened` of it. Songs that were not autoplay picks are ignored.
    pub fn finish(&mut self, id: &str, listened: f32) {
        let Some((vibe, x)) = self.pending.remove(id) else {
            return;
        };
        self.rows.push(Row {
            vibe: vibe.as_str().into(),
            x,
            y: listened.clamp(0.0, 1.0),
        });
        self.trim();
        self.save();
    }

    /// The fitted weights per vibe, for `Profile::learn`. A vibe with too few songs has `None`.
    pub fn learned(&self) -> [Option<Features>; 3] {
        [Vibe::Same, Vibe::Balanced, Vibe::Explore].map(|vibe| {
            let rows: Vec<(Features, f32)> = self
                .rows
                .iter()
                .filter(|r| r.vibe == vibe.as_str())
                .map(|r| (r.x, r.y))
                .collect();
            (rows.len() >= MIN_ROWS).then(|| fit_ridge(&rows, vibe.prior()))
        })
    }

    /// Deletes every logged song and pick. The file is removed too.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.pending.clear();
        if let Some(path) = &self.path {
            let _ = fs::remove_file(path);
        }
    }

    fn trim(&mut self) {
        let extra = self.rows.len().saturating_sub(MAX_ROWS);
        self.rows.drain(..extra);
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec(&Saved {
            rows: self.rows.clone(),
        }) else {
            return;
        };
        // Written beside the file and renamed, so a crash mid-write cannot leave half a file behind.
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, bytes).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
}

/// Least squares of `y` on `x`, pulled toward `prior`: minimises sum((y - w·x)²) + PULL·|w - prior|², solved exactly
/// (normal equations, Gauss-Jordan). The result is clamped so one odd batch of songs cannot make a weight huge.
pub fn fit_ridge(rows: &[(Features, f32)], prior: Features) -> Features {
    let mut a = [[0f64; FEATURES + 1]; FEATURES];
    for (x, y) in rows {
        for i in 0..FEATURES {
            for j in 0..FEATURES {
                a[i][j] += f64::from(x[i]) * f64::from(x[j]);
            }
            a[i][FEATURES] += f64::from(x[i]) * f64::from(*y);
        }
    }
    for i in 0..FEATURES {
        a[i][i] += PULL;
        a[i][FEATURES] += PULL * f64::from(prior[i]);
    }
    for col in 0..FEATURES {
        // Diagonal is at least PULL, so every pivot is non-zero.
        let pivot = (col..FEATURES)
            .max_by(|&p, &q| a[p][col].abs().total_cmp(&a[q][col].abs()))
            .unwrap_or(col);
        a.swap(col, pivot);
        let d = a[col][col];
        for v in &mut a[col][col..] {
            *v /= d;
        }
        let pivot_row = a[col];
        for r in (0..FEATURES).filter(|&r| r != col) {
            let f = a[r][col];
            for (cell, p) in a[r][col..].iter_mut().zip(&pivot_row[col..]) {
                *cell -= f * p;
            }
        }
    }
    let mut w = [0f32; FEATURES];
    for (i, out) in w.iter_mut().enumerate() {
        *out = (a[i][FEATURES] as f32).clamp(-4.0, 4.0);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIOR: Features = [1.0, 0.5, 1.0, 0.5, 0.5, 0.5];

    #[test]
    fn listened_share_is_a_bounded_fraction() {
        assert_eq!(listened_share(50_000, 200_000), 0.25);
        assert_eq!(listened_share(300_000, 200_000), 1.0);
        assert_eq!(listened_share(10, 0), 0.0);
    }

    #[test]
    fn fit_follows_the_songs_when_there_are_many() {
        // Songs with the mood term are played through; songs without it are skipped.
        let mut rows = Vec::new();
        for i in 0..40 {
            let rank = 1.0 / (1.0 + (i % 5) as f32);
            rows.push(([rank, 0.0, 1.0, 0.0, 0.0, 0.0], 0.95));
            rows.push(([rank, 0.0, 0.0, 0.0, 0.0, 0.0], 0.1));
        }
        let w = fit_ridge(&rows, PRIOR);
        assert!(
            w[2] > 0.6,
            "mood weight should rise toward the songs, got {}",
            w[2]
        );
        assert!(w[2] > w[0], "mood should outweigh rank here, got {w:?}");
    }

    #[test]
    fn few_songs_keep_the_prior() {
        let w = fit_ridge(&[([1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0.0)], PRIOR);
        assert!(
            (w[0] - PRIOR[0]).abs() < 0.5,
            "one song barely moves the prior, got {w:?}"
        );
        assert!(w[2] > 0.5, "mood stays near its prior, got {w:?}");
    }

    #[test]
    fn log_keeps_only_the_latest_songs_and_ignores_unknown_ids() {
        let mut log = TasteLog::open(None);
        log.finish("never-picked", 1.0);
        for i in 0..MAX_ROWS + 25 {
            log.expect(format!("id{i}"), Vibe::Balanced, PRIOR);
            log.finish(&format!("id{i}"), 0.5);
        }
        assert_eq!(log.rows.len(), MAX_ROWS);
        assert!(log.pending.is_empty());
    }

    #[test]
    fn a_vibe_needs_enough_songs_before_it_learns() {
        let mut log = TasteLog::open(None);
        for i in 0..MIN_ROWS - 1 {
            log.expect(format!("id{i}"), Vibe::Same, PRIOR);
            log.finish(&format!("id{i}"), 1.0);
        }
        assert_eq!(log.learned()[Vibe::Same.index()], None);
        log.expect("last".into(), Vibe::Same, PRIOR);
        log.finish("last", 1.0);
        assert!(log.learned()[Vibe::Same.index()].is_some());
        assert_eq!(log.learned()[Vibe::Explore.index()], None);
    }
}

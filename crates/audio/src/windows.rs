//! Which 10 s windows of a track are embedded.
//!
//! `ClapFeatureExtractor` picks ONE random 10 s crop (`rand_trunc`); for a
//! deterministic result we embed up to three evenly spaced windows instead,
//! skipping the first and last 5 % of the track (intros, fade-outs), and
//! average their embeddings. The Python reference (`tools/clap/export.py`)
//! uses the same rule.

use crate::mel::WINDOW;

/// Maximum number of windows embedded per track.
pub const MAX_WINDOWS: usize = 3;

/// Start offsets (in samples) of the windows for a track of `n` samples.
/// Empty when `n <= WINDOW` (the whole track is one padded window).
pub fn window_starts(n: usize) -> Vec<usize> {
    if n <= WINDOW {
        return Vec::new();
    }
    let edge = n / 20; // 5 %
    let (a, b) = (edge, n - edge);
    let usable = b - a;
    if usable < WINDOW {
        // Too short to skip the edges: one centred window.
        return vec![(n - WINDOW) / 2];
    }
    let count = (usable / WINDOW).clamp(1, MAX_WINDOWS);
    (0..count)
        .map(|i| {
            // Centre of the i-th equal part of [a, b), clamped so the window fits.
            let centre = a as f64 + (i as f64 + 0.5) * usable as f64 / count as f64;
            let start = (centre - WINDOW as f64 / 2.0).round().max(a as f64) as usize;
            start.min(b - WINDOW)
        })
        .collect()
}

/// The windows to embed, each exactly [`WINDOW`] samples long. A track shorter
/// than 10 s is repeated whole as many times as fits, then zero-padded
/// (`repeatpad`, as in transformers).
pub fn windows(samples: &[f32]) -> Vec<Vec<f32>> {
    let n = samples.len();
    if n == 0 {
        return Vec::new();
    }
    if n <= WINDOW {
        let reps = if n == WINDOW { 1 } else { WINDOW / n };
        let mut w = Vec::with_capacity(WINDOW);
        for _ in 0..reps.max(1) {
            w.extend_from_slice(samples);
        }
        w.resize(WINDOW, 0.0);
        return vec![w];
    }
    window_starts(n).into_iter().map(|s| samples[s..s + WINDOW].to_vec()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_track_gets_three_windows_inside_the_margins() {
        let n = 48_000 * 180;
        let s = window_starts(n);
        assert_eq!(s.len(), 3);
        for &st in &s {
            assert!(st >= n / 20 && st + WINDOW <= n - n / 20);
        }
        assert!(s[0] < s[1] && s[1] < s[2]);
    }

    #[test]
    fn medium_and_short_tracks() {
        assert_eq!(window_starts(48_000 * 25).len(), 2);
        assert_eq!(window_starts(48_000 * 11).len(), 1);
        let w = windows(&vec![1.0; 48_000 * 3]);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].len(), WINDOW);
        // 3 s repeated 3 times = 9 s of signal, then 1 s of zeros.
        assert_eq!(w[0][48_000 * 9 - 1], 1.0);
        assert_eq!(w[0][48_000 * 9], 0.0);
    }
}

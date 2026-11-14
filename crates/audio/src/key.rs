//! Key estimation: an STFT chroma vector summed over the track, correlated with
//! the 24 rotations of a major and a minor key profile (Krumhansl-Schmuckler
//! method). The profile tables are published research data:
//! Krumhansl & Kessler (1982) and Temperley (1999, "What's key for key?").

use crate::dsp::stft_mag;

pub const PITCH_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Krumhansl,
    Temperley,
}

impl Profile {
    fn tables(self) -> ([f32; 12], [f32; 12]) {
        match self {
            Profile::Krumhansl => (
                [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88],
                [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17],
            ),
            Profile::Temperley => (
                [5.0, 2.0, 3.5, 2.0, 4.5, 4.0, 2.0, 4.5, 2.0, 3.5, 1.5, 4.0],
                [5.0, 2.0, 3.5, 4.5, 2.0, 4.0, 2.0, 4.5, 3.5, 2.0, 1.5, 4.0],
            ),
        }
    }
}

/// Tuning of the chroma computation (exposed for evaluation).
#[derive(Debug, Clone, Copy)]
pub struct KeyConfig {
    pub profile: Profile,
    pub f_min: f32,
    pub f_max: f32,
    /// Compress note energies with `ln(1 + gain * e)`; 0 keeps raw magnitudes.
    pub log_gain: f32,
    /// Normalise each frame's chroma to unit maximum before summing.
    pub frame_norm: bool,
    /// Average (rather than sum) the bins of each semitone, so that every
    /// semitone weighs the same whatever its bandwidth (constant-Q like).
    /// Summing lets the many bins of high notes drown the bass and mids.
    pub note_avg: bool,
    /// Estimate the track's tuning offset (in cents from A440) and correct for it.
    pub tuning: bool,
}

impl Default for KeyConfig {
    fn default() -> Self {
        KeyConfig {
            profile: Profile::Temperley,
            f_min: 80.0,
            f_max: 2000.0,
            log_gain: 0.0,
            frame_norm: true,
            note_avg: true,
            tuning: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    /// Pitch class of the tonic, 0 = C.
    pub tonic: usize,
    pub minor: bool,
    /// Correlation of the chroma with the winning profile (-1..1).
    pub score: f32,
}

impl Key {
    pub fn name(&self) -> String {
        format!("{} {}", PITCH_NAMES[self.tonic], if self.minor { "minor" } else { "major" })
    }
}

/// Tuning offset in semitones (-0.5..0.5): the magnitude-weighted circular mean
/// of every bin's distance to the nearest equal-tempered semitone, on the
/// spectrum averaged over the track.
fn estimate_tuning(frames: &[Vec<f32>], bin_hz: f32, f_min: f32, f_max: f32) -> f32 {
    let n = frames[0].len();
    let mut mean = vec![0.0f32; n];
    for f in frames {
        for (m, v) in mean.iter_mut().zip(f) {
            *m += v;
        }
    }
    // Only local peaks: the skirts of a peak would pull the mean toward zero.
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for k in 1..n - 1 {
        let f = k as f32 * bin_hz;
        if f < f_min || f > f_max || mean[k] <= mean[k - 1] || mean[k] < mean[k + 1] {
            continue;
        }
        let p = 12.0 * (f / 440.0).log2() + 69.0;
        let d = (p - p.round()) as f64;
        let w = mean[k] as f64;
        s += w * (2.0 * std::f64::consts::PI * d).sin();
        c += w * (2.0 * std::f64::consts::PI * d).cos();
    }
    if s == 0.0 && c == 0.0 { 0.0 } else { (s.atan2(c) / (2.0 * std::f64::consts::PI)) as f32 }
}

/// 12-bin chroma of `x` (mono, rate `sr`), summed over the whole signal.
pub fn chroma(x: &[f32], sr: f32, cfg: &KeyConfig) -> Option<[f32; 12]> {
    // ~1.5 Hz resolution at 12 kHz so that low notes resolve into semitones.
    let n_fft = ((sr / 1.5) as usize).next_power_of_two();
    let hop = n_fft / 4;
    let frames = stft_mag(x, n_fft, hop);
    if frames.is_empty() {
        return None;
    }
    let bin_hz = sr / n_fft as f32;
    let tuning = if cfg.tuning { estimate_tuning(&frames, bin_hz, cfg.f_min, cfg.f_max) } else { 0.0 };
    // (bin, note index from the lowest note, weight): a bin counts for its nearest
    // semitone, weighted down as it falls between two semitones.
    let lowest = (12.0 * (cfg.f_min / 440.0).log2() + 69.0).round() as i32;
    let map: Vec<(usize, usize, f32)> = (1..n_fft / 2)
        .filter_map(|k| {
            let f = k as f32 * bin_hz;
            if f < cfg.f_min || f > cfg.f_max {
                return None;
            }
            let p = 12.0 * (f / 440.0).log2() + 69.0 - tuning;
            let r = p.round();
            let w = (std::f32::consts::PI * (p - r)).cos().powi(2);
            Some((k, (r as i32 - lowest).max(0) as usize, w))
        })
        .collect();
    let n_notes = map.iter().map(|m| m.1).max()? + 1;
    let mut wsum = vec![0.0f32; n_notes];
    for &(_, n, w) in &map {
        wsum[n] += w;
    }
    let mut total = [0.0f32; 12];
    let mut notes = vec![0.0f32; n_notes];
    for spec in &frames {
        notes.iter_mut().for_each(|v| *v = 0.0);
        for &(k, n, w) in &map {
            notes[n] += w * spec[k];
        }
        let mut c = [0.0f32; 12];
        for (n, &e) in notes.iter().enumerate() {
            let e = if cfg.note_avg && wsum[n] > 0.0 { e / wsum[n] } else { e };
            let e = if cfg.log_gain > 0.0 { (1.0 + cfg.log_gain * e).ln() } else { e };
            c[(n as i32 + lowest).rem_euclid(12) as usize] += e;
        }
        let max = c.iter().cloned().fold(0.0f32, f32::max);
        if max <= 1e-9 {
            continue;
        }
        let scale = if cfg.frame_norm { 1.0 / max } else { 1.0 };
        for i in 0..12 {
            total[i] += c[i] * scale;
        }
    }
    if total.iter().all(|&v| v <= 0.0) { None } else { Some(total) }
}

fn pearson(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        num += (a[i] - ma) * (b[i] - mb);
        da += (a[i] - ma).powi(2);
        db += (b[i] - mb).powi(2);
    }
    if da <= 0.0 || db <= 0.0 { 0.0 } else { num / (da * db).sqrt() }
}

/// Best of the 24 keys for a chroma vector.
pub fn key_from_chroma(c: &[f32; 12], profile: Profile) -> Key {
    let (major, minor) = profile.tables();
    let mut best = Key { tonic: 0, minor: false, score: f32::MIN };
    for tonic in 0..12 {
        for (is_minor, prof) in [(false, &major), (true, &minor)] {
            // Rotate the profile so that index 0 of the profile sits on `tonic`.
            let mut rot = [0.0f32; 12];
            for i in 0..12 {
                rot[(i + tonic) % 12] = prof[i];
            }
            let s = pearson(c, &rot);
            if s > best.score {
                best = Key { tonic, minor: is_minor, score: s };
            }
        }
    }
    best
}

pub fn estimate_with(x: &[f32], sr: f32, cfg: &KeyConfig) -> Option<Key> {
    if (x.len() as f32) < sr * 3.0 {
        return None;
    }
    let c = chroma(x, sr, cfg)?;
    let k = key_from_chroma(&c, cfg.profile);
    (k.score > 0.0).then_some(k)
}

/// Estimated key of `x` (mono, rate `sr`), or `None` for audio too short or atonal.
pub fn estimate(x: &[f32], sr: f32) -> Option<Key> {
    estimate_with(x, sr, &KeyConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn midi_hz(n: i32) -> f32 {
        440.0 * 2f32.powf((n - 69) as f32 / 12.0)
    }

    /// A chord progression of harmonic tones (I-IV-V-I or i-iv-v-i), 2 s each.
    fn progression(tonic_midi: i32, minor: bool, sr: f32) -> Vec<f32> {
        let third = if minor { 3 } else { 4 };
        let chords = [[0, third, 7], [5, 5 + third, 12], [7, 7 + 4, 14], [0, third, 7]];
        let seg = (sr * 2.0) as usize;
        let mut x = vec![0.0f32; seg * chords.len() * 2];
        for (ci, ch) in chords.iter().cycle().take(chords.len() * 2).enumerate() {
            for &iv in ch {
                let f0 = midi_hz(tonic_midi + iv);
                for h in 1..=4 {
                    let f = f0 * h as f32;
                    if f > sr / 2.0 {
                        break;
                    }
                    for i in 0..seg {
                        let t = i as f32 / sr;
                        x[ci * seg + i] += (2.0 * std::f32::consts::PI * f * t).sin() * 0.1 / h as f32;
                    }
                }
            }
            // Bass note on the chord root.
            let fb = midi_hz(tonic_midi + ch[0] - 12);
            for i in 0..seg {
                x[ci * seg + i] += (2.0 * std::f32::consts::PI * fb * i as f32 / sr).sin() * 0.15;
            }
        }
        x
    }

    #[test]
    fn finds_major_and_minor_keys() {
        let sr = 12_000.0;
        for &(tonic, minor) in &[(60, false), (57, true), (62, false), (64, true), (67, false), (65, true)] {
            let x = progression(tonic, minor, sr);
            let k = estimate(&x, sr).expect("key");
            assert_eq!((k.tonic, k.minor), ((tonic % 12) as usize, minor), "got {}", k.name());
        }
    }

    #[test]
    fn names() {
        assert_eq!(Key { tonic: 9, minor: true, score: 1.0 }.name(), "A minor");
        assert_eq!(Key { tonic: 6, minor: false, score: 1.0 }.name(), "F# major");
    }

    #[test]
    fn silence_has_no_key() {
        assert!(estimate(&vec![0.0; 12_000 * 10], 12_000.0).is_none());
    }
}

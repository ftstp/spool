//! Log-mel features identical to `transformers.ClapFeatureExtractor` for
//! `laion/larger_clap_music` on its non-fusion path (`truncation="rand_trunc"`):
//! 48 kHz mono, 10 s windows, `repeatpad` padding, periodic Hann window of 1024,
//! hop 480, centred frames with reflect padding, power spectrum, 64 Slaney mel
//! bands (Slaney scale and Slaney area normalisation) between 50 Hz and 14 kHz,
//! then `10 * log10(max(mel, 1e-10))` with no `top_db` clipping.
//! The result is `(1001 frames x 64 bands)`, frame-major.

use std::sync::{Arc, OnceLock};

use realfft::{RealFftPlanner, RealToComplex};

pub const SAMPLE_RATE: u32 = 48_000;
pub const N_FFT: usize = 1024;
pub const HOP: usize = 480;
pub const N_MELS: usize = 64;
pub const F_MIN: f64 = 50.0;
pub const F_MAX: f64 = 14_000.0;
/// Samples in one model window (10 s at 48 kHz).
pub const WINDOW: usize = 480_000;
/// Frames produced for one window: `1 + WINDOW / HOP` (centred framing).
pub const FRAMES: usize = 1 + WINDOW / HOP;
const N_BINS: usize = N_FFT / 2 + 1;

fn hz_to_mel_slaney(f: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if f >= min_log_hz { min_log_mel + (f / min_log_hz).ln() / logstep } else { f / f_sp }
}

fn mel_to_hz_slaney(m: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if m >= min_log_mel { min_log_hz * (logstep * (m - min_log_mel)).exp() } else { f_sp * m }
}

/// The `(513 x 64)` mel filter bank, row = FFT bin (same layout as transformers).
pub fn mel_filters() -> &'static [f64] {
    static F: OnceLock<Vec<f64>> = OnceLock::new();
    F.get_or_init(|| {
        let mel_min = hz_to_mel_slaney(F_MIN);
        let mel_max = hz_to_mel_slaney(F_MAX);
        let filter_freqs: Vec<f64> = (0..N_MELS + 2)
            .map(|i| mel_to_hz_slaney(mel_min + (mel_max - mel_min) * i as f64 / (N_MELS + 1) as f64))
            .collect();
        // np.linspace(0, sampling_rate // 2, num_frequency_bins)
        let nyq = (SAMPLE_RATE / 2) as f64;
        let fft_freqs: Vec<f64> = (0..N_BINS).map(|k| nyq * k as f64 / (N_BINS - 1) as f64).collect();
        let diff: Vec<f64> = filter_freqs.windows(2).map(|w| w[1] - w[0]).collect();
        let mut fb = vec![0.0f64; N_BINS * N_MELS];
        for (k, &f) in fft_freqs.iter().enumerate() {
            for m in 0..N_MELS {
                let down = -(filter_freqs[m] - f) / diff[m];
                let up = (filter_freqs[m + 2] - f) / diff[m + 1];
                let enorm = 2.0 / (filter_freqs[m + 2] - filter_freqs[m]);
                fb[k * N_MELS + m] = down.min(up).max(0.0) * enorm;
            }
        }
        fb
    })
}

fn hann_periodic() -> &'static [f64] {
    static W: OnceLock<Vec<f64>> = OnceLock::new();
    W.get_or_init(|| {
        (0..N_FFT).map(|n| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / N_FFT as f64).cos()).collect()
    })
}

fn fft() -> Arc<dyn RealToComplex<f64>> {
    static P: OnceLock<Arc<dyn RealToComplex<f64>>> = OnceLock::new();
    P.get_or_init(|| RealFftPlanner::<f64>::new().plan_fft_forward(N_FFT)).clone()
}

/// Log-mel spectrogram of exactly one window of [`WINDOW`] samples:
/// returns `FRAMES * N_MELS` values, frame-major.
pub fn log_mel(window: &[f32]) -> Vec<f32> {
    assert_eq!(window.len(), WINDOW, "log_mel expects exactly one 10 s window");
    let pad = N_FFT / 2;
    // Reflect padding (numpy "reflect": the edge sample is not repeated).
    let n = window.len();
    let mut padded = Vec::with_capacity(n + 2 * pad);
    for i in (1..=pad).rev() {
        padded.push(window[i] as f64);
    }
    padded.extend(window.iter().map(|&v| v as f64));
    for i in 0..pad {
        padded.push(window[n - 2 - i] as f64);
    }
    let win = hann_periodic();
    let fb = mel_filters();
    let plan = fft();
    let mut input = plan.make_input_vec();
    let mut spec = plan.make_output_vec();
    let mut scratch = plan.make_scratch_vec();
    let mut power = vec![0.0f64; N_BINS];
    let mut out = vec![0.0f32; FRAMES * N_MELS];
    for t in 0..FRAMES {
        let start = t * HOP;
        for i in 0..N_FFT {
            input[i] = padded[start + i] * win[i];
        }
        plan.process_with_scratch(&mut input, &mut spec, &mut scratch).expect("fft sizes are fixed");
        for (p, c) in power.iter_mut().zip(spec.iter()) {
            *p = c.re * c.re + c.im * c.im;
        }
        let row = &mut out[t * N_MELS..(t + 1) * N_MELS];
        let mut acc = [0.0f64; N_MELS];
        for (k, &p) in power.iter().enumerate() {
            if p == 0.0 {
                continue;
            }
            let f = &fb[k * N_MELS..(k + 1) * N_MELS];
            for m in 0..N_MELS {
                acc[m] += p * f[m];
            }
        }
        for m in 0..N_MELS {
            row[m] = (10.0 * acc[m].max(1e-10).log10()) as f32;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_bank_shape_and_norm() {
        let fb = mel_filters();
        assert_eq!(fb.len(), N_BINS * N_MELS);
        // Every band has some weight, and nothing above 14 kHz or below 50 Hz.
        for m in 0..N_MELS {
            let s: f64 = (0..N_BINS).map(|k| fb[k * N_MELS + m]).sum();
            assert!(s > 0.0, "band {m} empty");
        }
        let bin_hz = 24_000.0 / 512.0;
        for k in 0..N_BINS {
            let f = k as f64 * bin_hz;
            if f < F_MIN || f > F_MAX {
                for m in 0..N_MELS {
                    assert_eq!(fb[k * N_MELS + m], 0.0, "bin {k} ({f} Hz) band {m}");
                }
            }
        }
    }

    #[test]
    fn silence_is_floor() {
        let x = vec![0.0f32; WINDOW];
        let m = log_mel(&x);
        assert_eq!(m.len(), FRAMES * N_MELS);
        assert!(m.iter().all(|&v| (v + 100.0).abs() < 1e-4));
    }

    #[test]
    fn sine_lands_in_the_right_band() {
        // A 1 kHz sine: the loudest band in a middle frame must contain 1 kHz.
        let x: Vec<f32> = (0..WINDOW)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / 48_000.0).sin() as f32 * 0.5)
            .collect();
        let m = log_mel(&x);
        let row = &m[500 * N_MELS..501 * N_MELS];
        let best = (0..N_MELS).max_by(|&a, &b| row[a].total_cmp(&row[b])).unwrap();
        let mel_min = hz_to_mel_slaney(F_MIN);
        let mel_max = hz_to_mel_slaney(F_MAX);
        let lo = mel_to_hz_slaney(mel_min + (mel_max - mel_min) * best as f64 / 65.0);
        let hi = mel_to_hz_slaney(mel_min + (mel_max - mel_min) * (best + 2) as f64 / 65.0);
        assert!(lo < 1000.0 && 1000.0 < hi, "band {best} = {lo}..{hi} Hz");
    }

    /// Golden values from `transformers.ClapFeatureExtractor` (laion/larger_clap_music,
    /// truncation="rand_trunc") on the same synthetic 10 s signal; see `golden_signal`.
    #[test]
    fn matches_transformers_golden() {
        let m = log_mel(&golden_signal());
        for &(t, b, v) in GOLDEN {
            let got = m[t * N_MELS + b];
            assert!((got - v).abs() < 1e-3, "frame {t} band {b}: {got} vs {v}");
        }
    }

    /// Deterministic test signal: two sines and a linear chirp, with a quiet tail.
    pub(crate) fn golden_signal() -> Vec<f32> {
        (0..WINDOW)
            .map(|i| {
                let t = i as f64 / 48_000.0;
                let s = 0.3 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()
                    + 0.2 * (2.0 * std::f64::consts::PI * 3000.0 * t).sin()
                    + 0.1 * (2.0 * std::f64::consts::PI * (100.0 * t + 600.0 * t * t)).sin();
                (if t > 8.0 { s * 0.01 } else { s }) as f32
            })
            .collect()
    }

    include!("mel_golden.rs");
}

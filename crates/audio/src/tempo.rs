//! Tempo estimation: a spectral-flux onset-strength envelope at 200 frames/s,
//! its autocorrelation, and a comb score over 60-200 bpm (the beat period and
//! its first 8 multiples). The winning period is refined on a far multiple
//! with parabolic interpolation, then folded by octaves into [`FOLD_MIN`,
//! `FOLD_MAX`). No tempo prior: the folding settles octaves, and a prior
//! around 120 bpm turned 87/174 bpm tracks into 116 (a 4:3 error) on the
//! evaluation set.

use realfft::RealFftPlanner;

pub const FOLD_MIN: f32 = 80.0;
pub const FOLD_MAX: f32 = 180.0;
const N_FFT: usize = 512;

/// Onset-strength envelope of `x` (mono, rate `sr`) and its frame rate.
/// The hop is chosen to give 200 frames per second.
pub fn onset_envelope(x: &[f32], sr: f32) -> (Vec<f32>, f32) {
    let hop = ((sr / 200.0).round() as usize).max(1);
    let fps = sr / hop as f32;
    if x.len() < N_FFT + hop {
        return (Vec::new(), fps);
    }
    let plan = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);
    let win: Vec<f32> =
        (0..N_FFT).map(|n| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / N_FFT as f64).cos()) as f32).collect();
    let mut input = plan.make_input_vec();
    let mut spec = plan.make_output_vec();
    let mut scratch = plan.make_scratch_vec();
    // Bands: 30 log-spaced bands from 40 Hz to min(8 kHz, Nyquist).
    let n_bins = N_FFT / 2 + 1;
    let bin_hz = sr / N_FFT as f32;
    let f_hi = (sr / 2.0).min(8000.0);
    const N_BANDS: usize = 30;
    let edges: Vec<f32> = (0..=N_BANDS).map(|i| 40.0 * (f_hi / 40.0).powf(i as f32 / N_BANDS as f32)).collect();
    let band_of: Vec<Option<usize>> = (0..n_bins)
        .map(|k| {
            let f = k as f32 * bin_hz;
            if f < edges[0] || f >= edges[N_BANDS] { None } else { Some(edges.partition_point(|&e| e <= f) - 1) }
        })
        .collect();
    let frames = 1 + (x.len() - N_FFT) / hop;
    let mut prev = vec![0.0f32; N_BANDS];
    let mut cur = vec![0.0f32; N_BANDS];
    let mut env = Vec::with_capacity(frames);
    for t in 0..frames {
        let s = &x[t * hop..t * hop + N_FFT];
        for i in 0..N_FFT {
            input[i] = s[i] * win[i];
        }
        plan.process_with_scratch(&mut input, &mut spec, &mut scratch).expect("fixed sizes");
        cur.iter_mut().for_each(|v| *v = 0.0);
        for (k, c) in spec.iter().enumerate() {
            if let Some(b) = band_of[k] {
                cur[b] += c.re * c.re + c.im * c.im;
            }
        }
        for v in cur.iter_mut() {
            *v = (1.0 + 1000.0 * v.sqrt()).ln();
        }
        let flux: f32 = if t == 0 { 0.0 } else { cur.iter().zip(&prev).map(|(c, p)| (c - p).max(0.0)).sum() };
        env.push(flux);
        std::mem::swap(&mut prev, &mut cur);
    }
    // Remove the slow trend (moving average over ~0.5 s) and half-wave rectify.
    let w = (fps * 0.25) as usize;
    let mut csum = vec![0.0f64; env.len() + 1];
    for (i, &v) in env.iter().enumerate() {
        csum[i + 1] = csum[i] + v as f64;
    }
    let out = (0..env.len())
        .map(|i| {
            let a = i.saturating_sub(w);
            let b = (i + w + 1).min(env.len());
            let mean = (csum[b] - csum[a]) / (b - a) as f64;
            (env[i] as f64 - mean).max(0.0) as f32
        })
        .collect();
    (out, fps)
}

/// Autocorrelation of `x` for lags `0..max_lag`, each lag normalised by its
/// overlap and the whole by lag 0.
pub fn autocorrelation(x: &[f32], max_lag: usize) -> Vec<f32> {
    let n = x.len();
    let size = (2 * n).next_power_of_two();
    let mut planner = RealFftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(size);
    let inv = planner.plan_fft_inverse(size);
    let mut buf = fwd.make_input_vec();
    for (b, &v) in buf.iter_mut().zip(x) {
        *b = v as f64;
    }
    let mut spec = fwd.make_output_vec();
    fwd.process(&mut buf, &mut spec).expect("sizes");
    for c in spec.iter_mut() {
        *c = realfft::num_complex::Complex::new(c.norm_sqr(), 0.0);
    }
    inv.process(&mut spec, &mut buf).expect("sizes");
    let max_lag = max_lag.min(n.saturating_sub(1));
    let r0 = buf[0].max(1e-12);
    (0..=max_lag).map(|l| ((buf[l] / r0) * n as f64 / (n - l) as f64) as f32).collect()
}

fn interp(a: &[f32], x: f32) -> f32 {
    let i = x.floor() as usize;
    if i + 1 >= a.len() {
        return 0.0;
    }
    let f = x - i as f32;
    a[i] * (1.0 - f) + a[i + 1] * f
}

/// Tuning of the tempo search (exposed for evaluation).
#[derive(Debug, Clone, Copy)]
pub struct TempoConfig {
    /// Multiples of the beat period summed by the comb.
    pub multiples: usize,
    /// Width (in octaves) of the log-normal prior around 120 bpm; 0 = flat.
    pub prior_octaves: f32,
}

impl Default for TempoConfig {
    fn default() -> Self {
        TempoConfig { multiples: 8, prior_octaves: 0.0 }
    }
}

/// Estimated tempo in bpm (folded into 80-180), or `None` for audio too short
/// (< 6 s) or without rhythmic content.
pub fn estimate(x: &[f32], sr: f32) -> Option<f32> {
    estimate_with(x, sr, &TempoConfig::default())
}

pub fn estimate_with(x: &[f32], sr: f32, cfg: &TempoConfig) -> Option<f32> {
    let (env, fps) = onset_envelope(x, sr);
    if (env.len() as f32) < fps * 6.0 {
        return None;
    }
    let energy: f32 = env.iter().sum::<f32>() / env.len() as f32;
    if !(energy > 1e-4) {
        return None;
    }
    let multiples = cfg.multiples.max(1);
    let max_lag = (60.0 * fps / 60.0 * (multiples as f32 + 4.0)).ceil() as usize + 4;
    let acf = autocorrelation(&env, max_lag);
    let mut best = (0.0f32, f32::MIN);
    let mut b = 60.0f32;
    while b <= 200.0 {
        let lag = 60.0 * fps / b;
        let mut s = 0.0;
        for k in 1..=multiples {
            s += interp(&acf, lag * k as f32);
        }
        let prior =
            if cfg.prior_octaves > 0.0 { (-0.5 * ((b / 120.0).log2() / cfg.prior_octaves).powi(2)).exp() } else { 1.0 };
        let score = s / multiples as f32 * prior;
        if score > best.1 {
            best = (b, score);
        }
        b += 0.05;
    }
    if best.1 <= 0.0 {
        return None;
    }
    // Refine the period on the farthest multiple that fits.
    let lag = 60.0 * fps / best.0;
    let m = ((acf.len() as f32 - 3.0) / lag).floor().clamp(1.0, 8.0);
    let centre = (lag * m).round() as usize;
    let lo = centre.saturating_sub(3).max(1);
    let hi = (centre + 3).min(acf.len() - 2);
    let mut peak = centre.min(hi);
    for i in lo..=hi {
        if acf[i] > acf[peak] {
            peak = i;
        }
    }
    let (y0, y1, y2) = (acf[peak - 1], acf[peak], acf[peak + 1]);
    let denom = y0 - 2.0 * y1 + y2;
    let off = if denom.abs() > 1e-12 { (0.5 * (y0 - y2) / denom).clamp(-0.5, 0.5) } else { 0.0 };
    let refined = 60.0 * fps * m / (peak as f32 + off);
    // Only accept the refinement if it stays near the coarse estimate.
    let mut bpm = if (refined / best.0 - 1.0).abs() < 0.03 { refined } else { best.0 };
    while bpm < FOLD_MIN {
        bpm *= 2.0;
    }
    while bpm >= FOLD_MAX {
        bpm /= 2.0;
    }
    Some(bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click track: short decaying noise bursts every beat.
    fn clicks(bpm: f32, sr: f32, secs: f32) -> Vec<f32> {
        let n = (sr * secs) as usize;
        let period = 60.0 / bpm * sr;
        let mut x = vec![0.0f32; n];
        let mut seed = 12345u32;
        let mut k = 0.0f32;
        while (k * period) < n as f32 {
            let start = (k * period) as usize;
            for i in 0..(sr * 0.03) as usize {
                if start + i >= n {
                    break;
                }
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed >> 9) as f32 / (1u32 << 23) as f32 * 2.0 - 1.0;
                x[start + i] += noise * (-(i as f32) / (sr * 0.005)).exp();
            }
            k += 1.0;
        }
        x
    }

    #[test]
    fn finds_click_tempi() {
        for &bpm in &[90.0f32, 120.0, 128.0, 140.0, 174.0] {
            let x = clicks(bpm, 12_000.0, 30.0);
            let got = estimate(&x, 12_000.0).expect("tempo");
            assert!((got / bpm - 1.0).abs() < 0.01, "expected {bpm}, got {got}");
        }
    }

    #[test]
    fn slow_tempo_is_folded_up() {
        let x = clicks(70.0, 12_000.0, 30.0);
        let got = estimate(&x, 12_000.0).unwrap();
        assert!((got / 140.0 - 1.0).abs() < 0.01, "got {got}");
    }

    #[test]
    fn silence_has_no_tempo() {
        assert!(estimate(&vec![0.0; 12_000 * 20], 12_000.0).is_none());
        assert!(estimate(&vec![0.0; 12_000 * 2], 12_000.0).is_none());
    }
}

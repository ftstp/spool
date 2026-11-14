//! Small DSP helpers shared by tempo and key estimation.

use realfft::RealFftPlanner;

/// Decimates by an integer factor after a windowed-sinc low-pass
/// (cutoff at 0.45 x the new Nyquist band edge, Blackman window).
pub fn decimate(x: &[f32], factor: usize) -> Vec<f32> {
    if factor <= 1 {
        return x.to_vec();
    }
    let taps = 16 * factor + 1;
    let half = (taps / 2) as isize;
    let fc = 0.45 / factor as f64; // cycles per input sample
    let h: Vec<f32> = (0..taps)
        .map(|i| {
            let n = i as f64 - half as f64;
            let sinc = if n == 0.0 {
                2.0 * fc
            } else {
                (2.0 * std::f64::consts::PI * fc * n).sin() / (std::f64::consts::PI * n)
            };
            let w = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (taps - 1) as f64).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * i as f64 / (taps - 1) as f64).cos();
            (sinc * w) as f32
        })
        .collect();
    let n_out = x.len() / factor;
    let mut out = Vec::with_capacity(n_out);
    for o in 0..n_out {
        let c = (o * factor) as isize;
        let mut acc = 0.0f32;
        if c - half >= 0 && c + half < x.len() as isize {
            let s = &x[(c - half) as usize..=(c + half) as usize];
            for (a, b) in s.iter().zip(h.iter()) {
                acc += a * b;
            }
        } else {
            for (j, hv) in h.iter().enumerate() {
                let idx = c + j as isize - half;
                if idx >= 0 && (idx as usize) < x.len() {
                    acc += x[idx as usize] * hv;
                }
            }
        }
        out.push(acc);
    }
    out
}

/// Magnitude STFT (Hann window, no centring): returns frames of `n_fft/2+1` bins.
pub fn stft_mag(x: &[f32], n_fft: usize, hop: usize) -> Vec<Vec<f32>> {
    if x.len() < n_fft {
        return Vec::new();
    }
    let plan = RealFftPlanner::<f32>::new().plan_fft_forward(n_fft);
    let win: Vec<f32> =
        (0..n_fft).map(|n| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / n_fft as f64).cos()) as f32).collect();
    let mut input = plan.make_input_vec();
    let mut spec = plan.make_output_vec();
    let mut scratch = plan.make_scratch_vec();
    let frames = 1 + (x.len() - n_fft) / hop;
    let mut out = Vec::with_capacity(frames);
    for t in 0..frames {
        let s = &x[t * hop..t * hop + n_fft];
        for i in 0..n_fft {
            input[i] = s[i] * win[i];
        }
        plan.process_with_scratch(&mut input, &mut spec, &mut scratch).expect("fixed sizes");
        out.push(spec.iter().map(|c| (c.re * c.re + c.im * c.im).sqrt()).collect());
    }
    out
}

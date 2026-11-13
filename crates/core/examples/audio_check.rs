//! A render's measures: audio_check <file>...
fn main() {
    for a in std::env::args().skip(1) {
        let t = std::time::Instant::now();
        match app_core::audio::analyse(std::path::Path::new(&a)) {
            Ok(i) => println!("{a}\n  {:.1} s, {} Hz x{}, LUFS {:?}, peak {:.2} dBFS, RMS {:.2} dBFS, LRA {:?}, {} points, max {:.2} ({} ms)",
                i.duration, i.sample_rate, i.channels, i.lufs.map(|l| (l * 10.0).round() / 10.0), i.peak_db, i.rms_db, i.lra.map(|l| (l * 10.0).round() / 10.0), i.peaks.len(),
                i.peaks.iter().cloned().fold(0f32, f32::max), t.elapsed().as_millis()),
            Err(e) => println!("{a}: {e}"),
        }
    }
}

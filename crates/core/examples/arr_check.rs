//! What a project plays, on a COPY of a data folder: its tracks, audio clips and samples
//! (their size on disk), to see what playing it asks of the app.
//!     arr_check <data folder copy> <project rel>
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder (a copy)");
    let rel = a.next().expect("rel");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let t = std::time::Instant::now();
    let arr = app.arrangement(&rel).expect("arrangement");
    eprintln!("read in {:?}: tempo {:?}, len {} beats, {} tracks", t.elapsed(), arr.tempo, arr.len, arr.tracks.len());
    let mut samples: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let (mut clips, mut segs, mut notes) = (0, 0, 0);
    for tr in &arr.tracks {
        for c in &tr.clips {
            clips += 1;
            segs += c.segs.len();
            notes += c.notes.len();
            if let Some(s) = &c.sample {
                let e = samples.entry(s.clone()).or_default();
                e.0 += 1;
                e.1 += c.segs.len();
            }
        }
    }
    eprintln!("{clips} clips, {segs} audio pieces, {notes} notes, {} samples", samples.len());
    let mut total = 0u64;
    for (s, (n, k)) in &samples {
        let sz = std::fs::metadata(s).map(|m| m.len()).unwrap_or(0);
        total += sz;
        eprintln!("  {:>8.1} MB  {n} clips {k} pieces  {}", sz as f64 / 1e6, std::path::Path::new(s).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    }
    eprintln!("samples on disk: {:.1} MB", total as f64 / 1e6);
}

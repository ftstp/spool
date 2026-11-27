//! On a COPY of a data folder: the harmony of a few songs (their main project, only read).
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    for want in ["Roasted", "Fourmis", "ideaz courcoury", "Attitude", "Belfort"] {
        let Some(s) = app.songs().unwrap().into_iter().find(|s| s.title.as_deref() == Some(want)) else { continue };
        let d = app.song(s.id).unwrap().unwrap();
        let rel = d.files.iter().find(|f| f.latest).unwrap_or(&d.files[0]).rel.clone();
        let t = std::time::Instant::now();
        match app.harmony(&rel) {
            Ok(h) => {
                let seq: Vec<String> = h.bars.iter().take(16).map(|b| format!("{}:{}", b.bar + 1, b.name)).collect();
                eprintln!("{want}: key {:?}, {} bars with a chord ({:.1}s) {}", h.key, h.bars.len(), t.elapsed().as_secs_f64(), seq.join(" "));
            }
            Err(e) => eprintln!("{want}: {e}"),
        }
    }
}

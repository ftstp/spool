//! The MIDI files of a base's projects folders, scanned on a COPY of the base:
//! midi_check <base.sqlite> <scratch>
fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (a.next().unwrap(), std::path::PathBuf::from(a.next().unwrap()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::copy(&src, scratch.join("plugin-fixer.sqlite")).unwrap();
    let app = app_core::App::open(&scratch).unwrap();
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    println!("scan {:.1} s", t.elapsed().as_secs_f64());
    let m = app.midi_summary().unwrap();
    let m0 = &m;
    let m = &m0.mine;
    println!("{} MIDI files, kinds {:?}", m.files, m.kinds.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    println!("keys {:?}", m.keys.iter().take(8).map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    for p in m.progressions.iter().take(10) { println!("  {:22} x{} e.g. {:?} ({})", p.degrees, p.n, p.chords, p.example.rsplit('/').next().unwrap()); }
    println!("in projects: {}, loose: {}", m0.items.iter().filter(|i| i.in_project).count(), m0.items.iter().filter(|i| !i.in_project).count());
}

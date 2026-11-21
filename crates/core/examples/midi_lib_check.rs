//! A MIDI library folder analysed on a COPY of a base: midi_lib_check <base.sqlite> <scratch> <midi folder>
fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch, dir) = (a.next().unwrap(), std::path::PathBuf::from(a.next().unwrap()), a.next().unwrap());
    std::fs::create_dir_all(&scratch).unwrap();
    if !scratch.join("plugin-fixer.sqlite").exists() {
        std::fs::copy(&src, scratch.join("plugin-fixer.sqlite")).unwrap();
    }
    let app = app_core::App::open(&scratch).unwrap();
    app.set_config("midi_dirs", serde_json::json!([dir])).unwrap();
    let last = std::sync::Mutex::new(String::new());
    app.on_change(std::sync::Arc::new(move |st| {
        let line = format!("{} {}/{}", st.stage, st.done, st.total);
        let mut l = last.lock().unwrap();
        if st.stage != l.split(' ').next().unwrap_or("") || st.done % 20000 < 50 {
            eprintln!("{line}");
        }
        *l = line;
    }));
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    println!("scan {:.0} s", t.elapsed().as_secs_f64());
    let t = std::time::Instant::now();
    let m = app.midi_summary().unwrap();
    println!("summary {} ms: {} files", t.elapsed().as_millis(), m.files);
    let t = std::time::Instant::now();
    let _ = app.midi_summary().unwrap();
    println!("cached {} ms", t.elapsed().as_millis());
    for (name, m) in [("mine", &m.mine), ("library", &m.library)] {
        println!("== {name}: {} files, minor {} major {}", m.files, m.minor, m.major);
        println!("kinds {:?}", m.kinds.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
        println!("keys {:?}", m.keys.iter().take(8).map(|c| (&c.label, c.n)).collect::<Vec<_>>());
        println!("tempos {:?}", m.tempos.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
        for p in m.progressions.iter().take(6) { println!("  {:22} x{:5} e.g. {:?}", p.degrees, p.n, p.chords); }
    }
}

//! On a COPY of a data folder: one scan (the projects' MIDI clips read), then the MIDI page's
//! clips. The projects are only read.
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    eprintln!("scan {:.1} s", t.elapsed().as_secs_f64());
    for kind in [None, Some("chords"), Some("bass"), Some("melody"), Some("drums")] {
        let p = app.midi_page(app_core::midi_view::MidiQuery { source: Some("clips".into()), kind: kind.map(String::from), limit: 5, ..Default::default() }).unwrap();
        eprintln!("== {:?}: {} of {} clips", kind, p.total, p.all);
        for r in &p.rows {
            eprintln!("   {} | {} | {} {:?} {:?} {} bars {:?} x{}", r.name, r.folder, r.kind, r.key, r.bpm, r.bars, r.chords.iter().take(4).collect::<Vec<_>>(), r.projects);
        }
    }
}

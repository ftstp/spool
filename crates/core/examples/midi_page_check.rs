//! Opens a COPY of a data folder (migrating it) and times MIDI page queries: each source,
//! a filter, and the next pages ("200 more").
//!     cargo run --release -p core --example midi_page_check -- <data folder>
fn main() {
    use app_core::midi_view::MidiQuery;
    let dir = std::env::args().nth(1).expect("data folder");
    let t = std::time::Instant::now();
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    eprintln!("open (migrations included): {:.1} s", t.elapsed().as_secs_f64());
    let src = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
    for (label, q) in [
        ("clips", MidiQuery { source: src("clips"), limit: 200, ..Default::default() }),
        ("files", MidiQuery { source: src("files"), limit: 200, ..Default::default() }),
        ("all", MidiQuery { limit: 200, ..Default::default() }),
        ("all, page 3", MidiQuery { offset: 400, limit: 200, ..Default::default() }),
        ("files, page 50", MidiQuery { source: src("files"), offset: 10_000, limit: 200, ..Default::default() }),
        ("clips chords", MidiQuery { source: src("clips"), kind: Some("chords".into()), limit: 200, ..Default::default() }),
        ("chords in Am", MidiQuery { kind: Some("chords".into()), key: Some("Am".into()), limit: 200, ..Default::default() }),
        ("words", MidiQuery { q: "piano chord".into(), limit: 200, ..Default::default() }),
    ] {
        let t = std::time::Instant::now();
        let p = app.midi_page(q).expect("page");
        eprintln!("{label}: {} rows, {} of {} in {:.0} ms; first {:?}", p.rows.len(), p.total, p.all, t.elapsed().as_secs_f64() * 1000.0, p.rows.first().map(|r| &r.name));
    }
}

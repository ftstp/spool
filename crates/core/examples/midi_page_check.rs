//! Opens a COPY of a data folder (migrating it) and times MIDI page queries.
//!     cargo run --release -p core --example midi_page_check -- <data folder>
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let t = std::time::Instant::now();
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    eprintln!("open (migrations included): {:.1} s", t.elapsed().as_secs_f64());
    for (label, q) in [
        ("all", app_core::midi_view::MidiQuery { limit: 200, ..Default::default() }),
        ("chords in Am", app_core::midi_view::MidiQuery { kind: Some("chords".into()), key: Some("Am".into()), limit: 200, ..Default::default() }),
        ("Am at 174", app_core::midi_view::MidiQuery { key: Some("Am".into()), bpm: Some(174.0), limit: 200, ..Default::default() }),
        ("words", app_core::midi_view::MidiQuery { q: "piano chord".into(), limit: 200, ..Default::default() }),
    ] {
        let t = std::time::Instant::now();
        let p = app.midi_page(q).expect("page");
        eprintln!("{label}: {} of {} in {:.0} ms; {:?}", p.total, p.all, t.elapsed().as_secs_f64() * 1000.0, p.rows.first().map(|r| (&r.name, &r.key, &r.chords)));
    }
}

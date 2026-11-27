//! Where a song's key comes from, on a COPY of a data folder: what the sheet shows (value,
//! source, the other candidates) and the harmony read from its newest project.
//!     key_check <data folder copy> <song id>
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder (a copy)");
    let id: i64 = a.next().expect("song id").parse().unwrap();
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let d = app.song(id).expect("song").expect("no such song");
    eprintln!("key shown: {:?}", serde_json::to_string(&d.detected.key).unwrap());
    eprintln!("sound: {:?}", d.sound.as_ref().map(|s| serde_json::to_string(s).unwrap()));
    let rels: Vec<String> = app.songs().unwrap().iter().filter(|s| s.id == id).map(|s| s.rel.clone()).collect();
    for rel in rels.iter().filter(|r| !r.is_empty()) {
        match app.harmony(rel) {
            Ok(h) => eprintln!("harmony of {rel}: key {:?}, chords {:?}", h.key, h.bars.iter().take(16).map(|b| b.name.clone()).collect::<Vec<_>>()),
            Err(e) => eprintln!("harmony of {rel}: {e}"),
        }
    }
}

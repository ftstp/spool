//! releases_check <data dir COPY>: folder hints, a release made from the first, Ctrl+Z.
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let app = app_core::App::open(&dir).unwrap();
    let hints = app.release_hints().unwrap();
    for h in &hints {
        println!("hint {:7} {:30} {} titres  ({}) <- {}", h.kind, h.title, h.songs.len(), h.artist, h.folder);
    }
    let h = &hints[0];
    let r = app_core::releases::Release {
        id: String::new(), title: h.title.clone(), kind: h.kind.clone(), artist: h.artist.clone(), date: Some("2026-11-25".into()),
        cover_song: h.songs.first().copied(), notes: String::new(), songs: h.songs.iter().map(|s| Some(*s)).collect(),
        folder: Some(h.folder.clone()), created: 0.0,
    };
    let (id, st) = app.release_save(r).unwrap();
    println!("saved {id}: {:?}", st.label);
    let l = app.releases().unwrap();
    println!("list {} : {} songs resolved {}", l.len(), l[0].songs.len(), l[0].songs.iter().filter(|s| s.is_some()).count());
    println!("hints now {}", app.release_hints().unwrap().len());
    let (u, _) = app.undo().unwrap();
    println!("undo {:?} -> {} releases", u, app.releases().unwrap().len());
}

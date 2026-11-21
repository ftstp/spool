//! On a COPY of a data folder: songs with a style after opening (the migrations run), and
//! what the sheet detects for a few of them.
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let songs = app.songs().unwrap();
    let with = songs.iter().filter(|s| s.style.as_deref().is_some_and(|x| !x.is_empty())).count();
    eprintln!("{} songs, {} with a style", songs.len(), with);
    for s in songs.iter().filter(|s| s.dir.contains(" - electro - ") || s.dir.contains("DNB")).take(4) {
        let d = app.song(s.id).unwrap().unwrap();
        eprintln!("  {:?}: shown {:?} sheet {:?} src {:?}, detected {:?}", s.title, s.style, d.song.style, s.style_src, d.detected.style.shown.as_ref().map(|v| (v.value.clone(), v.src.clone())));
    }
}

//! A name made a featured artist, then taken back, on a COPY of a data folder: the songs it
//! named move to the main artist with the name in their featurings, then come back.
//!     feat_check <data folder copy> <name> [main artist]
fn tally(app: &app_core::App, name: &str) -> (usize, usize, Vec<String>) {
    let songs = app.songs().unwrap();
    let as_artist = songs.iter().filter(|s| s.artist.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(name))).count();
    let in_ft: Vec<String> = songs
        .iter()
        .filter(|s| s.ft.as_deref().is_some_and(|f| f.split(',').any(|x| x.trim().eq_ignore_ascii_case(name))))
        .map(|s| format!("{} / {} (ft {})", s.artist.clone().unwrap_or_default(), s.title.clone().unwrap_or_default(), s.ft.clone().unwrap_or_default()))
        .collect();
    (as_artist, in_ft.len(), in_ft)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder (a copy)");
    let name = a.next().expect("name");
    let main = a.next().unwrap_or_default();
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let (n0, f0, _) = tally(&app, &name);
    eprintln!("before: {n0} songs by {name}, {f0} featuring it; in the artists list: {}", app.config().artists.iter().any(|x| x.eq_ignore_ascii_case(&name)));
    app.artist_to_feat(&name, &main).expect("to feat");
    let (n1, f1, sample) = tally(&app, &name);
    eprintln!("as a featuring: {n1} songs by {name}, {f1} featuring it; in the list: {}; rule: {:?}", app.config().artists.iter().any(|x| x.eq_ignore_ascii_case(&name)), app.config().artist_feats);
    for s in sample.iter().take(4) {
        eprintln!("  {s}");
    }
    app.feat_back(&name).expect("back");
    let (n2, f2, _) = tally(&app, &name);
    eprintln!("taken back: {n2} songs by {name}, {f2} featuring it; in the list: {}", app.config().artists.iter().any(|x| x.eq_ignore_ascii_case(&name)));
}

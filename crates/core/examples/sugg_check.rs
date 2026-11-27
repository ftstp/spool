//! The names the Artists page offers to sort, on a COPY of a data folder, with why.
//!     sugg_check <data folder copy>
fn main() {
    let dir = std::env::args().nth(1).expect("data folder (a copy)");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    for g in app.artist_suggestions().unwrap() {
        eprintln!("{:<28} sure={} songs={} files={} folders={}", g.name, g.sure, g.songs, g.files, g.folders);
    }
    for c in app.artists().unwrap() {
        eprintln!("card {:<26} songs={} ft={}", c.name, c.songs, c.ft);
    }
}

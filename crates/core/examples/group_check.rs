//! group_check <data dir> <projects root> [artist]: a fresh base on that folder, then the songs
//! and the artist names the setup would offer.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let app = app_core::App::open(std::path::Path::new(&a[0])).unwrap();
    if app.config().roots.is_empty() {
        app.add_root(&a[1]).unwrap();
    }
    if let Some(ar) = a.get(2) {
        app.set_root_artist(&a[1], ar).unwrap();
    }
    app.scan(false).unwrap();
    for s in app.songs().unwrap() {
        let v = serde_json::to_value(&s).unwrap();
        println!("{:24} dir {:40} files {}", v["title"].as_str().unwrap_or(""), v["song_dir"].as_str().unwrap_or(""), v["n_files"]);
    }
    for g in app.artist_suggestions().unwrap() {
        println!("artist? {} sure {} files {} songs {}", g.name, g.sure, g.files, g.songs);
    }
}

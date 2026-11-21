//! On a data folder: what changed between the two newest saves of a few songs (read only).
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let mut n = 0;
    for s in app.songs().unwrap().iter().filter(|s| s.files >= 2) {
        let d = app.song(s.id).unwrap().unwrap();
        let mut f: Vec<_> = d.files.iter().map(|f| (f.mtime, f.rel.clone())).chain(d.versions.iter().map(|v| (v.mtime, v.rel.clone()))).collect();
        f.sort_by(|a, b| a.0.total_cmp(&b.0));
        if f.len() < 2 { continue; }
        let (a, b) = (&f[f.len() - 2].1, &f[f.len() - 1].1);
        match app.als_diff(a, b) {
            Ok(x) => eprintln!("{:?}: {} changes, +{} -{} tracks, {} changed, +{} -{} plugins, +{} -{} samples, same={}",
                s.title, x.changes.len(), x.tracks_added.len(), x.tracks_removed.len(), x.tracks_changed.len(), x.plugins_added.len(), x.plugins_removed.len(), x.samples_added.len(), x.samples_removed.len(), x.same),
            Err(e) => eprintln!("{:?}: ERR {e}", s.title),
        }
        if n == 4 { if let Ok(x) = app.als_diff(a, b) { for c in x.tracks_changed.iter().take(6) { eprintln!("   {:?}", c); } eprintln!("   added {:?} removed {:?}", x.tracks_added, x.tracks_removed); } }
        n += 1;
        if n >= 8 { break; }
    }
}

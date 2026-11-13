//! Two projects folders named "Projets" under "Kito Vale" and "NOVA" (a common layout),
//! each given its artist: every song gets it.  root_artist_check <project .als> <scratch>
fn main() {
    let mut a = std::env::args().skip(1);
    let (als, scratch) = (std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let _ = std::fs::remove_dir_all(&scratch);
    for (owner, songs) in [("Kito Vale", ["Danse", "Nuit"]), ("NOVA", ["312 Bbm_104", "327 Am_120"])] {
        for s in songs {
            let p = scratch.join(owner).join("Projets").join(format!("{s} Project"));
            std::fs::create_dir_all(&p).unwrap();
            std::fs::copy(&als, p.join(format!("{s}.als"))).unwrap();
        }
    }
    let app = app_core::App::open(&scratch.join("data")).unwrap();
    let r1 = scratch.join("Kito Vale").join("Projets");
    let r2 = scratch.join("NOVA").join("Projets");
    app.add_root(r1.to_str().unwrap()).unwrap();
    let cfg = app.add_root(r2.to_str().unwrap()).unwrap();
    println!("roots: {:?}", cfg.list.iter().map(|r| (&r.label, &r.path)).collect::<Vec<_>>());
    app.scan(false).unwrap();
    for r in &cfg.list {
        let who = if r.path.contains("Kito Vale") { "Kito Vale" } else { "NOVA" };
        app.set_root_artist(&r.path, who).unwrap();
    }
    for s in app.songs().unwrap() {
        println!("  {:14} artist {:?}  dir {:?}", s.title.unwrap_or_default(), s.artist, s.dir);
    }
}

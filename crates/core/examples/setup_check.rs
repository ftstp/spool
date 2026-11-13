//! The setup's pieces on a COPY of a base, in a scratch data dir: setup_check <base.sqlite> <scratch>
//! artist suggestions; two projects folders (the second one labelled) scanned with progress;
//! a project of the second folder resolves, opens and reveals by its name.
use std::sync::{Arc, Mutex};

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let data = scratch.join("data");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&data).unwrap();
    std::fs::copy(&src, data.join("plugin-fixer.sqlite")).unwrap();
    let app = app_core::App::open(&data).unwrap();
    // as after a fresh setup: no artists typed yet
    app.set_config("artists", serde_json::json!([])).unwrap();
    let sugg = app.artist_suggestions().unwrap();
    println!("artist suggestions ({}):", sugg.len());
    for g in sugg.iter().take(22) {
        println!("  {} {:24} songs {:4} files {:4} folders {:4}", if g.sure { "[x]" } else { "[ ]" }, g.name, g.songs, g.files, g.folders);
    }

    // a second folder: one real project copied into scratch
    let main = app.root().unwrap();
    let one = app.songs().unwrap().into_iter().find(|s| !s.dir.is_empty()).unwrap();
    let detail = app.song(one.id).unwrap().unwrap();
    let f = &detail.files[0];
    let src_als = std::path::Path::new(&main).join(&f.rel);
    let extra = scratch.join("Autre disque").join("Mon projet Project");
    std::fs::create_dir_all(&extra).unwrap();
    std::fs::copy(&src_als, extra.join("Mon projet.als")).unwrap();
    let cfg = app.add_root(scratch.join("Autre disque").to_str().unwrap()).unwrap();
    println!("roots: {:?}", cfg.list);
    assert!(app.add_root(&main).is_err(), "the same folder twice is refused");

    let stages = Arc::new(Mutex::new(Vec::<String>::new()));
    let s2 = Arc::clone(&stages);
    app.on_change(Arc::new(move |st| {
        let line = format!("{} {}/{}", st.stage, st.done, st.total);
        let mut v = s2.lock().unwrap();
        if v.last() != Some(&line) && !st.stage.is_empty() {
            v.push(line);
        }
    }));
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    let v = stages.lock().unwrap();
    println!("scan {:.1}s, {} progress events, stages:", t.elapsed().as_secs_f64(), v.len());
    let mut last = String::new();
    for l in v.iter() {
        let st: String = l.rsplitn(2, ' ').last().unwrap().to_string();
        if st != last {
            println!("  {l}");
            last = st;
        }
    }
    let label = &cfg.list[1].label;
    let rel = format!("{label}/Mon projet Project/Mon projet.als");
    let p = app.project_path(&rel).expect("resolves");
    println!("{rel} -> {}", p.display());
    let song = app.songs().unwrap().into_iter().find(|s| s.dir.starts_with(label.as_str())).expect("song from the second folder");
    println!("song from the 2nd folder: {:?} ({:?})", song.title, song.dir);
    let d = app.song(song.id).unwrap().unwrap();
    assert_eq!(d.files[0].rel, rel);
    println!("ok");
}

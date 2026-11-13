//! A real treatments pass, on a project COPIED into a scratch root (nothing written next to the
//! user's projects): scan, preview, write, then the copy read back by the .als reader and
//! scanned like any other.  atelier_check <project .als> <scratch dir>
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (PathBuf::from(a.next().unwrap()), PathBuf::from(a.next().unwrap()));
    let root = scratch.join("projets");
    let proj = als::project_root(&src);
    let dst_proj = root.join(proj.file_name().unwrap());
    let dst = dst_proj.join(src.strip_prefix(&proj).unwrap());
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    std::fs::copy(&src, &dst).unwrap();
    std::fs::create_dir_all(dst_proj.join("Ableton Project Info")).unwrap();

    let app = app_core::App::open(&scratch.join("data")).unwrap();
    app.set_root(root.to_str().unwrap()).unwrap();
    app.scan(false).unwrap();
    let song = app.songs().unwrap()[0].id;
    let mut preset = repair::studio::default_preset();
    preset["P"]["on"] = true.into();
    let t = std::time::Instant::now();
    let pv = app.atelier_preview(song, preset.clone()).unwrap();
    println!("preview in {:.0} ms: ok {}, out {}", t.elapsed().as_secs_f64() * 1000.0, pv["ok"], pv["out"]);
    for k in ["R", "P", "O", "C", "N", "M"] {
        if let Some(x) = pv.get(k) {
            println!("  {k}: {x}");
        }
    }
    let before: Vec<String> =
        pv["before"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap_or("").to_string()).collect();
    let after: Vec<String> =
        pv["after"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap_or("").to_string()).collect();
    println!("  before: {before:?}\n  after:  {after:?}");

    app.atelier_run(vec![song], preset).unwrap();
    while app.repair_state().running {
        std::thread::sleep(Duration::from_millis(200));
    }
    let r = &app.repair_state().results[0];
    println!("written: ok {}, {:?} {:?}", r.ok, r.out, r.error);
    let out = root.join(r.out.as_ref().unwrap());
    let info = als::read_als(&out, &als::Env::detect(None));
    assert!(info.error.is_none(), "{:?}", info.error);
    println!("the copy reads back: {} tracks, {} plugins", info.tracks.values().sum::<u32>(), info.plugins.len());
    assert!(dst.exists(), "the original is still there");
    let files = app.song(song).unwrap().unwrap().files;
    println!("after the scan the song has: {:?}", files.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
}

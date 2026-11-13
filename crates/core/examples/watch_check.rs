//! A project saved under the root is picked up within seconds, without waiting for the
//! periodic pass: watch_check <empty scratch dir>
use std::time::{Duration, Instant};

fn main() {
    let base = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let (data, root) = (base.join("data"), base.join("projets"));
    std::fs::create_dir_all(root.join("Morceau A Project")).unwrap();
    let app = app_core::App::open(&data).unwrap();
    app.set_root(root.to_str().unwrap()).unwrap();
    app.start_background(Duration::from_secs(3600)); // the periodic pass will not help here
    std::thread::sleep(Duration::from_secs(3));
    let n0 = app.stats().unwrap().projects.total;
    // a (broken) set saved by "Live": read as unreadable, but listed
    std::fs::write(root.join("Morceau A Project").join("Morceau A.als"), b"not gzip").unwrap();
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(250));
        let files = app.songs().unwrap().len();
        if files > 0 {
            println!("picked up after {:.1} s (projects before: {n0})", t.elapsed().as_secs_f64());
            return;
        }
    }
    eprintln!("NOT picked up within 20 s");
    std::process::exit(1);
}

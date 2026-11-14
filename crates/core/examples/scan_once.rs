//! One scan of a data folder (a COPY, never the user's own), timed, for checking a new reader
//! on a real catalogue: projects and libraries are only read.
//!     cargo run --release -p core --example scan_once -- <data folder>
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let t = std::time::Instant::now();
    let changed = app.scan(false).expect("scan");
    eprintln!("scan: {:.1} s, changed: {changed}", t.elapsed().as_secs_f64());
}

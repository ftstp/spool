//! What a page reload costs (the queries the interface asks after every change), on a COPY
//! of a data folder.
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    for round in 0..2 {
        let t = std::time::Instant::now();
        let n = app.songs().unwrap().len();
        let a = t.elapsed().as_millis();
        let t = std::time::Instant::now();
        app.overview().unwrap();
        let b = t.elapsed().as_millis();
        let t = std::time::Instant::now();
        app.stats().unwrap();
        let c = t.elapsed().as_millis();
        let t = std::time::Instant::now();
        let _ = app.home_stats();
        let d = t.elapsed().as_millis();
        eprintln!("round {round}: songs ({n}) {a} ms, overview {b} ms, stats {c} ms, home stats {d} ms");
    }
}

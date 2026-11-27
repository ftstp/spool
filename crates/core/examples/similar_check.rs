//! "Sounds alike" on a COPY of a data folder: a listened-to sample and one listened to on
//! demand, timed. Files are only read.
//!     SPOOL_MODEL_DIR=<model dir> cargo run --release -p core --example similar_check -- <data folder> <search word>
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder");
    let word = a.next().unwrap_or_else(|| "kick".into());
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    app.audio_enable(true).expect("enable");
    while !app.audio_state().ready {
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    let q = app_core::samples_view::SampleQuery { q: word, limit: 1, ..Default::default() };
    let first = app.samples(q).expect("samples").rows.into_iter().next().expect("a sample");
    for round in ["on demand", "listened to"] {
        let t = std::time::Instant::now();
        let s = app.similar_samples(&first.path, 8).expect("similar");
        eprintln!("{round}: {:.0} ms, compared with {} | {}", t.elapsed().as_secs_f64() * 1000.0, s.compared, s.of.name);
        for r in &s.rows {
            eprintln!("   {:.2}  {:?}  {}", r.sim, r.row.cat, r.row.name);
        }
    }
}

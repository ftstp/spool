//! Times Samples page queries on a COPY of a data folder.
//!     samples_page_check <data folder copy>
fn main() {
    use app_core::samples_view::SampleQuery;
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    for (label, q) in [
        ("all", SampleQuery { limit: 200, ..Default::default() }),
        ("vocal", SampleQuery { family: Some("vocal".into()), limit: 200, ..Default::default() }),
        ("vocal loops", SampleQuery { family: Some("vocal".into()), kind: Some("loop".into()), limit: 200, ..Default::default() }),
        ("bass page 5", SampleQuery { family: Some("bass".into()), offset: 800, limit: 200, ..Default::default() }),
        ("unsorted", SampleQuery { family: Some("none".into()), limit: 200, ..Default::default() }),
        ("words", SampleQuery { q: "kick dry".into(), limit: 200, ..Default::default() }),
    ] {
        let t = std::time::Instant::now();
        let p = app.samples(q).expect("page");
        eprintln!("{label}: {} rows of {} in {:.0} ms", p.rows.len(), p.total, t.elapsed().as_secs_f64() * 1000.0);
    }
}

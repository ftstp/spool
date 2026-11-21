//! The library's own model on a COPY of a data folder: unsorted samples before and after.
//!     learn_check <data folder copy>
fn main() {
    let dir = std::env::args().nth(1).expect("data folder (a copy)");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let count = || {
        let c = store::rusqlite::Connection::open(std::path::Path::new(&dir).join("plugin-fixer.sqlite")).unwrap();
        c.query_row("select count(*) from samples where family is null and a_family is null", [], |r| r.get::<_, i64>(0)).unwrap()
    };
    let before = count();
    let t = std::time::Instant::now();
    let n = app.sort_samples_by_library().expect("learn");
    println!("unsorted before {before}, sorted now {n} in {:.1} s, unsorted after {}", t.elapsed().as_secs_f32(), count());
    let c = store::rusqlite::Connection::open(std::path::Path::new(&dir).join("plugin-fixer.sqlite")).unwrap();
    let mut q = c.prepare("select a_family, coalesce(a_cat,'-'), count(*) from samples where family is null and a_family is not null and a_conf >= 0.9 group by 1,2 order by 3 desc limit 14").unwrap();
    for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))).unwrap().flatten() { println!("  {:>5} {}/{}", r.2, r.0, r.1); }
    let mut q = c.prepare("select name, a_family, coalesce(a_cat,'-') from samples where family is null and a_family is not null order by random() limit 12").unwrap();
    for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))).unwrap().flatten() { println!("  e.g. {:<55} -> {}/{}", r.0, r.1, r.2); }
}

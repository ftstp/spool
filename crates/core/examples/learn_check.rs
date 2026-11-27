//! The library's own model and the folders on a COPY of a data folder: unsorted samples before
//! and after, by where their sort comes from.
//!     learn_check <data folder copy>
fn main() {
    let dir = std::env::args().nth(1).expect("data folder (a copy)");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let db = std::path::Path::new(&dir).join("plugin-fixer.sqlite");
    let count = || {
        let c = store::rusqlite::Connection::open(&db).unwrap();
        c.query_row("select count(*) from samples where family is null and a_family is null and u_family is null", [], |r| r.get::<_, i64>(0)).unwrap()
    };
    let before = count();
    let t = std::time::Instant::now();
    let n = app.sort_samples_by_library().expect("learn");
    println!("unsorted before {before}, sorted now {n} in {:.1} s, unsorted after {}", t.elapsed().as_secs_f32(), count());
    let c = store::rusqlite::Connection::open(&db).unwrap();
    let mut q = c.prepare("select coalesce(a_src,'-'), a_family, coalesce(a_cat,'-'), count(*) from samples where family is null and a_family is not null group by 1,2,3 order by 4 desc limit 20").unwrap();
    for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))).unwrap().flatten() {
        println!("  {:>5} {:<6} {}/{}", r.3, r.0, r.1, r.2);
    }
    let g: (i64, f64) = c.query_row("select count(*), coalesce(avg(g_conf),0) from samples where g_family is not null", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    println!("guesses for the quick sort: {} (mean {:.2})", g.0, g.1);
    let apart: (i64, i64) = c.query_row("select sum(dur > 60), sum(coalesce(dur,0) <= 60 and a_error is not null) from samples where family is null and a_family is null", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    println!("left: long {} unreadable {}", apart.0, apart.1);
    let mut q = c.prepare("select name, folder, a_family, coalesce(a_cat,'-') from samples where a_src='folder' order by random() limit 12").unwrap();
    for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))).unwrap().flatten() {
        println!("  folder e.g. {:<40} [{}] -> {}/{}", r.0, r.1.chars().rev().take(45).collect::<String>().chars().rev().collect::<String>(), r.2, r.3);
    }
}

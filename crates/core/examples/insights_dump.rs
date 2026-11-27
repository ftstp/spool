//! The home statistics, a history point and the Progression data of a base, as JSON:
//! insights_dump <base.sqlite> <old_drive_samples.json>
fn main() {
    let mut a = std::env::args().skip(1);
    let st = store::Store::open(std::path::Path::new(&a.next().unwrap())).unwrap();
    let deep = std::path::PathBuf::from(a.next().unwrap_or_default());
    let rv = app_core::repair_view::repair_view(&st, None, &deep).unwrap();
    let songs = app_core::views::songs_list(&st.con).unwrap();
    let t = std::time::Instant::now();
    let home = app_core::insights::home_stats(&st.con, &songs, &rv).unwrap();
    eprintln!("home stats in {} ms", t.elapsed().as_millis());
    let out = serde_json::json!({
        "home": home,
        "summary": app_core::insights::summary(&st.con).unwrap(),
        "progress": app_core::insights::progress(&st.con, &rv).unwrap(),
    });
    println!("{out}");
}

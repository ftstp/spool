//! The views of a base as JSON (for comparisons): core-dump <base.sqlite> [--detail]
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let db = args.next().expect("usage: core-dump <base.sqlite> [--detail]");
    let mode = args.next();
    let detail = mode.as_deref() == Some("--detail");
    let st = store::Store::open(std::path::Path::new(&db)).expect("open base");
    let t = Instant::now();
    if mode.as_deref() == Some("--repair") {
        // the Repair page, for every period: core-dump <base> --repair <old_drive_samples.json>
        let deep = std::path::PathBuf::from(args.next().unwrap_or_default());
        let all = app_core::repair_view::repair_view(&st, None, &deep).expect("repair");
        let two = app_core::repair_view::repair_view(&st, Some(2), &deep).expect("repair");
        eprintln!("repair views in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        println!("{}", serde_json::json!({"all": all, "two": two}));
        return;
    }
    if detail {
        // every song's sheet, by id
        let ids: Vec<i64> = {
            let mut s = st.con.prepare("select id from songs where gone=0 order by id").unwrap();
            s.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        let mut out = serde_json::Map::new();
        for id in &ids {
            let d = app_core::detail::song_detail(&st.con, *id).expect("detail");
            out.insert(id.to_string(), serde_json::to_value(d).unwrap());
        }
        eprintln!("{} sheets in {:.0} ms", ids.len(), t.elapsed().as_secs_f64() * 1000.0);
        println!("{}", serde_json::Value::Object(out));
        return;
    }
    let songs = app_core::views::songs_list(&st.con).expect("songs");
    let overview = app_core::views::overview(&st.con).expect("overview");
    let ignored: Vec<String> = st.get("ignored").unwrap_or_default();
    let stats = app_core::views::stats(&st.con, &ignored).expect("stats");
    let fixable = app_core::repairs::fixable(&st).expect("fixable");
    let deep = std::path::Path::new(&db).with_file_name("old_drive_samples.json");
    let repair = app_core::repair_view::repair_view(&st, None, &deep).expect("repair");
    let home = app_core::insights::home_stats(&st.con, &songs, &repair).expect("home");
    let progress = app_core::insights::progress(&st.con, &repair).expect("progress");
    eprintln!("views in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    println!(
        "{}",
        serde_json::json!({"songs": songs, "overview": overview, "stats": stats, "fixable": fixable, "repair": repair,
            "home": home, "progress": progress})
    );
}

//! dbcheck <base.sqlite>: SQLite's own check, with the app's SQLite (run it on a COPY).
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let c = store::rusqlite::Connection::open(&p).unwrap();
    let v: String = c.query_row("select sqlite_version()", [], |r| r.get(0)).unwrap();
    let mut s = c.prepare("pragma integrity_check").unwrap();
    let rows: Vec<String> = s.query_map([], |r| r.get(0)).unwrap().flatten().take(10).collect();
    println!("sqlite {v}: {rows:?}");
}

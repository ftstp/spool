//! style_guess_check <sqlite COPY>: times the style guess of a few songs, noted styles included.
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let con = store::rusqlite::Connection::open(p).unwrap();
    let ids: Vec<(i64, Option<String>, Option<String>)> = {
        let mut s = con.prepare("select id, title, style from songs where gone=0 and style is not null and style != '' limit 6").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    };
    for (id, title, style) in ids {
        let t = std::time::Instant::now();
        let g = index::style_guess(&con, id).unwrap();
        println!("{} ms  {:?} noted {:?} -> {:?}", t.elapsed().as_millis(), title, style, g.iter().map(|o| (&o.style, o.pct, o.why.join(" | "))).collect::<Vec<_>>());
    }
}

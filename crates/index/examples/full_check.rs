//! A random sample of what the name rules call a full loop, on a COPY of a base.
//!     full_check <base copy.sqlite>
fn main() {
    let con = store::rusqlite::Connection::open(std::env::args().nth(1).expect("base copy")).unwrap();
    let mut q = con.prepare("select name, folder from samples where family is null order by random()").unwrap();
    let mut n = 0;
    for (name, folder) in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()))).unwrap().flatten() {
        let parts: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
        if index::samples::read_name(&name, &parts).family.as_deref() == Some("fullloop") {
            println!("{name:<50} | {}", parts.iter().rev().take(2).rev().cloned().collect::<Vec<_>>().join("/"));
            n += 1;
            if n == 30 { break; }
        }
    }
}

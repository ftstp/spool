//! What the name rules sort now, on a COPY of a base: the samples that have no family yet
//! (neither from the name nor from the audio), read again with the current rules.
//!     sample_gain <base copy.sqlite>
use std::collections::HashMap;

fn main() {
    let db = std::env::args().nth(1).expect("base copy");
    let con = store::rusqlite::Connection::open(db).unwrap();
    let mut q = con.prepare("select name, folder from samples where family is null and a_family is null").unwrap();
    let rows: Vec<(String, String)> = q.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()))).unwrap().flatten().collect();
    let mut fam: HashMap<String, i64> = HashMap::new();
    let mut ex: HashMap<String, Vec<String>> = HashMap::new();
    let mut got = 0;
    for (name, folder) in &rows {
        let stem = name.rsplit_once('.').map(|x| x.0).unwrap_or(name);
        let folders: Vec<&str> = folder.split(|c: char| c == '/' || c == std::path::MAIN_SEPARATOR).filter(|s| !s.is_empty()).collect();
        let n = index::samples::read_name(stem, &folders);
        if let Some(f) = n.family {
            got += 1;
            let k = format!("{f}/{}", n.cat.unwrap_or_default());
            *fam.entry(k.clone()).or_default() += 1;
            let e = ex.entry(k).or_default();
            if e.len() < 3 { e.push(stem.to_string()); }
        }
    }
    println!("unsorted {}: sorted by the names now {got}", rows.len());
    let mut v: Vec<_> = fam.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in v { println!("  {n:>5} {k:<22} e.g. {:?}", ex[&k]); }
}

//! Why samples stay unsorted by their names, on a COPY of a base: the ones whose name and
//! folders say no category (their most common words), and the ones that say several (which).
//!     sample_names <base copy.sqlite>
use std::collections::HashMap;

fn main() {
    let db = std::env::args().nth(1).expect("base copy");
    let con = store::rusqlite::Connection::open(db).unwrap();
    let mut q = con.prepare("select name, folder from samples where family is null").unwrap();
    let rows: Vec<(String, String)> = q.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()))).unwrap().flatten().collect();
    let (mut none, mut multi) = (0, 0);
    let mut pairs: HashMap<String, i64> = HashMap::new();
    let mut words: HashMap<String, i64> = HashMap::new();
    let mut ex: HashMap<String, Vec<String>> = HashMap::new();
    for (name, folder) in &rows {
        let stem = name.rsplit_once('.').map(|x| x.0).unwrap_or(name);
        let found = index::samples::cats_in(stem);
        if found.len() > 1 {
            multi += 1;
            let k = found.iter().map(|x| x.0).collect::<Vec<_>>().join("+");
            *pairs.entry(k.clone()).or_default() += 1;
            let e = ex.entry(k).or_default();
            if e.len() < 2 { e.push(stem.to_string()); }
        } else if found.is_empty() {
            none += 1;
            let t: String = stem.chars().map(|c| if c.is_alphabetic() { c.to_ascii_lowercase() } else { ' ' }).collect();
            for w in t.split_whitespace().filter(|w| w.len() >= 3) {
                *words.entry(w.to_string()).or_default() += 1;
            }
            let _ = folder;
        }
    }
    println!("unsorted {}: no category in the name {none}, several {multi}", rows.len());
    let mut p: Vec<_> = pairs.into_iter().collect();
    p.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in p.iter().take(25) { println!("  {n:>5} {k}  e.g. {:?}", ex[k]); }
    let mut w: Vec<_> = words.into_iter().collect();
    w.sort_by(|a, b| b.1.cmp(&a.1));
    println!("words in names with no category:");
    println!("  {}", w.iter().take(120).map(|(k, n)| format!("{k}:{n}")).collect::<Vec<_>>().join(" "));
}

//! What the current name rules change on a COPY of a base read with older ones: samples newly
//! sorted, sorted otherwise, no longer sorted, with examples of each.
//!     name_diff <base copy.sqlite>
use std::collections::HashMap;

fn main() {
    let db = std::env::args().nth(1).expect("base copy");
    let con = store::rusqlite::Connection::open(db).unwrap();
    let mut q = con.prepare("select name, folder, family, cat from samples").unwrap();
    let rows: Vec<(String, String, Option<String>, Option<String>)> = q
        .query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get(2)?, r.get(3)?)))
        .unwrap()
        .flatten()
        .collect();
    let mut kinds: HashMap<String, Vec<String>> = HashMap::new();
    for (name, folder, fam, cat) in &rows {
        let parts: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
        let n = index::samples::read_name(name, &parts);
        let key = match (fam, &n.family) {
            (None, Some(f)) => format!("new {f}/{}", n.cat.as_deref().unwrap_or("-")),
            (Some(_), None) => format!("lost {}/{}", fam.as_deref().unwrap_or("-"), cat.as_deref().unwrap_or("-")),
            (Some(a), Some(b)) if a != b || cat != &n.cat => {
                format!("moved {a}/{} -> {b}/{}", cat.as_deref().unwrap_or("-"), n.cat.as_deref().unwrap_or("-"))
            }
            _ => continue,
        };
        kinds.entry(key).or_default().push(name.clone());
    }
    let mut v: Vec<_> = kinds.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.len()));
    let total = |p: &str| v.iter().filter(|x| x.0.starts_with(p)).map(|x| x.1.len()).sum::<usize>();
    println!("samples {}: new {}, moved {}, lost {}", rows.len(), total("new"), total("moved"), total("lost"));
    for (k, names) in v.iter().take(40) {
        let step = (names.len() / 3).max(1);
        let ex: Vec<&str> = names.iter().step_by(step).take(3).map(|s| s.as_str()).collect();
        println!("{:>6}  {k}   e.g. {}", names.len(), ex.join(" | "));
    }
}

//! How many tracks change role with names matched as whole words (the app) instead of the
//! prototype's patterns: roles_diff <root> <files.json>
use repair::studio::{RoleMatch, roles_of, track_name};

fn main() {
    let mut a = std::env::args().skip(1);
    let root = a.next().unwrap();
    let rels: Vec<String> = serde_json::from_str(&std::fs::read_to_string(a.next().unwrap()).unwrap()).unwrap();
    let (mut tracks, mut changed, mut ex) = (0, 0, Vec::<String>::new());
    for rel in rels {
        let Ok(set) = repair::xml::read_set(&std::path::Path::new(&root).join(&rel)) else { continue };
        let Some(ts) = set.root.child("LiveSet").and_then(|l| l.child("Tracks")) else { continue };
        let (p, w) = (roles_of(&set.root, RoleMatch::Prototype), roles_of(&set.root, RoleMatch::Starts));
        for t in &ts.children {
            let id = t.get("Id").unwrap_or("").to_string();
            tracks += 1;
            if p.get(&id) != w.get(&id) {
                changed += 1;
                ex.push(format!("{} -> {} : {}", p[&id], w[&id], track_name(t).to_lowercase()));
            }
        }
    }
    println!("tracks {tracks}, role changed {changed}");
    let mut c: std::collections::BTreeMap<String, usize> = Default::default();
    for e in ex {
        *c.entry(e).or_default() += 1;
    }
    let mut v: Vec<_> = c.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (e, n) in v.iter().take(70) {
        println!("  {n:4}  {e}");
    }
}

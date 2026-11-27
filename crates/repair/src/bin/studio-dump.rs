//! For comparisons with the prototype: every set given, treated in memory with a preset, as
//! {rel: {rep, len, sha256}}.  studio-dump <root> <preset.json> <files.json>
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::path::Path;

fn main() {
    let mut a = std::env::args().skip(1);
    let root = a.next().expect("root");
    let preset: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(a.next().expect("preset")).unwrap()).unwrap();
    let rels: Vec<String> = serde_json::from_str(&std::fs::read_to_string(a.next().expect("files")).unwrap()).unwrap();
    let t = std::time::Instant::now();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap();
    let out: std::collections::BTreeMap<String, serde_json::Value> = pool.install(|| {
        rels.par_iter()
            .filter_map(|rel| {
                let mut set = repair::xml::read_set(&Path::new(&root).join(rel)).ok()?;
                set.root.child("LiveSet")?.child("Tracks")?;
                let rep = repair::studio::treat(&mut set.root, &preset, repair::studio::RoleMatch::Prototype);
                let b = repair::xml::set_bytes(&set);
                Some((
                    rel.clone(),
                    serde_json::json!({"rep": rep, "len": b.len(), "sha": format!("{:x}", Sha256::digest(&b))}),
                ))
            })
            .collect()
    });
    eprintln!("{} sets in {:.1} s", out.len(), t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&out).unwrap());
}

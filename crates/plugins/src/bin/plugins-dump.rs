//! Classifies plugin references read from stdin, one JSON line per project:
//! in  {"rel": ..., "plugins": [refs]}
//! out {"rel": ..., "states": [describe + "kind"]}
//! Usage: plugins-dump [old drive letter] < refs.jsonl
use plugins::{Matcher, Ref};
use std::io::{BufRead, Write};
use std::time::Instant;

fn main() {
    let old = std::env::args().nth(1);
    let t = Instant::now();
    let m = Matcher::load(old.as_deref());
    eprintln!("{} plugins installed, loaded in {:.0} ms", m.installed.len(), t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let mut out = std::io::stdout().lock();
    let mut n = 0;
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let v: serde_json::Value = serde_json::from_str(&line).expect("json line");
        let refs: Vec<Ref> = serde_json::from_value(v["plugins"].clone()).unwrap_or_default();
        let states: Vec<serde_json::Value> = refs
            .iter()
            .map(|r| {
                let mut d = serde_json::to_value(m.describe(r)).unwrap();
                d["kind"] = m.kind(r).into();
                d
            })
            .collect();
        writeln!(out, "{}", serde_json::json!({"rel": v["rel"], "states": states})).unwrap();
        n += 1;
    }
    eprintln!("{n} projects classified in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
}

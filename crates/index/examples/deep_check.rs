//! The old drive search on a base (read only on the old drive), written to a scratch file and
//! compared with an earlier search: deep_check <base.sqlite> <out.json> <earlier.json>
fn main() {
    let mut a = std::env::args().skip(1);
    let st = store::Store::open(std::path::Path::new(&a.next().unwrap())).unwrap();
    let out = std::path::PathBuf::from(a.next().unwrap());
    let earlier: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(a.next().unwrap()).unwrap()).unwrap();
    let t = std::time::Instant::now();
    let (root, n, found) = index::deep_search(&st, &out, &mut |_| {}).unwrap();
    println!("{root}: {n} files read, {found} names found, {:.1} s", t.elapsed().as_secs_f64());
    let now: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let names = |v: &serde_json::Value| v["hits"].as_object().map(|m| m.keys().cloned().collect::<std::collections::BTreeSet<_>>()).unwrap_or_default();
    let (a, b) = (names(&earlier), names(&now));
    println!("earlier: root {} files {} names {}", earlier["root"], earlier["files"], a.len());
    println!("names only earlier: {}, only now: {}", a.difference(&b).count(), b.difference(&a).count());
    for x in a.difference(&b).take(5) { println!("  - {x}"); }
}

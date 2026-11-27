//! The projects' clips analysed again from their kept notes, on a COPY of a base: how many
//! change kind, and between which.
//!     clip_kinds <base copy.sqlite>
fn main() {
    let st = std::sync::Mutex::new(store::Store::open(std::path::Path::new(&std::env::args().nth(1).expect("base copy"))).unwrap());
    let kinds = |st: &std::sync::Mutex<store::Store>| -> std::collections::HashMap<String, String> {
        let s = st.lock().unwrap();
        let mut q = s.con.prepare("select rel, coalesce(kind, '') from midi where clip = 1").unwrap();
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().flatten().collect()
    };
    let before = kinds(&st);
    let t = std::time::Instant::now();
    let n = index::clips::reanalyse(&st).unwrap();
    let after = kinds(&st);
    let mut moves: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for (rel, k) in &after {
        if let Some(b) = before.get(rel).filter(|b| *b != k) {
            *moves.entry((b.clone(), k.clone())).or_default() += 1;
        }
    }
    println!("{n} of {} clips changed kind in {:.1} s", after.len(), t.elapsed().as_secs_f32());
    for ((a, b), c) in moves.iter().filter(|x| *x.1 >= 5) {
        println!("  {c:>5} {a} -> {b}");
    }
}

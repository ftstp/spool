//! For comparisons with the prototype: every active project with something to repair,
//! repaired in memory (samples planned, never copied), as
//! {rel: {plan, done, len, sha256}}.  repair-dump <base.sqlite> <templates dir>
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn main() {
    let mut a = std::env::args().skip(1);
    let (db, tpl) = (a.next().expect("base"), PathBuf::from(a.next().expect("templates dir")));
    let st = store::Store::open(Path::new(&db)).unwrap();
    let root: String = st.get("root").expect("root");
    let ch = app_core::repairs::choices(&st);
    let rows: Vec<(String, String, String)> = {
        let mut s = st.con.prepare("select rel, data, plugstate from files order by rel").unwrap();
        s.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    let t = std::time::Instant::now();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap();
    let out: std::collections::BTreeMap<String, serde_json::Value> = pool.install(|| {
        rows.par_iter()
            .filter_map(|(rel, data, states)| {
                let data: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
                let states: serde_json::Value = serde_json::from_str(states).unwrap_or_default();
                let p = repair::fix::plan(rel, &data, &states, &ch);
                if !p.useful {
                    return None;
                }
                let src = Path::new(&root).join(rel);
                let v = match repair::xml::read_set(&src) {
                    Ok(mut set) => match repair::fix::repair_tree(&p, &mut set.root, &src, &tpl, &mut repair::fix::DryRun) {
                        Ok(done) => {
                            let b = repair::xml::set_bytes(&set);
                            serde_json::json!({"plan": p, "done": done, "len": b.len(), "sha": format!("{:x}", Sha256::digest(&b))})
                        }
                        Err(e) => serde_json::json!({"plan": p, "error": e}),
                    },
                    Err(e) => serde_json::json!({"plan": p, "error": e}),
                };
                Some((rel.clone(), v))
            })
            .collect()
    });
    eprintln!("{} repairs in {:.1} s", out.len(), t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&out).unwrap());
}

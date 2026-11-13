//! For comparisons with the prototype's writer: every .als under a folder, parsed and written
//! back unchanged, as {rel: [length, sha256]}.  xml-roundtrip <root>
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("als")) {
            out.push(p);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("usage: xml-roundtrip <root>"));
    let mut files = Vec::new();
    walk(&root, &mut files);
    let t = std::time::Instant::now();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(16).build().unwrap();
    let out: std::collections::BTreeMap<String, serde_json::Value> = pool.install(|| {
        files
            .par_iter()
            .map(|p| {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                let v = match repair::xml::read_set(p) {
                    Ok(set) => {
                        let b = repair::xml::set_bytes(&set);
                        serde_json::json!([b.len(), format!("{:x}", Sha256::digest(&b))])
                    }
                    Err(e) => serde_json::json!(["error", e]),
                };
                (rel, v)
            })
            .collect()
    });
    eprintln!("{} sets in {:.1} s", out.len(), t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&out).unwrap());
}

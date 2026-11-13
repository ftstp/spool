//! Reads many sets in parallel and prints one JSON line per set: {"rel": ..., "out": {...}}.
//!
//! Usage: als-dump <projects root> [old drive letter] < list-of-relative-paths.txt
//! Used to compare the Rust reader with the Python prototype on a real corpus.
use als::{Env, read_als};
use rayon::prelude::*;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("usage: als-dump <root> [old drive] < list"));
    let env = Env::detect(args.next());
    // past ~16 threads the big sets (100 MB of XML each) fight over memory bandwidth
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().ok();
    let rels: Vec<String> =
        std::io::stdin().lock().lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).collect();
    let t = Instant::now();
    let outs: Vec<String> = rels
        .par_iter()
        .map(|rel| {
            let t = Instant::now();
            let info = read_als(&root.join(rel), &env);
            serde_json::json!({"rel": rel, "ms": t.elapsed().as_millis() as u64, "out": info}).to_string()
        })
        .collect();
    let elapsed = t.elapsed();
    let mut stdout = std::io::stdout().lock();
    for line in outs {
        writeln!(stdout, "{line}").unwrap();
    }
    eprintln!("{} sets read in {:.2} s", rels.len(), elapsed.as_secs_f64());
}

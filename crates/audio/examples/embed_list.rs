//! The analysis of every file of a list (first column: a path), one JSON line each, for
//! measuring what the audio says about samples. Files are only read.
//!     cargo run --release -p audio --example embed_list -- <model dir> <list.tsv> <out.jsonl>
use std::io::{BufRead, Write};

fn main() {
    let mut a = std::env::args().skip(1);
    let (model, list, out) = (a.next().expect("model"), a.next().expect("list"), a.next().expect("out"));
    let an = audio::Analyzer::new(&model).expect("model");
    let mut w = std::io::BufWriter::new(std::fs::File::create(out).unwrap());
    let t = std::time::Instant::now();
    let lines: Vec<String> = std::io::BufReader::new(std::fs::File::open(list).unwrap()).lines().map(|l| l.unwrap()).collect();
    for (i, line) in lines.iter().enumerate() {
        let p = line.split('\t').next().unwrap();
        match an.analyze(p) {
            Ok(r) => writeln!(w, "{}", serde_json::json!({"path": p, "emb": r.embedding, "dur": r.duration_s, "bpm": r.bpm, "key": r.key})).unwrap(),
            Err(e) => writeln!(w, "{}", serde_json::json!({"path": p, "err": e.to_string()})).unwrap(),
        }
        if i % 100 == 0 {
            eprintln!("{i}/{} {:.0} s", lines.len(), t.elapsed().as_secs_f64());
        }
    }
}

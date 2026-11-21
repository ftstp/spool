//! Groups files into songs. stdin: {"files": [{rel, size, mtime}], "artists": [...],
//! "work_words": [...], "merges": {"<dir>|<key>": [dir, key] | null}}; stdout: the grouping.
use catalog::{FileIn, Rules, group};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::time::Instant;

#[derive(Deserialize)]
struct Input {
    files: Vec<FileIn>,
    artists: Vec<String>,
    work_words: Vec<String>,
    merges: HashMap<String, Option<(String, String)>>,
}

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let inp: Input = serde_json::from_str(&s).expect("input json");
    let t = Instant::now();
    let rules = Rules::new(&inp.artists, &inp.work_words);
    let g = group(&inp.files, &rules, &inp.merges);
    eprintln!("{} files -> {} songs in {:.0} ms", g.files.len(), g.songs.len(), t.elapsed().as_secs_f64() * 1000.0);
    println!("{}", serde_json::to_string(&g).unwrap());
}

//! Style options. stdin: {"artists", "work_words", "songs": [StyleSong]}; stdout: [[id, options]].
use catalog::{Rules, StyleSong, deduce};
use serde::Deserialize;
use std::io::Read;
use std::time::Instant;

#[derive(Deserialize)]
struct Input {
    artists: Vec<String>,
    work_words: Vec<String>,
    songs: Vec<StyleSong>,
}

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let inp: Input = serde_json::from_str(&s).expect("input json");
    let t = Instant::now();
    let out = deduce(&inp.songs, &Rules::new(&inp.artists, &inp.work_words));
    eprintln!("{} songs, {} with options, in {:.0} ms", inp.songs.len(), out.len(), t.elapsed().as_secs_f64() * 1000.0);
    println!("{}", serde_json::to_string(&out).unwrap());
}

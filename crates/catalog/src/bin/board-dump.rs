//! Card picture and automatic checklist per song. stdin: {"root", "artists", "work_words",
//! "song_dirs", "songs": [{id, song_dir, vkey, title, artist, stage, shared, newest_mtime,
//! sev, length_s, temoins: [[rel, kind, mtime]]}]}; stdout: [{id, checkauto, cover}].
use catalog::{CheckInput, Rules, auto_checks, card_picture, list_board, other_song_prefixes};
use rayon::prelude::*;
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Deserialize)]
struct Song {
    id: i64,
    song_dir: String,
    vkey: String,
    title: Option<String>,
    artist: Option<String>,
    stage: Option<String>,
    shared: bool,
    newest_mtime: f64,
    sev: Option<i64>,
    length_s: Option<i64>,
    temoins: Vec<(String, String, f64)>,
}

#[derive(Deserialize)]
struct Input {
    root: PathBuf,
    artists: Vec<String>,
    work_words: Vec<String>,
    song_dirs: Vec<String>,
    songs: Vec<Song>,
}

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let inp: Input = serde_json::from_str(&s).expect("input json");
    let t = Instant::now();
    let rules = Rules::new(&inp.artists, &inp.work_words);
    let out: Vec<serde_json::Value> = inp
        .songs
        .par_iter()
        .map(|s| {
            let others = other_song_prefixes(&inp.song_dirs, &s.song_dir);
            let b = list_board(
                &inp.root,
                &rules,
                &s.song_dir,
                &s.vkey,
                s.title.as_deref().unwrap_or(""),
                s.shared,
                &others,
            );
            let c = CheckInput {
                stage: s.stage.as_deref(),
                title: s.title.as_deref(),
                artist: s.artist.as_deref(),
                temoins: s.temoins.iter().map(|t| (t.0.as_str(), t.1.as_str(), t.2)).collect(),
                newest_mtime: s.newest_mtime,
                sev: s.sev,
                length_s: s.length_s,
            };
            let checks: serde_json::Map<String, serde_json::Value> =
                auto_checks(&c, &b).into_iter().map(|(k, d, w)| (k.to_string(), serde_json::json!([d, w]))).collect();
            serde_json::json!({"id": s.id, "checkauto": checks, "cover": card_picture(&b)})
        })
        .collect();
    eprintln!("{} songs in {:.0} ms", out.len(), t.elapsed().as_secs_f64() * 1000.0);
    println!("{}", serde_json::to_string(&out).unwrap());
}

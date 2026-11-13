//! Reference-render candidates and stage evidence per song. stdin: {"artists", "work_words",
//! "audio": [{rel, size, mtime}], "songs": [{id, song_dir, vkey, title, shared,
//! newest_mtime, newest}]}; stdout: [{id, cands, reasons, best, sig}].
use catalog::{Audio, Newest, Rules, best_stage, signature, song_dir_of, stage_evidence, temoin_candidates};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::time::Instant;

#[derive(Deserialize)]
struct Song {
    id: i64,
    song_dir: String,
    vkey: String,
    title: Option<String>,
    shared: bool,
    newest_mtime: f64,
    newest: Option<Newest>,
}

#[derive(Deserialize)]
struct Input {
    artists: Vec<String>,
    work_words: Vec<String>,
    audio: Vec<Audio>,
    songs: Vec<Song>,
}

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let inp: Input = serde_json::from_str(&s).expect("input json");
    let t = Instant::now();
    let rules = Rules::new(&inp.artists, &inp.work_words);
    // audio grouped by the song folder it sits in; "- MP3" renders go to every song
    let mut by_dir: HashMap<String, Vec<(Audio, String)>> = HashMap::new();
    let mut mp3: Vec<(Audio, String)> = Vec::new();
    for a in inp.audio {
        if a.rel.starts_with("- MP3/") {
            mp3.push((a, String::new()));
        } else {
            let d = song_dir_of(&a.rel, &rules);
            by_dir.entry(d.clone()).or_default().push((a, d));
        }
    }
    let out: Vec<serde_json::Value> = inp
        .songs
        .iter()
        .map(|s| {
            let mut audio = by_dir.get(&s.song_dir).cloned().unwrap_or_default();
            audio.extend(mp3.iter().cloned());
            let cands = temoin_candidates(
                &rules,
                &s.song_dir,
                &s.vkey,
                s.title.as_deref().unwrap_or(""),
                &audio,
                s.newest_mtime,
                s.shared,
            );
            let reasons = stage_evidence(&s.song_dir, s.newest.as_ref(), &cands);
            let best = best_stage(&reasons);
            serde_json::json!({"id": s.id, "cands": cands, "reasons": reasons, "best": best,
                               "sig": best.map(|b| signature(b, &reasons))})
        })
        .collect();
    eprintln!("{} songs in {:.0} ms", out.len(), t.elapsed().as_secs_f64() * 1000.0);
    println!("{}", serde_json::to_string(&out).unwrap());
}

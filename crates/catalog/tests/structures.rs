//! The grouping tried on real ways of keeping projects: every `structures/*.json` (a folder
//! tree of our own projects, names only, written by hand or from our own base) is grouped
//! and the songs it makes compared with `<name>.expected.txt` beside it. Never anyone
//! else's: the app collects nothing. A change of the naming rules
//! that changes someone's songs fails here, and the diff says whose and how.
//!
//! A new tree: drop its .json in `structures/`, run `BLESS=1 cargo test -p catalog --test
//! structures`, read the .expected.txt it writes (the songs as the app would show them),
//! fix what is wrong, keep the file. The folder stays out of the public mirror.
use catalog::{FileIn, OneSong, Rules, group_opts};
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn summary(json: &serde_json::Value) -> String {
    let st = &json["settings"];
    let list = |k: &str| -> Vec<String> { st[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect() };
    let rules = Rules::new(&list("artists"), &list("work_words"));
    let files: Vec<FileIn> = json["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some(FileIn { rel: f[0].as_str()?.to_string(), size: 1, mtime: f[1].as_f64().unwrap_or(0.0) }))
        .collect();
    let one = OneSong {
        all: st["group_mode"].as_str() == Some("folder"),
        dirs: list("one_song_dirs").into_iter().collect(),
        title_from_folder: match st["title_from"].as_str() {
            Some("folder") => Some(true),
            Some("file") => Some(false),
            _ => None,
        },
    };
    let merges: HashMap<String, Option<(String, String)>> = st["merges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| Some((format!("{}|{}", m[0].as_str()?, m[1].as_str()?), Some((m[2].as_str()?.to_string(), m[3].as_str()?.to_string())))))
        .collect();
    let own: HashSet<String> = list("own_files").into_iter().collect();
    let g = group_opts(&files, &rules, &merges, &own, &one);
    // one line per song: its folder, its title, its files (versions marked)
    let mut lines: Vec<String> = g
        .songs
        .iter()
        .filter(|s| s.merged_into.is_none())
        .map(|s| {
            let mut fs: Vec<String> = g
                .files
                .iter()
                .filter(|f| f.song_dir == s.song_dir && f.vkey == s.vkey)
                .map(|f| format!("{}{}", f.rel.rsplit('/').next().unwrap_or(&f.rel), if f.in_version { " (v)" } else { "" }))
                .collect();
            fs.sort();
            format!("{} | {} | {}", s.song_dir, s.title.as_deref().unwrap_or("?"), fs.join(", "))
        })
        .collect();
    lines.sort();
    format!("{} songs\n{}\n", lines.len(), lines.join("\n"))
}

#[test]
fn every_known_structure_groups_as_expected() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structures");
    // the public mirror has no structures: nothing to check there
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let bless = std::env::var_os("BLESS").is_some();
    let mut failed = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let got = summary(&json);
        let exp_path = p.with_extension("expected.txt");
        if bless || !exp_path.exists() {
            std::fs::write(&exp_path, &got).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&exp_path).unwrap().replace("\r\n", "\n");
        if got != want {
            let (w, g): (HashSet<&str>, HashSet<&str>) = (want.lines().collect(), got.lines().collect());
            let gone: Vec<&&str> = w.difference(&g).take(8).collect();
            let new: Vec<&&str> = g.difference(&w).take(8).collect();
            failed.push(format!("{}:\n  was:  {gone:?}\n  now:  {new:?}", p.file_name().unwrap().to_string_lossy()));
        }
    }
    assert!(failed.is_empty(), "the songs changed for:\n{}", failed.join("\n"));
}

//! How the app reads a library's folders, said back to the user with their own examples (the
//! "How Spool reads your folders" page), and what a change of rule would do before it is
//! made. Nothing of it leaves the computer.
use catalog::{Grouping, OneSong, group_opts};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct FolderSongs {
    pub dir: String,
    pub titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReadExample {
    /// A project file (or a folder), relative.
    pub rel: String,
    /// The song it was read as.
    pub title: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReadingReport {
    #[ts(type = "number")]
    pub files: usize,
    #[ts(type = "number")]
    pub songs: usize,
    /// Files whose name says a version ("1 v2.als"): how many, a few examples.
    #[ts(type = "number")]
    pub versions_n: usize,
    pub versions: Vec<ReadExample>,
    /// Files set aside as older versions by their folder (Backup, versions, old...).
    #[ts(type = "number")]
    pub version_dirs_n: usize,
    pub version_dirs: Vec<ReadExample>,
    /// Files inside a Live "... Project" folder.
    #[ts(type = "number")]
    pub project_dirs_n: usize,
    /// Songs titled after their folder (the folder is the song).
    #[ts(type = "number")]
    pub from_folder_n: usize,
    pub from_folder: Vec<ReadExample>,
    /// Songs named by a number alone.
    #[ts(type = "number")]
    pub numbered_n: usize,
    pub numbered: Vec<String>,
    /// Song folders holding several songs (not the folders of folders): where "a folder is a
    /// song" would change something.
    #[ts(type = "number")]
    pub multi_n: usize,
    pub multi: Vec<FolderSongs>,
    pub group_mode: String,
    pub one_song_dirs: Vec<String>,
    /// "auto", "file" or "folder".
    pub title_from: String,
    pub work_words: Vec<String>,
    pub artists: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct GroupChange {
    pub dir: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct GroupPreview {
    #[ts(type = "number")]
    pub now: usize,
    #[ts(type = "number")]
    pub after: usize,
    /// The folders that change, the most changed first (at most 40).
    #[ts(type = "number")]
    pub changed: usize,
    pub changes: Vec<GroupChange>,
}

static VERSION_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bv\s?\d").unwrap());

fn grouped(con: &Connection, one: &OneSong) -> rusqlite::Result<Grouping> {
    let tx = con.unchecked_transaction()?;
    let inp = index::group_inputs(&tx)?;
    Ok(group_opts(&inp.files, &inp.rules, &inp.merges, &inp.own, one))
}

/// The songs a grouping makes, by folder: (folder -> titles), merged variants left out.
fn by_folder(g: &Grouping) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in g.songs.iter().filter(|s| s.merged_into.is_none()) {
        m.entry(s.song_dir.clone()).or_default().push(s.title.clone().unwrap_or_else(|| s.vkey.clone()));
    }
    for v in m.values_mut() {
        v.sort();
    }
    m
}

fn setting<T: serde::de::DeserializeOwned>(con: &Connection, k: &str) -> Option<T> {
    con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0)).ok().and_then(|s| serde_json::from_str(&s).ok())
}

fn leaf(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn report(con: &Connection) -> rusqlite::Result<ReadingReport> {
    let tx = con.unchecked_transaction()?;
    let one = index::one_song_opts(&tx);
    drop(tx);
    let g = grouped(con, &one)?;
    let title_of: HashMap<(&str, &str), String> =
        g.songs.iter().map(|s| ((s.song_dir.as_str(), s.vkey.as_str()), s.title.clone().unwrap_or_else(|| s.vkey.clone()))).collect();
    let title = |sd: &str, k: &str| title_of.get(&(sd, k)).cloned().unwrap_or_default();
    let mut versions = Vec::new();
    let mut version_dirs = Vec::new();
    let (mut versions_n, mut version_dirs_n, mut project_dirs_n) = (0, 0, 0);
    for f in &g.files {
        if VERSION_NAME.is_match(leaf(&f.rel)) {
            versions_n += 1;
            if versions.len() < 4 {
                versions.push(ReadExample { rel: f.rel.clone(), title: title(&f.song_dir, &f.vkey) });
            }
        }
        if f.in_version {
            version_dirs_n += 1;
            if version_dirs.len() < 3 && !f.rel.to_lowercase().contains("/backup/") {
                version_dirs.push(ReadExample { rel: f.rel.clone(), title: title(&f.song_dir, &f.vkey) });
            }
        }
        if f.rel.split('/').any(|c| c.to_lowercase().ends_with(" project")) {
            project_dirs_n += 1;
        }
    }
    let live: Vec<&catalog::Derived> = g.songs.iter().filter(|s| s.merged_into.is_none()).collect();
    let from_folder: Vec<ReadExample> = live
        .iter()
        .filter(|s| {
            let t = s.title.as_deref().unwrap_or("").to_lowercase();
            let l = leaf(&s.song_dir).to_lowercase();
            !t.is_empty() && (l == t || l.trim_end_matches(" project") == t)
        })
        .map(|s| ReadExample { rel: s.song_dir.clone(), title: s.title.clone().unwrap_or_default() })
        .collect();
    let numbered: Vec<String> = live.iter().filter_map(|s| s.title.clone()).filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).collect();
    let folders = by_folder(&g);
    let dirs: Vec<&String> = folders.keys().collect();
    let container = |d: &str| d.is_empty() || dirs.iter().any(|o| o.starts_with(&format!("{d}/")));
    let mut multi: Vec<FolderSongs> = folders.iter().filter(|(d, t)| t.len() >= 2 && !container(d)).map(|(d, t)| FolderSongs { dir: d.clone(), titles: t.clone() }).collect();
    multi.sort_by(|a, b| b.titles.len().cmp(&a.titles.len()).then(a.dir.cmp(&b.dir)));
    let multi_n = multi.len();
    multi.truncate(30);
    let mut one_dirs: Vec<String> = one.dirs.into_iter().collect();
    one_dirs.sort();
    Ok(ReadingReport {
        files: g.files.len(),
        songs: live.len(),
        versions_n,
        versions,
        version_dirs_n,
        version_dirs,
        project_dirs_n,
        from_folder_n: from_folder.len(),
        from_folder: from_folder.into_iter().take(4).collect(),
        numbered_n: numbered.len(),
        numbered: numbered.into_iter().take(8).collect(),
        multi_n,
        multi,
        group_mode: if one.all { "folder".into() } else { "auto".into() },
        title_from: match one.title_from_folder {
            Some(true) => "folder".into(),
            Some(false) => "file".into(),
            None => "auto".into(),
        },
        one_song_dirs: one_dirs,
        work_words: setting(con, "work_words").unwrap_or_default(),
        artists: setting(con, "artists").unwrap_or_default(),
    })
}

/// What the grouping would make with `one` instead of the rules in force: nothing written.
pub fn preview(con: &Connection, one: &OneSong) -> rusqlite::Result<GroupPreview> {
    let tx = con.unchecked_transaction()?;
    let cur = index::one_song_opts(&tx);
    drop(tx);
    let a = by_folder(&grouped(con, &cur)?);
    let b = by_folder(&grouped(con, one)?);
    let mut changes: Vec<GroupChange> = a
        .keys()
        .chain(b.keys())
        .collect::<HashSet<_>>()
        .into_iter()
        .filter_map(|d| {
            let (x, y) = (a.get(d).cloned().unwrap_or_default(), b.get(d).cloned().unwrap_or_default());
            (x != y).then(|| GroupChange { dir: d.clone(), before: x, after: y })
        })
        .collect();
    changes.sort_by(|p, q| (q.before.len() as i64 - q.after.len() as i64).cmp(&(p.before.len() as i64 - p.after.len() as i64)).then(p.dir.cmp(&q.dir)));
    let changed = changes.len();
    changes.truncate(40);
    Ok(GroupPreview { now: a.values().map(Vec::len).sum(), after: b.values().map(Vec::len).sum(), changed, changes })
}

/// The song folders beside `dir` (same parent) that hold several songs too: what a "make it
/// one song" on `dir` proposes next.
pub fn neighbours(con: &Connection, dir: &str) -> rusqlite::Result<Vec<String>> {
    let tx = con.unchecked_transaction()?;
    let one = index::one_song_opts(&tx);
    drop(tx);
    let folders = by_folder(&grouped(con, &one)?);
    let parent = dir.rsplit_once('/').map(|p| p.0).unwrap_or("");
    let dirs: Vec<&String> = folders.keys().collect();
    let container = |d: &str| d.is_empty() || dirs.iter().any(|o| o.starts_with(&format!("{d}/")));
    Ok(folders
        .iter()
        .filter(|(d, t)| {
            t.len() >= 2 && d.as_str() != dir && !container(d) && d.rsplit_once('/').map(|p| p.0).unwrap_or("") == parent && !one.dirs.contains(*d)
        })
        .map(|(d, _)| d.clone())
        .collect())
}

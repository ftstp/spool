//! From the list of project files to songs: song folder, variants, versions, and the
//! names read from the folders. Pure: the caller keeps song ids and the user's edits.
use crate::names::{
    FolderName, Rules, artist_for, ascii_low, is_project_dir, is_version_dir, parse_folder_name, similarity,
    split_processed, status_hint, strip_project_suffix,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
pub struct FileIn {
    pub rel: String,
    pub size: i64,
    pub mtime: f64,
}

/// Where one file goes.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Placed {
    pub rel: String,
    pub song_dir: String,
    pub vkey: String,
    /// A superseded version (old folder, or replaced by a repaired copy).
    pub in_version: bool,
    /// The newest old version standing in for a folder with no active file.
    pub promoted: bool,
}

/// What the folders and file names say about one song (the user's edits win over these).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Derived {
    pub song_dir: String,
    pub vkey: String,
    /// The variant was merged by the user into this other song (song_dir, vkey): its files
    /// go there and nothing is derived for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<(String, String)>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub style: Option<String>,
    pub key: Option<String>,
    pub catalog: Option<String>,
    pub ft: Option<String>,
    pub folder_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Grouping {
    pub files: Vec<Placed>,
    pub songs: Vec<Derived>,
}

struct F {
    rel: String,
    mtime: f64,
    song_dir: String,
    stem: String,
    key: String,
    in_version_dir: bool,
    promoted: bool,
    /// The Live "... Project" folder right under the song folder the file sits in, if any.
    proj: Option<String>,
}

/// Python's `Path(name).stem`.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => &name[..i],
        _ => name,
    }
}

fn place(rel: &str, rules: &Rules) -> F {
    let rel_n = rel.replace('\\', "/");
    let parts: Vec<&str> = rel_n.split('/').filter(|p| !p.is_empty()).collect();
    let (folders, file) = parts.split_at(parts.len().saturating_sub(1));
    let st = stem(file.first().copied().unwrap_or(""));
    // the song folder: the deepest folder that is neither a version folder nor an Ableton
    // "... Project" folder
    let mut i = folders.len() as isize - 1;
    let mut in_version = false;
    while i >= 0 && (is_version_dir(folders[i as usize]) || is_project_dir(folders[i as usize])) {
        in_version = in_version || is_version_dir(folders[i as usize]);
        i -= 1;
    }
    let song_dir = if i >= 0 { folders[..=i as usize].join("/") } else { String::new() };
    let proj = folders.get((i + 1) as usize).filter(|f| is_project_dir(f)).map(|f| f.to_string());
    let key = match rules.variant_key(st) {
        k if k.is_empty() => ascii_low(st),
        k => k,
    };
    F {
        rel: rel.to_string(),
        mtime: 0.0,
        song_dir,
        stem: st.to_string(),
        key,
        in_version_dir: in_version,
        promoted: false,
        proj,
    }
}

/// `song_dir_of` against the song folders the grouping found: a file inside a Live project
/// folder that is a song of its own (see `group`) belongs to that folder.
pub fn song_dir_known(rel: &str, rules: &Rules, known: &HashSet<String>) -> String {
    let p = place(rel, rules);
    match &p.proj {
        Some(pr) => {
            let own = if p.song_dir.is_empty() { pr.clone() } else { format!("{}/{pr}", p.song_dir) };
            if known.contains(&own) { own } else { p.song_dir }
        }
        None => p.song_dir,
    }
}

/// The song folder a file (a .als, or an audio file) sits in.
pub fn song_dir_of(rel: &str, rules: &Rules) -> String {
    place(rel, rules).song_dir
}

/// First maximum, like Python's `max`.
fn first_max<T>(items: impl IntoIterator<Item = T>, key: impl Fn(&T) -> f64) -> Option<T> {
    let mut best: Option<(T, f64)> = None;
    for it in items {
        let k = key(&it);
        if best.as_ref().is_none_or(|(_, b)| k > *b) {
            best = Some((it, k));
        }
    }
    best.map(|b| b.0)
}

/// Files of one song folder -> variant key -> files. Active files with distinct keys make
/// distinct variants; versions attach to the most similar variant. A folder with no active
/// file: its newest version becomes the active one.
fn group_files(fs: &mut [F], idx: &[usize]) -> Vec<(String, Vec<usize>)> {
    let mut active: Vec<usize> = idx.iter().copied().filter(|&i| !fs[i].in_version_dir).collect();
    let mut versions: Vec<usize> = idx.iter().copied().filter(|&i| fs[i].in_version_dir).collect();
    if active.is_empty() && !versions.is_empty() {
        let newest = first_max(versions.iter().copied(), |&i| fs[i].mtime).unwrap();
        fs[newest].promoted = true;
        active = vec![newest];
        versions.retain(|&i| i != newest);
    }
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for i in active {
        // Only near-identical spellings merge ("noisia style" / "noisia style 2"):
        // "glass hour dub" must stay apart from "glass hour".
        let k = &fs[i].key;
        match groups.iter_mut().find(|(g, _)| similarity(g, k, 0.0) >= 0.95) {
            Some((_, v)) => v.push(i),
            None => groups.push((k.clone(), vec![i])),
        }
    }
    for i in versions {
        let k = fs[i].key.clone();
        let best = first_max(0..groups.len(), |&g| similarity(&groups[g].0, &k, 0.0)).unwrap();
        groups[best].1.push(i);
    }
    groups
}

/// Groups every project file into songs. `files` in a stable order (it decides ties);
/// `merges`: "<song_dir>|<variant key>" -> the song it was merged into, as (song_dir, vkey),
/// when that song still exists.
pub fn group(files: &[FileIn], rules: &Rules, merges: &HashMap<String, Option<(String, String)>>) -> Grouping {
    let mut fs: Vec<F> = files
        .iter()
        .map(|f| {
            let mut p = place(&f.rel, rules);
            p.mtime = f.mtime;
            p
        })
        .collect();
    // A processed copy "<name> [RCN].als" (or "<name> - fixed.als") next to "<name>.als"
    // replaces it: the original becomes a version, and so do older copies of it.
    let mut chains: Vec<(String, Vec<(bool, usize)>)> = Vec::new();
    let mut chain_at: HashMap<String, usize> = HashMap::new();
    for (i, f) in fs.iter().enumerate() {
        let full = &f.rel[..f.rel.len().saturating_sub(4)];
        let (base, codes) = split_processed(full);
        let b = base.to_lowercase();
        let at = *chain_at.entry(b.clone()).or_insert_with(|| {
            chains.push((b, Vec::new()));
            chains.len() - 1
        });
        chains[at].1.push((codes.is_some(), i));
    }
    for (_, chain) in &chains {
        let copies: Vec<usize> = chain.iter().filter(|(p, _)| *p).map(|(_, i)| *i).collect();
        let Some(newest) = first_max(copies.iter().copied(), |&i| fs[i].mtime) else { continue };
        for &(_, i) in chain {
            if i != newest {
                fs[i].in_version_dir = true;
            }
        }
    }
    // "Projets/ID 04.11.2026 Project/ID 04.11.2026.als", "Projets/ID 27.09.2023 Project/...":
    // a folder holding Live project folders of different names is a folder of songs, each
    // project folder is one (stripping it would leave "ID" twice, the dates being dropped
    // from the variant key). One project folder ("Song/Song Project", or "Song Project 2"
    // beside it) stays part of the song folder around it.
    let mut bases: HashMap<String, HashSet<String>> = HashMap::new();
    for f in &fs {
        if let Some(p) = &f.proj {
            bases.entry(f.song_dir.clone()).or_default().insert(ascii_low(strip_project_suffix(p)));
        }
    }
    for f in fs.iter_mut() {
        if let Some(p) = &f.proj
            && bases.get(&f.song_dir).is_some_and(|b| b.len() >= 2)
        {
            f.song_dir = if f.song_dir.is_empty() { p.clone() } else { format!("{}/{p}", f.song_dir) };
        }
    }
    let mut by_dir: Vec<(String, Vec<usize>)> = Vec::new();
    let mut dir_at: HashMap<String, usize> = HashMap::new();
    for (i, f) in fs.iter().enumerate() {
        let at = *dir_at.entry(f.song_dir.clone()).or_insert_with(|| {
            by_dir.push((f.song_dir.clone(), Vec::new()));
            by_dir.len() - 1
        });
        by_dir[at].1.push(i);
    }
    let mut groups: Vec<(String, String, Vec<usize>)> = Vec::new();
    for (sd, idx) in &by_dir {
        for (k, g) in group_files(&mut fs, idx) {
            groups.push((sd.clone(), k, g));
        }
    }
    let present: HashSet<(String, String)> = groups.iter().map(|(sd, k, _)| (sd.clone(), k.clone())).collect();
    // A folder holding other songs' folders is a category ("- ARCHIVE"): a .als lying in
    // it is named after the file, never after the category.
    let dirs: HashSet<&str> = groups.iter().map(|(sd, _, _)| sd.as_str()).collect();
    let containers: HashSet<&str> = dirs
        .iter()
        .copied()
        .filter(|d| d.is_empty() || dirs.iter().any(|o| *o != *d && o.starts_with(&format!("{d}/"))))
        .collect();

    let mut out = Grouping { files: Vec::new(), songs: Vec::new() };
    for (sd, k, g) in &groups {
        let target = merges.get(&format!("{sd}|{k}")).cloned().flatten();
        let merged_into = target.filter(|t| present.contains(t) && *t != (sd.clone(), k.clone()));
        // the files keep their own folder and key in the base; only their song changes
        for &i in g {
            let f = &fs[i];
            out.files.push(Placed {
                rel: f.rel.clone(),
                song_dir: sd.clone(),
                vkey: k.clone(),
                in_version: f.in_version_dir && !f.promoted,
                promoted: f.promoted,
            });
        }
        if merged_into.is_some() {
            out.songs.push(Derived {
                song_dir: sd.clone(),
                vkey: k.clone(),
                merged_into,
                title: None,
                artist: None,
                style: None,
                key: None,
                catalog: None,
                ft: None,
                folder_date: None,
            });
            continue;
        }
        out.songs.push(derive(&fs, sd, k, g, &groups, merges, rules, containers.contains(sd.as_str())));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn derive(
    fs: &[F],
    sd: &str,
    k: &str,
    g: &[usize],
    groups: &[(String, String, Vec<usize>)],
    merges: &HashMap<String, Option<(String, String)>>,
    rules: &Rules,
    container: bool,
) -> Derived {
    let parts: Vec<&str> = if sd.is_empty() { Vec::new() } else { sd.split('/').collect() };
    let folder: FolderName = parse_folder_name(strip_project_suffix(parts.last().copied().unwrap_or("")), &rules.artists);
    let variants = groups.iter().filter(|(d, key, _)| d == sd && !merges.contains_key(&format!("{d}|{key}"))).count();
    let active: Vec<usize> = g.iter().copied().filter(|&i| !fs[i].in_version_dir || fs[i].promoted).collect();
    let pool = if active.is_empty() { g.to_vec() } else { active };
    let newest = first_max(pool, |&i| fs[i].mtime).unwrap();
    // Folder title when the folder IS this song; the file name when the folder holds
    // several songs (variants, or a loose "category" folder).
    let title = match &folder.title {
        Some(t) if variants == 1 && !container && status_hint(&[parts[parts.len() - 1]]).is_none() => Some(t.clone()),
        _ => Some(rules.pretty_stem(&fs[newest].stem)),
    };
    Derived {
        song_dir: sd.to_string(),
        vkey: k.to_string(),
        merged_into: None,
        title,
        artist: folder.artist.clone().or_else(|| artist_for(&parts, &rules.artists)),
        style: folder.style,
        key: folder.key,
        catalog: folder.catalog,
        ft: folder.ft,
        folder_date: folder.date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(rels: &[&str]) -> Grouping {
        let files: Vec<FileIn> =
            rels.iter().enumerate().map(|(i, r)| FileIn { rel: r.to_string(), size: 1, mtime: i as f64 }).collect();
        group(&files, &Rules::new(&[], &[]), &HashMap::new())
    }
    fn dir_of<'a>(g: &'a Grouping, rel: &str) -> &'a str {
        &g.files.iter().find(|f| f.rel == rel).unwrap().song_dir
    }

    #[test]
    fn project_folders_side_by_side_are_songs() {
        // a common layout: every song a Live project folder in one "Projets" folder,
        // named "ID <date>": the dates are not part of the variant key
        let g = run(&[
            "Projets/ID 04.11.2026 Project/ID 04.11.2026.als",
            "Projets/ID 04.11.2026 Project/Backup/ID 04.11.2026 [2026-11-04 120000].als",
            "Projets/ID 27.09.2023 Project/ID 27.09.2023 v2.0.als",
            "Projets/ID 03.05.2025 Project/ID 03.05.2025 v2.0.als",
        ]);
        assert_eq!(g.songs.len(), 3, "{:?}", g.songs);
        assert_eq!(dir_of(&g, "Projets/ID 04.11.2026 Project/Backup/ID 04.11.2026 [2026-11-04 120000].als"), "Projets/ID 04.11.2026 Project");
        assert!(g.songs.iter().all(|s| !s.title.as_deref().unwrap_or("").to_lowercase().contains("project")), "{:?}", g.songs);
        // the same at the very root
        let g = run(&["A Project/A.als", "B Project/B.als"]);
        assert_eq!(dir_of(&g, "A Project/A.als"), "A Project");
    }

    #[test]
    fn one_project_folder_stays_in_its_song_folder() {
        let g = run(&[
            "Artist - Song/Song Project/Song.als",
            "Artist - Song/Song Project 2/Song v2.als",
            "Artist - Song/Song v3.als",
        ]);
        assert!(g.files.iter().all(|f| f.song_dir == "Artist - Song"), "{:?}", g.files);
        let known: HashSet<String> = g.songs.iter().map(|s| s.song_dir.clone()).collect();
        assert_eq!(song_dir_known("Artist - Song/Song Project/Song.wav", &Rules::default(), &known), "Artist - Song");
        let known: HashSet<String> = ["Projets/ID 1 Project".to_string()].into();
        assert_eq!(song_dir_known("Projets/ID 1 Project/ID 1.wav", &Rules::default(), &known), "Projets/ID 1 Project");
    }
}

//! A song's mood board, read side: what is in its "_moodboard" folder, the pictures already
//! lying in the song folder, the card's picture and the finishing checklist the files show.
//! (Writing to a board lives with the other file operations.)
use crate::names::{Rules, is_project_dir, is_version_dir, similarity};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub const BOARD_DIR: &str = "_moodboard";
const META: &str = "board.json";
const KINDS: &[(&str, &[&str])] = &[
    ("image", &[".png", ".jpg", ".jpeg", ".webp", ".bmp", ".svg"]),
    ("gif", &[".gif"]),
    ("video", &[".mp4", ".webm", ".mov", ".m4v"]),
    ("audio", &[".wav", ".mp3", ".aif", ".aiff", ".flac", ".m4a", ".ogg"]),
    ("font", &[".ttf", ".otf", ".woff", ".woff2"]),
    ("text", &[".md", ".txt"]),
];
static SLUG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static LYRICS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(paroles|lyrics|texte)").unwrap());

pub fn kind_of(name: &str) -> &'static str {
    let low = name.to_lowercase();
    KINDS.iter().find(|(_, exts)| exts.iter().any(|e| low.ends_with(e))).map(|k| k.0).unwrap_or("file")
}

fn is_visual(name: &str) -> bool {
    matches!(kind_of(name), "image" | "gif" | "video")
}

/// The board folder: "<song folder>/_moodboard", or "_moodboard/<song>" when the folder is
/// shared by several songs.
pub fn board_dir(root: &Path, song_dir: &str, vkey: &str, shared: bool) -> PathBuf {
    let base = root.join(song_dir).join(BOARD_DIR);
    if !shared {
        return base;
    }
    let slug = SLUG.replace_all(if vkey.is_empty() { "morceau" } else { vkey }, "-").trim_matches('-').to_string();
    base.join(if slug.is_empty() { "morceau".to_string() } else { slug })
}

/// (width, height) from the file header, for png / jpeg / gif / webp.
pub fn image_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 32];
    let n = f.read(&mut head).ok()?;
    let head = &head[..n];
    let be32 = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    if head.len() >= 24 && head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(&head[16..20]), be32(&head[20..24])));
    }
    if head.len() >= 10 && (head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a")) {
        return Some((u16::from_le_bytes([head[6], head[7]]) as u32, u16::from_le_bytes([head[8], head[9]]) as u32));
    }
    if head.len() >= 16 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        let le24 = |b: &[u8]| b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16;
        match &head[12..16] {
            b"VP8X" if head.len() >= 27 => {
                let w = le24(&head[24..27]) + 1;
                f.seek(SeekFrom::Start(27)).ok()?;
                let mut b = [0u8; 3];
                f.read_exact(&mut b).ok()?;
                return Some((w, le24(&b) + 1));
            }
            b"VP8 " => {
                f.seek(SeekFrom::Start(26)).ok()?;
                let mut b = [0u8; 4];
                f.read_exact(&mut b).ok()?;
                let (w, h) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
                return Some(((w & 0x3FFF) as u32, (h & 0x3FFF) as u32));
            }
            b"VP8L" if head.len() >= 25 => {
                let b = &head[21..25];
                let w = 1 + (((b[1] as u32 & 0x3F) << 8) | b[0] as u32);
                let h = 1 + (((b[3] as u32 & 0xF) << 10) | (b[2] as u32) << 2 | ((b[1] as u32 & 0xC0) >> 6));
                return Some((w, h));
            }
            _ => {}
        }
    }
    if head.starts_with(&[0xFF, 0xD8]) {
        f.seek(SeekFrom::Start(2)).ok()?;
        loop {
            let mut m = [0u8; 2];
            if f.read_exact(&mut m).is_err() || m[0] != 0xFF {
                return None;
            }
            if (0xC0..=0xCF).contains(&m[1]) && !matches!(m[1], 0xC4 | 0xC8 | 0xCC) {
                let mut b = [0u8; 7];
                f.read_exact(&mut b).ok()?;
                let (h, w) = (u16::from_be_bytes([b[3], b[4]]), u16::from_be_bytes([b[5], b[6]]));
                return Some((w as u32, h as u32));
            }
            let mut s = [0u8; 2];
            f.read_exact(&mut s).ok()?;
            let seg = u16::from_be_bytes(s) as i64;
            f.seek(SeekFrom::Current(seg - 2)).ok()?;
        }
    }
    None
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Meta {
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    palette: Vec<String>,
    #[serde(default)]
    captions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct Item {
    pub name: String,
    /// Relative to the projects root, "/" separated.
    pub rel: String,
    pub kind: String,
    #[ts(type = "number")]
    pub size: u64,
    pub mtime: f64,
    /// "board" (in the mood board folder) or "dossier" (already in the song folder).
    #[serde(rename = "where")]
    pub place: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub w: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub h: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct Board {
    pub dir: String,
    pub exists: bool,
    pub items: Vec<Item>,
    pub found: Vec<Item>,
    pub palette: Vec<String>,
    pub captions: BTreeMap<String, String>,
}

/// The real, full path (case and links resolved), without Windows' \\?\ prefix.
fn real(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = c.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

fn rel_to(p: &Path, root_real: &Path) -> String {
    real(p).strip_prefix(root_real).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default()
}

fn mtime(m: &std::fs::Metadata) -> f64 {
    m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn item(p: &Path, root_real: &Path, place: &str) -> Option<Item> {
    let md = std::fs::metadata(p).ok()?;
    let name = p.file_name()?.to_string_lossy().to_string();
    let kind = kind_of(&name);
    let wh = if matches!(kind, "image" | "gif") { image_size(p) } else { None };
    let text = (kind == "text" && md.len() < 200_000)
        .then(|| std::fs::read(p).ok().map(|b| String::from_utf8_lossy(&b).to_string()))
        .flatten();
    Some(Item {
        rel: rel_to(p, root_real),
        kind: kind.into(),
        size: md.len(),
        mtime: mtime(&md),
        place: place.into(),
        w: wh.map(|x| x.0),
        h: wh.map(|x| x.1),
        text,
        name,
    })
}

/// Every folder that is, or contains, ANOTHER song's folder: its pictures are not ours.
pub fn other_song_prefixes(song_dirs: &[String], mine: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for d in song_dirs {
        if !d.is_empty() && d != mine && (mine.is_empty() || d.starts_with(&format!("{mine}/"))) {
            let parts: Vec<&str> = d.split('/').collect();
            for i in 1..=parts.len() {
                out.insert(parts[..i].join("/"));
            }
        }
    }
    out
}

/// What a song's board shows. `song_dirs`: every song's folder (to leave other songs'
/// pictures out).
#[allow(clippy::too_many_arguments)]
pub fn list_board(
    root: &Path,
    rules: &Rules,
    song_dir: &str,
    vkey: &str,
    title: &str,
    shared: bool,
    others: &HashSet<String>,
) -> Board {
    let root_real = real(root);
    let d = board_dir(root, song_dir, vkey, shared);
    let mut items: Vec<Item> = Vec::new();
    let meta: Meta = if d.is_dir() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            if p.is_file() && n != META && !n.ends_with(".tmp") {
                items.extend(item(&p, &root_real, "board"));
            }
        }
        std::fs::read_to_string(d.join(META)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    } else {
        Meta::default()
    };
    let order: HashMap<&str, i64> = meta.order.iter().enumerate().map(|(i, n)| (n.as_str(), i as i64)).collect();
    items.sort_by(|a, b| {
        let ka = (order.get(a.name.as_str()).copied().unwrap_or(-1), -a.mtime);
        let kb = (order.get(b.name.as_str()).copied().unwrap_or(-1), -b.mtime);
        ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
    });
    // Pictures and videos already in the song folder (not in Samples / Backup / _moodboard).
    let mut found = Vec::new();
    if !song_dir.is_empty() {
        let sd = root.join(song_dir);
        if sd.is_dir() {
            let title_key = rules.variant_key(title);
            walk_found(&sd, &sd, root, &root_real, rules, vkey, &title_key, shared, others, &mut found);
        }
    }
    // a board folder that does not exist yet cannot be resolved: its path is built
    let dir = match rel_to(&d, &root_real) {
        r if r.is_empty() => d.strip_prefix(root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
        r => r,
    };
    Board { dir, exists: d.is_dir(), items, found, palette: meta.palette, captions: meta.captions }
}

/// A song's board when its folder may be one of the other projects folders: `label` is that
/// folder's label (empty for the main one), `song_dir` and `others` carry it, every name
/// returned carries it too.
#[allow(clippy::too_many_arguments)]
pub fn list_board_labeled(
    root: &Path,
    label: &str,
    rules: &Rules,
    song_dir: &str,
    vkey: &str,
    title: &str,
    shared: bool,
    others: &HashSet<String>,
) -> Board {
    if label.is_empty() {
        return list_board(root, rules, song_dir, vkey, title, shared, others);
    }
    let pre = format!("{label}/");
    let inner = if song_dir == label { "" } else { song_dir.strip_prefix(&pre).unwrap_or(song_dir) };
    let others: HashSet<String> = others.iter().filter_map(|o| o.strip_prefix(&pre).map(str::to_string)).collect();
    let mut b = list_board(root, rules, inner, vkey, title, shared, &others);
    let add = |r: &mut String| {
        *r = if r.is_empty() { label.to_string() } else { format!("{pre}{r}") };
    };
    add(&mut b.dir);
    for i in b.items.iter_mut().chain(b.found.iter_mut()) {
        if !i.rel.is_empty() {
            add(&mut i.rel);
        }
    }
    b
}

#[allow(clippy::too_many_arguments)]
fn walk_found(
    here: &Path,
    sd: &Path,
    root: &Path,
    root_real: &Path,
    rules: &Rules,
    key: &str,
    title_key: &str,
    shared: bool,
    others: &HashSet<String>,
    found: &mut Vec<Item>,
) {
    let (mut files, mut dirs): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for e in std::fs::read_dir(here).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) { dirs.push(n) } else { files.push(n) }
    }
    if here != sd {
        let rel_root = here.strip_prefix(root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        if others.contains(&rel_root) {
            return;
        }
        let rel: Vec<String> = here
            .strip_prefix(sd)
            .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        // A subfolder holding its own .als is ANOTHER song: its pictures are not ours.
        let own = rel.iter().all(|x| is_version_dir(x) || is_project_dir(x));
        if !own && files.iter().any(|f| f.to_lowercase().ends_with(".als")) {
            return;
        }
        // In a folder shared by several songs, only the song's own subfolders count.
        if shared && !own {
            return;
        }
    }
    const SKIP: [&str; 4] = ["samples", "backup", "_moodboard", "ableton project info"];
    dirs.retain(|x| !SKIP.contains(&x.to_lowercase().as_str()));
    for f in &files {
        if !is_visual(f) || found.len() >= 60 {
            continue;
        }
        if shared {
            // loose pictures of a shared folder: only those named like the song
            let stem = f.rsplit_once('.').map(|(s, _)| s).unwrap_or(f);
            let k = rules.variant_key(stem);
            if similarity(&k, key, 0.6).max(similarity(&k, title_key, 0.6)) < 0.6 {
                continue;
            }
        }
        found.extend(item(&here.join(f), root_real, "dossier"));
    }
    for d in dirs {
        walk_found(&here.join(d), sd, root, root_real, rules, key, title_key, shared, others, found);
    }
}

pub fn is_cover(i: &Item) -> bool {
    match (i.w, i.h) {
        (Some(w), Some(h)) if i.kind == "image" && w > 0 && h > 0 => {
            w.min(h) >= 1000 && (w as f64 - h as f64).abs() / (w.max(h) as f64) < 0.05
        }
        _ => false,
    }
}

/// The picture shown on the song's card: a square one (a cover), else the first one.
pub fn card_picture(b: &Board) -> Option<String> {
    let pics: Vec<&Item> =
        b.items.iter().chain(&b.found).filter(|i| matches!(i.kind.as_str(), "image" | "gif")).collect();
    let square = |i: &&&Item| match (i.w, i.h) {
        (Some(w), Some(h)) if w > 0 && h > 0 => ((w as f64 - h as f64).abs() / w.max(h) as f64) < 0.05,
        _ => false,
    };
    pics.iter().find(square).or(pics.first()).map(|i| i.rel.clone())
}

/// What a song's newest project and its renders say, for the checklist.
pub struct CheckInput<'a> {
    pub stage: Option<&'a str>,
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    /// Reference-render candidates: (rel, kind, mtime).
    pub temoins: Vec<(&'a str, &'a str, f64)>,
    pub newest_mtime: f64,
    /// Worst severity of the active projects, None without any.
    pub sev: Option<i64>,
    pub length_s: Option<i64>,
}

/// What the tool can see by itself: key -> (done, reason), in the checklist's order.
pub fn auto_checks(c: &CheckInput, b: &Board) -> Vec<(&'static str, bool, String)> {
    let rank = match c.stage.unwrap_or("") {
        "draft" => 0,
        "arrangement" => 1,
        "mix" => 2,
        "master" => 3,
        "done" => 4,
        _ => -1,
    };
    let master: Vec<&(&str, &str, f64)> = c
        .temoins
        .iter()
        .filter(|t| t.1 == "master" && c.newest_mtime != 0.0 && t.2 >= c.newest_mtime - 2.0 * 86400.0)
        .collect();
    let mix = c.temoins.iter().any(|t| t.1 == "mix" || t.1 == "master");
    let covers: Vec<&Item> = b.items.iter().chain(&b.found).filter(|i| is_cover(i)).collect();
    let lyrics = b
        .items
        .iter()
        .any(|i| i.kind == "text" && LYRICS.is_match(&i.name) && !i.text.as_deref().unwrap_or("").trim().is_empty());
    let name = |rel: &str| rel.rsplit('/').next().unwrap_or("").to_string();
    let plugins_ok = c.sev.is_some_and(|s| s <= 1);
    vec![
        ("arrangement", rank >= 1 || c.length_s.unwrap_or(0) >= 150, i18n::tr!("checks.arrangement")),
        ("mix", rank >= 2 || mix, i18n::tr!("checks.mix")),
        (
            "master",
            !master.is_empty(),
            master
                .first()
                .map(|m| i18n::tr!("checks.export", name = name(m.0)))
                .unwrap_or_else(|| i18n::tr!("checks.noMaster")),
        ),
        (
            "cover",
            !covers.is_empty(),
            covers
                .first()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| i18n::tr!("checks.noCover")),
        ),
        ("lyrics", lyrics, if lyrics { i18n::tr!("checks.lyrics") } else { i18n::tr!("checks.noLyrics") }),
        (
            "meta",
            c.title.is_some_and(|t| !t.is_empty()) && c.artist.is_some_and(|a| !a.is_empty()),
            i18n::tr!("checks.meta"),
        ),
        (
            "plugins",
            plugins_ok,
            if plugins_ok {
                i18n::tr!("checks.pluginsOk")
            } else {
                i18n::tr!("checks.pluginsMissing")
            },
        ),
        ("released", false, i18n::tr!("checks.released")),
    ]
}

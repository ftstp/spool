//! Real file operations on the user's folders: move a folder or a file, create a folder,
//! send media into a song's mood board, reorganise song folders. Ported from the
//! prototype's fileops.py.
//!
//! Rules (agreed with the user):
//! - a song folder moves whole (Samples included), and only when no project inside would
//!   lose a sample (als::move_risks, checked on every .als in it);
//! - nothing is ever overwritten: a name taken gets " (2)", " (3)"...;
//! - the base follows the move (files, audio, songs, settings), so a song keeps its stage,
//!   notes, mood board;
//! - every operation is journaled for undo, undone by moving back;
//! - an audio file a project uses as a sample is COPIED into a mood board, never moved;
//! - an undo never deletes: a copy the app made goes to the board's ".corbeille".
//!
//! Names are the app's ("<label>/..." for the other projects folders, see store::Roots); a
//! move stays inside one projects folder.
use i18n::tr;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use store::Roots;
use store::rusqlite::{self, Connection, OptionalExtension, params};
use ts_rs::TS;

pub type R<T> = Result<T, String>;

const SKIP_DIRS: [&str; 6] = ["backup", "samples", "ableton project info", "_moodboard", ".corbeille", "presets"];

pub fn norm_rel(rel: &str) -> String {
    rel.replace('\\', "/").trim_matches('/').to_string()
}

/// A folder or file name Windows accepts.
pub fn safe_part(name: &str) -> R<String> {
    let n: String = name.trim().chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '-' } else { c }).collect();
    let n = n.trim_matches(|c| c == ' ' || c == '.').to_string();
    if n.is_empty() { Err(tr!("fs.emptyName")) } else { Ok(n) }
}

fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn inside(p: &Path, folder: &Path) -> bool {
    real(p).starts_with(real(folder))
}

/// (projects folder, absolute path) of a name, refused when it leaves that folder.
fn under(roots: &Roots, rel: &str) -> R<(PathBuf, PathBuf)> {
    let rel = norm_rel(rel);
    let (r, inner) = roots.split(&rel).ok_or_else(|| tr!("fs.noRoot"))?;
    let base = PathBuf::from(&r.path);
    if inner.split('/').any(|x| x == "..") {
        return Err(tr!("fs.outside"));
    }
    let p = if inner.is_empty() { base.clone() } else { base.join(inner) };
    Ok((base, p))
}

fn name_of(roots: &Roots, p: &Path) -> R<String> {
    roots.rel(p).ok_or_else(|| tr!("fs.outside"))
}

/// `name` in `dir`, or "name (2)"... when taken. Never overwrites.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, suf) = match name.rfind('.') {
        Some(i) if i > 0 && !dir.join(name).is_dir() => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){suf}"))).find(|p| !p.exists()).unwrap()
}

// ---------------------------------------------------------------- the base follows the files

fn like(rel: &str) -> String {
    rel.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_") + "/%"
}

/// Every stored path at or under `old_rel` now points under `new_rel`.
pub fn repath(con: &Connection, roots: &Roots, old_rel: &str, new_rel: &str) -> rusqlite::Result<()> {
    let (old_rel, new_rel) = (norm_rel(old_rel), norm_rel(new_rel));
    let abs = |r: &str| roots.abs(r).map(|p| p.to_string_lossy().replace('\\', "/"));
    let (old_abs, new_abs) = (abs(&old_rel).unwrap_or_default(), abs(&new_rel).unwrap_or_default());
    let moved = |v: &str| -> Option<String> {
        let s = v.replace('\\', "/");
        for (base, to) in [(&old_rel, &new_rel), (&old_abs, &new_abs)] {
            if !base.is_empty() && (s == *base || s.starts_with(&format!("{base}/"))) {
                return Some(format!("{to}{}", &s[base.len()..]));
            }
        }
        None
    };
    for (table, col) in [("files", "rel"), ("audio", "rel"), ("backups", "dir")] {
        let rows: Vec<String> = {
            let mut s = con.prepare(&format!("select {col} from {table} where {col} = ? or {col} like ? escape '\\'"))?;
            s.query_map(params![old_rel, like(&old_rel)], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for r in rows {
            if let Some(m) = moved(&r) {
                con.execute(&format!("update {table} set {col}=? where {col}=?"), params![m, r])?;
                if table == "files" {
                    // read again at the next scan: the sample paths it remembers moved too
                    con.execute("update files set mtime=0 where rel=?", [&m])?;
                }
            }
        }
    }
    let songs: Vec<(i64, Option<String>, Option<String>, Option<String>)> = {
        let mut s = con.prepare("select id, song_dir, temoin, cover from songs")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?
    };
    for (id, dir, temoin, cover) in songs {
        let m = |v: &Option<String>| v.as_deref().filter(|x| !x.is_empty()).and_then(moved);
        let (d, t, c) = (m(&dir), m(&temoin), m(&cover));
        if d.is_some() || t.is_some() || c.is_some() {
            con.execute(
                "update songs set song_dir=coalesce(?, song_dir), temoin=coalesce(?, temoin), cover=coalesce(?, cover) where id=?",
                params![d, t, c, id],
            )?;
        }
    }
    // settings keyed by path: corrections file -> song, merges ("song_dir|vkey")
    let get = |k: &str| -> Option<Value> {
        con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0))
            .optional()
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
    };
    let set = |k: &str, v: &Value| con.execute("update settings set v=? where k=?", params![v.to_string(), k]);
    if let Some(Value::Object(fs)) = get("file_song") {
        let out: serde_json::Map<String, Value> = fs.into_iter().map(|(k, v)| (moved(&k).unwrap_or(k), v)).collect();
        set("file_song", &Value::Object(out))?;
    }
    // the releases' tracks and the boards' links name songs and files by path
    if let Some(Value::Array(rs)) = get("releases") {
        let fix = |t: &mut Value| {
            if let Some(d) = t.get("dir").and_then(Value::as_str).and_then(&moved) {
                t["dir"] = Value::String(d);
            }
        };
        let out: Vec<Value> = rs
            .into_iter()
            .map(|mut r| {
                if let Some(ts) = r.get_mut("tracks").and_then(Value::as_array_mut) {
                    ts.iter_mut().for_each(fix);
                }
                if let Some(c) = r.get_mut("cover_song").filter(|c| c.is_object()) {
                    fix(c);
                }
                if let Some(f) = r.get("folder").and_then(Value::as_str).and_then(&moved) {
                    r["folder"] = Value::String(f);
                }
                r
            })
            .collect();
        set("releases", &Value::Array(out))?;
    }
    for k in ["board_links", "board_hidden"] {
        if let Some(Value::Object(m)) = get(k) {
            let out: serde_json::Map<String, Value> = m
                .into_iter()
                .map(|(key, v)| {
                    let (sd, vk) = key.rsplit_once('|').unwrap_or((&key, ""));
                    let v = match v {
                        Value::Array(a) => Value::Array(
                            a.into_iter().map(|x| x.as_str().and_then(&moved).map(Value::String).unwrap_or(x)).collect(),
                        ),
                        o => o,
                    };
                    (format!("{}|{vk}", moved(sd).unwrap_or_else(|| sd.to_string())), v)
                })
                .collect();
            set(k, &Value::Object(out))?;
        }
    }
    if let Some(Value::Object(mg)) = get("merges") {
        let out: serde_json::Map<String, Value> = mg
            .into_iter()
            .map(|(k, v)| {
                let (sd, vk) = k.split_once('|').unwrap_or((&k, ""));
                (format!("{}|{vk}", moved(sd).unwrap_or_else(|| sd.to_string())), v)
            })
            .collect();
        set("merges", &Value::Object(out))?;
    }
    Ok(())
}

// ---------------------------------------------------------------- operations

pub fn mkdir(roots: &Roots, rel: &str) -> R<String> {
    let rel = norm_rel(rel);
    let (r, inner) = roots.split(&rel).ok_or_else(|| tr!("fs.noRoot"))?;
    let parts: Vec<String> = inner.split('/').filter(|x| !x.is_empty()).map(safe_part).collect::<R<_>>()?;
    if parts.is_empty() {
        return Err(tr!("fs.noFolderName"));
    }
    let name = Roots::join(&r.label, &parts.join("/"));
    let (_, p) = under(roots, &name)?;
    if p.exists() {
        return Err(tr!("fs.folderExists"));
    }
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(name)
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MovePlan {
    pub src: String,
    pub dst: String,
    /// The name was taken: it lands as "Name (2)".
    pub renamed: bool,
    /// Samples its projects would lose: [project, sample].
    pub risks: Vec<(String, String)>,
    pub is_dir: bool,
}

/// Where `src` would land in `dest_dir`, and what could go wrong. Nothing is touched.
pub fn plan_move(roots: &Roots, env: &als::Env, src_rel: &str, dest_dir_rel: &str, check: bool) -> R<MovePlan> {
    let (sroot, src) = under(roots, src_rel)?;
    let (droot, dest_dir) = under(roots, dest_dir_rel)?;
    if norm_rel(src_rel).is_empty() || real(&src) == real(&sroot) {
        return Err(tr!("fs.moveRoot"));
    }
    if real(&sroot) != real(&droot) {
        return Err(tr!("fs.crossRoot"));
    }
    if !src.exists() {
        return Err(tr!("fs.notFound", path = norm_rel(src_rel)));
    }
    if src.is_dir() && inside(&dest_dir, &src) {
        return Err(tr!("fs.intoItself"));
    }
    if src.parent().is_some_and(|p| real(p) == real(&dest_dir)) {
        return Err(tr!("fs.alreadyThere"));
    }
    let file = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dst = free_name(&dest_dir, &file);
    let risks = if check && src.is_dir() { als::move_risks(&src, env) } else { Vec::new() };
    Ok(MovePlan {
        src: norm_rel(src_rel),
        dst: name_of(roots, &dst)?,
        renamed: dst.file_name().map(|n| n.to_string_lossy() != file).unwrap_or(false),
        risks,
        is_dir: src.is_dir(),
    })
}

/// Moves and makes the base follow. `dst_rel` must be free (see plan_move).
pub fn do_move(con: &Connection, roots: &Roots, src_rel: &str, dst_rel: &str) -> R<()> {
    let (_, src) = under(roots, src_rel)?;
    let (_, dst) = under(roots, dst_rel)?;
    if dst.exists() {
        return Err(tr!("fs.destExists", path = norm_rel(dst_rel)));
    }
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&src, &dst).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
            tr!("fs.locked")
        } else {
            e.to_string()
        }
    })?;
    // the base could not follow: the folder goes back where it was, so disk and base agree
    if let Err(e) = repath(con, roots, src_rel, dst_rel) {
        return match std::fs::rename(&dst, &src) {
            Ok(()) => Err(tr!("fs.baseNotUpdated", e = e)),
            Err(back) => Err(tr!("fs.baseNotUpdatedStuck", e = e, path = norm_rel(dst_rel), back = back)),
        };
    }
    Ok(())
}

/// One journal step: ["move" | "copy" | "mkdir", src, dst].
pub type Op = (String, String, String);

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Skipped {
    pub src: String,
    pub why: String,
    pub risks: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct MoveResult {
    pub done: Vec<String>,
    pub skipped: Vec<Skipped>,
    #[serde(skip)]
    #[ts(skip)]
    pub ops: Vec<Op>,
}

/// Several moves, one journal. A move whose projects would lose a sample is skipped.
pub fn move_many(con: &Connection, roots: &Roots, env: &als::Env, moves: &[(String, String)]) -> MoveResult {
    let mut out = MoveResult::default();
    for (src, dest) in moves {
        let plan = match plan_move(roots, env, src, dest, true) {
            Ok(p) => p,
            Err(why) => {
                out.skipped.push(Skipped { src: src.clone(), why, risks: vec![] });
                continue;
            }
        };
        if !plan.risks.is_empty() {
            out.skipped.push(Skipped {
                src: src.clone(),
                why: tr!("fs.samplesLost"),
                risks: plan.risks.into_iter().take(10).collect(),
            });
            continue;
        }
        // folders the move creates are journaled too: an undo removes them once empty
        let mut made = Vec::new();
        if let Ok((_, dst)) = under(roots, &plan.dst) {
            let mut p = dst.parent().map(Path::to_path_buf);
            while let Some(d) = p.filter(|d| !d.exists()) {
                if let Ok(n) = name_of(roots, &d) {
                    made.insert(0, n);
                }
                p = d.parent().map(Path::to_path_buf);
            }
        }
        if let Err(why) = do_move(con, roots, &plan.src, &plan.dst) {
            out.skipped.push(Skipped { src: src.clone(), why, risks: vec![] });
            continue;
        }
        out.ops.extend(made.into_iter().map(|d| ("mkdir".to_string(), String::new(), d)));
        out.ops.push(("move".into(), plan.src, plan.dst.clone()));
        out.done.push(plan.dst);
    }
    out
}

/// File names (lower case, no extension) the projects use as samples.
fn used_sample_names(con: &Connection) -> HashSet<String> {
    let mut out = HashSet::new();
    if let Ok(mut s) = con.prepare("select data from files") {
        let rows: Vec<String> =
            s.query_map([], |r| r.get::<_, Option<String>>(0)).map(|m| m.flatten().flatten().collect()).unwrap_or_default();
        for d in rows {
            let v: Value = serde_json::from_str(&d).unwrap_or(Value::Null);
            for n in v["sample_names"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                out.insert(n.to_lowercase());
            }
        }
    }
    out
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)?.flatten() {
            copy_tree(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        repair::safe::copy_new(src, dst)
    }
}

fn is_audio(name: &str) -> bool {
    catalog::kind_of(name) == "audio"
}

fn stem_low(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Moves files / folders into a song's mood board folder. An audio file a project uses as a
/// sample is copied instead. Returns the journal.
pub fn to_board(con: &Connection, roots: &Roots, board: &Path, items: &[String]) -> R<Vec<Op>> {
    let mut ops: Vec<Op> = Vec::new();
    match to_board_steps(con, roots, board, items, &mut ops) {
        Ok(()) => Ok(ops),
        // stopped midway: what already moved goes back, so a failed step leaves no trace
        Err(e) => match undo_ops(con, roots, &ops) {
            Ok(()) => Err(e),
            Err(back) => Err(tr!("fs.putBackFailed", e = e, back = back)),
        },
    }
}

fn to_board_steps(con: &Connection, roots: &Roots, board: &Path, items: &[String], ops: &mut Vec<Op>) -> R<()> {
    let mut made = Vec::new();
    let mut p = board.to_path_buf();
    while !p.exists() {
        made.insert(0, name_of(roots, &p)?);
        match p.parent() {
            Some(q) => p = q.to_path_buf(),
            None => break,
        }
    }
    std::fs::create_dir_all(board).map_err(|e| e.to_string())?;
    ops.extend(made.into_iter().map(|d| ("mkdir".to_string(), String::new(), d)));
    let used = used_sample_names(con);
    for rel in items {
        let (_, src) = under(roots, rel)?;
        if !src.exists() {
            continue;
        }
        if inside(board, &src) {
            return Err(tr!("fs.intoItself"));
        }
        let file = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dst = free_name(board, &file);
        let dst_rel = name_of(roots, &dst)?;
        let sample = src.is_file() && is_audio(&file) && used.contains(&stem_low(&src));
        let has_sample = src.is_dir() && walk_files(&src).iter().any(|f| is_audio(&f.to_string_lossy()) && used.contains(&stem_low(f)));
        if sample || has_sample {
            // journaled first: a copy that stops halfway is set aside with the rest
            ops.push(("copy".into(), norm_rel(rel), dst_rel));
            copy_tree(&src, &dst).map_err(|e| e.to_string())?;
        } else {
            do_move(con, roots, rel, &dst_rel)?;
            ops.push(("move".into(), norm_rel(rel), dst_rel));
        }
    }
    Ok(())
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(e.path());
            } else {
                out.push(e.path());
            }
        }
    }
    out
}

/// Plays a journal backwards, last step first. A copy the app made is set aside in a
/// ".corbeille" next to it, never deleted. All or nothing: a step that fails puts the steps
/// already played back where they were, so the disk is as the base says, before or after.
pub fn undo_ops(con: &Connection, roots: &Roots, ops: &[Op]) -> R<()> {
    // what was done on disk, to play forward again if a later step fails
    enum Done {
        Moved(String, String),
        Renamed(PathBuf, PathBuf),
        Removed(PathBuf),
    }
    let mut done: Vec<Done> = Vec::new();
    let step = |kind: &str, src: &str, dst: &str, done: &mut Vec<Done>| -> R<()> {
        match kind {
            "move" => {
                if under(roots, src)?.1.exists() {
                    return Err(tr!("fs.cannotPutBack", path = src));
                }
                do_move(con, roots, dst, src)?;
                done.push(Done::Moved(src.to_string(), dst.to_string()));
            }
            "copy" => {
                let (_, p) = under(roots, dst)?;
                if p.exists() {
                    let bin = p.parent().map(|d| d.join(".corbeille")).ok_or_else(|| tr!("fs.notFoundShort"))?;
                    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let to = free_name(&bin, &name);
                    std::fs::rename(&p, &to).map_err(|e| e.to_string())?;
                    done.push(Done::Renamed(to, p));
                }
            }
            "mkdir" => {
                let (_, p) = under(roots, dst)?;
                if p.is_dir() && std::fs::read_dir(&p).map(|mut r| r.next().is_none()).unwrap_or(false) && std::fs::remove_dir(&p).is_ok() {
                    done.push(Done::Removed(p));
                }
            }
            _ => {}
        }
        Ok(())
    };
    for (kind, src, dst) in ops.iter().rev() {
        if let Err(e) = step(kind, src, dst, &mut done) {
            let mut stuck = Vec::new();
            for d in done.iter().rev() {
                let r = match d {
                    Done::Moved(src, dst) => do_move(con, roots, src, dst),
                    Done::Renamed(from, to) => std::fs::rename(from, to).map_err(|e| e.to_string()),
                    Done::Removed(p) => std::fs::create_dir(p).map_err(|e| e.to_string()),
                };
                if let Err(x) = r {
                    stuck.push(x);
                }
            }
            return Err(if stuck.is_empty() {
                tr!("fs.nothingMoved", e = e)
            } else {
                tr!("fs.someStuck", e = e, list = stuck.join(" ; "))
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- browsing

/// Every folder of the projects folders (Ableton's own sub-folders left out), for pickers.
pub fn all_folders(roots: &Roots, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    for r in &roots.list {
        let base = PathBuf::from(&r.path);
        if !r.label.is_empty() {
            out.push(r.label.clone());
        }
        let mut stack = vec![base.clone()];
        while let Some(d) = stack.pop() {
            let mut subs: Vec<PathBuf> = std::fs::read_dir(&d)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    !SKIP_DIRS.contains(&n.as_str()) && !n.starts_with('.')
                        && !(d == base && r.label.is_empty() && roots.labels().iter().any(|l| l.eq_ignore_ascii_case(&n)))
                })
                .map(|e| e.path())
                .collect();
            subs.sort();
            for s in subs.iter().rev() {
                stack.push(s.clone());
            }
            if d != base
                && let Some(n) = roots.rel(&d)
            {
                out.push(n);
            }
            if out.len() >= limit {
                return out;
            }
        }
    }
    out.sort_by_key(|a| a.to_lowercase());
    out
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MediaItem {
    pub name: String,
    pub rel: String,
    pub kind: String,
    #[ts(type = "number")]
    pub size: u64,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MediaFolder {
    pub dir: String,
    #[ts(type = "Record<string, number>")]
    pub counts: BTreeMap<String, i64>,
    pub items: Vec<MediaItem>,
    #[ts(type = "number")]
    pub n: i64,
}

/// Every folder (all the way down) holding media and no project of its own, outside a folder
/// that is ONE song's: the user's loose mood boards, references, pictures. A folder shared by
/// many songs (loose .als side by side) does not own its sub-folders, so they are listed.
/// `song_dirs`: each song folder with how many songs it holds.
pub fn media_folders(roots: &Roots, song_dirs: &[(String, i64)]) -> Vec<MediaFolder> {
    let own: Vec<&String> = song_dirs.iter().filter(|(d, n)| !d.is_empty() && *n == 1).map(|(d, _)| d).collect();
    let mut out = Vec::new();
    for r in &roots.list {
        let base = PathBuf::from(&r.path);
        // every folder, with its files
        let mut all: Vec<(PathBuf, Vec<String>)> = Vec::new();
        let mut stack = vec![base.clone()];
        while let Some(d) = stack.pop() {
            let mut files = Vec::new();
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(e.path());
                } else {
                    files.push(e.file_name().to_string_lossy().into_owned());
                }
            }
            all.push((d, files));
        }
        for (d, files) in &all {
            if files.iter().any(|f| f.to_lowercase().ends_with(".als")) {
                continue;
            }
            let Some(rel) = roots.rel(d) else { continue };
            if rel.split('/').any(|x| SKIP_DIRS.contains(&x.to_lowercase().as_str()) || x.starts_with('.')) {
                continue;
            }
            if own.iter().any(|s| rel == **s || rel.starts_with(&format!("{s}/"))) {
                continue;
            }
            let mut media: Vec<&String> = files.iter().filter(|f| catalog::kind_of(f) != "file").collect();
            if media.is_empty() {
                continue;
            }
            media.sort();
            let mut items = Vec::new();
            for f in media.iter().take(400) {
                let Ok(md) = std::fs::metadata(d.join(f)) else { continue };
                let mtime = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|x| x.as_secs_f64()).unwrap_or(0.0);
                items.push(MediaItem { name: (*f).clone(), rel: format!("{rel}/{f}"), kind: catalog::kind_of(f).into(), size: md.len(), mtime });
            }
            let mut counts = BTreeMap::new();
            for it in &items {
                *counts.entry(it.kind.clone()).or_insert(0) += 1;
            }
            out.push(MediaFolder { dir: rel, counts, items, n: media.len() as i64 });
        }
    }
    out.sort_by_key(|m| m.dir.to_lowercase());
    out
}

// ---------------------------------------------------------------- reorganise

fn norm_name(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReorgMove {
    pub src: String,
    pub dest: String,
    pub dst: String,
    pub songs: Vec<String>,
    pub artist: String,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct ReorgPlan {
    pub moves: Vec<ReorgMove>,
    pub skipped: Vec<Skipped>,
}

pub struct SongRow {
    pub song_dir: String,
    pub artist: Option<String>,
    pub style: Option<String>,
    pub title: Option<String>,
}

/// Moves that would file each song folder under its artist (and style / current EP), in the
/// main projects folder. Nothing is touched.
/// template: "artist" -> Artist/Song, "artist_style" -> Artist/Style/Song,
/// "artist_album" -> Artist/<current parent folder>/Song.
pub fn reorg_plan(roots: &Roots, songs: &[SongRow], template: &str) -> ReorgPlan {
    let Some(main) = roots.main() else { return ReorgPlan::default() };
    let base = PathBuf::from(&main.path);
    let mut top: HashMap<String, Vec<String>> = HashMap::new();
    let mut names: Vec<String> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for n in names {
        top.entry(norm_name(&n)).or_default().push(n);
    }
    // only the main folder's songs (the other folders keep their own order)
    let labels = roots.labels();
    let mine: Vec<&SongRow> = songs
        .iter()
        .filter(|s| !labels.iter().any(|l| s.song_dir == *l || s.song_dir.starts_with(&format!("{l}/"))))
        .collect();
    let artist_folder = |artist: &str| -> String {
        let cands = top.get(&norm_name(artist)).cloned().unwrap_or_default();
        if cands.is_empty() {
            return safe_part(artist).unwrap_or_else(|_| artist.to_string());
        }
        let held = |c: &String| {
            mine.iter().filter(|x| x.artist.as_deref().unwrap_or("").trim() == artist && norm_rel(&x.song_dir).starts_with(&format!("{c}/"))).count()
        };
        cands.iter().max_by_key(|c| held(c)).cloned().unwrap_or_default()
    };
    let mut units: BTreeMap<String, Vec<&SongRow>> = BTreeMap::new();
    for s in &mine {
        let d = norm_rel(&s.song_dir);
        if !d.is_empty() {
            units.entry(d).or_default().push(s);
        }
    }
    let dirs: Vec<String> = units.keys().cloned().collect();
    let inner: HashSet<&String> = dirs.iter().filter(|d| dirs.iter().any(|o| o != *d && d.starts_with(&format!("{o}/")))).collect();
    let mut plan = ReorgPlan::default();
    for d in &dirs {
        if inner.contains(d) {
            continue;
        }
        let group = &units[d];
        let artists: HashSet<String> = group.iter().map(|s| s.artist.as_deref().unwrap_or("").trim().to_string()).collect();
        let one = artists.iter().next().cloned().unwrap_or_default();
        if artists.len() != 1 || one.is_empty() {
            plan.skipped.push(Skipped {
                src: d.clone(),
                why: if artists.len() > 1 { tr!("fs.reorg.manyArtists") } else { tr!("fs.reorg.noArtist") },
                risks: vec![],
            });
            continue;
        }
        let mut parent = vec![artist_folder(&one)];
        let path = Path::new(d);
        if template == "artist_style" {
            let style = group[0].style.as_deref().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
            parent.push(if style.is_empty() {
                "Sans style".into()
            } else {
                let mut c = style.chars();
                let cap = c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default();
                safe_part(&cap).unwrap_or(cap)
            });
        } else if template == "artist_album" {
            let cur = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if !cur.is_empty() && norm_name(&cur) != norm_name(&one) {
                parent.push(cur);
            }
        }
        let target = parent.join("/");
        let cur_parent = path.parent().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        if cur_parent.to_lowercase() == target.to_lowercase() {
            continue;
        }
        let leaf = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        plan.moves.push(ReorgMove {
            src: d.clone(),
            dest: target.clone(),
            dst: format!("{target}/{leaf}"),
            songs: group.iter().take(6).map(|s| s.title.clone().unwrap_or_default()).collect(),
            artist: one,
        });
    }
    plan
}

/// The journal entry of a file operation, as undo_log stores it.
pub fn journal(ops: &[Op]) -> Value {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    json!({"kind": "ops", "ops": ops, "at": now})
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    fn tree(name: &str) -> (PathBuf, Roots, store::Store) {
        let base = std::env::temp_dir().join(format!("spool-fsops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["a", "b", "dest", "songs"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        std::fs::write(base.join("a/one.txt"), "a").unwrap();
        std::fs::write(base.join("b/two.txt"), "b").unwrap();
        std::fs::write(base.join("x.png"), "img").unwrap();
        let roots = Roots { list: vec![store::Root { label: String::new(), path: base.to_string_lossy().into_owned(), artist: String::new() }] };
        (base, roots, store::Store::memory().unwrap())
    }

    #[test]
    fn an_undo_that_fails_midway_puts_back_what_it_moved() {
        let (base, roots, st) = tree("undo");
        do_move(&st.con, &roots, "a", "dest/a").unwrap();
        do_move(&st.con, &roots, "b", "dest/b").unwrap();
        let ops: Vec<Op> = vec![("move".into(), "a".into(), "dest/a".into()), ("move".into(), "b".into(), "dest/b".into())];
        // someone made a new "a" meanwhile: "b" goes back first, then "a" cannot
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::write(base.join("a/new.txt"), "new").unwrap();
        let e = undo_ops(&st.con, &roots, &ops).unwrap_err();
        assert!(e.contains("rien n'a bougé"), "{e}");
        // all or nothing: "b" is back in dest, the new "a" untouched, the moved "a" still in dest
        assert!(base.join("dest/b/two.txt").is_file());
        assert!(!base.join("b").exists());
        assert_eq!(std::fs::read_to_string(base.join("a/new.txt")).unwrap(), "new");
        assert!(base.join("dest/a/one.txt").is_file());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_board_move_that_fails_midway_leaves_no_trace() {
        let (base, roots, st) = tree("board");
        let board = base.join("songs/_moodboard");
        // the picture moves, then a folder holding the board itself is refused
        let e = to_board(&st.con, &roots, &board, &["x.png".into(), "songs".into()]).unwrap_err();
        assert!(e.contains("lui-même"), "{e}");
        assert_eq!(std::fs::read_to_string(base.join("x.png")).unwrap(), "img");
        assert!(!board.exists(), "the board folder it made is gone again (it was empty)");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_full_undo_restores_everything() {
        let (base, roots, st) = tree("full");
        let board = base.join("songs/_moodboard");
        let ops = to_board(&st.con, &roots, &board, &["x.png".into(), "a".into()]).unwrap();
        assert!(board.join("x.png").is_file() && board.join("a/one.txt").is_file());
        undo_ops(&st.con, &roots, &ops).unwrap();
        assert_eq!(std::fs::read_to_string(base.join("x.png")).unwrap(), "img");
        assert!(base.join("a/one.txt").is_file());
        assert!(!board.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}

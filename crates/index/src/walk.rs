//! Walking the projects folder, and the index of sample files used to find moved samples.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub const AUDIO_EXT: [&str; 7] = [".wav", ".mp3", ".aif", ".aiff", ".flac", ".m4a", ".ogg"];
/// Never walked into. Mood board sounds are references, not renders of the song.
const SKIP_DIRS: [&str; 6] =
    ["samples", "ableton project info", "backup", "$recycle.bin", "system volume information", "_moodboard"];

/// (size, mtime, ctime) of a file.
pub type Stat = (i64, f64, f64);

#[derive(Default)]
pub struct Walk {
    /// Every .als, by path relative to the root ("/" separated).
    pub als: BTreeMap<String, Stat>,
    /// Every audio file outside Samples folders.
    pub audio: BTreeMap<String, Stat>,
    /// Every MIDI file, Samples folders included (producers keep their MIDI parts there).
    pub midi: BTreeMap<String, Stat>,
    /// Ableton's automatic backups, only counted per folder.
    pub backups: BTreeMap<String, i64>,
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> f64 {
    t.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default()
}

pub fn walk(root: &Path) -> Walk {
    let mut w = Walk::default();
    walk_into(&mut w, root, "", &[], &mut |_, _| {});
    w
}

/// Every projects folder into one walk: the other folders' names carry their label (see
/// store::Roots). `tick(files found, folder being read)` reports progress.
/// The MIDI files of library folders (construction kits, packs): named by their full path,
/// nothing else read.
pub fn walk_midi_dirs(dirs: &[String], w: &mut Walk, tick: &mut dyn FnMut(usize, &Path), stop: &dyn Fn() -> bool) {
    for d in dirs {
        let mut stack = vec![PathBuf::from(d)];
        let mut seen = 0usize;
        while let Some(dir) = stack.pop() {
            if stop() {
                return;
            }
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            seen += 1;
            if seen % 50 == 1 {
                tick(w.midi.len(), &dir);
            }
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    stack.push(e.path());
                    continue;
                }
                let low = e.file_name().to_string_lossy().to_lowercase();
                if (low.ends_with(".mid") || low.ends_with(".midi")) && !low.starts_with("._")
                    && let Ok(md) = e.metadata()
                {
                    w.midi.insert(e.path().to_string_lossy().replace('\\', "/"), (md.len() as i64, secs(md.modified()), secs(md.created())));
                }
            }
        }
    }
}

pub fn walk_roots(roots: &store::Roots, tick: &mut dyn FnMut(usize, &Path)) -> Walk {
    let mut w = Walk::default();
    let labels = roots.labels();
    for r in &roots.list {
        let skip: &[String] = if r.label.is_empty() { &labels } else { &[] };
        walk_into(&mut w, Path::new(&r.path), &r.label, skip, tick);
    }
    w
}

/// `label`: prefix of every name; `skip_top`: top-level entries left out (another folder's
/// label, so a name is never read two ways).
fn walk_into(w: &mut Walk, root: &Path, label: &str, skip_top: &[String], tick: &mut dyn FnMut(usize, &Path)) {
    let name_of = |p: &Path| store::Roots::join(label, &rel_of(root, p));
    // (folder, only MIDI files wanted there: a Samples folder)
    let mut stack = vec![(root.to_path_buf(), false)];
    let mut seen = 0usize;
    while let Some((dir, midi_only)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        seen += 1;
        if seen % 50 == 1 {
            tick(w.als.len() + w.audio.len(), &dir);
        }
        let top = dir == root;
        let mut subdirs = Vec::new();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if top && skip_top.iter().any(|l| l.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let low = name.to_lowercase();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if low == "backup" {
                    let n = std::fs::read_dir(e.path())
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|f| f.file_name().to_string_lossy().to_lowercase().ends_with(".als"))
                        .count() as i64;
                    if n > 0 {
                        w.backups.insert(name_of(&dir), n);
                    }
                }
                // dot folders are kept: ".in the future" and ".bof" hold real songs
                if !SKIP_DIRS.contains(&low.as_str()) {
                    subdirs.push((e.path(), midi_only));
                } else if low == "samples" {
                    subdirs.push((e.path(), true));
                }
                continue;
            }
            if name.starts_with("._") {
                continue;
            }
            if low.ends_with(".mid") || low.ends_with(".midi") {
                if let Ok(md) = e.metadata() {
                    w.midi.insert(name_of(&e.path()), (md.len() as i64, secs(md.modified()), secs(md.created())));
                }
                continue;
            }
            if midi_only {
                continue;
            }
            let is_als = low.ends_with(".als");
            if !is_als && !AUDIO_EXT.iter().any(|x| low.ends_with(x)) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            let st = (md.len() as i64, secs(md.modified()), secs(md.created()));
            let rel = name_of(&e.path());
            if is_als {
                w.als.insert(rel, st)
            } else {
                w.audio.insert(rel, st)
            };
        }
        stack.extend(subdirs.into_iter().rev());
    }
    tick(w.als.len() + w.audio.len(), root);
}

/// A sample file: (size, full path).
pub type Hit = (i64, String);

/// Sample files by lower-case name, in walking order.
pub type SampleIndex = HashMap<String, Vec<Hit>>;

pub fn sample_index(roots: &[PathBuf]) -> SampleIndex {
    let mut idx: SampleIndex = HashMap::new();
    for r in roots {
        let mut stack = vec![r.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut subdirs = Vec::new();
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    subdirs.push(e.path());
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_lowercase();
                if als::SAMPLE_EXT.iter().any(|x| name.ends_with(x))
                    && let Ok(md) = e.metadata()
                {
                    idx.entry(name).or_default().push((md.len() as i64, e.path().to_string_lossy().to_string()));
                }
            }
            stack.extend(subdirs.into_iter().rev());
        }
    }
    idx
}

/// Among files with the same name (and size), the one whose folders match the lost path's
/// most closely from the end, then share the most folder names: generic names like
/// " (Gelé)-1.aif" exist in many projects.
pub fn best_hit<'h>(path: &str, hits: &'h [(i64, String)]) -> Option<&'h (i64, String)> {
    let split = |p: &str| -> Vec<String> {
        let p = p.replace('\\', "/");
        let mut parts: Vec<String> = p.split('/').map(str::to_lowercase).collect();
        parts.pop();
        parts
    };
    let want = split(path);
    let score = |h: &(i64, String)| {
        let have = split(&h.1);
        let n = want.iter().rev().zip(have.iter().rev()).take_while(|(a, b)| a == b).count();
        let shared = want.iter().collect::<std::collections::HashSet<_>>().intersection(&have.iter().collect()).count();
        (n, shared)
    };
    let mut best: Option<(&Hit, (usize, usize))> = None;
    for h in hits {
        let s = score(h);
        if best.as_ref().is_none_or(|(_, b)| s > *b) {
            best = Some((h, s));
        }
    }
    best.map(|b| b.0)
}

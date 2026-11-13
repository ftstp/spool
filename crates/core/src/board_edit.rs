//! Changing a song's mood board: notes (ideas, lyrics), pictures added, items set aside,
//! order / palette / captions. Ported from the prototype's board.py. Nothing is ever
//! deleted: a removed item goes to the board's `.corbeille` folder.
use std::path::{Path, PathBuf};

/// The largest file added at once.
pub const MAX_UPLOAD: usize = 300 * 1024 * 1024;
const TRASH: &str = ".corbeille";
const META: &str = "board.json";

/// A file name that is safe on every system, without any folder part.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let s: String =
        base.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 0x20 { '_' } else { c }).collect();
    let s = s.trim_matches(|c| c == ' ' || c == '.');
    let s = if s.is_empty() { "fichier" } else { s };
    s.chars().take(120).collect()
}

/// `name`, or "name (2)", "name (3)"... the first one that does not exist yet.
pub fn unique(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (2..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap()
}

/// A file of the board itself (never outside it, never in a sub folder).
fn inside(dir: &Path, name: &str) -> Option<PathBuf> {
    let p = dir.join(safe_name(name));
    (p.parent() == Some(dir) && p.is_file()).then_some(p)
}

/// What a note was before a save: its file, and its old text (None: the note is new).
pub struct NoteSaved {
    pub name: String,
    pub path: PathBuf,
    pub old: Option<String>,
}

/// The version a note had, kept in the board's `.corbeille` before it is replaced
/// ("paroles (avant 2026-11-13 0412).md"), never overwritten.
pub fn keep_version(path: &Path) -> Result<(), String> {
    let Some(dir) = path.parent() else { return Err(i18n::tr!("board.noteNotFound")) };
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let t = dir.join(TRASH);
    std::fs::create_dir_all(&t).map_err(|e| e.to_string())?;
    let dest = unique(&t, &format!("{stem} (avant {}){ext}", chrono::Local::now().format("%Y-%m-%d %H%M%S")));
    repair::safe::copy_new(path, &dest).map_err(|e| i18n::tr!("board.versionNotKept", e = e))
}

/// Writes whole or not at all: a temporary file, then put in place.
pub fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    let dir = path.parent().ok_or_else(|| i18n::tr!("err.folderNotFound"))?;
    let tmp = unique(dir, &format!(".{}.tmp", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Writes a note: a new idea, the lyrics, or an existing note edited. The version it replaces
/// is kept first (in `.corbeille`), and given back for Ctrl+Z.
pub fn save_note(dir: &Path, text: &str, lyrics: bool, name: Option<&str>) -> Result<NoteSaved, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = match name.filter(|n| !n.is_empty()) {
        Some(n) => {
            let p = dir.join(safe_name(n));
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if p.parent() != Some(dir) || !(ext == "md" || ext == "txt") {
                return Err(i18n::tr!("board.noteNotFound"));
            }
            p
        }
        None => {
            let base = if lyrics {
                "paroles.md".to_string()
            } else {
                format!("idee {}.md", chrono::Local::now().format("%Y-%m-%d %H%M%S"))
            };
            unique(dir, &base)
        }
    };
    let old = if p.is_file() {
        let b = std::fs::read(&p).map_err(|e| e.to_string())?;
        let old = String::from_utf8_lossy(&b).into_owned();
        if old == text {
            return Ok(NoteSaved { name: p.file_name().unwrap().to_string_lossy().into_owned(), path: p, old: Some(old) });
        }
        keep_version(&p)?;
        Some(old)
    } else {
        None
    };
    write_whole(&p, text)?;
    Ok(NoteSaved { name: p.file_name().unwrap().to_string_lossy().into_owned(), path: p, old })
}

/// A file added to the board (a picture dropped on the sheet). Returns its name.
pub fn add_file(dir: &Path, name: &str, data: &[u8]) -> Result<String, String> {
    if data.len() > MAX_UPLOAD {
        return Err(i18n::tr!("board.tooBig"));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = unique(dir, &safe_name(name));
    std::fs::write(&p, data).map_err(|e| e.to_string())?;
    Ok(p.file_name().unwrap().to_string_lossy().into_owned())
}

/// Set aside, never deleted: moved to the board's `.corbeille`.
pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let p = inside(dir, name).ok_or_else(|| i18n::tr!("board.itemNotFound"))?;
    let t = dir.join(TRASH);
    std::fs::create_dir_all(&t).map_err(|e| e.to_string())?;
    let dest = unique(&t, &p.file_name().unwrap().to_string_lossy());
    std::fs::rename(&p, dest).map_err(|e| e.to_string())
}

/// Order, palette and captions, in the board's board.json (other keys kept).
pub fn set_meta(dir: &Path, patch: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    let path = dir.join(META);
    let mut meta: serde_json::Map<String, serde_json::Value> =
        std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    for k in ["palette", "order", "captions"] {
        if let Some(v) = patch.get(k) {
            meta.insert(k.into(), v.clone());
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{META}.tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(safe_name("../../x/a:b?.png"), "a_b_.png");
        assert_eq!(safe_name(" .. "), "fichier");
        assert_eq!(safe_name("C:\\temp\\photo.jpg"), "photo.jpg");
    }

    #[test]
    fn board_round_trip() {
        let dir = std::env::temp_dir().join(format!("pf-board-{}", std::process::id())).join("_moodboard");
        let n = save_note(&dir, "la la", true, None).unwrap();
        assert_eq!(n.name, "paroles.md");
        assert!(n.old.is_none());
        assert_eq!(save_note(&dir, "encore", true, None).unwrap().name, "paroles (2).md");
        let n = save_note(&dir, "edited", false, Some("paroles.md")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("paroles.md")).unwrap(), "edited");
        // the replaced version is kept aside, untouched, and handed back for Ctrl+Z
        assert_eq!(n.old.as_deref(), Some("la la"));
        let kept: Vec<_> = std::fs::read_dir(dir.join(".corbeille")).unwrap().flatten().collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read_to_string(kept[0].path()).unwrap(), "la la");
        // the same text again writes nothing and keeps no new version
        save_note(&dir, "edited", false, Some("paroles.md")).unwrap();
        assert_eq!(std::fs::read_dir(dir.join(".corbeille")).unwrap().count(), 1);
        assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
        assert!(save_note(&dir, "x", false, Some("../evil.md")).is_ok_and(|n| n.name == "evil.md"));
        assert!(save_note(&dir, "x", false, Some("image.png")).is_err());
        assert_eq!(add_file(&dir, "cover.png", b"png").unwrap(), "cover.png");
        remove(&dir, "cover.png").unwrap();
        assert!(dir.join(".corbeille").join("cover.png").is_file());
        assert!(remove(&dir, "cover.png").is_err());
        let patch = serde_json::json!({"palette": ["#112233"], "other": 1});
        set_meta(&dir, patch.as_object().unwrap()).unwrap();
        let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(META)).unwrap()).unwrap();
        assert_eq!(m, serde_json::json!({"palette": ["#112233"]}));
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}

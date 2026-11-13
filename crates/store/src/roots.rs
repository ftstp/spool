//! The projects folders. The first one ("root" in the settings) is the main folder: its
//! files are named by their path inside it, as they always were. Every other folder
//! ("more_roots") gets a label, and its files are named "<label>/<path inside it>". So adding
//! or removing a folder never renames the files of the others, and a name always says which
//! folder it lives in.
//!
//! A label is never the name of a top-level entry of the main folder (checked when the
//! folder is added, and the main folder's walk skips such an entry), so a name cannot be
//! read two ways.
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Root {
    /// Empty for the main folder.
    pub label: String,
    pub path: String,
    /// The artist of every song of this folder that names none ("NOVA" for
    /// C:/Users/NOVA/NOVA/Projects). Empty: none.
    #[serde(default)]
    pub artist: String,
}

#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub list: Vec<Root>,
}

fn setting<T: serde::de::DeserializeOwned>(con: &Connection, k: &str) -> Option<T> {
    let v: Option<String> = con.query_row("select v from settings where k=?", [k], |r| r.get(0)).optional().ok()??;
    serde_json::from_str(&v?).ok()
}

impl Roots {
    pub fn load(con: &Connection) -> Roots {
        let mut list = Vec::new();
        if let Some(p) = setting::<String>(con, "root").filter(|p| !p.is_empty()) {
            list.push(Root { label: String::new(), path: p, artist: setting::<String>(con, "root_artist").unwrap_or_default() });
        }
        list.extend(setting::<Vec<Root>>(con, "more_roots").unwrap_or_default().into_iter().filter(|r| !r.label.is_empty()));
        Roots { list }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn main(&self) -> Option<&Root> {
        self.list.iter().find(|r| r.label.is_empty())
    }

    /// The labels of the other folders (what the main folder's walk must skip).
    pub fn labels(&self) -> Vec<String> {
        self.list.iter().filter(|r| !r.label.is_empty()).map(|r| r.label.clone()).collect()
    }

    /// (the folder, its label, the path inside it) of a name.
    pub fn split<'a>(&'a self, rel: &'a str) -> Option<(&'a Root, &'a str)> {
        let first = rel.split('/').next().unwrap_or("");
        if let Some(r) = self.list.iter().find(|r| !r.label.is_empty() && r.label == first) {
            return Some((r, rel.get(first.len() + 1..).unwrap_or("")));
        }
        self.main().map(|r| (r, rel))
    }

    /// The real path of a name (None: no folder holds it).
    pub fn abs(&self, rel: &str) -> Option<PathBuf> {
        let (r, inner) = self.split(rel)?;
        Some(if inner.is_empty() { PathBuf::from(&r.path) } else { Path::new(&r.path).join(inner) })
    }

    /// The name of a real path under one of the folders.
    pub fn rel(&self, p: &Path) -> Option<String> {
        // the deepest folder wins (none is inside another, but be safe)
        let mut best: Option<(&Root, usize, String)> = None;
        for r in &self.list {
            if let Ok(inner) = p.strip_prefix(&r.path) {
                let depth = Path::new(&r.path).components().count();
                if best.as_ref().is_none_or(|b| depth > b.1) {
                    best = Some((r, depth, inner.to_string_lossy().replace('\\', "/")));
                }
            }
        }
        best.map(|(r, _, inner)| Self::join(&r.label, &inner))
    }

    /// "<label>/<inner>", or `inner` for the main folder.
    pub fn join(label: &str, inner: &str) -> String {
        match (label.is_empty(), inner.is_empty()) {
            (true, _) => inner.to_string(),
            (false, true) => label.to_string(),
            (false, false) => format!("{label}/{inner}"),
        }
    }

    /// Whether `p` lies in one of the folders (both canonicalized: case and links resolved).
    pub fn contains(&self, p: &Path) -> bool {
        let Ok(p) = p.canonicalize() else { return false };
        self.list.iter().any(|r| Path::new(&r.path).canonicalize().is_ok_and(|r| p.starts_with(r)))
    }

    /// A label for a new folder: its own name, made unique against the other labels and the
    /// main folder's top-level entries.
    pub fn new_label(&self, path: &Path) -> String {
        // a folder named "Projets" says nothing: the one above it does ("NOVA/Projets")
        const GENERIC: [&str; 9] = ["projets", "projects", "projet", "project", "ableton", "musique", "music", "prod", "sessions"];
        let leaf = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let path = if GENERIC.contains(&leaf.trim()) { path.parent().filter(|p| p.file_name().is_some()).unwrap_or(path) } else { path };
        let base: String = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().chars().filter(|c| c.is_alphanumeric()).collect())
            .trim()
            .to_string();
        let base = if base.is_empty() { "Dossier".to_string() } else { base };
        let taken = |l: &str| {
            self.list.iter().any(|r| r.label.eq_ignore_ascii_case(l))
                || self.main().is_some_and(|m| Path::new(&m.path).join(l).exists())
        };
        if !taken(&base) {
            return base;
        }
        (2..).map(|i| format!("{base} ({i})")).find(|l| !taken(l)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> Roots {
        Roots {
            list: vec![
                Root { label: String::new(), path: "C:/Musique".into(), artist: String::new() },
                Root { label: "Prod".into(), path: "D:/Prod".into(), artist: String::new() },
            ],
        }
    }

    #[test]
    fn names_resolve_both_ways() {
        let r = roots();
        assert_eq!(r.abs("A/b.als").unwrap(), Path::new("C:/Musique").join("A/b.als"));
        assert_eq!(r.abs("Prod/x/y.als").unwrap(), Path::new("D:/Prod").join("x/y.als"));
        assert_eq!(r.abs("Prod").unwrap(), PathBuf::from("D:/Prod"));
        assert_eq!(r.rel(Path::new("D:/Prod/x/y.als")).as_deref(), Some("Prod/x/y.als"));
        assert_eq!(r.rel(Path::new("C:/Musique/A/b.als")).as_deref(), Some("A/b.als"));
        assert_eq!(r.rel(Path::new("E:/ailleurs.als")), None);
        // "Production" is not the label "Prod"
        assert_eq!(r.abs("Production/a.als").unwrap(), Path::new("C:/Musique").join("Production/a.als"));
    }

    #[test]
    fn without_a_main_folder_plain_names_resolve_nowhere() {
        let r = Roots { list: vec![Root { label: "Prod".into(), path: "D:/Prod".into(), artist: String::new() }] };
        assert!(r.abs("A/b.als").is_none());
        assert!(r.abs("Prod/a.als").is_some());
    }
}

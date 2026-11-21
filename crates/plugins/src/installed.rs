//! What Live has scanned: its plugin database, here and on the previous system.
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Installed {
    /// vst2 / vst3
    pub format: String,
    /// instr / audiofx / midifx
    pub kind: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: String,
    /// The binary's file name without extension.
    pub file: String,
}

static DEV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^device:(vst3?):(\w+):([^?]+)").unwrap());

/// This machine's Live plugin database.
pub fn live_db() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    local.join("Ableton").join("Live Database").join("Live-plugins-1.db")
}

/// The plugins of one Live database. `check_files`: drop a plugin whose file was deleted
/// since Live's last scan (Live keeps it listed until it rescans).
pub fn load_installed(db: &Path, check_files: bool) -> rusqlite::Result<Vec<Installed>> {
    let con = rusqlite::Connection::open_with_flags(
        db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut st = con.prepare(
        "select p.dev_identifier, p.name, p.vendor, p.version, m.path \
         from plugins p join plugin_modules m using(module_id) where p.enabled = 1",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (dev, name, vendor, version, path) = row?;
        let Some(m) = DEV.captures(&dev) else { continue };
        if check_files && !path.is_empty() && !Path::new(&path).exists() {
            continue;
        }
        let file = Path::new(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        out.push(Installed {
            format: if &m[1] == "vst" { "vst2".into() } else { "vst3".into() },
            kind: m[2].to_string(),
            id: m[3].to_lowercase(),
            name,
            vendor,
            version,
            path,
            file,
        });
    }
    Ok(out)
}

/// The previous system's Live databases (every user of the old drive).
pub fn old_system_dbs(old_drive: Option<&str>) -> Vec<PathBuf> {
    let Some(d) = old_drive.filter(|d| !d.is_empty()) else { return Vec::new() };
    let users = PathBuf::from(format!("{d}:/Users"));
    let mut out: Vec<PathBuf> = std::fs::read_dir(&users)
        .into_iter()
        .flatten()
        .flatten()
        .map(|u| u.path().join("AppData/Local/Ableton/Live Database/Live-plugins-1.db"))
        .filter(|p| p.is_file())
        .collect();
    out.sort();
    out
}

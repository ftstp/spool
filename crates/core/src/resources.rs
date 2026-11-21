//! A song's resources (its sheet's Resources section): the samples its project plays, where
//! each lives (in the project's own folders, in a library elsewhere, or not found), and the
//! audio and MIDI kept in the project's Samples folders, even those no clip uses any more.
//! Only read: nothing is moved, copied or written.
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use store::rusqlite::{self, Connection, OptionalExtension};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Resource {
    pub name: String,
    /// The full path, "/" separated.
    pub path: String,
    /// Its folder, as shown: inside the song's folder from it ("Samples/Imported"), else whole.
    pub dir: String,
    /// "audio" or "midi".
    pub kind: String,
    /// "project" (in the song's folder), "library" (elsewhere) or "missing" (not found).
    pub place: String,
    /// Played by the project (not only lying in its Samples folder).
    pub used: bool,
    #[ts(type = "number")]
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Resources {
    /// The project read (its newest version), relative to the projects folders.
    pub project: String,
    pub items: Vec<Resource>,
    /// Left out past the list's length.
    pub more: u32,
}

const MAX: usize = 400;
const AUDIO: &[&str] = &["wav", "aif", "aiff", "mp3", "flac", "ogg", "m4a"];

fn kind(p: &Path) -> Option<&'static str> {
    let e = p.extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO.contains(&e.as_str()) {
        Some("audio")
    } else if e == "mid" || e == "midi" {
        Some("midi")
    } else {
        None
    }
}

/// The audio and MIDI of the song's folder that belong to the project: anything under a
/// "Samples" folder (Live's own: recorded, imported, frozen, consolidated), and MIDI files
/// anywhere. The rest (renders beside the set, pictures, notes) is not a resource: renders
/// are the song's references, the rest its mood board.
fn walk(dir: &Path, in_samples: bool, depth: usize, out: &mut Vec<(std::path::PathBuf, &'static str, u64)>) {
    if depth > 8 || out.len() > 3000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let n = e.file_name().to_string_lossy().to_lowercase();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if matches!(n.as_str(), "backup" | "_moodboard" | "ableton project info") || n.starts_with('.') {
                continue;
            }
            walk(&p, in_samples || n == "samples", depth + 1, out);
        } else if ft.is_file() {
            match kind(&p) {
                Some(k) if in_samples || k == "midi" => {
                    out.push((p, k, e.metadata().map(|m| m.len()).unwrap_or(0)));
                }
                _ => {}
            }
        }
    }
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The resources of song `sid`, read in its project `rel` (the one its sheet shows), else in
/// its newest.
pub fn resources(con: &Connection, sid: i64, rel: Option<&str>) -> rusqlite::Result<Resources> {
    let roots = store::Roots::load(con);
    let song_dir: Option<String> = con.query_row("select song_dir from songs where id=?", [sid], |r| r.get(0)).optional()?.flatten();
    let chosen: Option<(String, Option<String>)> = match rel {
        Some(rel) => con.query_row("select rel, unz(data) from files where song_id=? and rel=?", rusqlite::params![sid, rel], |r| Ok((r.get(0)?, r.get(1)?))).optional()?,
        None => None,
    };
    let newest: Option<(String, Option<String>)> = match chosen {
        Some(c) => Some(c),
        None => con
            .query_row(
                "select rel, unz(data) from files where song_id=? and in_version=0 and archived=0 order by mtime desc limit 1",
                [sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?,
    };
    let mut out = Resources::default();
    let data: serde_json::Value = newest.as_ref().and_then(|n| n.1.as_deref()).and_then(|d| serde_json::from_str(d).ok()).unwrap_or_default();
    if let Some((rel, _)) = &newest {
        out.project = rel.clone();
    }
    let home = song_dir.as_deref().and_then(|d| roots.abs(d)).filter(|p| p.is_dir());
    let inside = |p: &Path| home.as_ref().is_some_and(|h| p.starts_with(h));
    let shown_dir = |p: &Path| -> String {
        let parent = p.parent().unwrap_or(Path::new(""));
        match home.as_ref().and_then(|h| parent.strip_prefix(h).ok()) {
            Some(r) => slash(r),
            None => slash(parent),
        }
    };
    let mut by: HashMap<String, Resource> = HashMap::new();
    let strs = |k: &str| -> Vec<String> { data[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect() };
    for s in strs("sample_paths") {
        let p = Path::new(&s);
        let Some(k) = kind(p) else { continue };
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let place = if !p.is_file() { "missing" } else if inside(p) { "project" } else { "library" };
        by.insert(
            s.to_lowercase(),
            Resource { name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), path: s.clone(), dir: shown_dir(p), kind: k.into(), place: place.into(), used: true, size },
        );
    }
    for s in strs("samples_missing") {
        let p = Path::new(&s);
        by.entry(s.to_lowercase()).or_insert_with(|| Resource {
            name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.clone()),
            path: s.clone(),
            dir: shown_dir(p),
            kind: kind(p).unwrap_or("audio").into(),
            place: "missing".into(),
            used: true,
            size: 0,
        });
    }
    if let Some(h) = &home {
        let mut found = Vec::new();
        walk(h, false, 0, &mut found);
        for (p, k, size) in found {
            let s = slash(&p);
            by.entry(s.to_lowercase()).or_insert_with(|| Resource {
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                path: s.clone(),
                dir: shown_dir(&p),
                kind: k.into(),
                place: "project".into(),
                used: false,
                size,
            });
        }
    }
    let mut items: Vec<Resource> = by.into_values().collect();
    // what the project plays first, the missing on top of it; then by folder and name
    let rank = |r: &Resource| (!r.used, match r.place.as_str() { "missing" => 0, "project" => 1, _ => 2 });
    items.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.dir.to_lowercase().cmp(&b.dir.to_lowercase())).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out.more = items.len().saturating_sub(MAX) as u32;
    items.truncate(MAX);
    out.items = items;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_project_plays_and_what_its_samples_folder_keeps() {
        let d = std::env::temp_dir().join(format!("spool-res-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let song = d.join("Root").join("Tune Project");
        for f in ["Samples/Imported/kick.wav", "Samples/Recorded/vox 1.wav", "riff.mid", "Tune.wav", "cover.png", "Backup/Tune [old].als", "Tune.als"] {
            let p = song.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        let lib = d.join("Lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("snare.wav"), b"x").unwrap();
        let st = store::Store::memory().unwrap();
        st.set("root", &slash(&d.join("Root"))).unwrap();
        st.con.execute("insert into songs(id, song_dir, vkey, title) values(1, 'Tune Project', 'tune', 'Tune')", []).unwrap();
        let data = serde_json::json!({
            "sample_paths": [slash(&song.join("Samples/Imported/kick.wav")), slash(&lib.join("snare.wav"))],
            "samples_missing": ["E:/Gone/clap.wav"],
        });
        st.con.execute("insert into files(rel, song_id, mtime, data) values('Tune Project/Tune.als', 1, 1, z(?))", [data.to_string()]).unwrap();
        let r = resources(&st.con, 1, None).unwrap();
        let got: Vec<(String, String, bool, String)> = r.items.iter().map(|x| (x.name.clone(), x.place.clone(), x.used, x.dir.clone())).collect();
        assert_eq!(
            got,
            [
                ("clap.wav".into(), "missing".into(), true, "E:/Gone".into()),
                ("kick.wav".into(), "project".into(), true, "Samples/Imported".into()),
                ("snare.wav".into(), "library".into(), true, slash(&lib)),
                ("riff.mid".into(), "project".into(), false, "".into()),
                ("vox 1.wav".into(), "project".into(), false, "Samples/Recorded".into()),
            ],
            "the render beside the set, the picture and the backup are not resources"
        );
        assert_eq!(r.project, "Tune Project/Tune.als");
        let _ = std::fs::remove_dir_all(&d);
    }
}

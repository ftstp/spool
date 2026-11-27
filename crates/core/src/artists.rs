//! The Artists page: every artist the catalogue names (on songs, as a featuring, in the
//! settings' list, or with a profile written here), each with a profile (contact, links,
//! rights identifiers, notes), a photo, a mood board, and what the catalogue knows of them
//! (their songs, featurings, releases, styles, folders).
//!
//! The profiles are a setting ("artist_profiles", keyed by the name in lower case), so a
//! merge of two artists carries them like the rest of the artist settings. The mood board
//! and the photo live in Spool's own data folder (artists/<name>/), never in the projects.

use crate::{App, Result, err};
use i18n::tr;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// One artist on the Artists page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArtistCard {
    pub name: String,
    /// Songs of theirs (not archived), and songs they are featured on.
    pub songs: i64,
    pub ft: i64,
    pub releases: i64,
    /// Their most frequent styles, at most three.
    pub styles: Vec<String>,
    /// The newest save of one of their projects (0: none).
    pub last: f64,
    /// Their photo (an absolute path the app serves), when they have one.
    pub photo: Option<String>,
    /// A profile was written for them.
    pub profile: bool,
}

/// An artist's page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArtistPage {
    pub name: String,
    /// What was written about them: free fields (real_name, bio, email, links...).
    #[ts(type = "Record<string, string>")]
    pub profile: Map<String, Value>,
    /// Other names merged into this one (folder names saying them are read as this one).
    pub aliases: Vec<String>,
    pub songs: Vec<i64>,
    pub ft_songs: Vec<i64>,
    pub releases: Vec<String>,
    pub photo: Option<String>,
    pub board: catalog::Board,
}

const KEY: &str = "artist_profiles";

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// A folder name for an artist's own files ("Kito Vale" -> "kito vale").
fn slug(name: &str) -> String {
    let s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "_".into() } else { s.chars().take(80).collect() }
}

impl App {
    fn profiles(&self) -> BTreeMap<String, Map<String, Value>> {
        let st = self.reader.lock().unwrap();
        st.get::<BTreeMap<String, Map<String, Value>>>(KEY).unwrap_or_default()
    }

    /// The folder of an artist's photo and mood board (created on first use).
    pub fn artist_dir(&self, name: &str) -> PathBuf {
        self.data_dir.join("artists").join(slug(name))
    }

    /// Inside the artists' folder of Spool's data (their photos and boards): the interface may show it.
    pub(crate) fn in_artist_dirs(&self, p: &Path) -> bool {
        let (Ok(real), Ok(base)) = (p.canonicalize(), self.data_dir.join("artists").canonicalize()) else { return false };
        real.starts_with(base) && !real.components().any(|c| c.as_os_str() == ".corbeille")
    }

    /// Inside Spool's own pictures folder (a release's cover picked from the disk).
    pub(crate) fn in_pictures(&self, p: &Path) -> bool {
        let (Ok(real), Ok(base)) = (p.canonicalize(), self.data_dir.join("pictures").canonicalize()) else { return false };
        real.starts_with(base)
    }

    /// A picture picked from the disk for a cover, copied into Spool's pictures folder
    /// (the original is not touched): its absolute path, which the app serves.
    pub fn picture_add(&self, file: &str, data: &[u8]) -> Result<String> {
        let dir = self.data_dir.join("pictures");
        if catalog::kind_of(file) != "image" {
            return Err(tr!("artists.notAPicture"));
        }
        let n = crate::board_edit::add_file(&dir, file, data)?;
        Ok(dir.join(n).to_string_lossy().replace('\\', "/"))
    }

    fn photo_of(&self, name: &str, profile: Option<&Map<String, Value>>) -> Option<String> {
        let dir = self.artist_dir(name);
        let chosen = profile.and_then(|p| p.get("photo")).and_then(|v| v.as_str()).map(|f| dir.join(f)).filter(|p| p.is_file());
        // else the first picture of their board
        let first = || {
            let mut pics: Vec<PathBuf> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.file_name().is_some_and(|n| catalog::kind_of(&n.to_string_lossy()) == "image"))
                .collect();
            pics.sort();
            pics.into_iter().next()
        };
        chosen.or_else(first).map(|p| p.to_string_lossy().replace('\\', "/"))
    }

    /// Every artist, the busiest first.
    pub fn artists(&self) -> Result<Vec<ArtistCard>> {
        let songs = self.songs()?;
        let releases = self.releases()?;
        let profiles = self.profiles();
        let (listed, ft): (Vec<String>, Vec<(i64, String)>) = {
            let st = self.reader.lock().unwrap();
            let listed = st.get::<Vec<String>>("artists").unwrap_or_default();
            let mut q = st.con.prepare("select id, ft from songs where gone = 0 and coalesce(archived, 0) = 0 and coalesce(ft, '') <> ''").map_err(err)?;
            let ft = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(err)?.flatten().collect();
            (listed, ft)
        };
        // by the name in lower case: the spelling shown is the most used one
        struct Acc {
            names: HashMap<String, i64>,
            songs: i64,
            ft: i64,
            styles: HashMap<String, i64>,
            last: f64,
            sids: Vec<i64>,
        }
        let mut by: BTreeMap<String, Acc> = BTreeMap::new();
        fn acc_in<'a>(by: &'a mut BTreeMap<String, Acc>, name: &str) -> &'a mut Acc {
            let a = by.entry(name.trim().to_lowercase()).or_insert_with(|| Acc { names: HashMap::new(), songs: 0, ft: 0, styles: HashMap::new(), last: 0.0, sids: Vec::new() });
            *a.names.entry(name.trim().to_string()).or_default() += 1;
            a
        }
        for s in songs.iter().filter(|s| !s.archived) {
            let Some(a) = s.artist.as_deref().filter(|a| !a.trim().is_empty()) else { continue };
            let x = acc_in(&mut by, a);
            x.songs += 1;
            x.last = x.last.max(s.mtime);
            x.sids.push(s.id);
            for st in s.style.as_deref().unwrap_or("").split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()) {
                *x.styles.entry(st).or_default() += 1;
            }
        }
        for (_, f) in &ft {
            for a in f.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                acc_in(&mut by, a).ft += 1;
            }
        }
        for a in listed.iter().filter(|a| !a.trim().is_empty()) {
            acc_in(&mut by, a);
        }
        for (k, p) in &profiles {
            let shown = p.get("name").and_then(|v| v.as_str()).unwrap_or(k).to_string();
            acc_in(&mut by, &shown);
        }
        let mut out: Vec<ArtistCard> = by
            .into_iter()
            .map(|(k, a)| {
                let name = a.names.iter().max_by(|x, y| x.1.cmp(y.1).then(y.0.cmp(x.0))).map(|x| x.0.clone()).unwrap_or(k.clone());
                let mut styles: Vec<(String, i64)> = a.styles.into_iter().collect();
                styles.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
                let rel = releases.iter().filter(|r| same(&r.artist, &name) || r.songs.iter().flatten().any(|id| a.sids.contains(id))).count() as i64;
                let profile = profiles.get(&k);
                ArtistCard {
                    photo: self.photo_of(&name, profile),
                    profile: profile.is_some_and(|p| p.iter().any(|(f, v)| f != "name" && v.as_str().is_some_and(|s| !s.trim().is_empty()))),
                    name,
                    songs: a.songs,
                    ft: a.ft,
                    releases: rel,
                    styles: styles.into_iter().take(3).map(|x| x.0).collect(),
                    last: a.last,
                }
            })
            .collect();
        out.sort_by(|a, b| (b.songs + b.ft).cmp(&(a.songs + a.ft)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(out)
    }

    /// An artist's page.
    pub fn artist(&self, name: &str) -> Result<ArtistPage> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("artists.noName"));
        }
        let songs = self.songs()?;
        let releases = self.releases()?;
        let k = name.to_lowercase();
        let profile = self.profiles().remove(&k).unwrap_or_default();
        let (aliases, ft): (Vec<String>, Vec<i64>) = {
            let st = self.reader.lock().unwrap();
            let al: BTreeMap<String, String> = st.get("artist_aliases").unwrap_or_default();
            let mut q = st.con.prepare("select id, ft from songs where gone = 0 and coalesce(ft, '') <> ''").map_err(err)?;
            let ft = q
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map_err(err)?
                .flatten()
                .filter(|(_, f)| f.split(',').any(|a| same(a, name)))
                .map(|(id, _)| id)
                .collect();
            (al.into_iter().filter(|(_, into)| same(into, name)).map(|(from, _)| from).collect(), ft)
        };
        let mine: Vec<i64> = songs.iter().filter(|s| s.artist.as_deref().is_some_and(|a| same(a, name))).map(|s| s.id).collect();
        let rels = releases
            .iter()
            .filter(|r| same(&r.artist, name) || r.songs.iter().flatten().any(|id| mine.contains(id) || ft.contains(id)))
            .map(|r| r.id.clone())
            .collect();
        let dir = self.artist_dir(name);
        Ok(ArtistPage {
            photo: self.photo_of(name, Some(&profile)),
            board: catalog::list_dir_board(&dir),
            name: name.to_string(),
            profile,
            aliases,
            songs: mine,
            ft_songs: ft,
            releases: rels,
        })
    }

    /// Fields of an artist's profile written (an empty value removes the field).
    pub fn artist_save(&self, name: &str, patch: Map<String, Value>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("artists.noName"));
        }
        let st = self.store.lock().unwrap();
        let mut all: BTreeMap<String, Map<String, Value>> = st.get(KEY).unwrap_or_default();
        let p = all.entry(name.to_lowercase()).or_default();
        p.insert("name".into(), Value::String(name.to_string()));
        for (f, v) in patch {
            match v.as_str().map(str::trim) {
                Some("") | None if !v.is_object() && !v.is_array() => {
                    p.remove(&f);
                }
                _ => {
                    p.insert(f, v);
                }
            }
        }
        st.set(KEY, &all).map_err(err)?;
        self.state.lock().unwrap().version += 1;
        drop(st);
        self.notify();
        Ok(())
    }

    fn artist_board_done(&self) -> Result<()> {
        self.state.lock().unwrap().version += 1;
        self.notify();
        Ok(())
    }

    pub fn artist_board_add(&self, name: &str, file: &str, data: &[u8]) -> Result<String> {
        let dir = self.artist_dir(name);
        std::fs::create_dir_all(&dir).map_err(err)?;
        let n = crate::board_edit::add_file(&dir, file, data)?;
        self.artist_board_done()?;
        Ok(n)
    }

    /// A file the app shows (a picture of a song, of a library, of another board) copied onto
    /// the artist's board, and made their photo when `photo`. Read from disk, the original
    /// untouched; only a file the app may show is taken.
    pub fn artist_board_copy(&self, name: &str, path: &str, photo: bool) -> Result<String> {
        let src = self.media_path(path).ok_or_else(|| tr!("board.itemNotFound"))?;
        if photo && catalog::kind_of(&src.to_string_lossy()) != "image" {
            return Err(tr!("artists.notAPicture"));
        }
        let dir = self.artist_dir(name);
        // already on their board: used as it is, not copied twice
        let here = src.parent().and_then(|d| d.canonicalize().ok()) == dir.canonicalize().ok();
        let n = if here { src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { crate::board_edit::add_path(&dir, &src)? };
        if photo {
            let mut m = Map::new();
            m.insert("photo".into(), Value::String(n.clone()));
            self.artist_save(name, m)?;
        } else {
            self.artist_board_done()?;
        }
        Ok(n)
    }

    /// Files dropped from the system on the artist's board: copied onto it, wherever they
    /// come from (the originals stay where they are).
    pub fn artist_board_drop(&self, name: &str, paths: Vec<String>) -> Result<crate::BoardDrop> {
        let dir = self.artist_dir(name);
        std::fs::create_dir_all(&dir).map_err(err)?;
        let mut out = crate::BoardDrop::default();
        for p in paths {
            let path = std::path::PathBuf::from(&p);
            let n = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
            if !path.is_file() || catalog::kind_of(&n) == "file" {
                out.refused.push(n);
                continue;
            }
            if path.parent().and_then(|d| d.canonicalize().ok()) == dir.canonicalize().ok() {
                continue; // on the board already
            }
            match crate::board_edit::add_path(&dir, &path) {
                Ok(n) => out.copied.push(n),
                Err(e) => out.refused.push(format!("{n} ({e})")),
            }
        }
        if !out.copied.is_empty() {
            self.artist_board_done()?;
        }
        Ok(out)
    }

    /// A note on the artist's board (the version it replaces is kept in its .corbeille).
    pub fn artist_board_note(&self, name: &str, text: &str, lyrics: bool, old: Option<&str>) -> Result<String> {
        let dir = self.artist_dir(name);
        std::fs::create_dir_all(&dir).map_err(err)?;
        let n = crate::board_edit::save_note(&dir, text, lyrics, old)?;
        if n.old.as_deref() != Some(text) {
            let label = match &n.old {
                None => tr!("undo.ideaAdded"),
                Some(_) => tr!("undo.noteEdited", name = n.name.trim_end_matches(".md").trim_end_matches(".txt")),
            };
            let st = self.store.lock().unwrap();
            let tx = st.con.unchecked_transaction().map_err(err)?;
            let entry = serde_json::json!({"kind": "note", "path": n.path.to_string_lossy(), "old": n.old, "at": crate::actions::now()});
            crate::actions::push(&tx, &label, Some(entry)).map_err(err)?;
            tx.commit().map_err(err)?;
            self.after_change(&st)?;
        }
        Ok(n.name)
    }

    /// Set aside in the board's .corbeille (never deleted).
    pub fn artist_board_remove(&self, name: &str, file: &str) -> Result<()> {
        crate::board_edit::remove(&self.artist_dir(name), file)?;
        self.artist_board_done()
    }

    pub fn artist_board_meta(&self, name: &str, patch: Map<String, Value>) -> Result<()> {
        let dir = self.artist_dir(name);
        std::fs::create_dir_all(&dir).map_err(err)?;
        crate::board_edit::set_meta(&dir, &patch)?;
        self.artist_board_done()
    }
}

#[cfg(test)]
mod tests {
    use super::slug;
    #[test]
    fn an_artist_folder_name_is_safe() {
        assert_eq!(slug("Kito Vale"), "kito vale");
        assert_eq!(slug("AC/DC: live?"), "ac_dc_ live_");
        assert_eq!(slug(".."), "__");
        assert_eq!(slug("  "), "_");
    }
}

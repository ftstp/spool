//! References: released songs (or any audio) to compare one's work with. A reference is
//! attached to a song, to an artist (then it is a reference for all their songs) or to a
//! release; one in a project folder is never taken for the song's reference render.
//!
//! The files are only read. The list of references is the setting "ref_tracks" (absolute
//! paths, as Compare always kept it); what each is attached to is "ref_links".

use crate::{App, Result, err};
use i18n::tr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// A reference shown on a song's, an artist's or a release's page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RefItem {
    /// Absolute, "/" separated (the player serves it).
    pub path: String,
    pub name: String,
    /// Why it is here: "song", "artist" or "release" (a song's page also shows its artist's
    /// and its releases' references).
    pub via: String,
    /// The artist's name or the release's title, for "artist" and "release".
    pub from: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Links {
    #[serde(default)]
    songs: Vec<i64>,
    #[serde(default)]
    artists: Vec<String>,
    #[serde(default)]
    releases: Vec<String>,
}

impl Links {
    fn empty(&self) -> bool {
        self.songs.is_empty() && self.artists.is_empty() && self.releases.is_empty()
    }
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl App {
    fn ref_links(&self) -> BTreeMap<String, Links> {
        self.reader.lock().unwrap().get("ref_links").unwrap_or_default()
    }

    /// The references of a song (with its artist's and its releases'), an artist or a release.
    /// `kind`: "song" (key: its id), "artist" (its name), "release" (its id).
    pub fn refs(&self, kind: &str, key: &str) -> Result<Vec<RefItem>> {
        let links = self.ref_links();
        let releases = self.releases()?;
        let (sid, artist): (Option<i64>, Option<String>) = match kind {
            "song" => {
                let sid: i64 = key.parse().map_err(|_| tr!("err.songNotFound"))?;
                let st = self.reader.lock().unwrap();
                let a: Option<String> = st.con.query_row("select artist from songs where id=?", [sid], |r| r.get(0)).map_err(|_| tr!("err.songNotFound"))?;
                (Some(sid), a.filter(|a| !a.trim().is_empty()))
            }
            "artist" => (None, Some(key.to_string())),
            _ => (None, None),
        };
        let mut out = Vec::new();
        for (path, l) in &links {
            let mut add = |via: &str, from: String| {
                if out.iter().any(|r: &RefItem| r.path == *path) {
                    return;
                }
                let p = Path::new(path);
                out.push(RefItem {
                    path: path.clone(),
                    name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone()),
                    via: via.into(),
                    from,
                    exists: p.is_file(),
                });
            };
            match kind {
                "song" => {
                    if sid.is_some_and(|s| l.songs.contains(&s)) {
                        add("song", String::new());
                    }
                    if let Some(a) = artist.as_deref().and_then(|a| l.artists.iter().find(|x| same(x, a))) {
                        add("artist", a.clone());
                    }
                    for r in releases.iter().filter(|r| l.releases.contains(&r.id) && sid.is_some_and(|s| r.songs.contains(&Some(s)))) {
                        add("release", r.title.clone());
                    }
                }
                "artist" => {
                    if let Some(a) = artist.as_deref().and_then(|a| l.artists.iter().find(|x| same(x, a))) {
                        add("artist", a.clone());
                    }
                }
                _ => {
                    if l.releases.iter().any(|r| r == key) {
                        add("release", String::new());
                    }
                }
            }
        }
        // the song's own first, then its artist's, then its releases'
        let rank = |v: &str| match v {
            "song" => 0,
            "artist" => 1,
            _ => 2,
        };
        out.sort_by(|a, b| rank(&a.via).cmp(&rank(&b.via)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(out)
    }

    /// An audio file marked as a reference of a song, an artist or a release (`on`), or no
    /// longer (`!on`). `path`: absolute, or a name under the projects folders. A song's
    /// reference render that is this file stops being it (another is suggested).
    pub fn ref_mark(&self, path: &str, kind: &str, key: &str, on: bool) -> Result<Vec<RefItem>> {
        let p = Path::new(path);
        let abs: PathBuf = if p.is_absolute() { p.to_path_buf() } else { self.roots().abs(path).ok_or_else(|| tr!("refs.notAudio"))? };
        let s = norm(&abs);
        if on && (!abs.is_file() || catalog::kind_of(&s) != "audio") {
            return Err(tr!("refs.notAudio"));
        }
        let rel = self.roots().rel(&abs);
        {
            let st = self.store.lock().unwrap();
            let tx = st.con.unchecked_transaction().map_err(err)?;
            let get = |k: &str| -> Option<String> { tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok() };
            let mut links: BTreeMap<String, Links> = get("ref_links").and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
            let mut list: Vec<String> = get("ref_tracks").and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
            let l = links.entry(s.clone()).or_default();
            match kind {
                "song" => {
                    let sid: i64 = key.parse().map_err(|_| tr!("err.songNotFound"))?;
                    l.songs.retain(|x| *x != sid);
                    if on {
                        l.songs.push(sid);
                    }
                }
                "artist" => {
                    l.artists.retain(|x| !same(x, key));
                    if on {
                        l.artists.push(key.trim().to_string());
                    }
                }
                "release" => {
                    l.releases.retain(|x| x != key);
                    if on {
                        l.releases.push(key.to_string());
                    }
                }
                _ => return Err(tr!("refs.notAudio")),
            }
            let unused = l.empty();
            if unused {
                links.remove(&s);
            }
            if on && !list.contains(&s) {
                list.push(s.clone());
            }
            // attached to nothing any more: no longer a reference (a render again, if it is one)
            if !on && unused {
                list.retain(|x| *x != s);
            }
            let put = |k: &str, v: String| tx.execute("insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v", store::rusqlite::params![k, v]);
            put("ref_links", serde_json::to_string(&links).map_err(err)?).map_err(err)?;
            put("ref_tracks", serde_json::to_string(&list).map_err(err)?).map_err(err)?;
            if on {
                // a render chosen (by the user or the app) that is this file: chosen again
                for t in [Some(s.clone()), rel.clone()].into_iter().flatten() {
                    tx.execute("update songs set temoin = null, temoin_source = null where temoin = ?", [&t]).map_err(err)?;
                }
            }
            index::regroup(&tx).map_err(err)?;
            tx.commit().map_err(err)?;
            self.state.lock().unwrap().version += 1;
        }
        self.notify();
        self.refs(kind, key)
    }
}

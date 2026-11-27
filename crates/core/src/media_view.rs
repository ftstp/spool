//! Every song's media in one place (the Media page): what each mood board holds and what its
//! folder already has (pictures, videos, fonts, presets, notes), with where each file lives.
//! Only read: nothing is moved or written.
use catalog::{Item, Rules, card_picture, list_board_labeled, other_song_prefixes};
use rayon::prelude::*;
use store::rusqlite::{self, Connection};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MediaSong {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// The song's folder, relative to the projects folders.
    pub dir: String,
    /// Its picture: the one chosen, else the one the app would choose.
    pub cover: Option<String>,
    /// Chosen by hand (not the app's guess).
    pub cover_chosen: bool,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export)]
pub struct SongRef {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
}

/// An audio file of the projects folders that no song uses: neither a song's reference render
/// nor a sample played by a project, nor a track marked as a reference. Bounces nobody picked,
/// a "- MP3" folder, a recorder's memos: what may be a song's render, to be linked by hand.
#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[ts(export)]
pub struct LooseAudio {
    pub rel: String,
    #[ts(type = "number")]
    pub size: i64,
    pub mtime: f64,
    /// The songs whose folder holds it (empty: no song's folder).
    pub songs: Vec<SongRef>,
    /// Songs with a close name, for a file outside every song's folder (a hint, never a link).
    pub near: Vec<SongRef>,
}

/// Smaller than this, an audio file is a one-shot or a jingle, not a song's render.
const LOOSE_MIN: i64 = 200_000;

/// Every audio file no song uses (see `LooseAudio`), newest first.
pub fn loose_audio(con: &Connection) -> rusqlite::Result<Vec<LooseAudio>> {
    let artists: Vec<String> = setting(con, "artists").unwrap_or_default();
    let words: Vec<String> = setting(con, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    let roots = store::Roots::load(con);
    let refs = roots.reference_rels(con);
    let temoins: std::collections::HashSet<String> = {
        let mut s = con.prepare("select temoin from songs where gone=0 and temoin is not null")?;
        s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?
    };
    // samples are keyed by their real path, in lower case with forward slashes
    let used: std::collections::HashSet<String> = {
        let mut s = con.prepare("select distinct path from sample_use")?;
        s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?
    };
    struct S {
        id: i64,
        title: Option<String>,
        dir: String,
        keys: [String; 2],
    }
    let songs: Vec<S> = {
        let mut s = con.prepare("select id, title, song_dir, vkey from songs where gone=0 and archived=0 and song_dir is not null")?;
        s.query_map([], |r| {
            let title: Option<String> = r.get(1)?;
            let vkey: Option<String> = r.get(3)?;
            Ok((r.get(0)?, title, r.get(2)?, vkey))
        })?
        .collect::<Result<Vec<(i64, Option<String>, String, Option<String>)>, _>>()?
        .into_iter()
        .map(|(id, title, dir, vkey)| {
            let tk = rules.variant_key(title.as_deref().unwrap_or(""));
            S { id, title, dir, keys: [vkey.unwrap_or_default(), tk] }
        })
        .collect()
    };
    let mut by_dir: HashMap<&str, Vec<SongRef>> = HashMap::new();
    for s in &songs {
        by_dir.entry(s.dir.as_str()).or_default().push(SongRef { id: s.id, title: s.title.clone() });
    }
    let rows: Vec<(String, i64, f64)> = {
        let mut s = con.prepare("select rel, size, mtime from audio")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0), r.get::<_, Option<f64>>(2)?.unwrap_or(0.0))))?
            .collect::<Result<_, _>>()?
    };
    let mut out: Vec<LooseAudio> = rows
        .into_par_iter()
        .filter(|(rel, size, _)| *size >= LOOSE_MIN && !temoins.contains(rel) && !refs.contains(rel))
        .filter(|(rel, _, _)| {
            let abs = roots.abs(rel).map(|p| p.to_string_lossy().replace('\\', "/").to_lowercase());
            abs.is_none_or(|a| !used.contains(&a))
        })
        .map(|(rel, size, mtime)| {
            // the deepest song folder holding the file
            let mut owner: Vec<SongRef> = Vec::new();
            let mut d = rel.rsplit_once('/').map(|(d, _)| d);
            while let Some(p) = d {
                if let Some(v) = by_dir.get(p) {
                    owner = v.clone();
                    break;
                }
                d = p.rsplit_once('/').map(|(d, _)| d);
            }
            let near = if owner.is_empty() {
                let file = rel.rsplit('/').next().unwrap_or("");
                let key = rules.variant_key(file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file));
                let mut v: Vec<(f64, &S)> = songs
                    .iter()
                    .map(|s| (s.keys.iter().filter(|k| !k.is_empty()).map(|k| catalog::similarity(&key, k, 0.6)).fold(0.0, f64::max), s))
                    .filter(|(x, _)| *x >= 0.75)
                    .collect();
                v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                v.into_iter().take(3).map(|(_, s)| SongRef { id: s.id, title: s.title.clone() }).collect()
            } else {
                Vec::new()
            };
            LooseAudio { rel, size, mtime, songs: owner, near }
        })
        .collect();
    out.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

fn setting<T: serde::de::DeserializeOwned>(con: &Connection, k: &str) -> Option<T> {
    con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0)).ok().and_then(|s| serde_json::from_str(&s).ok())
}

/// The songs that have media, each with its files (the ones set aside on a board left out).
pub fn media(con: &Connection) -> rusqlite::Result<Vec<MediaSong>> {
    let artists: Vec<String> = setting(con, "artists").unwrap_or_default();
    let words: Vec<String> = setting(con, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    let roots = store::Roots::load(con);
    struct Row {
        id: i64,
        title: Option<String>,
        artist: Option<String>,
        dir: String,
        vkey: String,
    }
    let rows: Vec<Row> = {
        let mut s = con.prepare("select id, title, artist, song_dir, vkey from songs where gone=0 and archived=0 and song_dir is not null")?;
        s.query_map([], |r| Ok(Row { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, dir: r.get(3)?, vkey: r.get::<_, Option<String>>(4)?.unwrap_or_default() }))?
            .collect::<Result<_, _>>()?
    };
    let dirs: Vec<String> = rows.iter().map(|r| r.dir.clone()).collect();
    let mut per_dir: HashMap<&str, usize> = HashMap::new();
    for r in &rows {
        *per_dir.entry(r.dir.as_str()).or_default() += 1;
    }
    let hidden: HashMap<String, Vec<String>> = setting::<HashMap<String, Value>>(con, "board_hidden")
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()))
        .collect();
    let chosen: HashMap<i64, String> = {
        // a picture chosen by hand is kept in the song's own column, `cover`, and marked in
        // `cover_chosen` (see `App::set_cover`)
        setting::<HashMap<String, String>>(con, "cover_chosen").unwrap_or_default().into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect()
    };
    let out: Vec<MediaSong> = rows
        .par_iter()
        .filter_map(|r| {
            let (root, _) = roots.split(&r.dir)?;
            let shared = per_dir.get(r.dir.as_str()).copied().unwrap_or(0) > 1;
            let b = list_board_labeled(Path::new(&root.path), &root.label, &rules, &r.dir, &r.vkey, r.title.as_deref().unwrap_or(""), shared, &other_song_prefixes(&dirs, &r.dir));
            let hide = hidden.get(&format!("{}|{}", r.dir, r.vkey));
            let items: Vec<Item> = b.items.iter().chain(&b.found).filter(|i| hide.is_none_or(|h| !h.contains(&i.rel))).cloned().collect();
            if items.is_empty() {
                return None;
            }
            let hand = chosen.get(&r.id).cloned();
            let cover = hand.clone().or_else(|| card_picture(&b));
            Some(MediaSong { id: r.id, title: r.title.clone(), artist: r.artist.clone(), dir: r.dir.clone(), cover, cover_chosen: hand.is_some(), items })
        })
        .collect();
    Ok(out)
}

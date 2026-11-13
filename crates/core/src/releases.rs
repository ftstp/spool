//! Releases: a single, an EP, an LP... an ordered list of songs with a title, a date and a
//! cover. Kept in the setting "releases" (so in choices.json, restored with the rest), each
//! track named by its song's folder and key, not its id: a rebuilt base gives songs new ids,
//! never new folders. Nothing on disk is touched: a release lives in the app only.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

/// What is kept for a track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackRef {
    pub dir: String,
    pub vkey: String,
}

/// What is kept for a release.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stored {
    pub id: String,
    pub title: String,
    /// single / ep / lp / compil
    pub kind: String,
    #[serde(default)]
    pub artist: String,
    /// "2026-11-25"
    #[serde(default)]
    pub date: Option<String>,
    /// The song whose artwork is the release's (its position in the tracklist does not matter).
    #[serde(default)]
    pub cover_song: Option<TrackRef>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tracks: Vec<TrackRef>,
    /// The folder it was made from, when it was ("- LP - Paper Tides").
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub created: f64,
}

/// A release as the screens show it: its tracks as song ids (None: the song is gone).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub artist: String,
    pub date: Option<String>,
    #[ts(type = "number | null")]
    pub cover_song: Option<i64>,
    pub notes: String,
    #[ts(type = "Array<number | null>")]
    pub songs: Vec<Option<i64>>,
    pub folder: Option<String>,
    pub created: f64,
}

/// A folder that looks like a release ("- LP - Paper Tides"), not made one yet.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReleaseHint {
    pub folder: String,
    pub title: String,
    pub kind: String,
    pub artist: String,
    #[ts(type = "Array<number>")]
    pub songs: Vec<i64>,
}

static KIND: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)(^|[^a-z])(ep|lp|album|mixtape|compil\w*|single)([^a-z]|$)").unwrap()
});

pub fn kind_of(word: &str) -> &'static str {
    match word.to_lowercase().as_str() {
        "ep" => "ep",
        "single" => "single",
        "lp" | "album" => "lp",
        _ => "compil",
    }
}

/// "- LP - Paper Tides" -> "Paper Tides"; "LP - LUNA PIKE - HALCYON" keeps the rest.
pub fn title_of(folder_name: &str) -> String {
    let t = KIND.replace(folder_name, " ");
    let t: String = t.trim_matches(|c: char| c == '-' || c == '_' || c.is_whitespace()).to_string();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_start_matches(['-', ' ']).to_string();
    if t.is_empty() { folder_name.trim().to_string() } else { t }
}

pub fn load(con: &Connection) -> Vec<Stored> {
    let v: Option<String> = con.query_row("select v from settings where k='releases'", [], |r| r.get(0)).ok();
    v.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
}

/// (dir, vkey) -> song id, and back, for the songs present.
fn index(con: &Connection) -> rusqlite::Result<(HashMap<(String, String), i64>, HashMap<i64, TrackRef>)> {
    let mut s = con.prepare("select id, coalesce(song_dir,''), coalesce(vkey,'') from songs where gone = 0")?;
    let rows: Vec<(i64, String, String)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.flatten().collect();
    let mut by_key = HashMap::new();
    let mut by_id = HashMap::new();
    for (id, d, k) in rows {
        by_key.insert((d.clone(), k.clone()), id);
        by_id.insert(id, TrackRef { dir: d, vkey: k });
    }
    Ok((by_key, by_id))
}

pub fn list(con: &Connection) -> rusqlite::Result<Vec<Release>> {
    let (by_key, _) = index(con)?;
    let id_of = |t: &TrackRef| by_key.get(&(t.dir.clone(), t.vkey.clone())).copied();
    let mut v: Vec<Release> = load(con)
        .into_iter()
        .map(|r| Release {
            songs: r.tracks.iter().map(id_of).collect(),
            cover_song: r.cover_song.as_ref().and_then(id_of),
            id: r.id,
            title: r.title,
            kind: r.kind,
            artist: r.artist,
            date: r.date,
            notes: r.notes,
            folder: r.folder,
            created: r.created,
        })
        .collect();
    // the next to come out first, then the undated by newest
    v.sort_by(|a, b| match (&a.date, &b.date) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.created.total_cmp(&a.created),
    });
    Ok(v)
}

/// The stored form of a release edited on screen (its song ids turned back into folders; a
/// track whose song is gone keeps its old reference).
pub fn to_stored(con: &Connection, r: &Release, old: Option<&Stored>) -> rusqlite::Result<Stored> {
    let (_, by_id) = index(con)?;
    let tracks = r
        .songs
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Some(id) => by_id.get(id).cloned(),
            None => old.and_then(|o| o.tracks.get(i).cloned()),
        })
        .collect();
    Ok(Stored {
        id: r.id.clone(),
        title: r.title.trim().to_string(),
        kind: r.kind.clone(),
        artist: r.artist.trim().to_string(),
        date: r.date.clone().filter(|d| !d.is_empty()),
        cover_song: r.cover_song.and_then(|id| by_id.get(&id).cloned()),
        notes: r.notes.clone(),
        tracks,
        folder: r.folder.clone(),
        created: if r.created > 0.0 { r.created } else { old.map(|o| o.created).unwrap_or(0.0) },
    })
}

/// Folders whose name says EP / LP / album / mixtape and that hold 2 songs or more, not made a
/// release yet. Their songs are those in the folder or under it.
pub fn hints(con: &Connection) -> rusqlite::Result<Vec<ReleaseHint>> {
    let made: Vec<String> = load(con).into_iter().filter_map(|r| r.folder).collect();
    let mut s = con.prepare(
        "select id, coalesce(song_dir,''), coalesce(artist,''), coalesce(updated, created, 0) from songs where gone = 0 and archived = 0",
    )?;
    let rows: Vec<(i64, String, String, f64)> =
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.flatten().collect();
    let mut groups: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    for (id, dir, artist, _) in &rows {
        let parts: Vec<&str> = dir.split('/').collect();
        // the deepest folder of its path that names a release
        if let Some(i) = (0..parts.len()).rev().find(|i| KIND.is_match(parts[*i])) {
            groups.entry(parts[..=i].join("/")).or_default().push((*id, artist.clone()));
        }
    }
    let mut out: Vec<ReleaseHint> = groups
        .into_iter()
        .filter(|(f, s)| s.len() >= 2 && !made.contains(f))
        .map(|(folder, songs)| {
            let name = folder.rsplit('/').next().unwrap_or(&folder).to_string();
            let word = KIND.captures(&name).and_then(|c| c.get(2)).map(|m| m.as_str()).unwrap_or("lp");
            let mut by: HashMap<&str, usize> = HashMap::new();
            for (_, a) in songs.iter().filter(|(_, a)| !a.is_empty()) {
                *by.entry(a.as_str()).or_default() += 1;
            }
            let artist = by.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a.to_string()).unwrap_or_default();
            ReleaseHint { title: title_of(&name), kind: kind_of(word).into(), artist, songs: songs.iter().map(|s| s.0).collect(), folder }
        })
        .collect();
    out.sort_by(|a, b| b.songs.len().cmp(&a.songs.len()).then(a.folder.cmp(&b.folder)));
    Ok(out)
}

/// What "Ranger sur le disque" would do: the folder, the song folders that would move into
/// it, those left (shared with other songs), how many are there already.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArrangePlan {
    pub target: String,
    /// The folder is there already (else it is made).
    pub exists: bool,
    pub moves: Vec<String>,
    pub skipped: Vec<crate::fsops::Skipped>,
    #[ts(type = "number")]
    pub already: i64,
}

/// The whole list, to keep before a change (undo puts it back).
pub fn snapshot(con: &Connection) -> Value {
    let v: Option<String> = con.query_row("select v from settings where k='releases'", [], |r| r.get(0)).ok();
    serde_json::json!({"kind": "setting", "k": "releases", "v": v, "at": crate::actions::now()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn titles() {
        assert_eq!(title_of("- LP - Paper Tides"), "Paper Tides");
        assert_eq!(title_of("LP - LUNA PIKE & TOVEN - HALCYON"), "LUNA PIKE & TOVEN - HALCYON");
        assert_eq!(title_of("Ember EP"), "Ember");
        assert_eq!(kind_of("Album"), "lp");
        assert!(KIND.is_match("- LP - Ember"));
        assert!(!KIND.is_match("Help me"));
        assert!(!KIND.is_match("Lipstick"));
    }
}

//! Favourites: anything starred by the user (a song, an artist, a release, a sample, a MIDI
//! file, a mood board item, a project, a plugin), kept in the settings in the order starred,
//! the newest first. Each keeps its name as shown when starred, so the Home page can list them
//! without looking each one up.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const KINDS: [&str; 8] = ["song", "artist", "release", "sample", "midi", "board", "project", "plugin"];
/// At most this many (the oldest go first).
const MAX: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub struct Favorite {
    /// One of KINDS.
    pub kind: String,
    /// What it is known by: a song's id, an artist's name, a file's path...
    pub id: String,
    /// Its name, as shown when it was starred.
    pub label: String,
    /// What helps tell it apart (a folder, an artist, a song's title), or nothing.
    #[serde(default)]
    pub sub: String,
    /// The song it belongs to (a mood board item, a project), to open it.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    #[ts(type = "number")]
    pub at: f64,
}

/// The list with `fav` starred (first) or unstarred.
pub fn set(mut list: Vec<Favorite>, fav: Favorite, on: bool) -> Result<Vec<Favorite>, String> {
    if !KINDS.contains(&fav.kind.as_str()) || fav.id.is_empty() {
        return Err(format!("favourite: {} {}", fav.kind, fav.id));
    }
    list.retain(|f| !(f.kind == fav.kind && f.id == fav.id));
    if on {
        list.insert(0, fav);
        list.truncate(MAX);
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(kind: &str, id: &str) -> Favorite {
        Favorite { kind: kind.into(), id: id.into(), label: id.into(), sub: String::new(), song: None, at: 0.0 }
    }

    #[test]
    fn starred_first_once_and_unstarred() {
        let l = set(Vec::new(), f("song", "1"), true).unwrap();
        let l = set(l, f("sample", "/a.wav"), true).unwrap();
        let l = set(l, f("song", "1"), true).unwrap();
        assert_eq!(l.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["1", "/a.wav"]);
        let l = set(l, f("song", "1"), false).unwrap();
        assert_eq!(l.len(), 1);
        assert!(set(l, f("nothing", "x"), true).is_err());
    }
}

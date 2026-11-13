//! The MIDI files found in the projects folders, as the setup's summary and the project
//! generator see them: how many of each kind, their keys, the chord progressions that come
//! back (as degrees, so the same progression in two keys counts once), and the list.
use crate::insights::Count;
use serde::Serialize;
use std::collections::HashMap;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MidiItem {
    pub rel: String,
    pub kind: String,
    pub key: Option<String>,
    pub bpm: Option<f32>,
    pub bars: f32,
    #[ts(type = "number")]
    pub notes: usize,
    pub chords: Vec<String>,
    pub degrees: Vec<String>,
    /// Inside a project folder ("... Project"), else a loose MIDI folder.
    pub in_project: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Progression {
    /// "i VI III VII"
    pub degrees: String,
    #[ts(type = "number")]
    pub n: i64,
    /// One file it comes from, and its chords there.
    pub example: String,
    pub chords: Vec<String>,
}

/// What one collection of MIDI files holds.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct MidiStats {
    #[ts(type = "number")]
    pub files: i64,
    pub kinds: Vec<Count>,
    pub keys: Vec<Count>,
    /// Files in a minor key / in a major key.
    #[ts(type = "number")]
    pub minor: i64,
    #[ts(type = "number")]
    pub major: i64,
    /// Tempos of the files that say one, by 10 bpm ("150").
    pub tempos: Vec<Count>,
    pub progressions: Vec<Progression>,
}

/// The user's own MIDI (in the projects folders) apart from the libraries (construction kits,
/// packs): a library of 100,000 pop songs in C would drown 30 files of the user's own taste.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MidiSummary {
    #[ts(type = "number")]
    pub files: i64,
    pub mine: MidiStats,
    pub library: MidiStats,
    /// The user's own files (the library ones are many; they are browsed elsewhere).
    pub items: Vec<MidiItem>,
}

#[derive(Default)]
struct Acc {
    files: i64,
    kinds: HashMap<String, i64>,
    keys: HashMap<String, i64>,
    minor: i64,
    major: i64,
    tempos: HashMap<String, i64>,
    progs: HashMap<String, Progression>,
}

fn sorted(m: HashMap<String, i64>) -> Vec<Count> {
    let mut v: Vec<Count> = m.into_iter().map(|(label, n)| Count { label, n }).collect();
    v.sort_by(|a, b| b.n.cmp(&a.n).then(a.label.cmp(&b.label)));
    v
}

impl Acc {
    fn add(&mut self, it: &MidiItem) {
        self.files += 1;
        *self.kinds.entry(it.kind.clone()).or_default() += 1;
        if let Some(k) = &it.key {
            *self.keys.entry(k.clone()).or_default() += 1;
            if k.ends_with('m') { self.minor += 1 } else { self.major += 1 }
        }
        if let Some(b) = it.bpm.filter(|b| *b >= 40.0 && *b < 260.0) {
            *self.tempos.entry(format!("{}", (b / 10.0).floor() as i64 * 10)).or_default() += 1;
        }
        // the same root twice in a row is one chord (G then Gsus4: a colour, not a change)
        let mut roots: Vec<(String, String)> = Vec::new();
        for (d, c) in it.degrees.iter().zip(&it.chords) {
            if roots.last().is_none_or(|(ld, _)| ld.to_uppercase() != d.to_uppercase()) {
                roots.push((d.clone(), c.clone()));
            }
        }
        let degs: Vec<String> = roots.iter().map(|r| r.0.clone()).collect();
        if degs.len() >= 2 {
            let lp = loop_of(&degs);
            let key = lp.join(" ");
            let e = self.progs.entry(key.clone()).or_insert(Progression {
                degrees: key,
                n: 0,
                example: it.rel.clone(),
                chords: roots.iter().take(lp.len()).map(|r| r.1.clone()).collect(),
            });
            e.n += 1;
        }
    }
    fn done(self) -> MidiStats {
        let mut progressions: Vec<Progression> = self.progs.into_values().collect();
        progressions.sort_by(|a, b| b.n.cmp(&a.n).then(a.degrees.cmp(&b.degrees)));
        progressions.truncate(20);
        let mut tempos: Vec<Count> = self.tempos.into_iter().map(|(label, n)| Count { label, n }).collect();
        tempos.sort_by_key(|c| c.label.parse::<i64>().unwrap_or(0));
        MidiStats { files: self.files, kinds: sorted(self.kinds), keys: sorted(self.keys), minor: self.minor, major: self.major, tempos, progressions }
    }
}

/// A MIDI library file is named by its absolute path; a projects one by its root-relative name.
pub fn is_library(rel: &str) -> bool {
    rel.as_bytes().get(1) == Some(&b':') || rel.starts_with('/')
}

/// The first chords of a progression, up to where it repeats (4 at most): "i VI III VII".
fn loop_of(degrees: &[String]) -> Vec<String> {
    for len in 2..=4 {
        if degrees.len() >= len * 2 && degrees[..len] == degrees[len..len * 2] {
            return degrees[..len].to_vec();
        }
    }
    degrees.iter().take(4).cloned().collect()
}

pub fn summary(con: &Connection) -> rusqlite::Result<MidiSummary> {
    let mut s = con.prepare("select rel, data from midi")?;
    let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?;
    let (mut mine, mut library) = (Acc::default(), Acc::default());
    let mut items = Vec::new();
    for (rel, data) in rows.flatten() {
        let v: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
        if v["notes"].as_u64().unwrap_or(0) == 0 {
            continue;
        }
        let list = |k: &str| -> Vec<String> { v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect() };
        let it = MidiItem {
            in_project: rel.split('/').any(catalog::is_project_dir),
            kind: v["kind"].as_str().unwrap_or("").to_string(),
            key: v["key"].as_str().map(String::from),
            bpm: v["bpm"].as_f64().map(|b| b as f32),
            bars: v["bars"].as_f64().unwrap_or(0.0) as f32,
            notes: v["notes"].as_u64().unwrap_or(0) as usize,
            chords: list("chords"),
            degrees: list("degrees"),
            rel,
        };
        if is_library(&it.rel) {
            library.add(&it);
        } else {
            mine.add(&it);
            items.push(it);
        }
    }
    items.sort_by(|a, b| a.rel.cmp(&b.rel));
    let (mine, library) = (mine.done(), library.done());
    Ok(MidiSummary { files: mine.files + library.files, mine, library, items })
}

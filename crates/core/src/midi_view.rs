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

/// The MIDI page's filters.
#[derive(Debug, Clone, Default, serde::Deserialize, TS)]
#[ts(export)]
pub struct MidiQuery {
    /// Words, all found in the path.
    pub q: String,
    /// "chords", "bass", "melody", "drums", "multi"
    pub kind: Option<String>,
    /// A key: files in it or its relative.
    pub key: Option<String>,
    /// A tempo: within 3 %, or half or double (files that say one).
    pub bpm: Option<f64>,
    /// "clips" (the MIDI clips written in the projects) or "files" (MIDI files); both when
    /// None.
    #[serde(default)]
    pub source: Option<String>,
    #[ts(type = "number")]
    pub offset: i64,
    #[ts(type = "number")]
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MidiRow {
    /// Absolute for a library file, root-relative for a projects one.
    pub rel: String,
    pub name: String,
    pub folder: String,
    pub kind: String,
    pub key: Option<String>,
    pub bpm: Option<f32>,
    pub bars: f32,
    pub chords: Vec<String>,
    pub in_project: bool,
    /// A clip written in a project ("clip:<hash>"): the song it was found in first, and in how
    /// many projects it is.
    pub clip: bool,
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    #[ts(type = "number")]
    pub projects: i64,
}

/// Where a clip is: each project holding it, with its track and clip names.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ClipUse {
    pub rel: String,
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    pub title: String,
    pub track: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct MidiPage {
    #[ts(type = "number")]
    pub total: i64,
    #[ts(type = "number")]
    pub all: i64,
    pub rows: Vec<MidiRow>,
    /// Files per kind, with every filter but the kind.
    pub counts: Vec<Count>,
}

/// The key and its relative as the MIDI analysis writes them ("Ebm", "F#", "Bb"...).
fn key_spellings(k: &str) -> Option<Vec<String>> {
    let (t, minor) = crate::detect::pitch_class(k)?;
    let rel = if minor { ((t + 3) % 12, false) } else { ((t + 9) % 12, true) };
    Some(vec![midi::key_name(t, minor), midi::key_name(rel.0, rel.1)])
}

fn midi_filter(q: &MidiQuery, with_kind: bool) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value as Sql;
    let mut w: Vec<String> = vec!["notes > 0".into()];
    let mut v: Vec<Sql> = Vec::new();
    match q.source.as_deref() {
        Some("clips") => w.push("clip = 1".into()),
        Some("files") => w.push("clip = 0".into()),
        _ => {}
    }
    for word in q.q.split_whitespace().take(8) {
        w.push("lower(coalesce(label, rel)) like ? escape '\\'".into());
        let esc = word.to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        v.push(Sql::Text(format!("%{esc}%")));
    }
    if with_kind && let Some(k) = q.kind.as_deref().filter(|k| !k.is_empty()) {
        w.push("kind = ?".into());
        v.push(Sql::Text(k.into()));
    }
    if let Some(keys) = q.key.as_deref().and_then(key_spellings) {
        w.push(format!("mkey in ({})", keys.iter().map(|_| "?").collect::<Vec<_>>().join(",")));
        v.extend(keys.into_iter().map(Sql::Text));
    }
    if let Some(b) = q.bpm.filter(|b| *b > 0.0) {
        w.push("(bpm between ? and ? or bpm between ? and ? or bpm between ? and ?)".into());
        for m in [1.0, 0.5, 2.0] {
            v.push(Sql::Real(b * m * 0.97));
            v.push(Sql::Real(b * m * 1.03));
        }
    }
    (w.join(" and "), v)
}

/// A page of MIDI files (MIDI page): filtered in the base (the JSON read there), only the
/// page's rows decoded here.
pub fn midi_page(con: &Connection, q: &MidiQuery) -> rusqlite::Result<MidiPage> {
    use rusqlite::types::Value as Sql;
    let src = match q.source.as_deref() {
        Some("clips") => " and clip = 1",
        Some("files") => " and clip = 0",
        _ => "",
    };
    let all: i64 = con.query_row(&format!("select count(*) from midi where notes > 0{src}"), [], |r| r.get(0))?;
    // one pass for every count: per kind (with every filter but the kind) and per source; the
    // clips and files the filters keep are those of the kind asked (the page reads the clips
    // first, then the files)
    let (w, v) = midi_filter(q, true);
    let (wc, vc) = midi_filter(q, false);
    let kind = q.kind.as_deref().filter(|k| !k.is_empty());
    let mut by: Vec<(String, i64, i64)> = Vec::new();
    {
        let mut s = con.prepare(&format!("select coalesce(kind, ''), clip, count(*) from midi where {wc} group by 1, 2"))?;
        for r in s.query_map(rusqlite::params_from_iter(vc.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
            by.push(r?);
        }
    }
    let kept = |c: i64| by.iter().filter(|x| x.1 == c && kind.is_none_or(|k| x.0 == k)).map(|x| x.2).sum::<i64>();
    let (n_clips, n_files) = (kept(1), kept(0));
    let total = n_clips + n_files;
    let mut counts: Vec<Count> = Vec::new();
    for (k, _, n) in &by {
        match counts.iter_mut().find(|c| c.label == *k) {
            Some(c) => c.n += n,
            None => counts.push(Count { label: k.clone(), n: *n }),
        }
    }
    let (limit, offset) = (q.limit.clamp(1, 500), q.offset.max(0));
    // a clip: the song it is found in (the newest project holding it), its names there
    let mut use_of = con.prepare(
        "select c.track, c.name, f.song_id, coalesce(s.title, ''), (select count(distinct rel) from clips where hash = c.hash) \
         from clips c left join files f on f.rel = c.rel left join songs s on s.id = f.song_id \
         where c.hash = ? order by f.mtime desc limit 1",
    )?;
    // each part read in the order its index keeps (no row's JSON opened to sort): the clips,
    // short ones first then the fuller; the files by their path
    let mut raw: Vec<(String, String)> = Vec::new();
    let mut part = |order: &str, which: &str, lim: i64, off: i64| -> rusqlite::Result<()> {
        if lim <= 0 {
            return Ok(());
        }
        let mut vv = v.clone();
        vv.push(Sql::Integer(lim));
        vv.push(Sql::Integer(off));
        // the files in path order, through the index kept in that order (the planner took the
        // kind index and sorted 195,000 rows: a second a page); a narrowing filter (a key, a
        // tempo) is left to the planner
        let hint = if which == "clip = 0" && q.key.is_none() && q.bpm.is_none() { "indexed by midi_file_order" } else { "" };
        let mut s = con.prepare(&format!("select rel, unz(data) from midi {hint} where {w} and {which} order by {order} limit ? offset ?"))?;
        raw.extend(
            s.query_map(rusqlite::params_from_iter(vv.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?
                .flatten(),
        );
        Ok(())
    };
    let clip_off = offset.min(n_clips);
    let clip_lim = (n_clips - clip_off).min(limit);
    part("(coalesce(bars, 0) > 16), notes desc, rel", "clip = 1", clip_lim, clip_off)?;
    part("rel", "clip = 0", limit - clip_lim.max(0), (offset - n_clips).max(0))?;
    let rows = raw
        .into_iter()
        .map(|(rel, data)| {
            let d: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            let (folder, file) = rel.rsplit_once('/').unwrap_or(("", rel.as_str()));
            let clip = rel.strip_prefix("clip:").and_then(|h| {
                use_of
                    .query_row([h], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?)))
                    .ok()
            });
            let (name, folder) = match &clip {
                Some((track, name, _, title, _)) => {
                    let (track, name) = (clean(track), clean(name));
                    let n = if track.is_empty() { name } else if name.is_empty() || name == track { track } else { format!("{track} · {name}") };
                    (n, title.clone())
                }
                None => (file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file).to_string(), folder.to_string()),
            };
            MidiRow {
                clip: clip.is_some(),
                song: clip.as_ref().and_then(|c| c.2),
                projects: clip.as_ref().map(|c| c.4).unwrap_or(0),
                name,
                folder,
                in_project: rel.split('/').any(catalog::is_project_dir),
                kind: d["kind"].as_str().unwrap_or("").to_string(),
                key: d["key"].as_str().map(String::from),
                bpm: d["bpm"].as_f64().map(|b| b as f32),
                bars: d["bars"].as_f64().unwrap_or(0.0) as f32,
                chords: d["chords"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).take(8).collect(),
                rel: rel.clone(),
            }
                })
        .collect();
    Ok(MidiPage { total, all, rows, counts })
}

/// A track or clip name as Live numbers it ("45-Phase Plant", "3 16-Spire"): the numbers
/// Live puts in front go.
pub fn clean(s: &str) -> String {
    // a clip named after a browser path ("EZX Libraries/.../Variation 02 Full"): its last part
    let s = s.rsplit('/').next().unwrap_or(s);
    let t = s.trim();
    let t = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == ' ' || c == '.');
    if t.is_empty() { s.trim().to_string() } else { t.to_string() }
}

/// Every project holding a clip, newest first.
pub fn clip_uses(con: &Connection, hash: &str) -> rusqlite::Result<Vec<ClipUse>> {
    let mut s = con.prepare(
        "select c.rel, f.song_id, coalesce(s.title, ''), c.track, c.name from clips c left join files f on f.rel = c.rel \
         left join songs s on s.id = f.song_id where c.hash = ? order by f.mtime desc",
    )?;
    s.query_map([hash], |r| Ok(ClipUse { rel: r.get(0)?, song: r.get(1)?, title: r.get(2)?, track: r.get(3)?, name: r.get(4)? }))?.collect()
}

/// A clip's notes as a MIDI file, and whether they are drums.
pub fn clip_smf(con: &Connection, hash: &str) -> rusqlite::Result<(Vec<u8>, bool)> {
    con.query_row("select smf, coalesce(drums, 0) from clip_notes where hash = ?", [hash], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))
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
    let mut s = con.prepare("select rel, unz(data) from midi")?;
    let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?;
    let (mut mine, mut library) = (Acc::default(), Acc::default());
    let mut items = Vec::new();
    for (rel, data) in rows.flatten() {
        // the projects' clips are browsed on the MIDI page; the generator reads MIDI files
        if rel.starts_with("clip:") {
            continue;
        }
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

#[cfg(test)]
mod page_tests {
    use super::*;

    #[test]
    fn filters_by_kind_key_tempo_and_words() {
        let st = store::Store::memory().unwrap();
        let add = |rel: &str, kind: &str, key: &str, bpm: Option<f64>| {
            let d = serde_json::json!({"kind": kind, "key": key, "bpm": bpm, "notes": 12, "bars": 4.0, "chords": ["Am", "F"]});
            st.con
                .execute("insert into midi(rel, size, mtime, data, kind, mkey, bpm, notes) values(?, 1, 1, ?, ?, ?, ?, 12)", rusqlite::params![rel, d.to_string(), kind, key, bpm])
                .unwrap();
        };
        add("D:/Lib/Kit 1/Chords Am 120.mid", "chords", "Am", Some(120.0));
        add("D:/Lib/Kit 1/Bass C 60.mid", "bass", "C", Some(60.0));
        add("D:/Lib/Kit 2/Lead Ebm.mid", "melody", "Ebm", None);
        st.con.execute("insert into midi(rel, size, mtime, notes, data) values('D:/empty.mid', 1, 1, 0, '{\"notes\":0}')", []).unwrap();
        // a project's clip comes first
        st.con.execute("insert into midi(rel, size, mtime, notes, data, kind, clip, bars) values('clip:ab12', 0, 0, 30, '{\"kind\":\"melody\"}', 'melody', 1, 8)", []).unwrap();
        let p = midi_page(&st.con, &MidiQuery { limit: 2, ..Default::default() }).unwrap();
        assert_eq!((p.total, p.rows.len(), p.rows[0].rel.as_str()), (4, 2, "clip:ab12"));
        let p2 = midi_page(&st.con, &MidiQuery { limit: 2, offset: 2, ..Default::default() }).unwrap();
        assert_eq!(p2.rows.len(), 2, "the second page: the files after the clip");
        assert!(p2.rows.iter().all(|r| !r.clip) && !p2.rows.iter().any(|r| p.rows.iter().any(|x| x.rel == r.rel)));
        st.con.execute("delete from midi where rel = 'clip:ab12'", []).unwrap();
        let q = MidiQuery { limit: 50, ..Default::default() };
        let p = midi_page(&st.con, &q).unwrap();
        assert_eq!((p.total, p.all), (3, 3), "a file without notes is left out");
        // A minor: Am and its relative C; 120: 120 and 60 (half)
        let p = midi_page(&st.con, &MidiQuery { key: Some("Am".into()), bpm: Some(120.0), ..q.clone() }).unwrap();
        assert_eq!(p.total, 2);
        // spelled as the analysis spells it: D# minor is "Ebm"
        let p = midi_page(&st.con, &MidiQuery { key: Some("D#m".into()), ..q.clone() }).unwrap();
        assert_eq!(p.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Lead Ebm"]);
        let p = midi_page(&st.con, &MidiQuery { kind: Some("bass".into()), q: "kit 1".into(), ..q.clone() }).unwrap();
        assert_eq!(p.total, 1);
        assert_eq!(p.counts.iter().map(|c| c.n).sum::<i64>(), 2, "counts ignore the kind");
    }
}

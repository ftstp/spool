//! The Samples page: the samples of the sample folders (crates/index samples.rs), searched and
//! filtered in the base, one page at a time (a library holds 150,000 of them), with how many
//! of each category the other filters leave.
use serde::{Deserialize, Serialize};
use store::rusqlite::{self, Connection, types::Value as Sql};
use ts_rs::TS;

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct SampleQuery {
    /// Words, all found in the name or the folders.
    pub q: String,
    pub family: Option<String>,
    pub cat: Option<String>,
    /// "loop" or "shot"
    pub kind: Option<String>,
    /// A key ("Am", "F#", "Eb", "G dorien"): loops in it or its relative, one-shots on one of
    /// its notes.
    pub key: Option<String>,
    /// A tempo: loops within 3 %, or at half or double.
    pub bpm: Option<f64>,
    /// "used" (in a project at least) or "unused" (in none).
    #[serde(default)]
    pub used: Option<String>,
    #[ts(type = "number")]
    pub offset: i64,
    #[ts(type = "number")]
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SampleRow {
    pub path: String,
    pub name: String,
    pub folder: String,
    pub pack: String,
    pub cat: Option<String>,
    pub cat_src: Option<String>,
    pub family: Option<String>,
    pub is_loop: Option<bool>,
    pub dur: Option<f64>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub note: Option<String>,
    /// In how many songs' projects it is used.
    #[ts(type = "number")]
    pub used: i64,
    /// For an unsorted sample: the best guess (crate::sample_learn), what the quick sort
    /// proposes, with its probability.
    pub guess: Option<String>,
    pub guess_cat: Option<String>,
    pub guess_conf: Option<f32>,
    /// Its note heard (the name says none): shown as such.
    pub note_heard: bool,
    /// A loop whose name says no key: the notes heard in it, the strongest first ("A C E G").
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CatCount {
    pub family: Option<String>,
    pub cat: Option<String>,
    /// A loop, a one-shot, or not known.
    pub is_loop: Option<bool>,
    #[ts(type = "number")]
    pub n: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SamplePage {
    /// Samples matching, all pages.
    #[ts(type = "number")]
    pub total: i64,
    /// Samples indexed in all.
    #[ts(type = "number")]
    pub all: i64,
    pub rows: Vec<SampleRow>,
    /// Per category, with every filter but the category's own.
    pub counts: Vec<CatCount>,
}

/// A sample's family and category as shown: the user's, else its name's, else what the samples
/// named around it ("neighbours") or the audio said when sure (crate::sample_learn writes a_family only when sure). Unsorted,
/// a long recording (not listened to) and a file that cannot be read are shown apart.
pub(crate) const FAM: &str = "coalesce(u_family, family, a_family, case when dur > 60 then 'long' when a_error is not null then 'unreadable' end)";
const CAT: &str = "case when u_family is not null then u_cat when family is not null then cat else a_cat end";
const SRC: &str = "case when u_family is not null then 'user' when family is not null then cat_src when a_family is not null then case when a_src = 'folder' then 'neighbours' else 'audio' end end";
/// Loop or one-shot as shown: a drum loop, a full loop or a demo is a loop whatever its length
/// (a bar at 174 lasts 1.4 s: the length alone called it a one-shot).
const LOOP: &str = "case when coalesce(u_family, family, a_family) in ('drumloop', 'fullloop', 'demo') then 1 else is_loop end";
/// The columns of a row (see `row`).
const COLS: &str = "path, name, folder, pack, {CAT}, {SRC}, {FAM}, {LOOP}, dur, bpm, key, coalesce(note, a_note), {USED}, g_family, g_cat, g_conf, (note is null and a_note is not null), case when key is null and note is null then a_notes end";

fn cols() -> String {
    COLS.replace("{LOOP}", LOOP).replace("{CAT}", CAT).replace("{SRC}", SRC).replace("{FAM}", FAM).replace("{USED}", USED)
}

/// The families a sample can be sorted in by hand (index::samples::FAMILIES).
fn sortable(f: &str) -> bool {
    index::samples::FAMILIES.contains(&f)
}

/// Sorts samples by hand (`family` None: back to what the name, the folder or the audio says).
/// The category must be one of the family's, or none. Returns how many were changed.
pub fn sort_by_hand(con: &Connection, paths: &[String], family: Option<&str>, cat: Option<&str>) -> rusqlite::Result<usize> {
    let tx = con.unchecked_transaction()?;
    let n = sort_by_hand_in(&tx, paths, family, cat)?;
    tx.commit()?;
    Ok(n)
}

/// The same, inside a transaction already open (one undo with its photo).
pub fn sort_by_hand_in(con: &Connection, paths: &[String], family: Option<&str>, cat: Option<&str>) -> rusqlite::Result<usize> {
    if family.is_some_and(|f| !sortable(f)) {
        return Ok(0);
    }
    let cat = family.and(cat).filter(|c| index::samples::family_of(c) == family);
    let mut n = 0;
    let mut s = con.prepare("update samples set u_family=?, u_cat=? where path=?")?;
    for p in paths {
        n += s.execute(rusqlite::params![family, cat, p])?;
    }
    Ok(n)
}

/// The unsorted samples of one folder (not its subfolders), for sorting them all at once.
pub fn unsorted_in(con: &Connection, dir: &str) -> rusqlite::Result<Vec<String>> {
    let dir = dir.trim_end_matches('/');
    let mut s = con.prepare(&format!("select path from samples where substr(path, 1, ?1) = ?2 and instr(substr(path, ?1 + 1), '/') = 0 and {FAM} is null"))?;
    s.query_map(rusqlite::params![dir.len() as i64 + 1, format!("{dir}/")], |r| r.get(0))?.collect()
}

/// A folder of unsorted samples, for the quick sort by folder: listen to a few, sort them all.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct FolderToSort {
    pub dir: String,
    /// Its unsorted samples (the folder's own, not its subfolders').
    pub paths: Vec<String>,
    /// A few of them, spread over the folder, to listen to.
    pub examples: Vec<SampleRow>,
    /// The family most of them are guessed to be (sample_learn's guesses), and by how many.
    pub guess: Option<String>,
    #[ts(type = "number")]
    pub guess_n: i64,
}

/// The folders holding the most unsorted samples, largest first, but those in `skip`.
pub fn unsorted_folders(con: &Connection, skip: &[String], limit: usize) -> rusqlite::Result<Vec<FolderToSort>> {
    use std::collections::HashMap;
    let mut s = con.prepare(&format!("select path, g_family from samples where {FAM} is null order by path"))?;
    let mut by: HashMap<String, (Vec<String>, HashMap<String, i64>)> = HashMap::new();
    for (p, g) in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?.flatten() {
        let dir = p.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
        let e = by.entry(dir).or_default();
        e.0.push(p);
        if let Some(g) = g {
            *e.1.entry(g).or_default() += 1;
        }
    }
    let mut v: Vec<(String, (Vec<String>, HashMap<String, i64>))> = by.into_iter().filter(|x| !skip.contains(&x.0)).collect();
    v.sort_by(|a, b| b.1.0.len().cmp(&a.1.0.len()).then(a.0.cmp(&b.0)));
    let mut out = Vec::new();
    for (dir, (paths, guesses)) in v.into_iter().take(limit) {
        let (guess, guess_n) = guesses.into_iter().max_by_key(|x| (x.1, x.0.clone())).map(|x| (Some(x.0), x.1)).unwrap_or((None, 0));
        // three, spread: the first, the middle, the last
        let pick: Vec<String> = [0, paths.len() / 2, paths.len().saturating_sub(1)].iter().map(|i| paths[*i].clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let examples = rows_for(con, &pick)?;
        out.push(FolderToSort { dir, paths, examples, guess, guess_n });
    }
    Ok(out)
}

/// In how many songs a sample is used (index::sample_use).
const USED: &str = "(select count(distinct song_id) from sample_use u where u.path = lower(samples.path))";

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// (tonic, minor) of a key as shown or typed.
fn tonic(k: &str) -> Option<(usize, bool)> {
    crate::detect::pitch_class(k)
}

/// The key, its relative, and the notes of its scale, as the samples are written.
fn key_terms(k: &str) -> Option<(Vec<String>, Vec<String>)> {
    let (t, minor) = tonic(k)?;
    let name = |p: usize, m: bool| format!("{}{}", NOTES[p % 12], if m { "m" } else { "" });
    let rel = if minor { name(t + 3, false) } else { name(t + 9, true) };
    let steps: [usize; 7] = if minor { [0, 2, 3, 5, 7, 8, 10] } else { [0, 2, 4, 5, 7, 9, 11] };
    Some((vec![name(t, minor), rel], steps.iter().map(|s| NOTES[(t + s) % 12].to_string()).collect()))
}

/// The WHERE clause and its values, without the category and loop filters when `with_cat` is
/// false (the counts, split by them).
fn filter(q: &SampleQuery, with_cat: bool) -> (String, Vec<Sql>) {
    let mut w: Vec<String> = vec!["1=1".into()];
    let mut v: Vec<Sql> = Vec::new();
    for word in q.q.split_whitespace().take(8) {
        w.push("lower(name || ' ' || folder) like ? escape '\\'".into());
        let esc = word.to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        v.push(Sql::Text(format!("%{esc}%")));
    }
    if with_cat {
        if let Some(f) = q.family.as_deref().filter(|f| !f.is_empty()) {
            if f == "none" {
                w.push(format!("{FAM} is null"));
            } else {
                w.push(format!("{FAM} = ?"));
                v.push(Sql::Text(f.into()));
            }
        }
        if let Some(c) = q.cat.as_deref().filter(|c| !c.is_empty()) {
            w.push(format!("{CAT} = ?"));
            v.push(Sql::Text(c.into()));
        }
        // the counts are split by loop or one-shot instead (each family shows both)
        match q.kind.as_deref() {
            Some("loop") => w.push(format!("{LOOP} = 1")),
            Some("shot") => w.push(format!("{LOOP} = 0")),
            _ => {}
        }
    }
    match q.used.as_deref() {
        Some("used") => w.push("exists (select 1 from sample_use u where u.path = lower(samples.path))".into()),
        Some("unused") => w.push("not exists (select 1 from sample_use u where u.path = lower(samples.path))".into()),
        _ => {}
    }
    if let Some((keys, notes)) = q.key.as_deref().and_then(key_terms) {
        let ks = keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let ns = notes.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // a loop with no key named: its notes heard all in the key's scale
        let scale: i64 = notes.iter().filter_map(|n| NOTES.iter().position(|x| x == n)).fold(0, |m, i| m | (1 << i));
        w.push(format!("(key in ({ks}) or (key is null and coalesce(note, a_note) in ({ns})) or (key is null and note is null and a_notes_mask is not null and (a_notes_mask & {}) = 0))", !scale & 0xfff));
        v.extend(keys.into_iter().map(Sql::Text));
        v.extend(notes.into_iter().map(Sql::Text));
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

/// The samples that sound like one (crate::sound::App::similar_samples).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Similar {
    pub of: SampleRow,
    /// Closest first, with the cosine of their embeddings (1: the same sound).
    pub rows: Vec<SimilarRow>,
    /// Samples it was compared with (those listened to so far).
    #[ts(type = "number")]
    pub compared: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SimilarRow {
    pub row: SampleRow,
    pub sim: f32,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<SampleRow> {
    Ok(SampleRow {
        path: r.get(0)?,
        name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        folder: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        pack: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        cat: r.get(4)?,
        cat_src: r.get(5)?,
        family: r.get(6)?,
        is_loop: r.get::<_, Option<i64>>(7)?.map(|x| x != 0),
        dur: r.get(8)?,
        bpm: r.get(9)?,
        key: r.get(10)?,
        note: r.get(11)?,
        used: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
        guess: r.get(13)?,
        guess_cat: r.get(14)?,
        guess_conf: r.get::<_, Option<f64>>(15)?.map(|x| x as f32),
        note_heard: r.get::<_, Option<bool>>(16)?.unwrap_or(false),
        notes: r.get(17)?,
    })
}

/// The rows of these samples, in the order given (a path not in the base is left out).
pub fn rows_for(con: &Connection, paths: &[String]) -> rusqlite::Result<Vec<SampleRow>> {
    let mut s = con.prepare(&format!("select {} from samples where path = ?", cols()))?;
    let mut out = Vec::new();
    for p in paths {
        if let Some(r) = s.query_map([p], row)?.next() {
            out.push(r?);
        }
    }
    Ok(out)
}

pub fn samples(con: &Connection, q: &SampleQuery) -> rusqlite::Result<SamplePage> {
    let all: i64 = con.query_row("select count(*) from samples", [], |r| r.get(0))?;
    let (w, v) = filter(q, true);
    // every count in one pass (read from the samples_cover index): per family, category and
    // loop / one-shot, with every filter but those; the total is the part the page's own
    // family, category and kind keep
    let (wc, vc) = filter(q, false);
    let mut s = con.prepare(&format!("select {FAM}, {CAT}, {LOOP}, count(*) from samples where {wc} group by 1, 2, 3"))?;
    let counts = s
        .query_map(rusqlite::params_from_iter(vc.iter()), |r| Ok(CatCount { family: r.get(0)?, cat: r.get(1)?, is_loop: r.get::<_, Option<i64>>(2)?.map(|x| x != 0), n: r.get(3)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let fam = q.family.as_deref().filter(|f| !f.is_empty());
    let cat = q.cat.as_deref().filter(|c| !c.is_empty());
    let total: i64 = counts
        .iter()
        .filter(|c| match fam {
            None => true,
            Some("none") => c.family.is_none(),
            Some(f) => c.family.as_deref() == Some(f),
        })
        .filter(|c| cat.is_none_or(|k| c.cat.as_deref() == Some(k)))
        .filter(|c| match q.kind.as_deref() {
            Some("loop") => c.is_loop == Some(true),
            Some("shot") => c.is_loop == Some(false),
            _ => true,
        })
        .map(|c| c.n)
        .sum();
    let mut vv = v.clone();
    vv.push(Sql::Integer(q.limit.clamp(1, 500)));
    vv.push(Sql::Integer(q.offset.max(0)));
    let mut s = con.prepare(&format!(
        "select {} from samples where {w} order by pack collate nocase, folder collate nocase, name collate nocase limit ? offset ?",
        cols()
    ))?;
    let rows = s
        .query_map(rusqlite::params_from_iter(vv.iter()), row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SamplePage { total, all, rows, counts })
}

/// Copies of one sample: the same name and size, and the same first bytes.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Dupes {
    pub name: String,
    #[ts(type = "number")]
    pub size: i64,
    pub paths: Vec<String>,
}

/// The samples kept twice or more (a pack copied in two places), the most space first. Only
/// read: nothing is deleted, the copies are shown so the user decides.
pub fn dupes(con: &Connection) -> rusqlite::Result<Vec<Dupes>> {
    let mut s = con.prepare(
        "select min(name), size, group_concat(path, char(31)) from samples where size > 4096 \
         group by lower(name), size having count(*) > 1 order by size * (count(*) - 1) desc limit 2000",
    )?;
    let groups: Vec<(String, i64, String)> =
        s.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?, r.get(2)?)))?.flatten().collect();
    let mut out = Vec::new();
    for (name, size, paths) in groups {
        // the same first 64 KB too (a sample re-exported under the same name differs there)
        let mut by: std::collections::HashMap<Vec<u8>, Vec<String>> = std::collections::HashMap::new();
        for p in paths.split('\u{1f}') {
            let Ok(mut f) = std::fs::File::open(p) else { continue };
            let mut b = vec![0u8; 65536];
            let n = std::io::Read::read(&mut f, &mut b).unwrap_or(0);
            b.truncate(n);
            by.entry(b).or_default().push(p.to_string());
        }
        for (_, mut v) in by {
            if v.len() > 1 {
                v.sort();
                out.push(Dupes { name: name.clone(), size, paths: v });
            }
        }
        if out.len() >= 500 {
            break;
        }
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.size * (d.paths.len() as i64 - 1)));
    Ok(out)
}

/// The songs whose projects use this sample.
pub fn sample_songs(con: &Connection, path: &str) -> rusqlite::Result<Vec<i64>> {
    let mut s = con.prepare("select distinct song_id from sample_use where path = lower(?)")?;
    s.query_map([path], |r| r.get(0))?.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> store::Store {
        let st = store::Store::memory().unwrap();
        let add = |p: &str, cat: Option<&str>, fam: Option<&str>, lp: i64, bpm: Option<f64>, key: Option<&str>, note: Option<&str>| {
            st.con
                .execute(
                    "insert into samples(path, name, folder, pack, cat, family, is_loop, bpm, key, note) values(?,?,?,?,?,?,?,?,?,?)",
                    rusqlite::params![p, p.rsplit('/').next().unwrap(), "Pack/Sub", "Pack", cat, fam, lp, bpm, key, note],
                )
                .unwrap();
        };
        add("D:/S/Pack/Kick 1.wav", Some("kick"), Some("drums"), 0, None, None, None);
        add("D:/S/Pack/Bass Loop 174 Am.wav", Some("bass"), Some("bass"), 1, Some(174.0), Some("Am"), None);
        add("D:/S/Pack/Pad 87 C.wav", Some("pad"), Some("melodic"), 1, Some(87.0), Some("C"), None);
        add("D:/S/Pack/808 G.wav", Some("808"), Some("bass"), 0, None, None, Some("G"));
        add("D:/S/Pack/808 G#.wav", Some("808"), Some("bass"), 0, None, None, Some("G#"));
        add("D:/S/Pack/thing.wav", None, None, 0, None, None, None);
        add("D:/S/Pack/0042.wav", None, None, 0, None, None, None);
        st.con.execute("update samples set a_family='drums', a_cat='snare' where path like '%0042%'", []).unwrap();
        st
    }

    fn q() -> SampleQuery {
        SampleQuery { limit: 100, ..Default::default() }
    }

    #[test]
    fn used_in_projects_and_never_used() {
        let mut st = base();
        st.con
            .execute_batch(
                r#"insert into files(rel, song_id, data) values
                   ('A/a.als', 1, '{"sample_paths": ["d:/s/pack/kick 1.wav", "C:/Elsewhere/x.wav"]}'),
                   ('A/a2.als', 1, '{"sample_paths": ["D:/S/Pack/Kick 1.wav"]}'),
                   ('B/b.als', 2, '{"sample_paths": ["D:/S/Pack/Kick 1.wav", "D:/S/Pack/808 G.wav"]}');"#,
            )
            .unwrap();
        let tx = st.con.transaction().unwrap();
        index::sample_use(&tx).unwrap();
        tx.commit().unwrap();
        let p = samples(&st.con, &SampleQuery { used: Some("used".into()), ..q() }).unwrap();
        let mut got: Vec<(String, i64)> = p.rows.iter().map(|r| (r.name.clone(), r.used)).collect();
        got.sort();
        assert_eq!(got, [("808 G.wav".to_string(), 1), ("Kick 1.wav".to_string(), 2)], "two songs, whatever the case of the path");
        assert_eq!(samples(&st.con, &SampleQuery { used: Some("unused".into()), ..q() }).unwrap().total, 5);
        let mut songs = sample_songs(&st.con, "D:/S/Pack/Kick 1.wav").unwrap();
        songs.sort();
        assert_eq!(songs, [1, 2]);
    }

    #[test]
    fn sorted_by_hand_wins_and_long_or_unreadable_are_apart() {
        let st = base();
        let f = unsorted_folders(&st.con, &[], 10).unwrap();
        assert_eq!((f.len(), f[0].dir.as_str(), f[0].paths.len(), f[0].examples.len()), (1, "D:/S/Pack", 1, 1));
        assert!(unsorted_folders(&st.con, &["D:/S/Pack".into()], 10).unwrap().is_empty());
        assert_eq!(unsorted_in(&st.con, "D:/S/Pack").unwrap(), ["D:/S/Pack/thing.wav"]);
        // a category of another family is dropped, an unknown family refused
        assert_eq!(sort_by_hand(&st.con, &["D:/S/Pack/thing.wav".into()], Some("fx"), Some("kick")).unwrap(), 1);
        assert_eq!(sort_by_hand(&st.con, &["D:/S/Pack/thing.wav".into()], Some("nope"), None).unwrap(), 0);
        sort_by_hand(&st.con, &["D:/S/Pack/Kick 1.wav".into()], Some("drums"), Some("snare")).unwrap();
        let r = rows_for(&st.con, &["D:/S/Pack/thing.wav".into(), "D:/S/Pack/Kick 1.wav".into()]).unwrap();
        assert_eq!((r[0].family.as_deref(), r[0].cat.as_deref(), r[0].cat_src.as_deref()), (Some("fx"), None, Some("user")));
        assert_eq!((r[1].family.as_deref(), r[1].cat.as_deref()), (Some("drums"), Some("snare")));
        assert!(unsorted_in(&st.con, "D:/S/Pack").unwrap().is_empty());
        assert!(unsorted_folders(&st.con, &[], 10).unwrap().is_empty());
        // back to automatic
        sort_by_hand(&st.con, &["D:/S/Pack/Kick 1.wav".into()], None, None).unwrap();
        assert_eq!(rows_for(&st.con, &["D:/S/Pack/Kick 1.wav".into()]).unwrap()[0].cat.as_deref(), Some("kick"));
        st.con.execute("insert into samples(path, name, dur) values('D:/S/Rip/tape.wav', 'tape', 3600)", []).unwrap();
        st.con.execute("insert into samples(path, name, a_error) values('D:/S/Rip/bad.wav', 'bad', 'cannot decode')", []).unwrap();
        let p = samples(&st.con, &SampleQuery { family: Some("long".into()), ..q() }).unwrap();
        assert_eq!(p.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["tape"]);
        assert_eq!(samples(&st.con, &SampleQuery { family: Some("unreadable".into()), ..q() }).unwrap().total, 1);
        assert_eq!(samples(&st.con, &SampleQuery { family: Some("none".into()), ..q() }).unwrap().total, 0);
    }

    #[test]
    fn a_loop_with_no_key_named_matches_by_its_notes_heard() {
        let st = base();
        // A C E (1 << 9 | 1 << 0 | 1 << 4) and A C F# (1 << 6)
        st.con.execute("insert into samples(path, name, folder, is_loop, a_notes, a_notes_mask) values('D:/S/x/l1.wav', 'l1', 'x', 1, 'A C E', 529)", []).unwrap();
        st.con.execute("insert into samples(path, name, folder, is_loop, a_notes, a_notes_mask) values('D:/S/x/l2.wav', 'l2', 'x', 1, 'A C F#', 577)", []).unwrap();
        let p = samples(&st.con, &SampleQuery { key: Some("Am".into()), q: "l".into(), ..q() }).unwrap();
        let names: Vec<&str> = p.rows.iter().map(|r| r.name.as_str()).filter(|n| n.starts_with('l')).collect();
        assert_eq!(names, ["l1"]);
        assert_eq!(rows_for(&st.con, &["D:/S/x/l1.wav".into()]).unwrap()[0].notes.as_deref(), Some("A C E"));
    }

    #[test]
    fn copies_need_the_same_name_size_and_first_bytes() {
        let d = std::env::temp_dir().join(format!("pf-dupes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let w = |n: &str, b: &[u8]| {
            let p = d.join(n);
            std::fs::write(&p, b).unwrap();
            p.to_string_lossy().replace('\\', "/")
        };
        let same = vec![7u8; 9000];
        let mut other = same.clone();
        other[0] = 1;
        let (a, b) = (w("a.wav", &same), w("b.wav", &same));
        let c = w("c.wav", &other);
        let st = store::Store::memory().unwrap();
        for p in [&a, &b, &c] {
            st.con.execute("insert into samples(path, name, size) values(?, 'Kick.wav', 9000)", [p]).unwrap();
        }
        let g = dupes(&st.con).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].paths, [a.clone(), b.clone()], "c differs in its bytes");
        assert!(std::path::Path::new(&c).exists(), "nothing is ever deleted");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn filters_and_counts() {
        let st = base();
        let p = samples(&st.con, &q()).unwrap();
        assert_eq!((p.total, p.all), (7, 7));
        let p = samples(&st.con, &SampleQuery { family: Some("bass".into()), ..q() }).unwrap();
        assert_eq!(p.total, 3);
        // the counts ignore the category filter, so the other categories stay offered
        assert_eq!(p.counts.iter().map(|c| c.n).sum::<i64>(), 7);
        // sorted by the audio: in its family, its category, marked as such
        let p = samples(&st.con, &SampleQuery { family: Some("drums".into()), cat: Some("snare".into()), ..q() }).unwrap();
        assert_eq!(p.rows.len(), 1);
        assert_eq!((p.rows[0].name.as_str(), p.rows[0].cat_src.as_deref()), ("0042.wav", Some("audio")));
        let p = samples(&st.con, &SampleQuery { family: Some("none".into()), ..q() }).unwrap();
        assert_eq!(p.total, 1);
        let p = samples(&st.con, &SampleQuery { q: "loop 174".into(), ..q() }).unwrap();
        assert_eq!(p.rows[0].name, "Bass Loop 174 Am.wav");
        // a LIKE wildcard typed is a plain character
        assert_eq!(samples(&st.con, &SampleQuery { q: "%".into(), ..q() }).unwrap().total, 0);
    }

    #[test]
    fn a_songs_key_and_tempo() {
        let st = base();
        // A minor: the loop in Am and the pad in C (relative), the 808 on G (in the scale), not G#
        let p = samples(&st.con, &SampleQuery { key: Some("Am".into()), ..q() }).unwrap();
        let mut names: Vec<_> = p.rows.iter().map(|r| r.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["808 G.wav", "Bass Loop 174 Am.wav", "Pad 87 C.wav"]);
        // 174: the loop at 174 and the pad at 87 (half time)
        let p = samples(&st.con, &SampleQuery { bpm: Some(174.0), ..q() }).unwrap();
        assert_eq!(p.total, 2);
        let p = samples(&st.con, &SampleQuery { bpm: Some(174.0), kind: Some("loop".into()), key: Some("C".into()), ..q() }).unwrap();
        assert_eq!(p.total, 2);
    }
}

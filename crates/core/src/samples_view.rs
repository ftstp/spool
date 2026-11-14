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
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CatCount {
    pub family: Option<String>,
    pub cat: Option<String>,
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

/// The WHERE clause and its values, without the category filters when `with_cat` is false.
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
                w.push("family is null".into());
            } else {
                w.push("family = ?".into());
                v.push(Sql::Text(f.into()));
            }
        }
        if let Some(c) = q.cat.as_deref().filter(|c| !c.is_empty()) {
            w.push("cat = ?".into());
            v.push(Sql::Text(c.into()));
        }
    }
    match q.kind.as_deref() {
        Some("loop") => w.push("is_loop = 1".into()),
        Some("shot") => w.push("is_loop = 0".into()),
        _ => {}
    }
    if let Some((keys, notes)) = q.key.as_deref().and_then(key_terms) {
        let ks = keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let ns = notes.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        w.push(format!("(key in ({ks}) or (key is null and note in ({ns})))"));
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

pub fn samples(con: &Connection, q: &SampleQuery) -> rusqlite::Result<SamplePage> {
    let all: i64 = con.query_row("select count(*) from samples", [], |r| r.get(0))?;
    let (w, v) = filter(q, true);
    let total: i64 = con.query_row(&format!("select count(*) from samples where {w}"), rusqlite::params_from_iter(v.iter()), |r| r.get(0))?;
    let mut vv = v.clone();
    vv.push(Sql::Integer(q.limit.clamp(1, 500)));
    vv.push(Sql::Integer(q.offset.max(0)));
    let mut s = con.prepare(&format!(
        "select path, name, folder, pack, cat, cat_src, family, is_loop, dur, bpm, key, note from samples where {w} \
         order by pack collate nocase, folder collate nocase, name collate nocase limit ? offset ?"
    ))?;
    let rows = s
        .query_map(rusqlite::params_from_iter(vv.iter()), |r| {
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
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (wc, vc) = filter(q, false);
    let mut s = con.prepare(&format!("select family, cat, count(*) from samples where {wc} group by family, cat"))?;
    let counts = s
        .query_map(rusqlite::params_from_iter(vc.iter()), |r| Ok(CatCount { family: r.get(0)?, cat: r.get(1)?, n: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SamplePage { total, all, rows, counts })
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
        st
    }

    fn q() -> SampleQuery {
        SampleQuery { limit: 100, ..Default::default() }
    }

    #[test]
    fn filters_and_counts() {
        let st = base();
        let p = samples(&st.con, &q()).unwrap();
        assert_eq!((p.total, p.all), (6, 6));
        let p = samples(&st.con, &SampleQuery { family: Some("bass".into()), ..q() }).unwrap();
        assert_eq!(p.total, 3);
        // the counts ignore the category filter, so the other categories stay offered
        assert_eq!(p.counts.iter().map(|c| c.n).sum::<i64>(), 6);
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

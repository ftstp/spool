//! The quick sort as questions: what to ask the user next, so that each answer sorts as many
//! samples as can be sorted rightly with it.
//!
//! A question is a folder's unsorted samples, plus the unsorted samples elsewhere that sound
//! like them (ALIKE or more: measured on 2,272 samples sorted by hand, a sample that close to
//! one sorted by hand was of its family every time, its category 97 % of the time, across
//! folders too). The largest first: one answer, the most samples sorted. After an answer, the
//! samples that sound like the ones just sorted are asked next ("these sound like it: the
//! same?"), never sorted silently.
//!
//! The folder alone settles nothing more than sample_learn already does: neighbours sorted by
//! their names or their sound were the user's answer only 87 % of the time.
use crate::samples_view::{FAM, SampleRow, rows_for};
use serde::Serialize;
use std::collections::HashMap;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

/// Sounds alike enough to be asked together, or proposed after an answer.
pub const ALIKE: f32 = 0.93;
/// A sample sorted by hand this close settles the guess shown.
const HAND: f32 = 0.9;
/// At most this many alike samples added to a question (the closest).
const MAX_ALIKE: usize = 60;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Question {
    /// The folder (its path).
    pub dir: String,
    /// Its unsorted samples (the folder's own, not its subfolders').
    pub rows: Vec<SampleRow>,
    /// Unsorted samples of other folders that sound like them, closest first.
    pub alike: Vec<SampleRow>,
    /// The family (and category) Spool would answer, and how many of the samples lean to it.
    pub guess: Option<String>,
    pub guess_cat: Option<String>,
    #[ts(type = "number")]
    pub guess_n: i64,
    /// A sample sorted by hand that one of them sounds like (the guess comes from it).
    pub like: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Questions {
    pub items: Vec<Question>,
    /// Folders still holding unsorted samples: the questions left, at most.
    #[ts(type = "number")]
    pub folders: i64,
    #[ts(type = "number")]
    pub unsorted: i64,
}

fn dir_of(p: &str) -> &str {
    p.rsplit_once('/').map(|x| x.0).unwrap_or("")
}

/// An embedding kept as bytes, as a unit vector.
fn unit(e: &[u8]) -> Option<Vec<f32>> {
    if e.len() != 512 {
        return None;
    }
    let v: Vec<f32> = e.iter().map(|b| (*b as i8) as f32).collect();
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    (n > 0.0).then(|| v.into_iter().map(|x| x / n).collect())
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

struct Unsorted {
    path: String,
    emb: Option<Vec<f32>>,
    /// sample_learn's guess: family, category, probability (at most 1)
    guess: Option<(String, Option<String>, f32)>,
}

fn unsorted(con: &Connection) -> rusqlite::Result<Vec<Unsorted>> {
    let mut q = con.prepare(&format!("select path, emb, g_family, g_cat, g_conf from samples where {FAM} is null order by path"))?;
    q.query_map([], |r| {
        let e: Option<Vec<u8>> = r.get(1)?;
        let g: Option<String> = r.get(2)?;
        let c: Option<String> = r.get(3)?;
        let p: Option<f64> = r.get(4)?;
        Ok(Unsorted { path: r.get(0)?, emb: e.as_deref().and_then(unit), guess: g.map(|g| (g, c, p.unwrap_or(0.0).clamp(0.0, 1.0) as f32)) })
    })?
    .collect()
}

struct Hand {
    name: String,
    family: String,
    cat: Option<String>,
    emb: Vec<f32>,
}

/// The samples sorted by hand that were heard.
fn by_hand(con: &Connection) -> rusqlite::Result<Vec<Hand>> {
    let mut q = con.prepare("select name, u_family, u_cat, emb from samples where u_family is not null and emb is not null")?;
    let rows = q.query_map([], |r| {
        Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Vec<u8>>(3)?))
    })?;
    Ok(rows.flatten().filter_map(|(name, family, cat, e)| unit(&e).map(|emb| Hand { name, family, cat, emb })).collect())
}

/// The next questions, largest first, but the folders in `skip`.
pub fn questions(con: &Connection, skip: &[String], limit: usize) -> rusqlite::Result<Questions> {
    let all = unsorted(con)?;
    let mut by: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, u) in all.iter().enumerate() {
        by.entry(dir_of(&u.path)).or_default().push(i);
    }
    let folders = by.len() as i64;
    let mut dirs: Vec<(&str, Vec<usize>)> = by.into_iter().filter(|(d, _)| !skip.iter().any(|s| s == d)).collect();
    dirs.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    let hand = by_hand(con)?;
    let mut items = Vec::new();
    for (dir, idx) in dirs.into_iter().take(limit) {
        // the unsorted elsewhere that sound like one of them
        let mut alike: Vec<(f32, usize)> = Vec::new();
        for (j, o) in all.iter().enumerate() {
            let Some(oe) = &o.emb else { continue };
            if dir_of(&o.path) == dir {
                continue;
            }
            let best = idx.iter().filter_map(|i| all[*i].emb.as_ref()).map(|e| dot(e, oe)).fold(f32::MIN, f32::max);
            if best >= ALIKE {
                alike.push((best, j));
            }
        }
        alike.sort_by(|a, b| b.0.total_cmp(&a.0));
        alike.truncate(MAX_ALIKE);
        // the guess: what the samples sorted by hand they sound like say (twice the weight),
        // else sample_learn's guesses
        let mut votes: HashMap<(String, Option<String>), f32> = HashMap::new();
        let mut lean: HashMap<String, i64> = HashMap::new();
        let mut like: Option<(f32, String)> = None;
        for i in idx.iter().copied().chain(alike.iter().map(|x| x.1)) {
            let u = &all[i];
            let near = u.emb.as_ref().and_then(|e| {
                hand.iter().map(|h| (dot(e, &h.emb), h)).max_by(|a, b| a.0.total_cmp(&b.0)).filter(|x| x.0 >= HAND)
            });
            let fam = if let Some((s, h)) = near {
                *votes.entry((h.family.clone(), h.cat.clone())).or_default() += 2.0;
                if like.as_ref().is_none_or(|l| s > l.0) {
                    like = Some((s, h.name.clone()));
                }
                Some(h.family.clone())
            } else if let Some(g) = &u.guess {
                *votes.entry((g.0.clone(), g.1.clone())).or_default() += g.2.max(0.05);
                Some(g.0.clone())
            } else {
                None
            };
            if let Some(f) = fam {
                *lean.entry(f).or_default() += 1;
            }
        }
        // the family the most votes go to, then its most voted category
        let mut fam_votes: HashMap<&str, f32> = HashMap::new();
        for ((f, _), w) in &votes {
            *fam_votes.entry(f.as_str()).or_default() += w;
        }
        let guess = fam_votes.iter().max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(a.0))).map(|x| x.0.to_string());
        let guess_cat = guess.as_ref().and_then(|g| {
            votes.iter().filter(|((f, c), _)| f == g && c.is_some()).max_by(|a, b| a.1.total_cmp(b.1)).and_then(|((_, c), _)| c.clone())
        });
        let guess_n = guess.as_ref().and_then(|g| lean.get(g).copied()).unwrap_or(0);
        let paths: Vec<String> = idx.iter().map(|i| all[*i].path.clone()).collect();
        let others: Vec<String> = alike.iter().map(|x| all[x.1].path.clone()).collect();
        items.push(Question {
            dir: dir.to_string(),
            rows: rows_for(con, &paths)?,
            alike: rows_for(con, &others)?,
            guess,
            guess_cat,
            guess_n,
            like: like.map(|l| l.1),
        });
    }
    Ok(Questions { items, folders, unsorted: all.len() as i64 })
}

/// The unsorted samples that sound like these (just sorted), closest first: asked next.
pub fn alike_unsorted(con: &Connection, paths: &[String]) -> rusqlite::Result<Vec<SampleRow>> {
    let mut q = con.prepare("select emb from samples where path = ?")?;
    let seeds: Vec<Vec<f32>> = paths
        .iter()
        .filter_map(|p| q.query_row([p], |r| r.get::<_, Option<Vec<u8>>>(0)).ok().flatten())
        .filter_map(|e| unit(&e))
        .collect();
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut hits: Vec<(f32, String)> = unsorted(con)?
        .into_iter()
        .filter(|u| !paths.contains(&u.path))
        .filter_map(|u| {
            let e = u.emb?;
            let s = seeds.iter().map(|x| dot(x, &e)).fold(f32::MIN, f32::max);
            (s >= ALIKE).then_some((s, u.path))
        })
        .collect();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0));
    hits.truncate(MAX_ALIKE);
    rows_for(con, &hits.into_iter().map(|h| h.1).collect::<Vec<_>>())
}

/// Every sample of a folder (not its subfolders), sorted or not: the folder shown under the
/// question.
pub fn folder_rows(con: &Connection, dir: &str) -> rusqlite::Result<Vec<SampleRow>> {
    let dir = dir.trim_end_matches('/');
    let mut s = con.prepare(
        "select path from samples where substr(path, 1, ?1) = ?2 and instr(substr(path, ?1 + 1), '/') = 0 order by name collate nocase",
    )?;
    let paths: Vec<String> = s.query_map(rusqlite::params![dir.len() as i64 + 1, format!("{dir}/")], |r| r.get(0))?.collect::<Result<_, _>>()?;
    rows_for(con, &paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sound: the same seed, the same sound; another seed, far.
    fn emb(seed: u8) -> Vec<u8> {
        (0..512u32).map(|i| if (i * seed as u32 + seed as u32) % 7 < 3 { 100u8 } else { 0 }).collect()
    }

    #[test]
    fn a_question_is_a_folder_and_what_sounds_like_it() {
        let st = store::Store::memory().unwrap();
        let ins = |p: &str, e: Vec<u8>, u: Option<&str>| {
            st.con
                .execute("insert into samples(path, name, emb, u_family) values(?, ?, ?, ?)", rusqlite::params![p, p.rsplit('/').next().unwrap(), e, u])
                .unwrap();
        };
        ins("/lib/A/a1.wav", emb(3), None);
        ins("/lib/A/a2.wav", emb(5), None);
        ins("/lib/B/b1.wav", emb(3), None); // sounds like a1
        ins("/lib/C/c1.wav", emb(11), None);
        ins("/lib/D/d1.wav", emb(3), Some("bass")); // sorted by hand, like a1
        let q = questions(&st.con, &[], 10).unwrap();
        assert_eq!(q.unsorted, 4);
        assert_eq!(q.folders, 3);
        let a = &q.items[0];
        assert_eq!(a.dir, "/lib/A");
        assert_eq!(a.rows.len(), 2);
        assert_eq!(a.alike.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/lib/B/b1.wav"]);
        assert_eq!(a.guess.as_deref(), Some("bass"));
        assert_eq!(a.like.as_deref(), Some("d1.wav"));
        // skipped: the others first
        let q = questions(&st.con, &["/lib/A".into()], 10).unwrap();
        assert!(q.items.iter().all(|x| x.dir != "/lib/A"));
        // after an answer, what sounds like it
        let al = alike_unsorted(&st.con, &["/lib/A/a1.wav".into()]).unwrap();
        assert_eq!(al.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/lib/B/b1.wav"]);
        assert_eq!(folder_rows(&st.con, "/lib/D").unwrap().len(), 1);
    }
}

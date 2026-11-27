//! The samples the names leave unsorted, sorted by what the user's own library teaches.
//!
//! Most of a library sorts itself by its names (some 120,000 of 150,000 samples on the library
//! measured), and those samples, heard (their CLAP embedding), show how this library's kicks,
//! pads and vocals sound. A small model learnt on them, on this computer (a softmax
//! regression, a few seconds on the processor, never saved, nothing sent anywhere), sorts the
//! others: on folders it had not seen, right 97 % of the time on the family and 95 % on the
//! category when sure (0.9 or more). What it is not sure of, or a library with too few named
//! samples to learn from, falls back on the small family model shipped with Spool
//! (audio::sample_family), from 0.95.
//!
//! Before the audio, the folder: a sample whose folder holds at least 5 samples sorted by their
//! names (or by the user), 90 % of them of one family, is of that family (measured: 99.5 %
//! right). The user's own sorting (u_family) wins over everything and teaches the model like a
//! name does; what the folder or the audio said never does (it would learn from itself).
//!
//! It is learnt again whenever the named, the heard or the user's samples change, and only
//! writes what it says (a_family, a_cat, a_src) and its best guess for the others (g_family,
//! g_cat, g_conf: the quick sort's proposal): the name's own category always wins.
use store::rusqlite::{self, Connection, params};

/// Fewer named samples than this: nothing learnt (too few examples to be right).
const MIN_EXAMPLES: usize = 2_000;
/// At most this many examples (a random part of a larger library): the learning stays short.
const MAX_EXAMPLES: usize = 60_000;
/// Written from this probability (measured: 97 % right on the family, 95 % on the category).
const SURE: f32 = 0.9;
const DIM: usize = 512;
/// The model shipped with Spool (audio::sample_family) writes from this probability: 97 %
/// right there on a library it was not learnt on.
const SHIPPED_SURE: f32 = 0.95;
/// A folder settles a sample's family from this many samples sorted around it...
const FOLDER_MIN: usize = 5;
/// ...this share of them of one family (measured: 99.5 % right).
const FOLDER_SHARE: f64 = 0.9;

struct Model {
    classes: Vec<String>,
    w: Vec<f32>, // DIM x K, row-major by input
    b: Vec<f32>,
}

impl Model {
    fn probs(&self, x: &[f32], out: &mut Vec<f32>) {
        let k = self.classes.len();
        out.clear();
        out.extend_from_slice(&self.b);
        for (i, xi) in x.iter().enumerate() {
            if *xi == 0.0 {
                continue;
            }
            let row = &self.w[i * k..(i + 1) * k];
            for (o, w) in out.iter_mut().zip(row) {
                *o += xi * w;
            }
        }
        let max = out.iter().cloned().fold(f32::MIN, f32::max);
        let mut sum = 0.0;
        for o in out.iter_mut() {
            *o = (*o - max).exp();
            sum += *o;
        }
        for o in out.iter_mut() {
            *o /= sum;
        }
    }

    /// Softmax regression by mini-batch gradient descent: 6 passes, batches of 256.
    fn learn(xs: &[Vec<f32>], labels: &[String]) -> Option<Model> {
        let mut classes: Vec<String> = labels.to_vec();
        classes.sort();
        classes.dedup();
        if classes.len() < 2 {
            return None;
        }
        let k = classes.len();
        let y: Vec<usize> = labels.iter().map(|l| classes.binary_search(l).unwrap_or(0)).collect();
        let mut m = Model { classes, w: vec![0.0; DIM * k], b: vec![0.0; k] };
        let mut order: Vec<usize> = (0..xs.len()).collect();
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let (lr, l2) = (0.5f32, 1e-5f32);
        let mut p = Vec::with_capacity(k);
        let mut gw = vec![0.0f32; DIM * k];
        let mut gb = vec![0.0f32; k];
        for _ in 0..6 {
            // a fixed shuffle (the same learning every time on the same library)
            for i in (1..order.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                order.swap(i, (seed % (i as u64 + 1)) as usize);
            }
            for batch in order.chunks(256) {
                gw.iter_mut().for_each(|g| *g = 0.0);
                gb.iter_mut().for_each(|g| *g = 0.0);
                for &j in batch {
                    m.probs(&xs[j], &mut p);
                    p[y[j]] -= 1.0;
                    for (i, xi) in xs[j].iter().enumerate() {
                        if *xi == 0.0 {
                            continue;
                        }
                        let row = &mut gw[i * k..(i + 1) * k];
                        for (g, d) in row.iter_mut().zip(&p) {
                            *g += xi * d;
                        }
                    }
                    for (g, d) in gb.iter_mut().zip(&p) {
                        *g += d;
                    }
                }
                let n = batch.len() as f32;
                for (w, g) in m.w.iter_mut().zip(&gw) {
                    *w -= lr * (g / n + l2 * *w);
                }
                for (b, g) in m.b.iter_mut().zip(&gb) {
                    *b -= lr * g / n;
                }
            }
        }
        Some(m)
    }
}

/// An embedding kept as bytes (sound::emb_i8) back to floats.
fn unbyte(e: &[u8]) -> Option<Vec<f32>> {
    (e.len() == DIM).then(|| e.iter().map(|b| (*b as i8) as f32 / 127.0).collect())
}

/// A family (and category) said of an unsorted sample.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Said {
    pub family: String,
    pub cat: Option<String>,
    pub conf: f32,
}

/// What `plan` says of one unsorted sample: sorted (sure, with where from: "folder" or
/// "audio"), or only a guess for the quick sort.
#[derive(Debug, Clone)]
pub(crate) struct Sorted {
    pub path: String,
    pub sure: Option<(Said, &'static str)>,
    pub guess: Option<Said>,
}

fn dir_of(p: &str) -> &str {
    p.rsplit_once('/').map(|x| x.0).unwrap_or("")
}

/// The family of each folder whose sorted samples settle it (true), or only lean to it (false:
/// 2 or more, 60 %), for a guess.
fn folders(con: &Connection) -> rusqlite::Result<std::collections::HashMap<String, (Said, bool)>> {
    use std::collections::HashMap;
    let mut q = con.prepare(
        "select path, coalesce(u_family, family) from samples where coalesce(u_family, family) is not null",
    )?;
    let mut by: HashMap<String, Vec<String>> = HashMap::new();
    for r in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.flatten() {
        by.entry(dir_of(&r.0).to_string()).or_default().push(r.1);
    }
    let mut out = HashMap::new();
    for (dir, v) in by {
        let mut fams: HashMap<&str, usize> = HashMap::new();
        for f in &v {
            *fams.entry(f.as_str()).or_default() += 1;
        }
        let Some((f, n)) = fams.into_iter().max_by_key(|x| (x.1, x.0)) else { continue };
        let share = n as f64 / v.len() as f64;
        let sure = v.len() >= FOLDER_MIN && share >= FOLDER_SHARE;
        if !sure && !(v.len() >= 2 && share >= 0.6) {
            continue;
        }
        // the family only: a folder's category was measured far less sure ("Foley" holding
        // mostly keys, "Reverse Instruments" mostly guitars)
        // a lean from few samples is a weak guess: 2 of 2 is not "100 %" (shown as 40 %)
        // (at most the share itself: a large folder half of one family is not "300 %")
        let conf = if sure { share } else { share * (v.len() as f64 / FOLDER_MIN as f64).min(1.0) };
        out.insert(dir, (Said { family: f.to_string(), cat: None, conf: conf as f32 }, sure));
    }
    Ok(out)
}

/// Learns from the samples sorted by their names (or by the user), then says which unsorted
/// ones it is sure of, and its best guess for the others. Reads only (on the reader's
/// connection: the base stays free meanwhile).
pub(crate) fn plan(con: &Connection) -> rusqlite::Result<Vec<Sorted>> {
    // the examples: samples named (or sorted by the user) that were heard (a drum loop is its
    // own family here: the sound tells a loop from a hit)
    let mut q = con.prepare(
        "select coalesce(u_family, family), case when u_family is not null then u_cat else cat end, emb from samples \
         where coalesce(u_family, family) is not null and emb is not null",
    )?;
    let mut ex: Vec<(String, Option<String>, Vec<f32>)> = q
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Vec<u8>>(2)?)))?
        .flatten()
        .filter_map(|(f, c, e)| unbyte(&e).map(|v| (f, c, v)))
        .collect();
    // the library's own models, when it has enough named samples to learn from
    let mut models: Option<(Model, Option<Model>, std::collections::HashMap<String, String>)> = None;
    if ex.len() >= MIN_EXAMPLES {
        if ex.len() > MAX_EXAMPLES {
            // every n-th: a part of the whole library, not of its first folders
            let step = ex.len() as f64 / MAX_EXAMPLES as f64;
            ex = (0..MAX_EXAMPLES).map(|i| std::mem::take(&mut ex[(i as f64 * step) as usize])).collect();
        }
        let xs: Vec<Vec<f32>> = ex.iter().map(|e| e.2.clone()).collect();
        let fams: Vec<String> = ex.iter().map(|e| e.0.clone()).collect();
        if let Some(fam) = Model::learn(&xs, &fams) {
            // the categories, from the examples that have one; each category's family
            let named: Vec<usize> = (0..ex.len()).filter(|i| ex[*i].1.is_some()).collect();
            let cat = Model::learn(&named.iter().map(|i| xs[*i].clone()).collect::<Vec<_>>(), &named.iter().map(|i| ex[*i].1.clone().unwrap_or_default()).collect::<Vec<_>>());
            let cat_family = ex.iter().filter_map(|e| e.1.clone().map(|c| (c, e.0.clone()))).collect();
            models = Some((fam, cat, cat_family));
        }
    }
    drop(ex);
    let dirs = folders(con)?;
    // every unsorted sample, heard or not (the folder needs no listening)
    let todo: Vec<(String, Option<Vec<u8>>)> = {
        let mut q = con.prepare("select path, emb from samples where family is null and u_family is null")?;
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten().collect()
    };
    let mut out = Vec::new();
    let (mut pf, mut pc) = (Vec::new(), Vec::new());
    let best = |p: &[f32]| p.iter().enumerate().fold((0, 0.0f32), |a, (i, p)| if *p > a.1 { (i, *p) } else { a });
    for (path, e) in todo {
        let folder = dirs.get(dir_of(&path));
        if let Some((said, true)) = folder {
            out.push(Sorted { path, sure: Some((said.clone(), "folder")), guess: None });
            continue;
        }
        let mut guess: Option<Said> = folder.map(|f| f.0.clone());
        if let Some(x) = e.as_deref().and_then(unbyte) {
            if let Some((fam, cat, cat_family)) = &models {
                fam.probs(&x, &mut pf);
                let (fi, fp) = best(&pf);
                let f = fam.classes[fi].clone();
                let c = cat.as_ref().and_then(|m| {
                    m.probs(&x, &mut pc);
                    let (ci, cp) = best(&pc);
                    let c = &m.classes[ci];
                    (cat_family.get(c) == Some(&f)).then(|| (c.clone(), cp))
                });
                if fp >= SURE {
                    let said = Said { family: f, cat: c.filter(|c| c.1 >= SURE).map(|c| c.0), conf: fp };
                    out.push(Sorted { path, sure: Some((said, "audio")), guess: None });
                    continue;
                }
                // the guess keeps the category it leans to, sure or not
                let g = Said { family: f, cat: c.map(|c| c.0), conf: fp };
                if guess.as_ref().is_none_or(|x| x.conf < g.conf) {
                    guess = Some(g);
                }
            }
            // what the library does not settle: the model shipped with Spool, from 0.95 (its
            // family only)
            let (f, p) = audio::sample_family::family(&x);
            if p >= SHIPPED_SURE {
                out.push(Sorted { path, sure: Some((Said { family: f, cat: None, conf: p }, "audio")), guess: None });
                continue;
            }
            if guess.is_none() {
                guess = Some(Said { family: f, cat: None, conf: p });
            }
        }
        if guess.is_some() {
            out.push(Sorted { path, sure: None, guess });
        }
    }
    Ok(out)
}

/// Writes what `plan` found, in one short transaction. A sample sorted by its name or by the
/// user meanwhile is left as it is.
pub(crate) fn write(con: &Connection, sorted: &[Sorted]) -> rusqlite::Result<usize> {
    let tx = con.unchecked_transaction()?;
    // what was said before (an earlier learning, an earlier model) gives way to this one: a
    // sample it is not sure of is unsorted again
    tx.execute(
        "update samples set a_family=null, a_cat=null, a_conf=null, a_src=null, g_family=null, g_cat=null, g_conf=null \
         where family is null and (a_family is not null or g_family is not null)",
        [],
    )?;
    let mut n = 0;
    {
        let mut sure = tx.prepare("update samples set a_family=?, a_cat=?, a_conf=?, a_src=? where path=? and family is null and u_family is null")?;
        let mut guess = tx.prepare("update samples set g_family=?, g_cat=?, g_conf=? where path=? and family is null and u_family is null")?;
        for s in sorted {
            if let Some((x, src)) = &s.sure {
                n += sure.execute(params![x.family, x.cat, x.conf as f64, src, s.path])?;
            } else if let Some(g) = &s.guess {
                guess.execute(params![g.family, g.cat, g.conf as f64, s.path])?;
            }
        }
    }
    tx.commit()?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> store::Store {
        let st = store::Store::memory().unwrap();
        for i in 0..6 {
            st.con.execute("insert into samples(path, name, family, cat) values(?, 'k', 'drums', 'kick')", [format!("/lib/Kicks A/k{i}.wav")]).unwrap();
        }
        st.con.execute("insert into samples(path, name) values('/lib/Kicks A/thing.wav', 'thing')", []).unwrap();
        st.con.execute("insert into samples(path, name, family) values('/lib/Mixed/a.wav', 'a', 'bass')", []).unwrap();
        st.con.execute("insert into samples(path, name, family) values('/lib/Mixed/b.wav', 'b', 'bass')", []).unwrap();
        st.con.execute("insert into samples(path, name) values('/lib/Mixed/c.wav', 'c')", []).unwrap();
        st
    }

    #[test]
    fn a_folder_sorted_around_settles_a_sample() {
        let st = base();
        let plan = plan(&st.con).unwrap();
        assert_eq!(write(&st.con, &plan).unwrap(), 1);
        let r: (String, Option<String>, String) = st
            .con
            .query_row("select a_family, a_cat, a_src from samples where name='thing'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!(r, ("drums".into(), None, "folder".into()));
        // two around it: only a guess, for the quick sort
        let g: (Option<String>, Option<String>) =
            st.con.query_row("select a_family, g_family from samples where name='c'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(g, (None, Some("bass".into())));
    }

    #[test]
    fn the_users_sorting_counts_and_is_never_overwritten() {
        let st = base();
        st.con.execute("update samples set u_family='fx' where name='thing'", []).unwrap();
        st.con.execute("update samples set u_family='bass' where name='c'", []).unwrap();
        let plan = plan(&st.con).unwrap();
        assert!(plan.iter().all(|s| !s.path.ends_with("thing.wav") && !s.path.ends_with("c.wav")));
        write(&st.con, &plan).unwrap();
        let a: Option<String> = st.con.query_row("select a_family from samples where name='thing'", [], |r| r.get(0)).unwrap();
        assert_eq!(a, None);
    }
}

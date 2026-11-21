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
//! It is learnt again whenever the named or the heard samples change, and only writes what the
//! audio says (a_family, a_cat): the name's own category always wins.
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

/// What the audio says of an unsorted sample: (path, family, category, probability).
pub(crate) type Sorted = (String, String, Option<String>, f32);

/// Learns from the samples sorted by their names, then says which unsorted ones it is sure
/// of. Reads only (on the reader's connection: the base stays free meanwhile).
pub(crate) fn plan(con: &Connection) -> rusqlite::Result<Vec<Sorted>> {
    // the examples: named samples that were heard (a drum loop is its own family here: the
    // sound tells a loop from a hit)
    let mut q = con.prepare("select family, cat, emb from samples where family is not null and emb is not null")?;
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
    // the unsorted ones heard
    let todo: Vec<(String, Vec<u8>)> = {
        let mut q = con.prepare("select path, emb from samples where family is null and emb is not null")?;
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.flatten().collect()
    };
    let mut out = Vec::new();
    let (mut pf, mut pc) = (Vec::new(), Vec::new());
    for (path, e) in todo {
        let Some(x) = unbyte(&e) else { continue };
        if let Some((fam, cat, cat_family)) = &models {
            fam.probs(&x, &mut pf);
            let (fi, fp) = pf.iter().enumerate().fold((0, 0.0f32), |a, (i, p)| if *p > a.1 { (i, *p) } else { a });
            if fp >= SURE {
                let f = fam.classes[fi].clone();
                let c = cat.as_ref().and_then(|m| {
                    m.probs(&x, &mut pc);
                    let (ci, cp) = pc.iter().enumerate().fold((0, 0.0f32), |a, (i, p)| if *p > a.1 { (i, *p) } else { a });
                    let c = &m.classes[ci];
                    (cp >= SURE && cat_family.get(c) == Some(&f)).then(|| c.clone())
                });
                out.push((path, f, c, fp));
                continue;
            }
        }
        // what the library does not settle: the model shipped with Spool, from 0.95 (its
        // family only)
        let (f, p) = audio::sample_family::family(&x);
        if p >= SHIPPED_SURE {
            out.push((path, f, None, p));
        }
    }
    Ok(out)
}

/// Writes what `plan` found, in one short transaction. A sample sorted by its name meanwhile
/// (a scan in between) is left as it is.
pub(crate) fn write(con: &Connection, sorted: &[Sorted]) -> rusqlite::Result<usize> {
    let tx = con.unchecked_transaction()?;
    // what the audio said before (an earlier learning, an earlier model)
    // gives way to this one: a sample it is not sure of is unsorted again
    tx.execute("update samples set a_family=null, a_cat=null, a_conf=null where family is null and a_family is not null", [])?;
    let mut n = 0;
    for (path, f, c, p) in sorted {
        n += tx.execute("update samples set a_family=?, a_cat=?, a_conf=? where path=? and family is null", params![f, c, *p as f64, path])?;
    }
    tx.commit()?;
    Ok(n)
}

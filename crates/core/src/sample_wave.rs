//! A sample's shape for a list row (Samples page): BINS loudness values, enough to tell a hit
//! from a pad, a riser or a drum loop at a glance. Worked out the first time a row shows it,
//! then kept in the base (`sample_wave`) until the file changes.
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use store::rusqlite::{self, Connection, params};

pub const BINS: usize = 48;
/// Longer (a recording of an hour: 600 MB decoded) is not drawn.
const MAX_DUR: f64 = 600.0;

pub const TABLE: &str = "create table if not exists sample_wave (path text primary key, size integer, mtime real, w blob)";

/// The loudest point of each slice, 0..255, against the sample's own loudest (a quiet sample
/// keeps its shape), on a square-root scale (a tail stays visible). Empty for silence.
pub fn shape(path: &Path) -> Option<Vec<u8>> {
    let pcm = ::audio::decode::decode_mono(path).ok()?;
    let s = &pcm.samples;
    if s.is_empty() {
        return Some(vec![0; BINS]);
    }
    let bins: Vec<f32> = (0..BINS)
        .map(|i| {
            let a = i * s.len() / BINS;
            let b = ((i + 1) * s.len() / BINS).max(a + 1).min(s.len());
            s[a.min(s.len() - 1)..b].iter().fold(0f32, |m, x| m.max(x.abs()))
        })
        .collect();
    let top = bins.iter().fold(0f32, |m, x| m.max(*x));
    if top <= 1e-6 {
        return Some(vec![0; BINS]);
    }
    Some(bins.iter().map(|x| ((x / top).sqrt() * 255.0).round() as u8).collect())
}

/// What is known of `paths`: the sample's size and date (None: not a sample of the base, or too
/// long to draw) and
/// its shape kept (None: not yet, or the file changed since).
pub fn known(con: &Connection, paths: &[String]) -> rusqlite::Result<Vec<Option<((i64, f64), Option<Vec<u8>>)>>> {
    let mut q = con.prepare(
        "select s.size, s.mtime, case when w.size = s.size and w.mtime = s.mtime then w.w end \
         from samples s left join sample_wave w on w.path = s.path          where s.path = ? and coalesce(s.dur, 0) <= ?",
    )?;
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let r = q
            .query_row(params![p, MAX_DUR], |r| Ok(((r.get::<_, Option<i64>>(0)?.unwrap_or(0), r.get::<_, Option<f64>>(1)?.unwrap_or(0.0)), r.get::<_, Option<Vec<u8>>>(2)?)))
            .ok();
        out.push(r);
    }
    Ok(out)
}

/// The shapes of the samples still without one, decoded side by side.
pub fn compute(todo: Vec<(String, (i64, f64))>) -> Vec<(String, (i64, f64), Vec<u8>)> {
    todo.into_par_iter().filter_map(|(p, k)| shape(Path::new(&p)).map(|w| (p, k, w))).collect()
}

pub fn keep(con: &Connection, done: &[(String, (i64, f64), Vec<u8>)]) -> rusqlite::Result<()> {
    let tx = con.unchecked_transaction()?;
    for (p, (size, mtime), w) in done {
        tx.execute("insert or replace into sample_wave(path, size, mtime, w) values(?, ?, ?, ?)", params![p, size, mtime, w])?;
    }
    tx.commit()
}

pub type Waves = HashMap<String, Vec<u8>>;

#[cfg(test)]
mod tests {
    #[test]
    fn a_missing_file_has_no_shape() {
        assert!(super::shape(std::path::Path::new("Z:/nothing/here.wav")).is_none());
    }
}

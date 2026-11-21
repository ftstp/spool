//! The MIDI clips written in the projects (als::midi_clips), each kept once by its content: the
//! same progression copied in ten versions of a song is one row of the MIDI page, which knows
//! every project holding it. A clip is a `midi` row named "clip:<hash>" (analysed like a MIDI
//! file, its notes kept as a small MIDI file in `clip_notes`); `clips` says where it is found.
//! A project is read for its clips again only when it changed (`clip_src`).
use crate::{Tick, background, say};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;
use store::Store;
use store::rusqlite::{self, params};

pub struct Found {
    pub hash: String,
    pub track: String,
    pub name: String,
    pub smf: Vec<u8>,
    pub drums: bool,
    pub info: midi::MidiInfo,
}

/// A project's clips worth keeping (two notes or more; a drum line on one single pitch, a
/// kick or a sidechain trigger, says little), once each.
pub fn read_project(p: &std::path::Path) -> Vec<Found> {
    let (tempo, clips) = als::midi_clips(p);
    let bpm = tempo.unwrap_or(120.0);
    let mut out: Vec<Found> = Vec::new();
    for c in clips {
        let pitches: std::collections::HashSet<u8> = c.notes.iter().map(|n| n.pitch).collect();
        if c.drums && pitches.len() < 2 {
            continue;
        }
        // a fragment (under half a bar, or two notes) is not an idea to come back to
        if c.len < 2.0 || c.notes.len() < 3 {
            continue;
        }
        let hash = als::clip_hash(&c);
        if out.iter().any(|f| f.hash == hash) {
            continue;
        }
        let notes: Vec<(u8, f64, f64, u8)> = c.notes.iter().map(|n| (n.pitch, n.start, n.len, n.vel)).collect();
        let label = if c.name.is_empty() { c.track.clone() } else { format!("{} / {}", c.track, c.name) };
        let smf = midi::write_smf(&label, &notes, c.len, bpm, c.drums);
        let mut info = midi::analyse(&smf, &format!("{}/{}", c.track, c.name));
        // the clip's own length (the analysis stops at the last note)
        info.bars = ((c.len / 4.0) * 4.0).round() as f32 / 4.0;
        out.push(Found { hash, track: c.track, name: c.name, smf, drums: c.drums, info });
    }
    out
}

/// Reads the clips of the projects new or changed since their last reading, and forgets
/// those of the projects gone. True when something changed.
pub fn sync(st: &Mutex<Store>, roots: &store::Roots, stopped: &dyn Fn() -> bool, tick: Tick) -> rusqlite::Result<bool> {
    let (todo, gone) = {
        let st = st.lock().unwrap();
        let mut s = st.con.prepare(
            "select f.rel, f.size, f.mtime from files f left join clip_src c on c.rel = f.rel \
             where f.rel like '%.als' and (c.rel is null or c.size is not f.size or c.mtime is not f.mtime)",
        )?;
        let todo: Vec<(String, Option<i64>, Option<f64>)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let mut s = st.con.prepare("select rel from clip_src where rel not in (select rel from files)")?;
        let gone: Vec<String> = s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        (todo, gone)
    };
    if todo.is_empty() && gone.is_empty() {
        return Ok(false);
    }
    {
        let st = st.lock().unwrap();
        let tx = st.con.unchecked_transaction()?;
        for g in &gone {
            tx.execute("delete from clips where rel=?", [g])?;
            tx.execute("delete from clip_src where rel=?", [g])?;
        }
        tx.commit()?;
    }
    let (stage, total) = ("Lecture des clips MIDI des projets", todo.len());
    say(tick, stage, 0, total, "");
    let count = std::sync::atomic::AtomicUsize::new(0);
    for batch in todo.chunks(64) {
        if stopped() {
            break;
        }
        let read: Vec<(&(String, Option<i64>, Option<f64>), Vec<Found>)> = background::run(|| {
            batch
                .par_iter()
                .map(|row| {
                    let found = roots.abs(&row.0).map(|p: PathBuf| read_project(&p)).unwrap_or_default();
                    let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    say(tick, stage, n, total, &row.0);
                    (row, found)
                })
                .collect()
        });
        let st = st.lock().unwrap();
        let _urgent = background::Urgent::now();
        let tx = st.con.unchecked_transaction()?;
        for ((rel, size, mtime), found) in &read {
            tx.execute("delete from clips where rel=?", [rel])?;
            for f in found {
                let label = format!("{} {} {}", f.track, f.name, rel.rsplit('/').next().unwrap_or(rel));
                let data = serde_json::to_string(&f.info).unwrap_or_default();
                tx.execute(
                    "insert into midi(rel, size, mtime, data, kind, mkey, bpm, notes, label) values(?, 0, 0, z(?), ?, ?, ?, ?, ?) \
                     on conflict(rel) do nothing",
                    params![format!("clip:{}", f.hash), data, f.info.kind, f.info.key, f.info.bpm, f.info.notes as i64, label],
                )?;
                tx.execute("insert into clip_notes(hash, smf, drums) values(?, ?, ?) on conflict(hash) do nothing", params![f.hash, f.smf, f.drums])?;
                tx.execute("insert into clips(hash, rel, track, name) values(?, ?, ?, ?)", params![f.hash, rel, f.track, f.name])?;
            }
            tx.execute(
                "insert into clip_src(rel, size, mtime) values(?, ?, ?) on conflict(rel) do update set size=excluded.size, mtime=excluded.mtime",
                params![rel, size, mtime],
            )?;
        }
        tx.commit()?;
    }
    // a clip no project holds any more goes
    let st = st.lock().unwrap();
    let tx = st.con.unchecked_transaction()?;
    tx.execute("delete from midi where rel like 'clip:%' and substr(rel, 6) not in (select hash from clips)", [])?;
    tx.execute("delete from clip_notes where hash not in (select hash from clips)", [])?;
    tx.commit()?;
    Ok(true)
}

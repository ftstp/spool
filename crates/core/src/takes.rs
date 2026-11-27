//! What was played on the MIDI keyboard, kept: every take (notes played with no pause of more
//! than a few seconds) is saved as it ends, kept 30 days unless kept for good, and can be
//! written as a MIDI file (to drag into Live, or put on a song's or an artist's board). It is
//! all on this computer, in the base; nothing is sent anywhere.
use serde::Serialize;
use store::rusqlite::{self, Connection, params};
use ts_rs::TS;

/// A take not kept for good goes after this many days.
pub const KEEP_DAYS: f64 = 30.0;

/// One note: pitch, start and length in seconds from the take's start, velocity.
pub type Note = (u8, f64, f64, u8);

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Take {
    #[ts(type = "number")]
    pub id: i64,
    /// When it started (seconds since 1970).
    pub at: f64,
    /// Seconds.
    pub dur: f64,
    #[ts(type = "number")]
    pub n: i64,
    /// The key its notes say ("Am"), when they say one.
    pub key: Option<String>,
    /// Kept for good under this name (a MIDI file was written).
    pub name: Option<String>,
    /// The MIDI file written for it, when kept.
    pub file: Option<String>,
    /// [pitch, start s, length s, velocity]
    #[ts(type = "Array<[number, number, number, number]>")]
    pub notes: Vec<Note>,
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// The take as a standard MIDI file at `bpm` (the seconds played become beats at that tempo).
pub fn smf(name: &str, notes: &[Note], bpm: f64) -> Vec<u8> {
    let b = bpm / 60.0;
    let n: Vec<(u8, f64, f64, u8)> = notes.iter().map(|&(p, s, l, v)| (p, s * b, (l * b).max(0.05), v.max(1))).collect();
    let len = n.iter().map(|x| x.1 + x.2).fold(0.0, f64::max).max(1.0);
    midi::write_smf(name, &n, (len / 4.0).ceil() * 4.0, bpm, false)
}

fn key_of(notes: &[Note]) -> Option<String> {
    if notes.len() < 6 {
        return None;
    }
    midi::analyse(&smf("take", notes, 120.0), "take").key
}

/// Saves a take that ended; takes past their days go at the same time. Returns its id.
pub fn save(con: &Connection, at: f64, notes: &[Note]) -> rusqlite::Result<i64> {
    let dur = notes.iter().map(|n| n.1 + n.2).fold(0.0, f64::max);
    con.execute(
        "insert into midi_takes(at, dur, n, key, notes) values(?, ?, ?, ?, ?)",
        params![at, dur, notes.len() as i64, key_of(notes), serde_json::to_string(notes).unwrap_or_default()],
    )?;
    let id = con.last_insert_rowid();
    con.execute("delete from midi_takes where name is null and at < ?", [now() - KEEP_DAYS * 86_400.0])?;
    Ok(id)
}

/// Every take, the newest first.
pub fn list(con: &Connection) -> rusqlite::Result<Vec<Take>> {
    let mut q = con.prepare("select id, at, dur, n, key, name, file, notes from midi_takes order by at desc limit 500")?;
    q.query_map([], |r| {
        Ok(Take {
            id: r.get(0)?,
            at: r.get(1)?,
            dur: r.get(2)?,
            n: r.get(3)?,
            key: r.get(4)?,
            name: r.get(5)?,
            file: r.get(6)?,
            notes: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
        })
    })?
    .collect()
}

pub fn get(con: &Connection, id: i64) -> rusqlite::Result<Take> {
    list(con)?.into_iter().find(|t| t.id == id).ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn delete(con: &Connection, id: i64) -> rusqlite::Result<()> {
    con.execute("delete from midi_takes where id = ?", [id])?;
    Ok(())
}

/// A file name without what Windows refuses.
pub fn clean_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('.').trim().to_string();
    if s.is_empty() { "Prise".into() } else { s.chars().take(80).collect() }
}

/// Kept for good: written as `<dir>/<name>.mid` (a free name: never over another file) at
/// `bpm`, and no longer let go after 30 days. Returns the file's path.
pub fn keep(con: &Connection, dir: &std::path::Path, id: i64, name: &str, bpm: f64) -> Result<String, String> {
    let t = get(con, id).map_err(|e| e.to_string())?;
    let name = clean_name(name);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = (1..1000)
        .map(|i| dir.join(if i == 1 { format!("{name}.mid") } else { format!("{name} {i}.mid") }))
        .find(|p| !p.exists())
        .ok_or_else(|| "no free name".to_string())?;
    std::fs::write(&path, smf(&name, &t.notes, bpm)).map_err(|e| e.to_string())?;
    let p = path.to_string_lossy().into_owned();
    con.execute("update midi_takes set name = ?, file = ? where id = ?", params![name, p, id]).map_err(|e| e.to_string())?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_is_kept_listed_and_written() {
        let st = store::Store::memory().unwrap();
        let notes: Vec<Note> = [57u8, 60, 64, 69, 72, 76, 57, 60].iter().enumerate().map(|(i, p)| (*p, i as f64 * 0.25, 0.2, 90)).collect();
        let id = save(&st.con, now(), &notes).unwrap();
        // an old one, not kept: gone at the next save
        st.con.execute("insert into midi_takes(at, dur, n, notes) values(?, 1, 1, '[]')", [now() - 40.0 * 86_400.0]).unwrap();
        save(&st.con, now(), &notes[..2]).unwrap();
        let l = list(&st.con).unwrap();
        assert_eq!(l.len(), 2);
        let t = l.iter().find(|t| t.id == id).unwrap();
        assert_eq!((t.n, t.notes.len()), (8, 8));
        assert!((t.dur - 1.95).abs() < 1e-9);
        let dir = std::env::temp_dir().join(format!("pf-takes-{}", std::process::id()));
        let p = keep(&st.con, &dir, id, "Idée: piano?", 120.0).unwrap();
        assert!(p.ends_with("Idée piano.mid"));
        let p2 = keep(&st.con, &dir, id, "Idée: piano?", 120.0).unwrap();
        assert!(p2.ends_with("Idée piano 2.mid"));
        let info = midi::analyse(&std::fs::read(&p).unwrap(), "x");
        assert_eq!(info.notes, 8);
        std::fs::remove_dir_all(&dir).unwrap();
        // kept: never let go
        st.con.execute("update midi_takes set at = 0 where id = ?", [id]).unwrap();
        save(&st.con, now(), &notes[..2]).unwrap();
        assert!(list(&st.con).unwrap().iter().any(|t| t.id == id));
    }
}

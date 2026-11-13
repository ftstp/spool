//! Files -> songs in the base: song ids survive moves and renames, the fields the user
//! edited are never overwritten, merges and forced files are honoured.
use crate::setting;
use catalog::{Derived, FileIn, Rules, group as group_files};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use store::rusqlite::{self, OptionalExtension, Transaction, params};

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

struct SongRow {
    id: i64,
    song_dir: String,
    vkey: String,
    user_fields: HashSet<String>,
}

fn song_row(tx: &Transaction, where_: &str, p: impl rusqlite::Params) -> rusqlite::Result<Option<SongRow>> {
    tx.query_row(&format!("select id, song_dir, vkey, user_fields from songs where {where_}"), p, |r| {
        let uf: Option<String> = r.get(3)?;
        Ok(SongRow {
            id: r.get(0)?,
            song_dir: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            vkey: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            user_fields: serde_json::from_str(uf.as_deref().unwrap_or("[]")).unwrap_or_default(),
        })
    })
    .optional()
}

/// Groups every file into songs and writes it down. Returns the number of live songs.
pub fn group(tx: &Transaction) -> rusqlite::Result<usize> {
    let roots = store::Roots::load(tx);
    let artists: Vec<String> = setting(tx, "artists").unwrap_or_default();
    let words: Vec<String> = setting(tx, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    let forced: HashMap<String, i64> = setting(tx, "file_song").unwrap_or_default();
    let merges: HashMap<String, i64> = setting(tx, "merges").unwrap_or_default();
    let files: Vec<FileIn> = {
        let mut s = tx.prepare("select rel, size, mtime from files order by rowid")?;
        s.query_map([], |r| Ok(FileIn { rel: r.get(0)?, size: r.get(1)?, mtime: r.get(2)? }))?
            .collect::<Result<_, _>>()?
    };
    let live_ids: HashMap<i64, (String, String)> = {
        let mut s = tx.prepare("select id, song_dir, vkey from songs where gone = 0")?;
        s.query_map([], |r| {
            Ok((
                r.get(0)?,
                (
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ),
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let merge_targets: HashMap<String, Option<(String, String)>> =
        merges.iter().map(|(k, id)| (k.clone(), live_ids.get(id).cloned())).collect();
    let g = group_files(&files, &rules, &merge_targets);
    let groups: HashSet<(String, String)> = g.songs.iter().map(|s| (s.song_dir.clone(), s.vkey.clone())).collect();

    let file_song: HashMap<String, i64> = {
        let mut s = tx.prepare("select rel, song_id from files where song_id is not null")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?
    };
    // (name, size, rounded mtime) -> previous song id, to follow a moved / renamed folder
    let mut sig: HashMap<(String, i64, i64), i64> = HashMap::new();
    {
        let mut s = tx.prepare("select rel, size, mtime, song_id from files")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })?;
        for row in rows {
            let (rel, size, mtime, sid) = row?;
            if let Some(sid) = sid {
                let name = rel.rsplit('/').next().unwrap_or("").to_lowercase();
                sig.insert((name, size.unwrap_or(0), mtime.unwrap_or(0.0).round_ties_even() as i64), sid);
            }
        }
    }
    let mut files_of: HashMap<(String, String), Vec<&catalog::Placed>> = HashMap::new();
    for f in &g.files {
        files_of.entry((f.song_dir.clone(), f.vkey.clone())).or_default().push(f);
    }
    let size_of: HashMap<&str, (i64, f64)> = files.iter().map(|f| (f.rel.as_str(), (f.size, f.mtime))).collect();

    let t = now();
    let mut seen: Vec<i64> = Vec::new();
    let mut seen_set: HashSet<i64> = HashSet::new();
    let mut file_rows: Vec<(i64, &catalog::Placed)> = Vec::new();
    for s in &g.songs {
        let key = (s.song_dir.clone(), s.vkey.clone());
        let fs = files_of.get(&key).cloned().unwrap_or_default();
        if s.merged_into.is_some() {
            let tid = merges[&format!("{}|{}", s.song_dir, s.vkey)];
            file_rows.extend(fs.iter().map(|f| (tid, *f)));
            continue;
        }
        let mut song = song_row(tx, "song_dir=? and vkey=? and gone=0", params![s.song_dir, s.vkey])?;
        if song.is_none() {
            // moved / renamed folder: a file of this group belonged to a song before
            let prev: Vec<i64> = fs
                .iter()
                .filter_map(|f| {
                    file_song.get(&f.rel).copied().or_else(|| {
                        let (size, mtime) = size_of[f.rel.as_str()];
                        let name = f.rel.rsplit('/').next().unwrap_or("").to_lowercase();
                        sig.get(&(name, size, mtime.round_ties_even() as i64)).copied()
                    })
                })
                .filter(|p| !seen_set.contains(p))
                .collect();
            let cand = match prev.first() {
                Some(p) => song_row(tx, "id=?", [p])?,
                None => None,
            };
            song = match cand {
                Some(c) if !groups.contains(&(c.song_dir.clone(), c.vkey.clone())) => {
                    tx.execute(
                        "update songs set song_dir=?, vkey=?, gone=0, updated=? where id=?",
                        params![s.song_dir, s.vkey, t, c.id],
                    )?;
                    Some(c)
                }
                _ => {
                    tx.execute(
                        "insert into songs(song_dir, vkey, created, updated) values(?,?,?,?)",
                        params![s.song_dir, s.vkey, t, t],
                    )?;
                    Some(SongRow {
                        id: tx.last_insert_rowid(),
                        song_dir: s.song_dir.clone(),
                        vkey: s.vkey.clone(),
                        user_fields: HashSet::new(),
                    })
                }
            };
        }
        let song = song.unwrap();
        if seen_set.insert(song.id) {
            seen.push(song.id);
        }
        derive(tx, &song, s, &roots)?;
        for f in fs {
            file_rows.push((forced.get(&f.rel).copied().unwrap_or(song.id), f));
        }
    }
    {
        let mut up = tx.prepare(
            "update files set song_id=?, song_dir=?, vkey=?, in_version=?, promoted=?, archived=? where rel=?",
        )?;
        for (sid, f) in &file_rows {
            up.execute(params![sid, f.song_dir, f.vkey, f.in_version as i64, f.promoted as i64, 0, f.rel])?;
        }
    }
    // A song whose files now belong to another song was merged into it (better grouping
    // rules): hand over what the user gave it, where the absorber has nothing.
    let new_owner: HashMap<&str, i64> = file_rows.iter().map(|(sid, f)| (f.rel.as_str(), *sid)).collect();
    let alive: HashSet<i64> = seen.iter().copied().collect();
    let orphans: Vec<i64> = {
        let mut s = tx.prepare("select id from songs where gone=0")?;
        s.query_map([], |r| r.get(0))?
            .collect::<Result<Vec<i64>, _>>()?
            .into_iter()
            .filter(|id| !alive.contains(id))
            .collect()
    };
    for old in &orphans {
        let owners: HashSet<i64> = file_song
            .iter()
            .filter(|(_, sid)| *sid == old)
            .filter_map(|(rel, _)| new_owner.get(rel.as_str()).copied())
            .collect();
        if owners.len() != 1 {
            continue;
        }
        let target = *owners.iter().next().unwrap();
        for (col, source) in [("notes", None), ("stage", Some("stage_source")), ("checklist", None), ("tempo", None)] {
            let sel =
                format!("select {col}{} from songs where id=?", source.map(|s| format!(", {s}")).unwrap_or_default());
            let get = |id: i64| -> rusqlite::Result<(Value, Value)> {
                tx.query_row(&sel, [id], |r| {
                    let v: rusqlite::types::Value = r.get(0)?;
                    let src: Option<rusqlite::types::Value> = if source.is_some() { Some(r.get(1)?) } else { None };
                    Ok((to_json(v), src.map(to_json).unwrap_or(Value::Null)))
                })
            };
            let ((ov, osrc), (tv, _)) = (get(*old)?, get(target)?);
            if truthy(&ov) && !truthy(&tv) {
                match source {
                    Some(sc) => tx.execute(
                        &format!("update songs set {col}=?, {sc}=? where id=?"),
                        params![sql(&ov), sql(&osrc), target],
                    )?,
                    None => tx.execute(&format!("update songs set {col}=? where id=?"), params![sql(&ov), target])?,
                };
            }
        }
    }
    for old in &orphans {
        tx.execute("update songs set gone=1 where id=?", [old])?;
    }
    Ok(seen.len())
}

fn to_json(v: rusqlite::types::Value) -> Value {
    use rusqlite::types::Value as V;
    match v {
        V::Null => Value::Null,
        V::Integer(i) => Value::from(i),
        V::Real(f) => Value::from(f),
        V::Text(t) => Value::String(t),
        V::Blob(_) => Value::Null,
    }
}

fn sql(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as V;
    match v {
        Value::Null => V::Null,
        Value::Number(n) => n.as_i64().map(V::Integer).unwrap_or_else(|| V::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => V::Text(s.clone()),
        other => V::Text(other.to_string()),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// Scan-derived fields; anything the user edited is left alone.
fn derive(tx: &Transaction, song: &SongRow, d: &Derived, roots: &store::Roots) -> rusqlite::Result<()> {
    // no artist in the folder names: the projects folder's own artist, when set
    let folder_artist = d.artist.clone().or_else(|| {
        roots.split(&d.song_dir).map(|(r, _)| r.artist.clone()).filter(|a| !a.trim().is_empty())
    });
    let vals: [(&str, &Option<String>); 7] = [
        ("title", &d.title),
        ("artist", &folder_artist),
        ("style", &d.style),
        ("key", &d.key),
        ("catalog", &d.catalog),
        ("ft", &d.ft),
        ("folder_date", &d.folder_date),
    ];
    let todo: Vec<&(&str, &Option<String>)> = vals.iter().filter(|(c, _)| !song.user_fields.contains(*c)).collect();
    let sets: Vec<String> = todo.iter().map(|(c, _)| format!("{c}=?")).collect();
    let mut args: Vec<rusqlite::types::Value> = todo
        .iter()
        .map(|(_, v)| {
            v.as_ref().map(|s| rusqlite::types::Value::Text(s.clone())).unwrap_or(rusqlite::types::Value::Null)
        })
        .collect();
    args.push(rusqlite::types::Value::Integer(song.id));
    let sql = if sets.is_empty() {
        "update songs set gone=0 where id=?".to_string()
    } else {
        format!("update songs set {}, gone=0 where id=?", sets.join(", "))
    };
    tx.execute(&sql, rusqlite::params_from_iter(args))?;
    Ok(())
}

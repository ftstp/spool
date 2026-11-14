//! After a pass: every song's reference render, card picture and finishing checks, the stage it seems
//! to be at, and a style for the songs without one. The app proposes, the user decides: a
//! refused suggestion comes back only when its evidence changes.
use crate::setting;
use catalog::{
    Audio, CheckInput, Newest, Rules, StyleData, StyleSong, auto_checks, best_stage, card_picture, deduce, Board, list_board_labeled,
    other_song_prefixes, signature, song_dir_known, stage_evidence,
};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use store::rusqlite::{self, OptionalExtension, Transaction, params};

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

struct Song {
    id: i64,
    song_dir: String,
    vkey: String,
    title: Option<String>,
    artist: Option<String>,
    stage: Option<String>,
    temoin_source: Option<String>,
    archived: bool,
}

struct FileRow {
    mtime: f64,
    in_version: bool,
    archived: bool,
    sev: i64,
    data: Value,
}

/// What one song gets from the pass, computed in parallel, written after.
struct Out {
    id: i64,
    temoin: Option<Option<String>>,
    checkauto: String,
    cover: Option<String>,
    proposal: Option<(String, String, Vec<String>)>,
}

pub fn refresh_all(tx: &Transaction, roots: &store::Roots) -> rusqlite::Result<()> {
    let rules = refresh(tx, roots, None)?;
    refresh_styles(tx, &rules)
}

/// One song only, archived or not (after a stage change, a check or a board edit): reference render,
/// checks, card picture, stage suggestion. Styles are left alone.
pub fn refresh_song(tx: &Transaction, roots: &store::Roots, sid: i64) -> rusqlite::Result<()> {
    refresh(tx, roots, Some(sid)).map(|_| ())
}

fn refresh(tx: &Transaction, roots: &store::Roots, only: Option<i64>) -> rusqlite::Result<Rules> {
    let artists: Vec<String> = setting(tx, "artists").unwrap_or_default();
    let words: Vec<String> = setting(tx, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    // audio by the song folder it sits in; "- MP3" renders are offered to every song
    let mut by_dir: HashMap<String, Vec<(Audio, String)>> = HashMap::new();
    let mut mp3: Vec<(Audio, String)> = Vec::new();
    let known: HashSet<String> = {
        let mut s = tx.prepare("select distinct song_dir from songs where gone=0 and song_dir is not null")?;
        s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    {
        let mut s = tx.prepare("select rel, size, mtime from audio")?;
        let rows = s.query_map([], |r| Ok(Audio { rel: r.get(0)?, size: r.get(1)?, mtime: r.get(2)? }))?;
        for a in rows {
            let a = a?;
            if a.rel.starts_with("- MP3/") {
                mp3.push((a, String::new()));
            } else {
                let d = song_dir_known(&a.rel, &rules, &known);
                by_dir.entry(d.clone()).or_default().push((a, d));
            }
        }
    }
    // every song's project files (names only): which sub-folder of a shared folder is whose
    let mut rels_of: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut s = tx.prepare("select song_id, rel from files where song_id is not null")?;
        for r in s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (sid, rel) = r?;
            rels_of.entry(sid).or_default().push(rel);
        }
    }
    let mut files: HashMap<i64, Vec<FileRow>> = HashMap::new();
    {
        // one song: only its own files are read (the whole table is ~40 MB of JSON)
        let mut s = tx.prepare(
            "select song_id, mtime, in_version, archived, sev, data from files where song_id is not null              and (?1 is null or song_id = ?1)",
        )?;
        let rows = s.query_map([only], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                FileRow {
                    mtime: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    in_version: r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
                    archived: r.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                    sev: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    data: serde_json::from_str(&r.get::<_, Option<String>>(5)?.unwrap_or_default())
                        .unwrap_or(Value::Null),
                },
            ))
        })?;
        for row in rows {
            let (sid, f) = row?;
            files.entry(sid).or_default().push(f);
        }
    }
    let songs: Vec<Song> = {
        let mut s = tx.prepare(
            "select id, song_dir, vkey, title, artist, stage, temoin_source, archived from songs where gone=0",
        )?;
        s.query_map([], |r| {
            Ok(Song {
                id: r.get(0)?,
                song_dir: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                vkey: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                title: r.get(3)?,
                artist: r.get(4)?,
                stage: r.get(5)?,
                temoin_source: r.get(6)?,
                archived: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
            })
        })?
        .collect::<Result<_, _>>()?
    };
    // a whole pass covers the live songs; one song is refreshed even when archived, against
    // the folders of every song still there (as the prototype's refresh_one)
    let songs: Vec<Song> = if only.is_some() { songs } else { songs.into_iter().filter(|s| !s.archived).collect() };
    let mut per_dir: HashMap<&str, usize> = HashMap::new();
    for s in &songs {
        *per_dir.entry(s.song_dir.as_str()).or_default() += 1;
    }
    let mut dirs: Vec<String> = Vec::new();
    for s in &songs {
        if !dirs.contains(&s.song_dir) {
            dirs.push(s.song_dir.clone());
        }
    }
    let empty: Vec<FileRow> = Vec::new();
    // per shared folder, every song's own sub-folders
    let mut own_of: HashMap<i64, Vec<String>> = HashMap::new();
    let mut all_songs: Vec<(i64, String)> = Vec::new();
    {
        let mut st = tx.prepare("select id, coalesce(song_dir, '') from songs where gone=0")?;
        for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            all_songs.push(r?);
        }
    }
    let mut by_folder: HashMap<String, Vec<i64>> = HashMap::new();
    for (id, d) in &all_songs {
        by_folder.entry(d.clone()).or_default().push(*id);
        let rels: Vec<&str> = rels_of.get(id).map(|v| v.iter().map(String::as_str).collect()).unwrap_or_default();
        own_of.insert(*id, catalog::own_dirs(d, &rels));
    }
    let none: Vec<String> = Vec::new();
    let outs: Vec<Out> = songs
        .par_iter()
        .filter(|s| only.is_none_or(|id| s.id == id))
        .map(|s| {
            let fs = files.get(&s.id).unwrap_or(&empty);
            let active: Vec<&FileRow> = fs.iter().filter(|f| !f.in_version && !f.archived).collect();
            let newest_mtime = active.iter().map(|f| f.mtime).fold(0.0f64, f64::max);
            let newest = active
                .iter()
                .copied()
                .fold(None::<&FileRow>, |b, f| if b.is_none_or(|b| f.mtime > b.mtime) { Some(f) } else { b });
            let shared = per_dir[s.song_dir.as_str()] > 1;
            let mut audio = by_dir.get(&s.song_dir).cloned().unwrap_or_default();
            audio.extend(mp3.iter().cloned());
            let own = own_of.get(&s.id).unwrap_or(&none);
            let foreign: Vec<String> = by_folder
                .get(&s.song_dir)
                .into_iter()
                .flatten()
                .filter(|o| **o != s.id)
                .flat_map(|o| own_of.get(o).cloned().unwrap_or_default())
                .filter(|d| !own.contains(d))
                .collect();
            let cands = catalog::temoin_candidates_owned(
                &rules,
                &s.song_dir,
                &s.vkey,
                s.title.as_deref().unwrap_or(""),
                &audio,
                newest_mtime,
                shared,
                own,
                &foreign,
            );
            let temoin = (s.temoin_source.as_deref() != Some("user"))
                .then(|| cands.iter().find(|c| c.pickable).map(|c| c.rel.clone()));
            let others = other_song_prefixes(&dirs, &s.song_dir);
            let b = match roots.split(&s.song_dir) {
                Some((r, _)) => list_board_labeled(
                    Path::new(&r.path),
                    &r.label,
                    &rules,
                    &s.song_dir,
                    &s.vkey,
                    s.title.as_deref().unwrap_or(""),
                    shared,
                    &others,
                ),
                None => Board::default(),
            };
            let sev = active.iter().map(|f| f.sev).max();
            let length = newest.and_then(|f| f.data["length_s"].as_i64());
            let checks = auto_checks(
                &CheckInput {
                    stage: s.stage.as_deref(),
                    title: s.title.as_deref(),
                    artist: s.artist.as_deref(),
                    temoins: cands.iter().map(|c| (c.rel.as_str(), c.kind.as_str(), c.mtime)).collect(),
                    newest_mtime,
                    sev,
                    length_s: length,
                },
                &b,
            );
            let checkauto: serde_json::Map<String, Value> =
                checks.into_iter().map(|(k, d, w)| (k.to_string(), json!([d, w]))).collect();
            let nd: Option<Newest> = newest.map(|f| serde_json::from_value(f.data.clone()).unwrap_or_default());
            let reasons = stage_evidence(&s.song_dir, nd.as_ref(), &cands);
            let proposal = best_stage(&reasons).filter(|b| Some(*b) != s.stage.as_deref()).map(|b| {
                let texts = reasons.iter().filter(|r| r.stage == b).map(|r| r.text.clone()).collect();
                (b.to_string(), signature(b, &reasons), texts)
            });
            Out { id: s.id, temoin, checkauto: Value::Object(checkauto).to_string(), cover: card_picture(&b), proposal }
        })
        .collect();

    let t = now();
    let stage_of: HashMap<i64, Option<&str>> = songs.iter().map(|s| (s.id, s.stage.as_deref())).collect();
    for o in &outs {
        if let Some(tm) = &o.temoin {
            tx.execute(
                "update songs set temoin=?, temoin_source=? where id=?",
                params![tm, tm.as_ref().map(|_| "auto"), o.id],
            )?;
        }
        tx.execute("update songs set checkauto=?, cover=? where id=?", params![o.checkauto, o.cover, o.id])?;
        let mut keep: HashSet<i64> = HashSet::new();
        if let Some((value, sig, texts)) = &o.proposal {
            let prev: Option<(i64, String)> = tx
                .query_row(
                    "select id, state from suggestions where song_id=? and sig=? and kind='stage' order by id desc limit 1",
                    params![o.id, sig],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            match prev {
                Some((id, state)) if state == "open" || state == "refused" => {
                    keep.insert(id);
                }
                Some((_, state)) if state == "accepted" && stage_of[&o.id] == Some(value.as_str()) => {}
                _ => {
                    tx.execute(
                        "insert into suggestions(song_id, kind, value, reasons, sig, created) values(?,?,?,?,?,?)",
                        params![o.id, "stage", value, serde_json::to_string(texts).unwrap(), sig, t],
                    )?;
                    keep.insert(tx.last_insert_rowid());
                }
            }
        }
        // open suggestions whose evidence is gone are withdrawn
        let open: Vec<i64> = {
            let mut s = tx.prepare("select id from suggestions where song_id=? and state='open' and kind='stage'")?;
            s.query_map([o.id], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for id in open.into_iter().filter(|i| !keep.contains(i)) {
            tx.execute("update suggestions set state='stale' where id=?", [id])?;
        }
    }
    Ok(rules)
}

/// What the app would say of a song's style, even when one is noted: the song is read as if
/// it had none and compared to the others (its own style never votes for itself).
pub fn style_guess(con: &rusqlite::Connection, rules: &Rules, sid: i64) -> rusqlite::Result<Vec<catalog::StyleOption>> {
    let songs: Vec<StyleSong> = style_songs(con)?
        .into_iter()
        .filter(|s| s.id == sid || s.style.as_deref().is_some_and(|x| !x.is_empty()))
        .map(|mut s| {
            if s.id == sid {
                s.style = None;
            }
            s
        })
        .collect();
    Ok(deduce(&songs, rules).into_iter().find(|(id, _)| *id == sid).map(|(_, o)| o).unwrap_or_default())
}

/// Style options for the songs without a style (see catalog::deduce).
pub fn refresh_styles(tx: &Transaction, rules: &Rules) -> rusqlite::Result<()> {
    let songs = style_songs(tx)?;
    let t = now();
    let mut keep: HashSet<i64> = HashSet::new();
    for (sid, opts) in deduce(&songs, rules) {
        let sig = format!("style|{}", opts.iter().map(|o| o.style.as_str()).collect::<Vec<_>>().join(","));
        let reasons = serde_json::to_string(&opts).unwrap();
        let prev: Option<(i64, String)> = tx
            .query_row(
                "select id, state from suggestions where song_id=? and kind='style' and sig=? order by id desc limit 1",
                params![sid, sig],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match prev {
            Some((id, state)) if state == "open" || state == "refused" => {
                if state == "open" {
                    // same styles, fresher percentages and reasons
                    tx.execute(
                        "update suggestions set value=?, reasons=? where id=?",
                        params![opts[0].style, reasons, id],
                    )?;
                }
                keep.insert(id);
            }
            _ => {
                tx.execute(
                    "insert into suggestions(song_id, kind, value, reasons, sig, created) values(?,?,?,?,?,?)",
                    params![sid, "style", opts[0].style, reasons, sig, t],
                )?;
                keep.insert(tx.last_insert_rowid());
            }
        }
    }
    let open: Vec<i64> = {
        let mut s = tx.prepare("select id from suggestions where kind='style' and state='open'")?;
        s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for id in open.into_iter().filter(|i| !keep.contains(i)) {
        tx.execute("update suggestions set state='stale' where id=?", [id])?;
    }
    Ok(())
}

/// Every song with its newest active project, its file and audio names.
/// What the projects alone say of every song's style: no style the user wrote counts, no
/// audio. For measuring the detection (tools), not shown in the app.
pub fn blind_styles(con: &rusqlite::Connection, rules: &Rules) -> rusqlite::Result<Vec<(i64, Vec<catalog::StyleOption>)>> {
    let songs: Vec<StyleSong> = style_songs(con)?
        .into_iter()
        .map(|mut s| {
            s.style = None;
            s.sound.clear();
            s
        })
        .collect();
    Ok(deduce(&songs, rules))
}

fn style_songs(tx: &rusqlite::Connection) -> rusqlite::Result<Vec<StyleSong>> {
    let mut files: HashMap<i64, (Vec<String>, Option<String>)> = HashMap::new();
    {
        let mut s = tx.prepare(
            "select song_id, rel, data from files where song_id is not null and in_version=0 and archived=0 order by mtime",
        )?;
        let rows =
            s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?;
        for row in rows {
            let (sid, rel, data) = row?;
            let e = files.entry(sid).or_default();
            let name = rel.rsplit('/').next().unwrap_or("");
            e.0.push(name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).to_string());
            e.1 = data; // ordered by mtime: the last one is the newest
        }
    }
    let mut audio: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut s = tx.prepare("select rel from audio")?;
        for rel in s.query_map([], |r| r.get::<_, String>(0))? {
            let rel = rel?;
            let (d, name) = rel.rsplit_once('/').unwrap_or(("", rel.as_str()));
            audio
                .entry(d.to_string())
                .or_default()
                .push(name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).to_string());
        }
    }
    let rows: Vec<(i64, Option<String>, Option<String>, String)> = {
        let mut s = tx.prepare("select id, title, style, song_dir from songs where gone=0 and archived=0")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?.unwrap_or_default())))?
            .collect::<Result<_, _>>()?
    };
    let mut per_dir: HashMap<&str, usize> = HashMap::new();
    for r in &rows {
        *per_dir.entry(r.3.as_str()).or_default() += 1;
    }
    let sound = sound_neighbours(tx, &rows)?;
    let songs: Vec<StyleSong> = rows
        .iter()
        .filter_map(|(id, title, style, dir)| {
            let (stems, data) = files.get(id)?;
            let data: StyleData = data.as_deref().and_then(|d| serde_json::from_str(d).ok()).unwrap_or_default();
            Some(StyleSong {
                id: *id,
                title: title.clone(),
                style: style.clone(),
                dir: dir.clone(),
                stems: stems.clone(),
                audio: if dir.is_empty() { Vec::new() } else { audio.get(dir).cloned().unwrap_or_default() },
                shared: per_dir[dir.as_str()] > 1,
                data,
                sound: sound.get(id).cloned().unwrap_or_default(),
            })
        })
        .collect();
    Ok(songs)
}

/// For every song whose reference render was analysed (audio analysis, opt-in, current
/// model): the other analysed songs, closest render first, those of the same folder or with
/// the same render left out (other versions of the same idea would vote for themselves).
fn sound_neighbours(
    con: &rusqlite::Connection,
    rows: &[(i64, Option<String>, Option<String>, String)],
) -> rusqlite::Result<HashMap<i64, Vec<(i64, f64)>>> {
    let mut emb: Vec<(i64, &str, String, Vec<f32>)> = Vec::new();
    {
        let dirs: HashMap<i64, &str> = rows.iter().map(|r| (r.0, r.3.as_str())).collect();
        let mut s = con.prepare(
            "select s.id, a.embedding, a.rel from songs s join audio_analysis a on a.rel = s.temoin \
             where s.gone = 0 and a.model = ? and a.embedding is not null",
        )?;
        let rows = s.query_map([crate::AUDIO_MODEL], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, String>(2)?))
        })?;
        for r in rows {
            let (id, blob, rel) = r?;
            if let Some(d) = dirs.get(&id) {
                let v: Vec<f32> = blob.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                emb.push((id, d, rel, v));
            }
        }
    }
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum::<f64>();
    let mut out = HashMap::new();
    for (id, dir, rel, v) in &emb {
        let mut near: Vec<(i64, f64)> = emb
            .iter()
            .filter(|(o, d, r, _)| o != id && r != rel && (dir.is_empty() || d != dir))
            .map(|(o, _, _, w)| (*o, dot(v, w)))
            .collect();
        near.sort_by(|a, b| b.1.total_cmp(&a.1));
        near.truncate(20);
        out.insert(*id, near);
    }
    Ok(out)
}

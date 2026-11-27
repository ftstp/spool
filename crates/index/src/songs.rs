//! Files -> songs in the base: song ids survive moves and renames, the fields the user
//! edited are never overwritten, merges and forced files are honoured.
use crate::setting;
use catalog::{Derived, FileIn, OneSong, Rules, group_opts};
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

/// The folders read as one song (settings "group_mode" = "folder": every song folder;
/// "one_song_dirs": the ones listed).
pub fn one_song_opts(tx: &Transaction) -> OneSong {
    OneSong {
        all: setting::<String>(tx, "group_mode").as_deref() == Some("folder"),
        dirs: setting::<Vec<String>>(tx, "one_song_dirs").unwrap_or_default().into_iter().collect(),
        title_from_folder: match setting::<String>(tx, "title_from").as_deref() {
            Some("folder") => Some(true),
            Some("file") => Some(false),
            _ => None,
        },
    }
}

/// What the grouping reads: the project files, the naming rules, the merges, the files made
/// songs of their own. (`group` writes its result; the reading-rules page only looks at it.)
pub struct Inputs {
    pub files: Vec<FileIn>,
    pub rules: Rules,
    pub merges: HashMap<String, Option<(String, String)>>,
    pub own: HashSet<String>,
}

pub fn inputs(tx: &Transaction) -> rusqlite::Result<Inputs> {
    let mut artists: Vec<String> = setting(tx, "artists").unwrap_or_default();
    let aliases: HashMap<String, String> = setting::<HashMap<String, String>>(tx, "artist_aliases").unwrap_or_default();
    artists.extend(aliases.keys().cloned());
    artists.extend(setting::<HashMap<String, String>>(tx, "artist_feats").unwrap_or_default().into_keys());
    let words: Vec<String> = setting(tx, "work_words").unwrap_or_default();
    let merges: HashMap<String, i64> = setting(tx, "merges").unwrap_or_default();
    let files: Vec<FileIn> = {
        let mut s = tx.prepare("select rel, size, mtime from files order by rowid")?;
        s.query_map([], |r| Ok(FileIn { rel: r.get(0)?, size: r.get(1)?, mtime: r.get(2)? }))?.collect::<Result<_, _>>()?
    };
    let live_ids: HashMap<i64, (String, String)> = {
        let mut s = tx.prepare("select id, song_dir, vkey from songs where gone = 0")?;
        s.query_map([], |r| Ok((r.get(0)?, (r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, Option<String>>(2)?.unwrap_or_default()))))?
            .collect::<Result<_, _>>()?
    };
    Ok(Inputs {
        files,
        rules: Rules::new(&artists, &words),
        merges: merges.iter().map(|(k, id)| (k.clone(), live_ids.get(id).cloned())).collect(),
        own: setting::<Vec<String>>(tx, "own_files").unwrap_or_default().into_iter().collect(),
    })
}

/// Groups every file into songs and writes it down. Returns the number of live songs.
pub fn group(tx: &Transaction) -> rusqlite::Result<usize> {
    let roots = store::Roots::load(tx);
    let mut artists: Vec<String> = setting(tx, "artists").unwrap_or_default();
    // merged artists: a folder naming the old name is still read as an artist, then shown
    // under the name it was merged into
    let aliases: HashMap<String, String> = setting::<HashMap<String, String>>(tx, "artist_aliases").unwrap_or_default();
    artists.extend(aliases.keys().cloned());
    let aliases: HashMap<String, String> = aliases.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
    // featured artists: a folder naming one is still read (the name is not taken for the
    // title), then the song goes to the main artist with the name in its featurings
    let feats: HashMap<String, String> = setting::<HashMap<String, String>>(tx, "artist_feats").unwrap_or_default();
    artists.extend(feats.keys().cloned());
    let feats: HashMap<String, (String, String)> = feats.into_iter().map(|(k, v)| (k.to_lowercase(), (k, v))).collect();
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
    // files the user made songs of their own
    let own: HashSet<String> = setting::<Vec<String>>(tx, "own_files").unwrap_or_default().into_iter().collect();
    let g = group_opts(&files, &rules, &merge_targets, &own, &one_song_opts(tx));
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

    // the projects folders taken out of the list, to find a song that comes back through
    // another one (its parent folder added)
    let removed: Vec<store::Root> = setting(tx, "removed_roots").unwrap_or_default();
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
            // no file to follow (they were forgotten when their projects folder was taken out
            // of the list): the song that left from this very place on disk, when one did
            let cand = match cand {
                Some(c) => Some(c),
                None => match back_from(tx, &roots, &removed, &s.song_dir, &s.vkey, &seen_set)? {
                    Some(id) => song_row(tx, "id=?", [id])?,
                    None => None,
                },
            };
            song = match cand {
                // (never another group's song; one gone from this very place is this one)
                Some(c) if (c.song_dir == s.song_dir && c.vkey == s.vkey) || !groups.contains(&(c.song_dir.clone(), c.vkey.clone())) => {
                    tx.execute(
                        "update songs set song_dir=?, vkey=?, gone=0, updated=? where id=?",
                        params![s.song_dir, s.vkey, t, c.id],
                    )?;
                    // what names it by its place (its releases, its comments) follows it
                    rekey(tx, (&c.song_dir, &c.vkey), (&s.song_dir, &s.vkey))?;
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
        derive(tx, &song, s, &roots, &aliases, &feats)?;
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
    // once: the songs made again from scratch before the line above existed get back what
    // the user had given them
    // (how many, kept in the setting, for a diagnostic report)
    if setting::<Value>(tx, "reunited_v1").is_none() {
        let n = reunite(tx)?;
        crate::store_set(tx, "reunited_v1", &n)?;
    }
    Ok(seen.len())
}

/// A path compared with another: one separator, no case, no trailing separator.
fn norm_path(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// The song gone from the base that lived at the same place on disk as `dir` (same folder,
/// same project name): its projects folder was taken out of the list, then a folder holding
/// it was added. Only when exactly one fits.
fn back_from(
    tx: &Transaction,
    roots: &store::Roots,
    removed: &[store::Root],
    dir: &str,
    vkey: &str,
    taken: &HashSet<i64>,
) -> rusqlite::Result<Option<i64>> {
    let here = roots.abs(dir).map(|p| norm_path(&p));
    // where a name pointed, by the folder it was read in then
    let was = |old: &str| -> Option<String> {
        let first = old.split('/').next().unwrap_or("");
        let r = removed.iter().rev().find(|r| !r.label.is_empty() && r.label == first);
        let (path, inner) = match r {
            Some(r) => (r.path.as_str(), old.get(first.len() + 1..).unwrap_or("")),
            None => (removed.iter().rev().find(|r| r.label.is_empty())?.path.as_str(), old),
        };
        let p = std::path::Path::new(path);
        Some(norm_path(&if inner.is_empty() { p.to_path_buf() } else { p.join(inner) }))
    };
    let mut s = tx.prepare("select id, coalesce(song_dir, '') from songs where gone=1 and vkey=?")?;
    let fits: Vec<i64> = s
        .query_map([vkey], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(id, old)| !taken.contains(id) && (old == dir || (here.is_some() && was(old) == here)))
        .map(|(id, _)| id)
        .collect();
    Ok(if fits.len() == 1 { Some(fits[0]) } else { None })
}

/// What names a song by its place ("<folder>|<project>": its comments; its folder and
/// project: its releases' tracks) moved from one place to another.
fn rekey(tx: &Transaction, from: (&str, &str), to: (&str, &str)) -> rusqlite::Result<()> {
    if from == to {
        return Ok(());
    }
    if let Some(mut rels) = setting::<Value>(tx, "releases") {
        let mut changed = false;
        let mut move_ref = |t: &mut Value| {
            if t.is_object() && t["dir"].as_str() == Some(from.0) && t["vkey"].as_str() == Some(from.1) {
                t["dir"] = Value::from(to.0);
                t["vkey"] = Value::from(to.1);
                changed = true;
            }
        };
        for r in rels.as_array_mut().into_iter().flatten() {
            for t in r.get_mut("tracks").and_then(|t| t.as_array_mut()).into_iter().flatten() {
                move_ref(t);
            }
            if let Some(c) = r.get_mut("cover_song") {
                move_ref(c);
            }
        }
        if changed {
            crate::store_set(tx, "releases", &rels)?;
        }
    }
    if let Some(mut m) = setting::<serde_json::Map<String, Value>>(tx, "comments")
        && let Some(old) = m.remove(&format!("{}|{}", from.0, from.1))
    {
        let key = format!("{}|{}", to.0, to.1);
        let mut list = m.get(&key).and_then(|v| v.as_array().cloned()).unwrap_or_default();
        list.extend(old.as_array().cloned().unwrap_or_default());
        m.insert(key, Value::Array(list));
        crate::store_set(tx, "comments", &m)?;
    }
    Ok(())
}

/// The part of a song's folder name that stays when its projects folder is added again
/// another way: the whole name, and without its first part (the folder's label then). Two
/// folders deep at least: a bare "Song A" could be any song of that name.
fn tails(dir: &str) -> Vec<&str> {
    let mut v = vec![dir];
    if let Some((_, rest)) = dir.split_once('/') {
        v.push(rest);
    }
    v.into_iter().filter(|t| t.contains('/')).collect()
}

fn ends_at(dir: &str, tail: &str) -> bool {
    dir == tail || dir.ends_with(&format!("/{tail}"))
}

/// Before `back_from` existed, a song whose projects folder was taken out of the list and
/// then added again through its parent came back as a new, empty song, the old one gone with
/// the stars, styles, stages and notes given to it. Each such pair (same project name, the
/// new folder ending like the old one, one candidate each way) is put back together: the new
/// song stays and keeps everything set on it since; what it has not got, it takes from the
/// old one.
fn reunite(tx: &Transaction) -> rusqlite::Result<usize> {
    let rows = |sql: &str| -> rusqlite::Result<Vec<(i64, String, String)>> {
        let mut s = tx.prepare(sql)?;
        s.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, Option<String>>(2)?.unwrap_or_default())))?
            .collect()
    };
    // the old songs worth it: something given by the user
    let old = rows(
        "select id, song_dir, vkey from songs where gone=1 and (user_fields != '[]' or stage_source is not null \
         or temoin_source = 'user' or notes is not null or notion is not null or checklist is not null \
         or archived = 1 or rating is not null or (kinds is not null and kinds != '[]'))",
    )?;
    if old.is_empty() {
        return Ok(0);
    }
    let live = rows("select id, song_dir, vkey from songs where gone=0")?;
    // an old song of a folder still listed was deleted, not read again another way: only the
    // same place brings it back
    let labels: HashSet<String> = store::Roots::load(tx).labels().into_iter().collect();
    let listed = |d: &str| labels.contains(d.split('/').next().unwrap_or(""));
    let fits = |o: &(i64, String, String), l: &(i64, String, String)| {
        o.2 == l.2 && (o.1 == l.1 || (!listed(&o.1) && tails(&o.1).iter().any(|t| ends_at(&l.1, t))))
    };
    let mut n = 0;
    // the folder names each pair says moved ("Beats" -> "Music/Beats"), how many times
    let mut moves: HashMap<(String, String), usize> = HashMap::new();
    for o in &old {
        let mine: Vec<&(i64, String, String)> = live.iter().filter(|l| fits(o, l)).collect();
        if mine.len() != 1 {
            continue;
        }
        let l = mine[0];
        // the new song must be the only old one's match too
        if old.iter().filter(|x| fits(x, l)).count() != 1 {
            continue;
        }
        fill_from(tx, o.0, l.0, &o.1, &l.1)?;
        rekey(tx, (&o.1, &o.2), (&l.1, &l.2))?;
        n += 1;
        // its old folder's name (the label of the folder taken out) and where the path inside
        // it lands now
        if let Some((label, inner)) = o.1.split_once('/')
            && l.1.ends_with(&format!("/{inner}"))
        {
            let to = l.1[..l.1.len() - inner.len() - 1].to_string();
            // never a name still in use (a folder of the main projects folder of that name)
            let in_use = live.iter().any(|x| x.1 == label || x.1.starts_with(&format!("{label}/")));
            if label != to && !listed(label) && !in_use {
                *moves.entry((label.to_string(), to)).or_default() += 1;
            }
        }
    }
    // the rest of what was kept under the old names (markers, merges, songs made of a file,
    // boards, comments, pictures...) follows, under each old folder's most frequent new name;
    // what was set since under the new name stays
    let mut best: HashMap<String, (String, usize)> = HashMap::new();
    for ((from, to), k) in moves {
        let b = best.entry(from).or_insert((to.clone(), 0));
        if k > b.1 {
            *b = (to, k);
        }
    }
    for (from, (to, _)) in best {
        crate::paths::rename_names(tx, &from, &to)?;
    }
    Ok(n)
}

/// The new song `to` takes from the old song `from` what it has not got: a field the user
/// set on `to` stays as it is.
fn fill_from(tx: &Transaction, from: i64, to: i64, from_dir: &str, to_dir: &str) -> rusqlite::Result<()> {
    type V = rusqlite::types::Value;
    let cols = [
        "title", "artist", "style", "key", "catalog", "ft", "folder_date", "stage", "stage_source", "temoin", "temoin_source", "notes",
        "notion", "checklist", "archived", "rating", "kinds", "tempo", "date", "cover", "user_fields",
    ];
    let get = |id: i64| -> rusqlite::Result<HashMap<&str, V>> {
        tx.query_row(&format!("select {} from songs where id=?", cols.join(", ")), [id], |r| {
            cols.iter().enumerate().map(|(i, c)| Ok((*c, r.get::<_, V>(i)?))).collect()
        })
    };
    let (a, b) = (get(from)?, get(to)?);
    let set_ = |v: &V| !matches!(v, V::Null) && !matches!(v, V::Text(t) if t.is_empty() || t == "[]") && !matches!(v, V::Integer(0));
    let text = |v: &V| if let V::Text(t) = v { t.clone() } else { String::new() };
    let fields = |v: &V| serde_json::from_str::<Vec<String>>(&text(v)).unwrap_or_default();
    // a path inside the old folder, named inside the new one
    let moved = |v: &V| match v {
        V::Text(t) if !from_dir.is_empty() && t.starts_with(&format!("{from_dir}/")) => V::Text(format!("{to_dir}{}", &t[from_dir.len()..])),
        other => other.clone(),
    };
    let (af, mut bf) = (fields(&a["user_fields"]), fields(&b["user_fields"]));
    let mut sets: Vec<(&str, V)> = Vec::new();
    // the names the scan reads (title, artist...): the old one's own choice, where the new
    // one has none of its own
    for c in ["title", "artist", "style", "key", "catalog", "ft", "folder_date"] {
        if af.iter().any(|x| x == c) && !bf.iter().any(|x| x == c) {
            sets.push((c, a[c].clone()));
            bf.push(c.to_string());
        }
    }
    if !set_(&b["stage_source"]) && set_(&a["stage_source"]) {
        sets.push(("stage", a["stage"].clone()));
        sets.push(("stage_source", a["stage_source"].clone()));
    }
    if text(&b["temoin_source"]) != "user" && text(&a["temoin_source"]) == "user" {
        sets.push(("temoin", moved(&a["temoin"])));
        sets.push(("temoin_source", a["temoin_source"].clone()));
    }
    for c in ["notes", "notion", "checklist", "archived", "rating", "kinds", "tempo", "date"] {
        if !set_(&b[c]) && set_(&a[c]) {
            sets.push((c, a[c].clone()));
        }
    }
    if !set_(&b["cover"]) && set_(&a["cover"]) {
        sets.push(("cover", moved(&a["cover"])));
    }
    if !sets.is_empty() {
        sets.push(("user_fields", V::Text(serde_json::to_string(&bf).unwrap_or_else(|_| "[]".into()))));
        let sql = format!("update songs set {} where id=?", sets.iter().map(|(c, _)| format!("{c}=?")).collect::<Vec<_>>().join(", "));
        let mut args: Vec<V> = sets.into_iter().map(|(_, v)| v).collect();
        args.push(V::Integer(to));
        tx.execute(&sql, rusqlite::params_from_iter(args))?;
    }
    // its history, and the suggestions decided on it that the new one has not decided
    tx.execute("update stage_log set song_id=? where song_id=?", [to, from])?;
    tx.execute(
        "update suggestions set song_id=? where song_id=? and state in ('accepted', 'refused') and not exists \
         (select 1 from suggestions s where s.song_id=? and s.kind=suggestions.kind and s.state in ('accepted', 'refused'))",
        [to, from, to],
    )?;
    // what names it by its id: its stars in the favourites, its chosen picture
    if let Some(mut favs) = setting::<Vec<Value>>(tx, "favorites") {
        let (fs, ts) = (from.to_string(), to.to_string());
        let has = favs.iter().any(|f| f["kind"] == "song" && f["id"] == ts.as_str());
        let mut changed = false;
        favs.retain(|f| !(has && f["kind"] == "song" && f["id"] == fs.as_str()));
        for f in favs.iter_mut() {
            if f["kind"] == "song" && f["id"] == fs.as_str() {
                f["id"] = Value::from(ts.clone());
                changed = true;
            }
            if f["song"] == from {
                f["song"] = Value::from(to);
                changed = true;
            }
        }
        if changed || has {
            crate::store_set(tx, "favorites", &favs)?;
        }
    }
    if let Some(mut m) = setting::<serde_json::Map<String, Value>>(tx, "cover_chosen")
        && let Some(v) = m.remove(&from.to_string())
    {
        let v = match v {
            Value::String(p) if !from_dir.is_empty() && p.starts_with(&format!("{from_dir}/")) => Value::String(format!("{to_dir}{}", &p[from_dir.len()..])),
            other => other,
        };
        m.entry(to.to_string()).or_insert(v);
        crate::store_set(tx, "cover_chosen", &m)?;
    }
    Ok(())
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
fn derive(
    tx: &Transaction,
    song: &SongRow,
    d: &Derived,
    roots: &store::Roots,
    aliases: &HashMap<String, String>,
    feats: &HashMap<String, (String, String)>,
) -> rusqlite::Result<()> {
    // no artist in the folder names: the projects folder's own artist, when set; a merged
    // name shown as the one it was merged into
    let root_artist = || roots.split(&d.song_dir).map(|(r, _)| r.artist.clone()).filter(|a| !a.trim().is_empty());
    let mut folder_artist = d.artist.clone().or_else(root_artist).map(|a| aliases.get(&a.trim().to_lowercase()).cloned().unwrap_or(a));
    // a featured artist: the song is the main artist's (the one chosen, else the projects
    // folder's own), the name goes into its featurings
    let mut ft = d.ft.clone();
    if let Some((name, main)) = folder_artist.as_ref().and_then(|a| feats.get(&a.trim().to_lowercase())).cloned() {
        folder_artist = Some(main).filter(|m| !m.trim().is_empty()).or_else(root_artist).filter(|m| !m.trim().eq_ignore_ascii_case(&name));
        let mut list: Vec<String> = ft.iter().flat_map(|f| f.split(',')).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        if !list.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            list.insert(0, name);
        }
        ft = Some(list.join(", "));
    }
    // the artist is never one of its own featurings ("Ana x Mos Marni", Ana a guest: the
    // song is Mos Marni's, with Ana)
    if let (Some(a), Some(f)) = (folder_artist.as_deref(), ft.as_deref()) {
        let l: Vec<&str> = f.split(',').map(str::trim).filter(|x| !x.is_empty() && !x.eq_ignore_ascii_case(a.trim())).collect();
        ft = (!l.is_empty()).then(|| l.join(", "));
    }
    // the style is never read from folder names ("- 25.08.15 - electro - Disco" said a style
    // the user had not chosen, shown as theirs): the user's own, else what Spool detects
    let no_style: Option<String> = None;
    let vals: [(&str, &Option<String>); 7] = [
        ("title", &d.title),
        ("artist", &folder_artist),
        ("style", &no_style),
        ("key", &d.key),
        ("catalog", &d.catalog),
        ("ft", &ft),
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

#[cfg(test)]
mod back_tests {
    use super::*;

    fn set(tx: &Transaction, k: &str, v: Value) {
        crate::store_set(tx, k, &v).unwrap();
    }
    fn files(tx: &Transaction, rels: &[&str]) {
        tx.execute("delete from files", []).unwrap();
        for r in rels {
            tx.execute("insert into files(rel, size, mtime) values(?, 100, 1.0)", [r]).unwrap();
        }
    }
    fn one<T: rusqlite::types::FromSql>(tx: &Transaction, sql: &str) -> T {
        tx.query_row(sql, [], |r| r.get(0)).unwrap()
    }
    /// A song with stars, a style, a stage, a release: its folder "Beats" (C:/Music/Beats)
    /// taken out of the list, the scan run, then "C:/Music" added.
    fn before(tx: &Transaction, remember_removal: bool) -> i64 {
        set(tx, "root", json_str("C:/Projets"));
        set(tx, "more_roots", serde_json::json!([{"label": "Beats", "path": "C:/Music/Beats", "artist": ""}]));
        files(tx, &["Beats/Ana/Song A/Song A.als", "Other/Song B/Song B.als"]);
        group(tx).unwrap();
        let id: i64 = one(tx, "select id from songs where song_dir like 'Beats/%'");
        let dir: String = one(tx, &format!("select song_dir from songs where id={id}"));
        let vkey: String = one(tx, &format!("select vkey from songs where id={id}"));
        tx.execute(
            "update songs set rating=4, style='dnb', stage='mix', stage_source='user', user_fields='[\"style\"]' where id=?",
            [id],
        )
        .unwrap();
        set(tx, "releases", serde_json::json!([{"id": "r1", "title": "EP", "kind": "ep", "tracks": [{"dir": dir, "vkey": vkey}]}]));
        set(tx, "comments", serde_json::json!({format!("{dir}|{vkey}"): [{"id": 1, "text": "drop"}]}));
        set(tx, "markers", serde_json::json!({format!("{dir}/Song A.als"): [[8.0, "drop"]]}));
        set(tx, "one_song_dirs", serde_json::json!([dir.clone()]));
        // the folder taken out
        set(tx, "more_roots", serde_json::json!([]));
        if remember_removal {
            set(tx, "removed_roots", serde_json::json!([{"label": "Beats", "path": "C:/Music/Beats", "artist": ""}]));
        }
        files(tx, &["Other/Song B/Song B.als"]);
        group(tx).unwrap();
        assert_eq!(one::<i64>(tx, &format!("select gone from songs where id={id}")), 1);
        // its parent added
        set(tx, "more_roots", serde_json::json!([{"label": "Music", "path": "C:/Music", "artist": ""}]));
        files(tx, &["Music/Beats/Ana/Song A/Song A.als", "Other/Song B/Song B.als"]);
        id
    }
    fn json_str(s: &str) -> Value {
        Value::from(s)
    }

    #[test]
    fn a_folder_taken_out_then_added_through_its_parent_keeps_its_songs() {
        let st = store::Store::memory().unwrap();
        let tx = st.con.unchecked_transaction().unwrap();
        let id = before(&tx, true);
        group(&tx).unwrap();
        // the same song, alive, at its new place, with all it had
        assert_eq!(one::<i64>(&tx, "select count(*) from songs where gone=0 and song_dir like 'Music/%'"), 1);
        assert_eq!(one::<i64>(&tx, "select id from songs where gone=0 and song_dir like 'Music/%'"), id);
        assert_eq!(one::<i64>(&tx, &format!("select rating from songs where id={id}")), 4);
        assert_eq!(one::<String>(&tx, &format!("select style from songs where id={id}")), "dnb");
        assert_eq!(one::<String>(&tx, &format!("select stage from songs where id={id}")), "mix");
        // its release and its comments follow it
        let dir: String = one(&tx, &format!("select song_dir from songs where id={id}"));
        let rels: Value = setting(&tx, "releases").unwrap();
        assert_eq!(rels[0]["tracks"][0]["dir"], dir.as_str());
        let com: Value = setting(&tx, "comments").unwrap();
        assert!(com.as_object().unwrap().keys().all(|k| k.starts_with(&format!("{dir}|"))));
    }

    #[test]
    fn songs_made_again_from_scratch_get_back_what_they_had_never_over_what_was_set_since() {
        let st = store::Store::memory().unwrap();
        let tx = st.con.unchecked_transaction().unwrap();
        // as the versions before: the removal not remembered, a new empty song made
        let old = before(&tx, false);
        group(&tx).unwrap();
        let new: i64 = one(&tx, "select id from songs where gone=0 and song_dir like 'Music/%'");
        assert_ne!(new, old);
        // the user already set new stars on it since
        tx.execute("update songs set rating=2 where id=?", [new]).unwrap();
        // the update: the repair runs once
        tx.execute("delete from settings where k='reunited_v1'", []).unwrap();
        group(&tx).unwrap();
        assert_eq!(one::<String>(&tx, "select v from settings where k='reunited_v1'"), "1");
        assert_eq!(one::<i64>(&tx, &format!("select rating from songs where id={new}")), 2, "what was set since stays");
        assert_eq!(one::<String>(&tx, &format!("select style from songs where id={new}")), "dnb");
        assert_eq!(one::<String>(&tx, &format!("select stage from songs where id={new}")), "mix");
        assert_eq!(one::<i64>(&tx, &format!("select gone from songs where id={old}")), 1);
        let dir: String = one(&tx, &format!("select song_dir from songs where id={new}"));
        let rels: Value = setting(&tx, "releases").unwrap();
        assert_eq!(rels[0]["tracks"][0]["dir"], dir.as_str());
        // the choices kept by path follow
        let marks: Value = setting(&tx, "markers").unwrap();
        assert!(marks.get(format!("{dir}/Song A.als")).is_some(), "markers: {marks}");
        let dirs: Value = setting(&tx, "one_song_dirs").unwrap();
        assert_eq!(dirs[0], dir.as_str());
        // never twice
        tx.execute("update songs set style='house' where id=?", [new]).unwrap();
        group(&tx).unwrap();
        assert_eq!(one::<String>(&tx, &format!("select style from songs where id={new}")), "house");
    }

    #[test]
    fn a_deleted_song_never_lends_its_stars_to_another_of_the_same_name() {
        let st = store::Store::memory().unwrap();
        let tx = st.con.unchecked_transaction().unwrap();
        set(&tx, "root", Value::from("C:/Projets"));
        files(&tx, &["Ana/Song A/Song A.als"]);
        group(&tx).unwrap();
        tx.execute("update songs set rating=5", []).unwrap();
        // deleted, then a song of the same name made in another artist's folder
        files(&tx, &["Leo/Song A/Song A.als"]);
        group(&tx).unwrap();
        tx.execute("delete from settings where k='reunited_v1'", []).unwrap();
        group(&tx).unwrap();
        assert_eq!(one::<Option<i64>>(&tx, "select rating from songs where gone=0 and song_dir like 'Leo/%'"), None);
    }
}

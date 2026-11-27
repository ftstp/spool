//! Every path the base keeps for a folder, moved with it: the files read, the songs' folders,
//! renders and pictures, and the choices kept by path (corrections, merges, releases, boards,
//! markers, comments, songs made of a file or a folder). Used when the app moves a folder, and
//! when a projects folder comes back through another one (its parent added).
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use store::{Roots, rusqlite};

fn norm_rel(rel: &str) -> String {
    rel.replace('\\', "/").trim_matches('/').to_string()
}

fn like(rel: &str) -> String {
    rel.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_") + "/%"
}

/// Every stored path at or under `old_rel` now points under `new_rel` (and the same for the
/// real paths kept, absolute: a library's, a picture's).
pub fn repath(con: &Connection, roots: &Roots, old_rel: &str, new_rel: &str) -> rusqlite::Result<()> {
    let (o, n) = (norm_rel(old_rel), norm_rel(new_rel));
    let abs = |r: &str| roots.abs(r).map(|p| p.to_string_lossy().replace('\\', "/"));
    repath_names(con, &o, &n, Some((abs(&o).unwrap_or_default(), abs(&n).unwrap_or_default())))
}

/// The same for the names only: a folder read again under another name (its projects folder
/// added again another way), where it stands on disk has not changed. What is already kept
/// under the new name stays (set since): the old one only fills what is missing.
pub fn rename_names(con: &Connection, old_rel: &str, new_rel: &str) -> rusqlite::Result<()> {
    let (o, n) = (norm_rel(old_rel), norm_rel(new_rel));
    if o.is_empty() || o == n {
        return Ok(());
    }
    repath_names(con, &o, &n, None)
}

fn repath_names(con: &Connection, old_rel: &str, new_rel: &str, abs: Option<(String, String)>) -> rusqlite::Result<()> {
    let (old_abs, new_abs) = abs.unwrap_or_default();
    let moved = |v: &str| -> Option<String> {
        let s = v.replace('\\', "/");
        for (base, to) in [(old_rel, new_rel), (old_abs.as_str(), new_abs.as_str())] {
            if !base.is_empty() && (s == base || s.starts_with(&format!("{base}/"))) {
                return Some(format!("{to}{}", &s[base.len()..]));
            }
        }
        None
    };
    for (table, col) in [("files", "rel"), ("audio", "rel"), ("backups", "dir")] {
        let rows: Vec<String> = {
            let mut s = con.prepare(&format!("select {col} from {table} where {col} = ? or {col} like ? escape '\\'"))?;
            s.query_map(params![old_rel, like(old_rel)], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for r in rows {
            if let Some(m) = moved(&r) {
                // a row already there under the new name is the one kept
                let taken: bool = con.query_row(&format!("select count(*) from {table} where {col}=?"), [&m], |r| r.get::<_, i64>(0))? > 0;
                if taken {
                    con.execute(&format!("delete from {table} where {col}=?"), [&r])?;
                    continue;
                }
                con.execute(&format!("update {table} set {col}=? where {col}=?"), params![m, r])?;
                if table == "files" {
                    // read again at the next scan: the sample paths it remembers moved too
                    con.execute("update files set mtime=0 where rel=?", [&m])?;
                }
            }
        }
    }
    let songs: Vec<(i64, Option<String>, Option<String>, Option<String>)> = {
        let mut s = con.prepare("select id, song_dir, temoin, cover from songs")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?
    };
    for (id, dir, temoin, cover) in songs {
        let m = |v: &Option<String>| v.as_deref().filter(|x| !x.is_empty()).and_then(moved);
        let (d, t, c) = (m(&dir), m(&temoin), m(&cover));
        if d.is_some() || t.is_some() || c.is_some() {
            con.execute(
                "update songs set song_dir=coalesce(?, song_dir), temoin=coalesce(?, temoin), cover=coalesce(?, cover) where id=?",
                params![d, t, c, id],
            )?;
        }
    }
    let get = |k: &str| -> Option<Value> {
        con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0))
            .optional()
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
    };
    let set = |k: &str, v: &Value| con.execute("update settings set v=? where k=?", params![v.to_string(), k]);
    // a map keyed by a name ("<folder>|<project>" for some): each key moved; one already under
    // the new name is kept (comments: both lists, nothing lost)
    let rekey = |m: serde_json::Map<String, Value>, piped: bool, value: &dyn Fn(Value) -> Value| -> serde_json::Map<String, Value> {
        let key_of = |k: &str| -> String {
            if piped && let Some((sd, vk)) = k.rsplit_once('|') {
                moved(sd).map(|d| format!("{d}|{vk}")).unwrap_or_else(|| k.to_string())
            } else {
                moved(k).unwrap_or_else(|| k.to_string())
            }
        };
        let mut out = serde_json::Map::new();
        let mut later: Vec<(String, Value)> = Vec::new();
        for (k, v) in m {
            let nk = key_of(&k);
            let v = value(v);
            if nk == k {
                out.insert(k, v);
            } else {
                later.push((nk, v));
            }
        }
        for (k, v) in later {
            match (out.get_mut(&k), v) {
                (Some(Value::Array(have)), Value::Array(more)) => {
                    let ids: Vec<Value> = have.iter().map(|x| x["id"].clone()).collect();
                    have.extend(more.into_iter().filter(|x| x["id"].is_null() || !ids.contains(&x["id"])));
                }
                (Some(_), _) => {}
                (None, v) => {
                    out.insert(k, v);
                }
            }
        }
        out
    };
    let same = |v: Value| v;
    let paths_in = |v: Value| match v {
        Value::Array(a) => Value::Array(a.into_iter().map(|x| x.as_str().and_then(&moved).map(Value::String).unwrap_or(x)).collect()),
        Value::String(s) => moved(&s).map(Value::String).unwrap_or(Value::String(s)),
        o => o,
    };
    let comment_rels = |v: Value| match v {
        Value::Array(a) => Value::Array(
            a.into_iter()
                .map(|mut c| {
                    if let Some(r) = c.get("rel").and_then(Value::as_str).and_then(&moved) {
                        c["rel"] = Value::String(r);
                    }
                    c
                })
                .collect(),
        ),
        o => o,
    };
    if let Some(Value::Object(m)) = get("file_song") {
        set("file_song", &Value::Object(rekey(m, false, &same)))?;
    }
    if let Some(Value::Object(m)) = get("markers") {
        set("markers", &Value::Object(rekey(m, false, &same)))?;
    }
    if let Some(Value::Object(m)) = get("merges") {
        set("merges", &Value::Object(rekey(m, true, &same)))?;
    }
    if let Some(Value::Object(m)) = get("comments") {
        set("comments", &Value::Object(rekey(m, true, &comment_rels)))?;
    }
    for k in ["board_links", "board_hidden"] {
        if let Some(Value::Object(m)) = get(k) {
            set(k, &Value::Object(rekey(m, true, &paths_in)))?;
        }
    }
    if let Some(Value::Object(m)) = get("cover_chosen") {
        set("cover_chosen", &Value::Object(m.into_iter().map(|(k, v)| (k, paths_in(v))).collect()))?;
    }
    // lists of names: a file made a song of its own, a folder read as one song
    for k in ["own_files", "one_song_dirs"] {
        if let Some(Value::Array(a)) = get(k) {
            let mut out: Vec<Value> = Vec::new();
            for x in a {
                let x = x.as_str().and_then(&moved).map(Value::String).unwrap_or(x);
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            set(k, &Value::Array(out))?;
        }
    }
    // the releases' tracks, their picture and mood board, the folder they were made from
    if let Some(Value::Array(rs)) = get("releases") {
        let fix = |t: &mut Value| {
            if let Some(d) = t.get("dir").and_then(Value::as_str).and_then(&moved) {
                t["dir"] = Value::String(d);
            }
        };
        let out: Vec<Value> = rs
            .into_iter()
            .map(|mut r| {
                if let Some(ts) = r.get_mut("tracks").and_then(Value::as_array_mut) {
                    ts.iter_mut().for_each(fix);
                }
                if let Some(c) = r.get_mut("cover_song").filter(|c| c.is_object()) {
                    fix(c);
                }
                for f in ["folder", "cover"] {
                    if let Some(v) = r.get(f).and_then(Value::as_str).and_then(&moved) {
                        r[f] = Value::String(v);
                    }
                }
                if let Some(b) = r.get_mut("board").and_then(Value::as_array_mut) {
                    for x in b.iter_mut() {
                        if let Some(v) = x.as_str().and_then(&moved) {
                            *x = Value::String(v);
                        }
                    }
                }
                r
            })
            .collect();
        set("releases", &Value::Array(out))?;
    }
    Ok(())
}

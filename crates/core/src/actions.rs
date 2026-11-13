//! What the user changes on songs: fields, stage, finishing checks, suggestions decided, and
//! the undo of all of it. Ported from the prototype's song_edit / check_set /
//! suggestion_act and undo.py.
//!
//! Every change runs in one transaction that first photographs the songs it touches (their
//! whole row and the state of their suggestions): an undo puts the photo back. The photos
//! live in the base (`undo_log`), so an undo survives a restart.
use i18n::tr;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use store::rusqlite::{self, OptionalExtension, Transaction, params, types::ValueRef};

pub const EDITABLE: [&str; 10] = ["title", "artist", "style", "key", "notes", "tempo", "date", "rating", "kinds", "ft"];
pub const STAGES: [&str; 6] = ["draft", "arrangement", "mix", "master", "done", "pause"];
pub const CHECKS: [&str; 3] = ["cover", "lyrics", "meta"];
/// Undo entries kept.
const KEEP: i64 = 100;

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// A refusal the user reads, or a database error.
#[derive(Debug)]
pub enum Refused {
    User(String),
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for Refused {
    fn from(e: rusqlite::Error) -> Self {
        Refused::Db(e)
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Refused::User(s) => f.write_str(s),
            Refused::Db(e) => write!(f, "{e}"),
        }
    }
}

fn no(s: &str) -> Refused {
    Refused::User(s.to_string())
}

type R<T> = Result<T, Refused>;

/// A field's name in the undo label; the fields not listed are named by their key.
fn field_name(k: &str) -> String {
    match k {
        "title" | "artist" | "key" | "stage" | "archived" | "temoin" => tr!(&format!("undo.field.{k}")),
        other => other.to_string(),
    }
}

pub fn title_of(tx: &Transaction, sid: i64) -> rusqlite::Result<String> {
    let t: Option<Option<String>> =
        tx.query_row("select title from songs where id=?", [sid], |r| r.get(0)).optional()?;
    Ok(match t {
        Some(t) => tr!("undo.quoted", title = t.unwrap_or_default()),
        None => tr!("undo.aSong"),
    })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// "Title, stage of \"X\"": what the undo button will say.
pub fn edit_label(tx: &Transaction, sid: i64, body: &Map<String, Value>) -> rusqlite::Result<String> {
    let fields: Vec<String> = body.keys().map(|k| field_name(k)).collect();
    let what = capitalize(&fields.join(", "));
    Ok(tr!("undo.edit", what = if what.is_empty() { tr!("undo.editDefault") } else { what }, title = title_of(tx, sid)?))
}

/// Python's `v or None` / `v not in (None, "")`: what counts as "nothing".
fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

fn to_sql(v: &Value) -> Option<rusqlite::types::Value> {
    use rusqlite::types::Value as V;
    if is_empty(v) {
        return None;
    }
    Some(match v {
        Value::String(s) => V::Text(s.clone()),
        Value::Number(n) => n.as_i64().map(V::Integer).unwrap_or_else(|| V::Real(n.as_f64().unwrap_or(0.0))),
        Value::Bool(b) => V::Integer(*b as i64),
        other => V::Text(other.to_string()),
    })
}

fn python_round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// One song's fields, stage, archive flag or reference render. The keys of `body` are applied in order.
pub fn song_edit(tx: &Transaction, sid: i64, body: &Map<String, Value>) -> R<()> {
    let user: Option<Option<String>> =
        tx.query_row("select user_fields from songs where id=?", [sid], |r| r.get(0)).optional()?;
    let Some(user) = user else { return Err(no(&tr!("err.songNotFound"))) };
    let mut user: BTreeSet<String> = serde_json::from_str(user.as_deref().unwrap_or("[]")).unwrap_or_default();
    let t = now();
    for (k, v) in body {
        let k = k.as_str();
        if EDITABLE.contains(&k) {
            let mut v = v.clone();
            if k == "tempo" && !is_empty(&v) {
                let s = match &v {
                    Value::String(s) => s.replace(',', "."),
                    other => other.to_string(),
                };
                let f: f64 = s.trim().parse().map_err(|_| no(&tr!("err.badTempo")))?;
                v = json!(python_round2(f));
            }
            if k == "rating" && !is_empty(&v) {
                let n = v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(|| no(&tr!("err.badRating")))?;
                if !(0..=5).contains(&n) {
                    return Err(no(&tr!("err.ratingRange")));
                }
                v = json!(n);
            }
            if k == "kinds" && !is_empty(&v) {
                let list: Vec<String> = match &v {
                    Value::Array(a) => a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                    Value::String(s) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                    _ => return Err(no(&tr!("err.badKinds"))),
                };
                v = if list.is_empty() { Value::Null } else { Value::String(serde_json::to_string(&list).unwrap()) };
            }
            if k == "date"
                && !is_empty(&v)
                && !v.as_str().is_some_and(|d| {
                    d.len() == 10
                        && d.bytes()
                            .enumerate()
                            .all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() })
                })
            {
                return Err(no(&tr!("err.badDate")));
            }
            tx.execute(&format!("update songs set {k}=?, updated=? where id=?"), params![to_sql(&v), t, sid])?;
            if is_empty(&v) {
                user.remove(k);
            } else {
                user.insert(k.to_string());
            }
            if k == "style" && !is_empty(&v) {
                // a style written down settles the style suggestion
                tx.execute(
                    "update suggestions set state=case when lower(value)=lower(?) then 'accepted' else 'refused' end, \
                     decided=? where song_id=? and state='open' and kind='style'",
                    params![v.as_str().unwrap_or(""), t, sid],
                )?;
            } else if k == "style" {
                // emptied: the deduced styles come back at once, those set aside included
                tx.execute(
                    "update songs set user_fields=? where id=?",
                    params![serde_json::to_string(&user).unwrap(), sid],
                )?;
                tx.execute(
                    "update suggestions set state='stale' where song_id=? and kind='style' and state in ('accepted', 'refused')",
                    [sid],
                )?;
                index::refresh_styles(tx)?;
            }
        } else if k == "stage" {
            let stage = match v {
                Value::Null => None,
                Value::String(s) if STAGES.contains(&s.as_str()) => Some(s.as_str()),
                _ => return Err(no(&tr!("err.unknownStage"))),
            };
            tx.execute(
                "update songs set stage=?, stage_source=?, updated=? where id=?",
                params![stage, stage.map(|_| "user"), t, sid],
            )?;
            tx.execute("insert into stage_log(song_id, stage, at) values(?,?,?)", params![sid, stage, t])?;
            index::refresh_song(tx, sid)?; // the checklist follows the stage
            // a decision on the stage settles the open stage suggestions of this song
            tx.execute(
                "update suggestions set state=case when value=? then 'accepted' else 'refused' end, decided=? \
                 where song_id=? and state='open' and kind='stage'",
                params![stage, t, sid],
            )?;
        } else if k == "archived" {
            let on = !is_empty(v);
            tx.execute("update songs set archived=?, updated=? where id=?", params![on as i64, t, sid])?;
            if on {
                // an archived song leaves the suggestion queue
                tx.execute("update suggestions set state='stale' where song_id=? and state='open'", [sid])?;
            }
        } else if k == "temoin" {
            let tm = v.as_str().filter(|s| !s.is_empty());
            tx.execute("update songs set temoin=?, temoin_source=? where id=?", params![tm, tm.map(|_| "user"), sid])?;
        }
    }
    tx.execute("update songs set user_fields=? where id=?", params![serde_json::to_string(&user).unwrap(), sid])?;
    Ok(())
}

/// A finishing check set by hand: true / false / "na" (does not apply), null gives it back
/// to what the tool sees.
pub fn check_set(tx: &Transaction, sid: i64, key: &str, value: &Value) -> R<()> {
    let cur: Option<Option<String>> =
        tx.query_row("select checklist from songs where id=?", [sid], |r| r.get(0)).optional()?;
    let (Some(cur), true) = (cur, CHECKS.contains(&key)) else { return Err(no(&tr!("err.unknownCheck"))) };
    let mut over: Map<String, Value> = serde_json::from_str(cur.as_deref().unwrap_or("{}")).unwrap_or_default();
    match value {
        Value::Null => {
            over.remove(key);
        }
        Value::Bool(_) => {
            over.insert(key.into(), value.clone());
        }
        Value::String(s) if s == "na" => {
            over.insert(key.into(), value.clone());
        }
        _ => {}
    }
    let v = (!over.is_empty()).then(|| Value::Object(over).to_string());
    tx.execute("update songs set checklist=? where id=?", params![v, sid])?;
    index::refresh_song(tx, sid)?;
    Ok(())
}

/// The song a suggestion is about, and what the undo button will say about deciding it.
pub fn suggestion_label(tx: &Transaction, gid: i64, accept: bool) -> R<(i64, String)> {
    let g: Option<(i64, String)> = tx
        .query_row("select song_id, kind from suggestions where id=?", [gid], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let Some((sid, kind)) = g else { return Err(no(&tr!("err.suggestionNotFound"))) };
    let key = match (kind != "style", accept) {
        (true, true) => "undo.stageAccepted",
        (true, false) => "undo.stageRefused",
        (false, true) => "undo.styleAccepted",
        (false, false) => "undo.styleRefused",
    };
    Ok((sid, tr!(key, title = title_of(tx, sid)?)))
}

/// Accept (with `choice` = another of the proposed styles, or another stage) or refuse.
pub fn suggestion_act(tx: &Transaction, gid: i64, accept: bool, choice: Option<&str>) -> R<()> {
    let g: Option<(i64, String, String)> = tx
        .query_row("select song_id, kind, value from suggestions where id=?", [gid], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<String>>(2)?.unwrap_or_default()))
        })
        .optional()?;
    let Some((sid, kind, value)) = g else { return Err(no(&tr!("err.suggestionNotFound"))) };
    if accept {
        let v = choice.filter(|c| !c.is_empty()).unwrap_or(&value).to_string();
        let mut body = Map::new();
        body.insert(if kind == "style" { "style" } else { "stage" }.into(), Value::String(v));
        return song_edit(tx, sid, &body);
    }
    tx.execute("update suggestions set state='refused', decided=? where id=?", params![now(), gid])?;
    Ok(())
}

// ---------------------------------------------------------------- undo

fn sql_to_json(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(_) => Value::Null,
    }
}

/// The songs `sids` as they are now: put back by `undo`.
pub fn snapshot(tx: &Transaction, sids: &[i64]) -> rusqlite::Result<Option<Value>> {
    let sids: BTreeSet<i64> = sids.iter().copied().filter(|s| *s != 0).collect();
    if sids.is_empty() {
        return Ok(None);
    }
    let marks = vec!["?"; sids.len()].join(",");
    let ids: Vec<i64> = sids.into_iter().collect();
    let mut s = tx.prepare(&format!("select * from songs where id in ({marks})"))?;
    let names: Vec<String> = s.column_names().iter().map(|c| c.to_string()).collect();
    let songs: Vec<Value> = s
        .query_map(rusqlite::params_from_iter(&ids), |r| {
            let mut m = Map::new();
            for (i, n) in names.iter().enumerate() {
                m.insert(n.clone(), sql_to_json(r.get_ref(i)?));
            }
            Ok(Value::Object(m))
        })?
        .collect::<Result<_, _>>()?;
    let mut s = tx.prepare(&format!("select id, state, decided from suggestions where song_id in ({marks})"))?;
    let sugg: Vec<Value> = s
        .query_map(rusqlite::params_from_iter(&ids), |r| {
            Ok(json!({"id": r.get::<_, i64>(0)?, "state": r.get::<_, Option<String>>(1)?, "decided": r.get::<_, Option<f64>>(2)?}))
        })?
        .collect::<Result<_, _>>()?;
    let max: i64 = tx.query_row("select coalesce(max(id), 0) from suggestions", [], |r| r.get(0))?;
    Ok(Some(json!({"at": now(), "songs": songs, "sugg": sugg, "max_sugg": max})))
}

pub fn push(tx: &Transaction, label: &str, snap: Option<Value>) -> rusqlite::Result<()> {
    let Some(snap) = snap else { return Ok(()) };
    tx.execute(
        "insert into undo_log(at, label, data) values(?,?,?)",
        params![snap["at"].as_f64(), label, snap.to_string()],
    )?;
    tx.execute(
        &format!("delete from undo_log where id not in (select id from undo_log order by id desc limit {KEEP})"),
        [],
    )?;
    Ok(())
}

/// What the last change was (the undo button's label), and how many can be undone.
pub fn last(con: &rusqlite::Connection) -> rusqlite::Result<(Option<String>, i64)> {
    let label = con.query_row("select label from undo_log order by id desc limit 1", [], |r| r.get(0)).optional()?;
    let n = con.query_row("select count(*) from undo_log", [], |r| r.get(0))?;
    Ok((label, n))
}

/// What an undo did: its label, the songs it put back. `ops` = a file operation, played
/// backwards by the caller (file moves come with a later step).
pub struct Undone {
    pub label: String,
    pub sids: Vec<i64>,
    pub ops: Option<Value>,
    /// A note's text put back by the caller (`{"path", "old"}`), the entry dropped once done.
    pub note: Option<Value>,
    pub id: i64,
}

/// The newest entry moves files on disk (its Ctrl+Z would move them back).
pub fn last_moves_files(con: &rusqlite::Connection) -> bool {
    con.query_row("select data from undo_log order by id desc limit 1", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|d| serde_json::from_str::<Value>(&d).ok())
        .is_some_and(|v| v["kind"] == "ops")
}

/// Puts the newest photo back. A file operation is left to the caller, and dropped only once
/// it was played backwards (see `drop_entry`).
pub fn undo(tx: &Transaction) -> rusqlite::Result<Option<Undone>> {
    let r: Option<(i64, f64, String, String)> = tx
        .query_row("select id, at, label, data from undo_log order by id desc limit 1", [], |r| {
            Ok((r.get(0)?, r.get::<_, Option<f64>>(1)?.unwrap_or(0.0), r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let Some((id, at, label, data)) = r else { return Ok(None) };
    let snap: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
    if snap["kind"] == "ops" {
        return Ok(Some(Undone { label, sids: Vec::new(), ops: Some(snap["ops"].clone()), note: None, id }));
    }
    if snap["kind"] == "note" {
        return Ok(Some(Undone { label, sids: Vec::new(), ops: None, note: Some(snap), id }));
    }
    // a setting kept whole (the releases): its old value back
    if snap["kind"] == "setting" {
        let k = snap["k"].as_str().unwrap_or("");
        match snap["v"].as_str() {
            Some(v) => tx.execute("insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v", params![k, v])?,
            None => tx.execute("delete from settings where k=?", [k])?,
        };
        tx.execute("delete from undo_log where id=?", [id])?;
        return Ok(Some(Undone { label, sids: Vec::new(), ops: None, note: None, id }));
    }
    let mut sids = Vec::new();
    for s in snap["songs"].as_array().into_iter().flatten() {
        let Some(obj) = s.as_object() else { continue };
        let Some(sid) = obj.get("id").and_then(Value::as_i64) else { continue };
        sids.push(sid);
        // only the columns this base has: an older photo may miss a newer column
        let cols: Vec<&String> = obj.keys().filter(|c| *c != "id").collect();
        let have: BTreeSet<String> = {
            let mut st = tx.prepare("select name from pragma_table_info('songs')")?;
            st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        let cols: Vec<&String> = cols.into_iter().filter(|c| have.contains(*c)).collect();
        let sql = format!(
            "update songs set {} where id=?",
            cols.iter().map(|c| format!("\"{c}\"=?")).collect::<Vec<_>>().join(", ")
        );
        let mut vals: Vec<rusqlite::types::Value> = cols.iter().map(|c| json_to_sql(&obj[*c])).collect();
        vals.push(rusqlite::types::Value::Integer(sid));
        tx.execute(&sql, rusqlite::params_from_iter(vals))?;
    }
    for g in snap["sugg"].as_array().into_iter().flatten() {
        tx.execute(
            "update suggestions set state=?, decided=? where id=?",
            params![g["state"].as_str(), g["decided"].as_f64(), g["id"].as_i64()],
        )?;
    }
    if !sids.is_empty() {
        let marks = vec!["?"; sids.len()].join(",");
        let max = snap["max_sugg"].as_i64().unwrap_or(0);
        let at = snap["at"].as_f64().unwrap_or(at);
        // suggestions born from the change (a style emptied asks again) and its stage history go
        let mut p: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Integer(max)];
        p.extend(sids.iter().map(|s| rusqlite::types::Value::Integer(*s)));
        tx.execute(
            &format!("delete from suggestions where id > ? and song_id in ({marks})"),
            rusqlite::params_from_iter(p),
        )?;
        let mut p: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Real(at)];
        p.extend(sids.iter().map(|s| rusqlite::types::Value::Integer(*s)));
        tx.execute(
            &format!("delete from stage_log where at >= ? and song_id in ({marks})"),
            rusqlite::params_from_iter(p),
        )?;
    }
    tx.execute("delete from undo_log where id=?", [id])?;
    Ok(Some(Undone { label, sids, ops: None, note: None, id }))
}

pub fn drop_entry(tx: &Transaction, id: i64) -> rusqlite::Result<()> {
    tx.execute("delete from undo_log where id=?", [id]).map(|_| ())
}

fn json_to_sql(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as V;
    match v {
        Value::Null => V::Null,
        Value::Bool(b) => V::Integer(*b as i64),
        Value::Number(n) => n.as_i64().map(V::Integer).unwrap_or_else(|| V::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => V::Text(s.clone()),
        other => V::Text(other.to_string()),
    }
}

// ---------------------------------------------------------------- choices mirror

/// The user's decisions, also written to a JSON file beside the base on every change, so a
/// rebuilt base loses nothing (same shape as the prototype's data/choices.json).
pub fn choices(con: &rusqlite::Connection) -> rusqlite::Result<Value> {
    let mut s = con.prepare(
        "select id, song_dir, vkey, title, artist, style, key, tempo, date, stage, stage_source, temoin, \
         temoin_source, notes, notion, user_fields, checklist, archived, rating, kinds, ft from songs where gone = 0 and \
         (user_fields != '[]' or stage_source is not null or temoin_source = 'user' \
          or notes is not null or notion is not null or checklist is not null or archived = 1)",
    )?;
    let names: Vec<String> = s.column_names().iter().map(|c| c.to_string()).collect();
    let songs: Vec<Value> = s
        .query_map([], |r| {
            let mut m = Map::new();
            for (i, n) in names.iter().enumerate() {
                m.insert(n.clone(), sql_to_json(r.get_ref(i)?));
            }
            Ok(Value::Object(m))
        })?
        .collect::<Result<_, _>>()?;
    let mut s =
        con.prepare("select song_id, kind, value, sig, state from suggestions where state in ('accepted', 'refused')")?;
    let sugg: Vec<Value> = s
        .query_map([], |r| {
            Ok(json!({
                "song_id": r.get::<_, i64>(0)?, "kind": r.get::<_, Option<String>>(1)?,
                "value": r.get::<_, Option<String>>(2)?, "sig": r.get::<_, Option<String>>(3)?,
                "state": r.get::<_, Option<String>>(4)?,
            }))
        })?
        .collect::<Result<_, _>>()?;
    let mut settings = Map::new();
    for k in [
        "root",
        "old_drive",
        "artists",
        "ignored",
        "ignored_keep",
        "trials",
        "file_song",
        "sample_dirs",
        "merges",
        "studio_presets",
        "work_words",
        "more_roots",
        "root_artist",
        "to_vst3",
        "midi_dirs",
        "setup_done",
        "releases",
    ] {
        let v: Option<String> = con.query_row("select v from settings where k=?", [k], |r| r.get(0)).optional()?;
        settings.insert(k.into(), v.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or(Value::Null));
    }
    Ok(json!({"saved": now(), "settings": settings, "songs": songs, "suggestions": sugg}))
}

/// Written beside, then renamed over: a crash never leaves half a file.
pub fn save_choices(con: &rusqlite::Connection, path: &std::path::Path) -> std::io::Result<()> {
    let v = choices(con).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&v)?)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::Store;

    fn base() -> Store {
        let st = Store::memory().unwrap();
        st.con
            .execute_batch(
                "insert into songs(id, song_dir, vkey, title, stage) values(1, 'A', 'a', 'Alpha', 'draft');
                 insert into suggestions(id, song_id, kind, value, sig, state) values(5, 1, 'stage', 'mix', 'mix|x', 'open');
                 insert into suggestions(id, song_id, kind, value, sig, state) values(6, 1, 'style', 'Dubstep', 'style|Dubstep', 'open');",
            )
            .unwrap();
        st
    }

    fn get(st: &Store, col: &str) -> Value {
        st.con
            .query_row(&format!("select {col} from songs where id=1"), [], |r| Ok(sql_to_json(r.get_ref(0)?)))
            .unwrap()
    }

    #[test]
    fn edit_then_undo() {
        let st = base();
        let tx = st.con.unchecked_transaction().unwrap();
        let body: Map<String, Value> =
            serde_json::from_str(r#"{"title": "Beta", "tempo": "140,5", "style": "dubstep"}"#).unwrap();
        let label = edit_label(&tx, 1, &body).unwrap();
        let snap = snapshot(&tx, &[1]).unwrap();
        song_edit(&tx, 1, &body).unwrap();
        push(&tx, &label, snap).unwrap();
        tx.commit().unwrap();
        assert_eq!(get(&st, "title"), json!("Beta"));
        assert_eq!(get(&st, "tempo"), json!(140.5));
        assert_eq!(get(&st, "user_fields"), json!(r#"["style","tempo","title"]"#));
        let state: String = st.con.query_row("select state from suggestions where id=6", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "accepted");
        assert_eq!(last(&st.con).unwrap(), (Some("Style, tempo, titre de « Alpha »".into()), 1));

        let tx = st.con.unchecked_transaction().unwrap();
        let u = undo(&tx).unwrap().unwrap();
        tx.commit().unwrap();
        assert_eq!(u.sids, vec![1]);
        assert_eq!(get(&st, "title"), json!("Alpha"));
        assert_eq!(get(&st, "tempo"), Value::Null);
        let state: String = st.con.query_row("select state from suggestions where id=6", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "open");
        assert_eq!(last(&st.con).unwrap(), (None, 0));
    }

    #[test]
    fn refusals() {
        let st = base();
        let tx = st.con.unchecked_transaction().unwrap();
        let bad = |s: &str| song_edit(&tx, 1, &serde_json::from_str(s).unwrap()).unwrap_err().to_string();
        assert_eq!(bad(r#"{"tempo": "vite"}"#), "tempo invalide");
        assert_eq!(bad(r#"{"date": "12/03/2024"}"#), "date invalide (AAAA-MM-JJ)");
        assert_eq!(bad(r#"{"stage": "fini"}"#), "étape inconnue");
        assert_eq!(song_edit(&tx, 9, &Map::new()).unwrap_err().to_string(), "morceau introuvable");
        assert_eq!(check_set(&tx, 1, "nope", &json!(true)).unwrap_err().to_string(), "case inconnue");
    }

    #[test]
    fn refuse_and_checks() {
        let st = base();
        let tx = st.con.unchecked_transaction().unwrap();
        suggestion_act(&tx, 5, false, None).unwrap();
        let state: String = tx.query_row("select state from suggestions where id=5", [], |r| r.get(0)).unwrap();
        assert_eq!(state, "refused");
        check_set(&tx, 1, "cover", &json!("na")).unwrap();
        check_set(&tx, 1, "lyrics", &json!(true)).unwrap();
        check_set(&tx, 1, "cover", &Value::Null).unwrap();
        let c: String = tx.query_row("select checklist from songs where id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(c, r#"{"lyrics":true}"#);
    }
}

//! The treatments from the application's side: presets kept in the settings, one run on one
//! project (a dry run for the preview), the copy recorded so it is never overwritten once
//! saved in Live. Ported from the prototype's studio.run / source_of.
use crate::repairs::{RepairResult, choices, free_path, untouched_copy};
use repair::studio::{self, RoleMatch};
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::Mutex;
use store::Store;
use store::rusqlite::{self, OptionalExtension};

/// Roles read the app's way: a role word must start a word (see studio::RoleMatch).
pub const ROLES: RoleMatch = RoleMatch::Starts;

pub fn presets(st: &Store) -> Vec<Value> {
    st.get::<Vec<Value>>("studio_presets").filter(|v| !v.is_empty()).unwrap_or_else(|| vec![studio::default_preset()])
}

/// (source rel, codes it already carries). A processed copy still exactly as this tool wrote
/// it is redone from its original; one retouched in Live is the source itself (the work
/// saved in it must not be lost).
fn source_of(con: &rusqlite::Connection, roots: &store::Roots, rel: &str) -> (String, String) {
    let stem = Path::new(rel).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let (base, codes) = repair::paths::split_processed(&stem);
    let Some(codes) = codes else { return (rel.into(), String::new()) };
    let parent = rel.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
    let orig = format!("{parent}{base}.als");
    let (Some(cur), Some(o)) = (roots.abs(rel), roots.abs(&orig)) else { return (rel.into(), codes) };
    if untouched_copy(con, roots, &cur) && o.exists() {
        return (orig, String::new());
    }
    (rel.into(), codes)
}

fn plan_of(st: &Store, rel: &str) -> Option<repair::fix::Plan> {
    let (d, p): (String, String) = st
        .con
        .query_row("select data, plugstate from files where rel=?", [rel], |r| {
            Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, Option<String>>(1)?.unwrap_or_default()))
        })
        .optional()
        .ok()
        .flatten()?;
    Some(repair::fix::plan(
        rel,
        &serde_json::from_str(&d).unwrap_or_default(),
        &serde_json::from_str(&p).unwrap_or_default(),
        &choices(st),
    ))
}

/// Applies a preset to one project. `write` false: a dry run (report + track lists only).
/// `in_place`: work on `rel` itself even when it is an untouched copy (it keeps its codes).
pub fn run(st: &Mutex<Store>, rel: &str, preset: &Value, write: bool, in_place: bool, templates: &Path) -> Value {
    let err = |e: String| json!({"rel": rel, "ok": false, "error": e});
    let (root, src_rel, carried, plan) = {
        let s = st.lock().unwrap();
        let root = store::Roots::load(&s.con);
        if root.is_empty() {
            return err(i18n::tr!("fs.noRoot"));
        }
        let (src_rel, carried) = if in_place {
            let stem = Path::new(rel).file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
            (rel.to_string(), repair::paths::split_processed(&stem).1.unwrap_or_default())
        } else {
            source_of(&s.con, &root, rel)
        };
        let plan = if preset.get("R").and_then(Value::as_bool) == Some(true) { plan_of(&s, &src_rel) } else { None };
        (root, src_rel, carried, plan)
    };
    let Some(src) = root.abs(&src_rel) else { return err(i18n::tr!("err.rootNotFound")) };
    let mut set = match repair::xml::read_set(&src) {
        Ok(x) => x,
        Err(e) => return err(i18n::tr!("err.reading", e = e)),
    };
    if set.root.child("LiveSet").and_then(|l| l.child("Tracks")).is_none() {
        return err(i18n::tr!("atelier.notLive"));
    }
    let before = studio::track_list(&set.root, ROLES);
    let mut rep = Map::new();
    rep.insert("rel".into(), json!(rel));
    rep.insert("source".into(), json!(src_rel));
    let mut codes: String = carried.clone();
    if let Some(p) = plan.filter(|p| p.useful) {
        let r = if write {
            match repair::fix::repair_tree(&p, &mut set.root, &src, templates, &mut repair::fix::Disk) {
                Ok(d) => serde_json::to_value(d).unwrap(),
                Err(e) => return err(e),
            }
        } else {
            json!({"renamed": p.renames.len(), "removed": p.removes.len(), "tried": p.trials.len(), "samples": p.copies.len()})
        };
        rep.insert("R".into(), r);
        codes.push('R');
    }
    if let Value::Object(t) = studio::treat(&mut set.root, preset, ROLES) {
        rep.extend(t);
    }
    for c in studio::CODES.chars().skip(1) {
        if rep.contains_key(&c.to_string()) {
            codes.push(c);
        }
    }
    rep.insert("after".into(), json!(studio::track_list(&set.root, ROLES)));
    rep.insert("before".into(), json!(before));
    if codes.is_empty() {
        rep.insert("ok".into(), json!(false));
        rep.insert("error".into(), json!(i18n::tr!("atelier.nothingToDo")));
        return Value::Object(rep);
    }
    let codes: String = studio::CODES.chars().filter(|c| codes.contains(*c)).collect();
    let s = st.lock().unwrap();
    let Some(out) = free_path(&src, &codes) else { return err(i18n::tr!("err.tooManyCopies")) };
    let out_rel = root.rel(&out).unwrap_or_default();
    rep.insert("out".into(), json!(out_rel));
    if !write {
        rep.insert("ok".into(), json!(true));
        rep.insert("dry".into(), json!(true));
        return Value::Object(rep);
    }
    if let Err(e) = repair::xml::write_set(&set, &out) {
        return err(i18n::tr!("err.writing", e = e));
    }
    // the full preset is kept, so a later repair of this copy can redo the same treatments
    let mut actions =
        json!({"preset": preset.get("name").cloned().unwrap_or(Value::Null), "options": preset, "codes": codes});
    for c in studio::CODES.chars() {
        if let Some(x) = rep.get(&c.to_string()) {
            actions[c.to_string()] = x.clone();
        }
    }
    let md = std::fs::metadata(&out).ok();
    let mtime = md
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    // the copy is on disk either way: if the base could not record it, say so (it would
    // otherwise not be known as the app's own copy)
    let recorded = s.con.execute(
        "insert into repairs(fixed, source, at, size, mtime, actions) values(?,?,?,?,?,?) on conflict(fixed) do update set \
         source=excluded.source, at=excluded.at, size=excluded.size, mtime=excluded.mtime, actions=excluded.actions",
        rusqlite::params![out_rel, src_rel, now, md.map(|m| m.len() as i64), mtime, actions.to_string()],
    );
    if let Err(e) = recorded {
        rep.insert("error".into(), json!(i18n::tr!("err.copyNotRecorded", e = e)));
    }
    rep.insert("ok".into(), json!(true));
    Value::Object(rep)
}

/// A run, as the batch's results list shows it.
pub fn as_result(r: &Value) -> RepairResult {
    let n = |k: &str, f: &str| r[k][f].as_i64().unwrap_or(0);
    RepairResult {
        rel: r["rel"].as_str().unwrap_or("").into(),
        ok: r["ok"].as_bool().unwrap_or(false),
        error: r["error"].as_str().map(str::to_string),
        out: r["out"].as_str().map(str::to_string),
        renamed: n("R", "renamed"),
        removed: n("R", "removed"),
        tried: n("R", "tried"),
        samples: n("R", "samples"),
        copied: n("R", "copied"),
        complete: true,
        manual: Vec::new(),
        lost: 0,
    }
}

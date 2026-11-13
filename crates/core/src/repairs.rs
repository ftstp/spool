//! Repaired copies, from the application's side: what the user chose (ignored plugins,
//! trial replacements) read from the settings, and which copies may be (re)written.
use repair::fix::Choices;
use std::path::Path;
use store::Store;
use store::rusqlite::{self, OptionalExtension};

pub fn choices(st: &Store) -> Choices {
    let low = |k: &str| -> std::collections::HashSet<String> {
        st.get::<Vec<String>>(k).unwrap_or_default().into_iter().map(|s| s.to_lowercase()).collect()
    };
    Choices {
        ignored: low("ignored"),
        kept: low("ignored_keep"),
        trials: st.get::<serde_json::Value>("trials").and_then(|v| v.as_object().cloned()).unwrap_or_default(),
        to_vst3: st.get::<serde_json::Value>("to_vst3").and_then(|v| v.as_object().cloned()).unwrap_or_default(),
    }
}

/// A copy is written only where no file is: an earlier copy, even one never opened since,
/// is the user's file once it is in their folder, and is never overwritten.
pub fn can_write(out: &Path) -> bool {
    !out.exists()
}

/// Still exactly the copy this tool wrote (size and date as recorded), never retouched in
/// Live: the treatments redo it from its original rather than stacking treatments.
pub fn untouched_copy(con: &rusqlite::Connection, roots: &store::Roots, out: &Path) -> bool {
    let Ok(md) = std::fs::metadata(out) else { return true };
    let rel = roots.rel(out).unwrap_or_default();
    let rec: Option<(i64, f64)> = con
        .query_row("select size, mtime from repairs where fixed=?", [&rel], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .ok()
        .flatten();
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    rec.is_some_and(|(size, m)| size as u64 == md.len() && (m - mtime).abs() < 1.0)
}

/// Where to write: "Name [codes].als" unless that one was retouched in Live (then
/// "Name [codes] 2.als", 3...).
pub fn free_path(src: &Path, codes: &str) -> Option<std::path::PathBuf> {
    (1..100).map(|n| repair::paths::processed_path(src, codes, n)).find(|p| can_write(p))
}

/// A copy the app wrote (a repair, a treatments pass), as its own record says.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct WrittenCopy {
    pub fixed: String,
    pub source: String,
    pub at: f64,
    /// The treatments in its name ("R", "RCN"...), and the preset that made it.
    pub codes: String,
    pub preset: Option<String>,
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    /// Still on disk.
    pub exists: bool,
    /// Saved since in Live (size or date changed): the user's own file now.
    pub retouched: bool,
    #[ts(type = "number | null")]
    pub song: Option<i64>,
    pub title: Option<String>,
}

/// Every copy the app wrote, the newest first.
pub fn written(con: &rusqlite::Connection, roots: &store::Roots) -> rusqlite::Result<Vec<WrittenCopy>> {
    let mut q = con.prepare(
        "select r.fixed, coalesce(r.source, ''), coalesce(r.at, 0), r.size, r.mtime, coalesce(r.actions, '{}'), s.id, s.title          from repairs r left join files f on f.rel = r.source left join songs s on s.id = f.song_id          order by r.at desc",
    )?;
    let rows = q.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<f64>>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        let (fixed, source, at, size, mtime, actions, song, title) = row;
        let a: serde_json::Value = serde_json::from_str(&actions).unwrap_or_default();
        let codes = match &a["codes"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(v) => v.iter().filter_map(|x| x.as_str()).collect(),
            _ => String::new(),
        };
        let md = roots.abs(&fixed).and_then(|p| std::fs::metadata(p).ok());
        let disk_mtime = md
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64());
        let retouched = match (&md, size, mtime, disk_mtime) {
            (Some(m), Some(s), Some(t), Some(d)) => m.len() as i64 != s || (d - t).abs() > 2.0,
            _ => false,
        };
        out.push(WrittenCopy {
            exists: md.is_some(),
            retouched,
            size: md.map(|m| m.len() as i64).or(size),
            preset: a["preset"].as_str().map(str::to_string),
            fixed,
            source,
            at,
            codes,
            song,
            title,
        });
    }
    Ok(out)
}

/// A project with something to repair, for the Repair page.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Fixable {
    pub rel: String,
    #[ts(type = "number")]
    pub song: i64,
    pub title: Option<String>,
    #[ts(type = "number")]
    pub renames: i64,
    /// Already a repaired copy, with something new to fix (a plugin installed since...).
    pub again: bool,
    #[ts(type = "number")]
    pub copies: i64,
    #[ts(type = "number")]
    pub removes: i64,
    #[ts(type = "number")]
    pub trials: i64,
    /// The copy would open with nothing missing.
    pub complete: bool,
    /// Bytes of samples to copy into the project.
    #[ts(type = "number")]
    pub bytes: i64,
}

/// Every active project (not an old version, not archived, of a live song) a repair improves.
pub fn fixable(st: &Store) -> rusqlite::Result<Vec<Fixable>> {
    let ch = choices(st);
    let mut s = st.con.prepare(
        "select f.rel, f.song_id, s.title, f.sev, f.data, f.plugstate from files f join songs s on s.id = f.song_id \
         where f.in_version = 0 and f.archived = 0 and s.gone = 0 and s.archived = 0",
    )?;
    let rows = s.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            r.get::<_, Option<String>>(5)?.unwrap_or_default(),
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (rel, song, title, sev, data, states) = row?;
        let data: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
        let states: serde_json::Value = serde_json::from_str(&states).unwrap_or_default();
        let p = repair::fix::plan(&rel, &data, &states, &ch);
        if !p.useful {
            continue;
        }
        out.push(Fixable {
            again: repair::paths::is_processed(&rel),
            renames: p.renames.len() as i64,
            copies: p.copies.len() as i64,
            removes: p.removes.len() as i64,
            trials: p.trials.len() as i64,
            complete: p.complete && sev <= 1,
            bytes: p.copies.iter().map(|c| c["size"].as_i64().unwrap_or(0)).sum(),
            rel,
            song,
            title,
        });
    }
    Ok(out)
}

/// One repair, as reported to the interface.
#[derive(Debug, Clone, Default, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RepairResult {
    pub rel: String,
    pub ok: bool,
    pub error: Option<String>,
    /// The copy written, relative to the root.
    pub out: Option<String>,
    #[ts(type = "number")]
    pub renamed: i64,
    #[ts(type = "number")]
    pub removed: i64,
    #[ts(type = "number")]
    pub tried: i64,
    #[ts(type = "number")]
    pub samples: i64,
    #[ts(type = "number")]
    pub copied: i64,
    /// Opens with nothing missing.
    pub complete: bool,
    /// Plugins left in the copy to replace by hand.
    pub manual: Vec<String>,
    #[ts(type = "number")]
    pub lost: i64,
}

/// The batch running in the background.
#[derive(Debug, Clone, Default, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RepairState {
    pub running: bool,
    #[ts(type = "number")]
    pub done: i64,
    #[ts(type = "number")]
    pub total: i64,
    pub results: Vec<RepairResult>,
    pub at: Option<f64>,
}

/// Repairs one project: "Name [R].als" next to it (or "Name [PR].als" for a copy already
/// treated, which keeps its treatments). `store` is locked only to read the plan and to
/// record the copy, never while the set is read or written.
pub fn repair_one(st: &std::sync::Mutex<Store>, rel: &str, templates: &Path) -> RepairResult {
    let fail = |e: String| RepairResult { rel: rel.into(), error: Some(e), ..Default::default() };
    let (roots, p) = {
        let s = st.lock().unwrap();
        let roots = store::Roots::load(&s.con);
        let row: Option<(String, String)> = s
            .con
            .query_row("select data, plugstate from files where rel=?", [rel], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            })
            .optional()
            .ok()
            .flatten();
        let Some((data, states)) = row else { return fail(i18n::tr!("err.projectNotFound")) };
        let p = repair::fix::plan(
            rel,
            &serde_json::from_str(&data).unwrap_or_default(),
            &serde_json::from_str(&states).unwrap_or_default(),
            &choices(&s),
        );
        (roots, p)
    };
    if !p.useful {
        return fail(i18n::tr!("repair.nothingToRepair"));
    }
    let Some(src) = roots.abs(rel) else { return fail(i18n::tr!("err.rootNotFound")) };
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let carried = repair::paths::split_processed(&stem).1.unwrap_or_default();
    let codes: String = repair::paths::CODES.chars().filter(|c| *c == 'R' || carried.contains(*c)).collect();
    let mut set = match repair::xml::read_set(&src) {
        Ok(s) => s,
        Err(e) => return fail(i18n::tr!("err.reading", e = e)),
    };
    let done = match repair::fix::repair_tree(&p, &mut set.root, &src, templates, &mut repair::fix::Disk) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    let s = st.lock().unwrap();
    let Some(out) = free_path(&src, &codes) else { return fail(i18n::tr!("err.tooManyCopies")) };
    if let Err(e) = repair::xml::write_set(&set, &out) {
        return fail(i18n::tr!("err.writing", e = e));
    }
    let fixed = roots.rel(&out).unwrap_or_default();
    let md = std::fs::metadata(&out).ok();
    let mtime = md
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());
    let actions = serde_json::json!({
        "preset": "Réparation", "options": {"name": "Réparation", "R": true}, "codes": codes, "R": done,
    });
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    // the copy is on disk either way: if the base could not record it, say so (it would
    // otherwise not be known as the app's own copy)
    let recorded = s.con.execute(
        "insert into repairs(fixed, source, at, size, mtime, actions) values(?,?,?,?,?,?) on conflict(fixed) do update set \
         source=excluded.source, at=excluded.at, size=excluded.size, mtime=excluded.mtime, actions=excluded.actions",
        rusqlite::params![fixed, rel, now, md.map(|m| m.len() as i64), mtime, actions.to_string()],
    );
    RepairResult {
        rel: rel.into(),
        ok: true,
        error: recorded.err().map(|e| i18n::tr!("err.copyNotRecorded", e = e)),
        out: Some(fixed),
        renamed: done.renamed as i64,
        removed: done.removed as i64,
        tried: done.tried as i64,
        samples: done.samples as i64,
        copied: done.copied as i64,
        complete: p.complete,
        manual: p.manual.iter().map(|m| m.name.clone()).collect(),
        lost: p.lost.len() as i64,
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;
    use std::sync::Mutex;

    const SET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="11.0_433">
  <LiveSet>
    <Tracks>
      <MidiTrack Id="1">
        <DeviceChain><DeviceChain><Devices>
          <PluginDevice Id="2"><PluginDesc><VstPluginInfo Id="0"><PlugName Value="OldSynth" /><UniqueId Value="123" /></VstPluginInfo></PluginDesc></PluginDevice>
          <PluginDevice Id="3"><PluginDesc><VstPluginInfo Id="0"><PlugName Value="KeptSynth" /><UniqueId Value="456" /></VstPluginInfo></PluginDesc></PluginDevice>
        </Devices></DeviceChain></DeviceChain>
      </MidiTrack>
    </Tracks>
  </LiveSet>
</Ableton>
"#;

    /// A projects folder with Song.als, its row in a base, and OldSynth set aside (ignored,
    /// removed from the copies).
    fn setup(name: &str) -> (std::path::PathBuf, Mutex<Store>) {
        let root = std::env::temp_dir().join(format!("spool-repair-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("tpl")).unwrap();
        std::fs::write(root.join("Song.als"), SET).unwrap();
        let st = Store::memory().unwrap();
        let set = |k: &str, v: serde_json::Value| {
            st.con.execute("insert into settings(k, v) values(?, ?)", rusqlite::params![k, v.to_string()]).unwrap();
        };
        set("root", serde_json::json!(root.to_string_lossy()));
        set("ignored", serde_json::json!(["OldSynth"]));
        let data = serde_json::json!({"plugins": [
            {"name": "OldSynth", "format": "vst2", "id": "123"},
            {"name": "KeptSynth", "format": "vst2", "id": "456"},
        ]});
        let states = serde_json::json!([{"status": "missing"}, {"status": "ok"}]);
        st.con
            .execute("insert into files(rel, data, plugstate) values('Song.als', ?, ?)", rusqlite::params![data.to_string(), states.to_string()])
            .unwrap();
        (root, Mutex::new(st))
    }

    #[test]
    fn a_repair_writes_a_new_copy_and_never_touches_the_original() {
        let (root, st) = setup("copy");
        let before = std::fs::read(root.join("Song.als")).unwrap();
        let r = repair_one(&st, "Song.als", &root.join("tpl"));
        assert!(r.ok, "{:?}", r.error);
        assert!(r.error.is_none(), "recorded in the base");
        assert_eq!(r.removed, 1);
        assert_eq!(r.out.as_deref(), Some("Song [R].als"));
        // the original, byte for byte
        assert_eq!(std::fs::read(root.join("Song.als")).unwrap(), before);
        // the copy: the set-aside plugin gone, the other one kept
        let copy = repair::xml::read_set(&root.join("Song [R].als")).unwrap();
        let names: Vec<String> = copy.root.descendants().filter(|e| e.tag == "PlugName").filter_map(|e| e.get("Value").map(str::to_string)).collect();
        assert_eq!(names, vec!["KeptSynth".to_string()]);
        // known as the app's own copy
        let w = {
            let s = st.lock().unwrap();
            written(&s.con, &store::Roots::load(&s.con)).unwrap()
        };
        assert_eq!(w.len(), 1);
        assert!(w[0].exists && !w[0].retouched);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_second_repair_never_overwrites_the_first_copy() {
        let (root, st) = setup("again");
        assert!(repair_one(&st, "Song.als", &root.join("tpl")).ok);
        // the user opened the copy in Live and saved it: it is theirs now
        std::fs::write(root.join("Song [R].als"), b"my edits").unwrap();
        let r = repair_one(&st, "Song.als", &root.join("tpl"));
        assert!(r.ok, "{:?}", r.error);
        assert_eq!(r.out.as_deref(), Some("Song [R] 2.als"));
        assert_eq!(std::fs::read(root.join("Song [R].als")).unwrap(), b"my edits");
        std::fs::remove_dir_all(&root).unwrap();
    }
}

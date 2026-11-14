//! What the interface shows, read from the base. Ported from the prototype's views.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use store::rusqlite::{self, Connection};
use ts_rs::TS;

const CHECKS: [&str; 3] = ["cover", "lyrics", "meta"];

/// One song in the lists (every view of the Songs page uses this).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SongItem {
    /// 1-5 stars, none when not rated.
    #[ts(type = "number | null")]
    pub rating: Option<i64>,
    /// "Original", "Remix", "Collab"...
    pub kinds: Vec<String>,
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// Shown: the user's, else detected (`style_src` says which, as `key_src`, `tempo_src`;
    /// crate::detect).
    pub style: Option<String>,
    pub style_src: Option<String>,
    pub key: Option<String>,
    pub key_src: Option<String>,
    pub tempo: Option<f64>,
    pub tempo_src: Option<String>,
    /// Fields whose detections disagree ("style", "key", "tempo"): worth a look.
    pub doubt: Vec<String>,
    #[ts(type = "number | null")]
    pub len: Option<i64>,
    pub stage: Option<String>,
    /// An open stage suggestion.
    pub sugg: Option<String>,
    /// The best open style suggestion.
    pub ssugg: Option<StyleHint>,
    /// Worst severity of its active projects: 0 opens, 1 repairable, 2 settings lost, 3 missing.
    #[ts(type = "number")]
    pub sev: i64,
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number")]
    pub hidden: i64,
    pub mtime: f64,
    /// Its newest active project.
    pub rel: String,
    pub created: f64,
    pub dir: String,
    pub temoin: bool,
    /// The reference render's file (its waveform on the wide list).
    pub temoin_rel: Option<String>,
    pub live: Option<String>,
    pub catalog: Option<String>,
    pub ft: Option<String>,
    pub instr: Vec<String>,
    pub err: bool,
    /// Finishing checks done, out of `checks`.
    #[ts(type = "number")]
    pub check: i64,
    #[ts(type = "number")]
    pub checks: i64,
    pub archived: bool,
    pub date: Option<String>,
    pub cover: Option<String>,
    /// Days a project of the song was saved, newest first (the calendar's dots).
    pub days: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StyleHint {
    pub style: String,
    #[ts(type = "number")]
    pub pct: i64,
}

struct FileR {
    rel: String,
    mtime: f64,
    sev: i64,
    in_version: bool,
    data: Option<String>,
}

fn local_day(t: f64) -> String {
    use chrono::{Local, TimeZone};
    Local.timestamp_opt(t.floor() as i64, 0).single().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// Finishing checks done: the user's overrides (true / false / "na") over what was seen.
fn checks_done(checkauto: Option<&str>, checklist: Option<&str>) -> i64 {
    let auto: Value = serde_json::from_str(checkauto.unwrap_or("{}")).unwrap_or(Value::Null);
    let over: Value = serde_json::from_str(checklist.unwrap_or("{}")).unwrap_or(Value::Null);
    CHECKS
        .iter()
        .filter(|k| {
            let o = &over[**k];
            if o == "na" {
                return true;
            }
            match o.as_bool() {
                Some(b) => b,
                None => auto[**k][0].as_bool().unwrap_or(false),
            }
        })
        .count() as i64
}

pub fn songs_list(con: &Connection) -> rusqlite::Result<Vec<SongItem>> {
    let mut files: HashMap<i64, Vec<FileR>> = HashMap::new();
    {
        let mut s =
            con.prepare("select song_id, rel, mtime, sev, in_version, data from files where song_id is not null")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                FileR {
                    rel: r.get(1)?,
                    mtime: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    sev: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    in_version: r.get::<_, Option<i64>>(4)?.unwrap_or(0) != 0,
                    data: r.get(5)?,
                },
            ))
        })?;
        for row in rows {
            let (sid, f) = row?;
            files.entry(sid).or_default().push(f);
        }
    }
    let mut sugg: HashMap<i64, String> = HashMap::new();
    {
        let mut s = con.prepare("select song_id, value from suggestions where state='open' and kind='stage'")?;
        for r in s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = r?;
            sugg.insert(k, v);
        }
    }
    let mut ssugg: HashMap<i64, StyleHint> = HashMap::new();
    let mut style_opts: HashMap<i64, Vec<(String, i64)>> = HashMap::new();
    let audio = crate::detect_audio(con);
    {
        let mut s = con.prepare("select song_id, reasons from suggestions where state='open' and kind='style'")?;
        for r in s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))? {
            let (k, v) = r?;
            style_opts.insert(k, crate::detect::style_options(v.as_deref()));
            let v: Value = serde_json::from_str(&v.unwrap_or_default()).unwrap_or(Value::Null);
            if let Some(first) = v.get(0) {
                ssugg.insert(
                    k,
                    StyleHint {
                        style: first["style"].as_str().unwrap_or("").into(),
                        pct: first["pct"].as_i64().unwrap_or(0),
                    },
                );
            }
        }
    }
    let mut out = Vec::new();
    let mut s = con.prepare(
        "select id, title, artist, style, key, tempo, stage, song_dir, temoin, catalog, ft, checkauto, checklist, archived, date, cover, \
         rating, kinds from songs where gone = 0",
    )?;
    let rows = s.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            [r.get::<_, Option<String>>(1)?, r.get(2)?, r.get(3)?, r.get(4)?],
            r.get::<_, Option<f64>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?.unwrap_or_default(),
            r.get::<_, Option<String>>(8)?,
            [r.get::<_, Option<String>>(9)?, r.get(10)?, r.get(11)?, r.get(12)?],
            r.get::<_, Option<i64>>(13)?.unwrap_or(0) != 0,
            [r.get::<_, Option<String>>(14)?, r.get(15)?],
            r.get::<_, Option<i64>>(16)?,
            r.get::<_, Option<String>>(17)?,
        ))
    })?;
    for row in rows {
        let (
            id,
            [title, artist, style, ukey],
            tempo,
            stage,
            dir,
            temoin,
            [catalog, ft, checkauto, checklist],
            archived,
            [date, cover],
            rating,
            kinds,
        ) = row?;
        let Some(fs) = files.get(&id) else { continue };
        let act: Vec<&FileR> = fs.iter().filter(|f| !f.in_version).collect();
        let Some(newest) = act
            .iter()
            .copied()
            .fold(None::<&FileR>, |b, f| if b.is_none_or(|b| f.mtime > b.mtime) { Some(f) } else { b })
        else {
            continue;
        };
        let d: Value = serde_json::from_str(newest.data.as_deref().unwrap_or("{}")).unwrap_or(Value::Null);
        let heard = audio.get(&id).cloned().unwrap_or_default();
        let fk = crate::detect::key(ukey.as_deref(), &d, &heard);
        let ft_ = crate::detect::tempo(tempo, &d, &heard);
        let fs_ = crate::detect::style(style.as_deref(), style_opts.get(&id).map(Vec::as_slice).unwrap_or(&[]));
        let doubt: Vec<String> =
            [("style", fs_.doubt), ("key", fk.doubt), ("tempo", ft_.doubt)].iter().filter(|x| x.1).map(|x| x.0.to_string()).collect();
        let instr: Vec<String> = d["devices"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x["kind"] == "instr")
                    .take(3)
                    .map(|x| x["name"].as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default();
        let days: BTreeSet<String> = fs.iter().map(|f| local_day(f.mtime)).collect();
        out.push(SongItem {
            rating,
            kinds: kinds.and_then(|k| serde_json::from_str(&k).ok()).unwrap_or_default(),
            id,
            title,
            artist,
            style: fs_.shown.as_ref().map(|x| x.value.clone()),
            style_src: fs_.shown.map(|x| x.src),
            key: fk.shown.as_ref().map(|x| x.value.clone()),
            key_src: fk.shown.map(|x| x.src),
            tempo: ft_.shown.as_ref().and_then(|x| x.value.parse().ok()),
            tempo_src: ft_.shown.map(|x| x.src),
            doubt,
            len: d["length_s"].as_i64(),
            stage,
            sugg: sugg.get(&id).cloned(),
            ssugg: ssugg.get(&id).cloned(),
            sev: act.iter().map(|f| f.sev).max().unwrap_or(0),
            files: act.len() as i64,
            hidden: (fs.len() - act.len()) as i64,
            mtime: newest.mtime,
            rel: newest.rel.clone(),
            created: fs.iter().map(|f| f.mtime).fold(f64::INFINITY, f64::min),
            dir,
            temoin: temoin.as_deref().is_some_and(|t| !t.is_empty()),
            temoin_rel: temoin.clone().filter(|t| !t.is_empty()),
            live: d["live_version"].as_str().map(str::to_string),
            catalog,
            ft,
            instr,
            err: d["error"].as_str().is_some_and(|e| !e.is_empty()),
            check: checks_done(checkauto.as_deref(), checklist.as_deref()),
            checks: CHECKS.len() as i64,
            archived,
            date,
            cover,
            days: days.into_iter().rev().take(120).collect(),
        });
    }
    Ok(out)
}

/// The header's numbers: projects, plugins and songs, each split into ok / to fix / blocked.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Split {
    #[ts(type = "number")]
    pub total: i64,
    #[ts(type = "number")]
    pub ok: i64,
    #[ts(type = "number")]
    pub fix: i64,
    /// Opens with a newer or other-format plugin: some settings may be lost.
    #[ts(type = "number")]
    pub lost: i64,
    /// Something to install (lost included).
    #[ts(type = "number")]
    pub bad: i64,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Stats {
    pub projects: Split,
    pub plugins: Split,
    pub songs: Split,
}

use plugins::status_severity as severity;

pub fn stats(con: &Connection, ignored: &[String]) -> rusqlite::Result<Stats> {
    let ign: BTreeSet<String> = ignored.iter().map(|s| s.to_lowercase()).collect();
    let mut s = con.prepare(
        "select f.song_id, f.sev, f.data, f.plugstate from files f join songs s on s.id = f.song_id \
         where f.in_version = 0 and f.archived = 0 and s.gone = 0 and s.archived = 0",
    )?;
    let rows = s.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<i64>>(1)?.unwrap_or(0),
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    let bucket = |sev: i64, sp: &mut Split| {
        sp.total += 1;
        match sev {
            0 => sp.ok += 1,
            1 => sp.fix += 1,
            2 => {
                sp.lost += 1;
                sp.bad += 1;
            }
            _ => sp.bad += 1,
        }
    };
    let mut st = Stats::default();
    let (mut worst_song, mut worst_plug): (HashMap<i64, i64>, HashMap<(String, String), i64>) = Default::default();
    for row in rows {
        let (sid, sev, data, plugstate) = row?;
        bucket(sev, &mut st.projects);
        let w = worst_song.entry(sid).or_insert(0);
        *w = (*w).max(sev);
        let d: Value = serde_json::from_str(data.as_deref().unwrap_or("{}")).unwrap_or(Value::Null);
        let ps: Value = serde_json::from_str(plugstate.as_deref().unwrap_or("[]")).unwrap_or(Value::Null);
        for (r, p) in d["plugins"].as_array().into_iter().flatten().zip(ps.as_array().into_iter().flatten()) {
            let name = r["name"].as_str().unwrap_or("");
            let k = (r["format"].as_str().unwrap_or("").to_string(), name.to_string());
            let sev = if ign.contains(&name.to_lowercase()) { 0 } else { severity(p["status"].as_str().unwrap_or("")) };
            let w = worst_plug.entry(k).or_insert(0);
            *w = (*w).max(sev);
        }
    }
    for v in worst_song.values() {
        bucket(*v, &mut st.songs);
    }
    for v in worst_plug.values() {
        bucket(*v, &mut st.plugins);
    }
    Ok(st)
}

/// A stage suggestion waiting for a yes / no.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StageSugg {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub song_id: i64,
    pub value: String,
    pub reasons: Vec<String>,
    pub title: Option<String>,
    pub stage: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StyleSugg {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub song_id: i64,
    pub title: Option<String>,
    pub options: Vec<StyleOpt>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct StyleOpt {
    pub style: String,
    #[ts(type = "number")]
    pub pct: i64,
    pub why: Vec<String>,
}

/// The home page: what to pick up, what is nearly finished, what waits for a decision.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Overview {
    #[ts(type = "number")]
    pub songs: i64,
    #[ts(type = "number")]
    pub archived: i64,
    #[ts(type = "Record<string, number>")]
    pub stages: HashMap<String, i64>,
    pub queue: Vec<StageSugg>,
    pub style_queue: Vec<StyleSugg>,
    pub recent: Vec<SongItem>,
    pub close: Vec<SongItem>,
}

pub fn overview(con: &Connection) -> rusqlite::Result<Overview> {
    let every = songs_list(con)?;
    let songs: Vec<&SongItem> = every.iter().filter(|s| !s.archived).collect();
    let mut stages: HashMap<String, i64> = HashMap::new();
    for s in &songs {
        *stages.entry(s.stage.clone().unwrap_or_else(|| "none".into())).or_default() += 1;
    }
    let queue = {
        let mut s = con.prepare(
            "select g.id, g.song_id, g.value, g.reasons, s.title, s.stage from suggestions g join songs s on s.id=g.song_id \
             where g.state='open' and g.kind='stage' and s.gone=0 and s.archived=0 order by g.created desc",
        )?;
        s.query_map([], |r| {
            Ok(StageSugg {
                id: r.get(0)?,
                song_id: r.get(1)?,
                value: r.get(2)?,
                reasons: serde_json::from_str(&r.get::<_, Option<String>>(3)?.unwrap_or_default()).unwrap_or_default(),
                title: r.get(4)?,
                stage: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    let mut style_queue = {
        let mut s = con.prepare(
            "select g.id, g.song_id, g.reasons, s.title from suggestions g join songs s on s.id=g.song_id \
             where g.state='open' and g.kind='style' and s.gone=0 and s.archived=0",
        )?;
        s.query_map([], |r| {
            Ok(StyleSugg {
                id: r.get(0)?,
                song_id: r.get(1)?,
                options: serde_json::from_str(&r.get::<_, Option<String>>(2)?.unwrap_or_default()).unwrap_or_default(),
                title: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    style_queue.sort_by_key(|q| std::cmp::Reverse(q.options.first().map(|o| o.pct).unwrap_or(0)));
    let mut recent: Vec<SongItem> = songs.iter().map(|s| (*s).clone()).collect();
    recent.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
    recent.truncate(24);
    let mut close: Vec<SongItem> = songs
        .iter()
        .filter(|s| !matches!(s.stage.as_deref(), Some("done") | Some("pause")))
        .map(|s| (*s).clone())
        .collect();
    // nearly finished: the furthest stage first, then the finishing checks, then the newest
    let rank = |s: &SongItem| match s.stage.as_deref() { Some("master") => 3, Some("mix") => 2, Some("arrangement") => 1, _ => 0 };
    close.sort_by(|a, b| {
        rank(b).cmp(&rank(a)).then(b.check.cmp(&a.check)).then(b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal))
    });
    close.truncate(8);
    Ok(Overview {
        songs: songs.len() as i64,
        archived: (every.len() - songs.len()) as i64,
        stages,
        queue,
        style_queue,
        recent,
        close,
    })
}

//! The numbers behind the home page's statistics and the Progression page, ported from the
//! prototype's views.overview (its "stats" part), views.summary and views.progress.
//!
//! A scan that changed something leaves one history point (`snapshot`): how many projects
//! open, how many are complete, songs per stage. The Progression page draws those points.
use crate::repair_view::RepairView;
use crate::views::SongItem;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use store::rusqlite::{self, Connection, OptionalExtension};
use ts_rs::TS;

struct Active {
    sev: i64,
    data: Value,
    plugstate: Value,
}

/// Files that count: not superseded, not archived, attached to a live song.
fn active_files(con: &Connection) -> rusqlite::Result<Vec<Active>> {
    let mut s = con.prepare(
        "select f.sev, f.data, f.plugstate from files f join songs s on s.id = f.song_id \
         where f.in_version = 0 and f.archived = 0 and s.gone = 0 and s.archived = 0",
    )?;
    s.query_map([], |r| {
        Ok(Active {
            sev: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
            data: serde_json::from_str(&r.get::<_, Option<String>>(1)?.unwrap_or_default()).unwrap_or(Value::Null),
            plugstate: serde_json::from_str(&r.get::<_, Option<String>>(2)?.unwrap_or_default()).unwrap_or(Value::Null),
        })
    })?
    .collect()
}

fn ignored(con: &Connection) -> HashSet<String> {
    con.query_row("select v from settings where k='ignored'", [], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|n| n.to_lowercase())
        .collect()
}

fn pairs(f: &Active) -> impl Iterator<Item = (&Value, &Value)> {
    let refs = f.data["plugins"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let sts = f.plugstate.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    refs.iter().zip(sts.iter())
}

/// What a project still needs installed. AU (Mac) plugins cannot be installed on Windows.
fn missing_keys(f: &Active, ign: &HashSet<String>) -> HashSet<String> {
    pairs(f)
        .filter(|(r, st)| {
            st["status"] == "missing"
                && r["format"] != "au"
                && !ign.contains(&r["name"].as_str().unwrap_or("").to_lowercase())
        })
        .map(|(r, _)| format!("{}|{}", r["format"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or("")))
        .collect()
}

/// One history point, the prototype's `summary`.
pub fn summary(con: &Connection) -> rusqlite::Result<Value> {
    let files = active_files(con)?;
    let ign = ignored(con);
    let miss: Vec<HashSet<String>> = files.iter().map(|f| missing_keys(f, &ign)).collect();
    let mut stages: BTreeMap<String, i64> = BTreeMap::new();
    {
        let mut s = con.prepare("select stage from songs where gone = 0 and archived = 0")?;
        for st in s.query_map([], |r| r.get::<_, Option<String>>(0))? {
            *stages.entry(st?.unwrap_or_else(|| "none".into())).or_default() += 1;
        }
    }
    let absent: HashSet<&String> = miss.iter().flatten().collect();
    Ok(json!({
        "files": files.len(),
        "sev": (0..4).map(|i| files.iter().filter(|f| f.sev == i).count()).collect::<Vec<_>>(),
        "complete": miss.iter().filter(|m| m.is_empty()).count(),
        "absent": absent.len(),
        "songs": stages.values().sum::<i64>(),
        "stages": stages,
    }))
}

/// Records a history point when it differs from the last one. True when one was added.
pub fn snapshot(con: &Connection) -> rusqlite::Result<bool> {
    let s = summary(con)?;
    if s["files"] == 0 {
        return Ok(false);
    }
    let last: Option<String> = con.query_row("select summary from scans order by id desc limit 1", [], |r| r.get(0)).optional()?;
    if last.and_then(|l| serde_json::from_str::<Value>(&l).ok()).as_ref() == Some(&s) {
        return Ok(false);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    con.execute("insert into scans(at, summary) values(?, ?)", rusqlite::params![now, s.to_string()])?;
    Ok(true)
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Count {
    pub label: String,
    #[ts(type = "number")]
    pub n: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeviceUse {
    pub name: String,
    /// One of Live's own devices.
    pub native: bool,
    /// Projects using it.
    #[ts(type = "number")]
    pub n: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArtistStages {
    pub artist: String,
    #[ts(type = "Record<string, number>")]
    pub stages: HashMap<String, i64>,
}

/// The home page's statistics block.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct HomeStats {
    /// Active projects, and how many open (severity 0 and 1: as is, or once repaired).
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number[]")]
    pub sev: Vec<i64>,
    pub by_artist: Vec<ArtistStages>,
    /// Plugins missing from the most songs.
    pub blocking: Vec<Count>,
    /// Songs per 5 bpm.
    pub tempos: Vec<Count>,
    pub keys: Vec<Count>,
    #[ts(type = "Record<string, number>")]
    pub key_src: HashMap<String, i64>,
    pub styles: Vec<Count>,
    /// Songs by the year of their first file.
    pub years: Vec<Count>,
    pub lives: Vec<Count>,
    pub instr: Vec<DeviceUse>,
    pub fx: Vec<DeviceUse>,
    /// Songs saved per week, the last 26 weeks (oldest first).
    #[ts(type = "number[]")]
    pub weeks: Vec<i64>,
    #[ts(type = "number")]
    pub with_tempo: i64,
    #[ts(type = "number")]
    pub with_key: i64,
    #[ts(type = "number")]
    pub with_style: i64,
    #[ts(type = "number")]
    pub with_audio: i64,
    #[ts(type = "number")]
    pub total: i64,
}

fn top(c: HashMap<String, i64>, n: usize) -> Vec<Count> {
    let mut v: Vec<Count> = c.into_iter().map(|(label, n)| Count { label, n }).collect();
    v.sort_by(|a, b| b.n.cmp(&a.n).then(a.label.cmp(&b.label)));
    v.truncate(n);
    v
}

fn sorted(c: HashMap<String, i64>) -> Vec<Count> {
    let mut v: Vec<Count> = c.into_iter().map(|(label, n)| Count { label, n }).collect();
    v.sort_by(|a, b| a.label.cmp(&b.label));
    v
}

pub fn home_stats(con: &Connection, every: &[SongItem], rv: &RepairView) -> rusqlite::Result<HomeStats> {
    let songs: Vec<&SongItem> = every.iter().filter(|s| !s.archived).collect();
    let mut by_artist: HashMap<String, HashMap<String, i64>> = HashMap::new();
    let (mut tempos, mut keys, mut key_src, mut styles, mut years, mut lives) =
        (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
    for s in &songs {
        let stage = s.stage.clone().unwrap_or_else(|| "none".into());
        *by_artist.entry(s.artist.clone().unwrap_or_else(|| "Sans artiste".into())).or_default().entry(stage).or_default() +=
            1;
        if let Some(t) = s.tempo.filter(|t| *t > 0.0) {
            *tempos.entry(format!("{:03}", (t / 5.0).floor() as i64 * 5)).or_insert(0) += 1;
        }
        if let Some(k) = &s.key {
            *keys.entry(k.clone()).or_insert(0) += 1;
        }
        if let Some(k) = &s.key_src {
            *key_src.entry(k.clone()).or_insert(0) += 1;
        }
        if let Some(st) = s.style.as_ref().filter(|x| !x.is_empty()) {
            *styles.entry(st.to_lowercase()).or_insert(0) += 1;
        }
        if s.created > 0.0 {
            let y = chrono::DateTime::from_timestamp(s.created as i64, 0).map(|d| d.format("%Y").to_string());
            if let Some(y) = y {
                *years.entry(y).or_insert(0) += 1;
            }
        }
        let live = s.live.as_deref().unwrap_or("?").split('.').next().unwrap_or("?").to_string();
        *lives.entry(live).or_insert(0) += 1;
    }
    // devices, once per project
    let mut dev: HashMap<(String, bool, bool), i64> = HashMap::new(); // (name, instr, native)
    for f in active_files(con)? {
        let mut seen: HashSet<(String, bool, bool)> = HashSet::new();
        for d in f.data["devices"].as_array().into_iter().flatten() {
            seen.insert((d["name"].as_str().unwrap_or("").to_string(), d["kind"] == "instr", true));
        }
        for (r, st) in pairs(&f) {
            seen.insert((r["name"].as_str().unwrap_or("").to_string(), st["kind"] == "instr", false));
        }
        for k in seen.into_iter().filter(|k| !k.0.is_empty()) {
            *dev.entry(k).or_default() += 1;
        }
    }
    let mut devs: Vec<((String, bool, bool), i64)> = dev.into_iter().collect();
    devs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.0.cmp(&b.0.0)));
    let pick = |instr: bool| -> Vec<DeviceUse> {
        devs.iter()
            .filter(|((_, i, _), _)| *i == instr)
            .take(12)
            .map(|((name, _, native), n)| DeviceUse { name: name.clone(), native: *native, n: *n })
            .collect()
    };
    // songs saved per week (a song counts once a week)
    let today = chrono::Local::now().date_naive();
    let mut weeks = vec![0i64; 26];
    for s in &songs {
        let mut hit: HashSet<usize> = HashSet::new();
        for d in &s.days {
            if let Ok(day) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                let ago = (today - day).num_days();
                if (0..26 * 7).contains(&ago) {
                    hit.insert(25 - (ago / 7) as usize);
                }
            }
        }
        for w in hit {
            weeks[w] += 1;
        }
    }
    let mut by_artist: Vec<ArtistStages> =
        by_artist.into_iter().map(|(artist, stages)| ArtistStages { artist, stages }).collect();
    by_artist.sort_by(|a, b| b.stages.values().sum::<i64>().cmp(&a.stages.values().sum::<i64>()).then(a.artist.cmp(&b.artist)));
    let mut blocking: Vec<Count> = rv
        .plugins
        .iter()
        .filter(|p| p.status == "missing")
        .map(|p| Count { label: p.name.clone(), n: p.n })
        .collect();
    blocking.sort_by(|a, b| b.n.cmp(&a.n));
    blocking.truncate(8);
    Ok(HomeStats {
        files: rv.files,
        sev: rv.sev.clone(),
        by_artist,
        blocking,
        tempos: sorted(tempos),
        keys: top(keys, 12),
        key_src,
        styles: top(styles, 12),
        years: sorted(years),
        lives: sorted(lives),
        instr: pick(true),
        fx: pick(false),
        weeks,
        with_tempo: songs.iter().filter(|s| s.tempo.is_some_and(|t| t > 0.0)).count() as i64,
        with_key: songs.iter().filter(|s| s.key.is_some()).count() as i64,
        with_style: songs.iter().filter(|s| s.style.as_ref().is_some_and(|x| !x.is_empty())).count() as i64,
        with_audio: songs.iter().filter(|s| s.temoin).count() as i64,
        total: songs.len() as i64,
    })
}

/// The Progression page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Progress {
    /// Every history point: {at, files, sev, complete, absent, songs, stages}.
    #[ts(type = "Array<{ at: number, files: number, sev: number[], complete: number, absent: number, songs: number, stages: Record<string, number> }>")]
    pub history: Vec<Value>,
    /// Songs marked finished, per month ("2026-03").
    pub done_by_month: Vec<Count>,
    /// Projects complete after each plugin of the install order.
    #[ts(type = "number[]")]
    pub projection: Vec<i64>,
    pub projection_names: Vec<String>,
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number")]
    pub complete_now: i64,
}

pub fn progress(con: &Connection, rv: &RepairView) -> rusqlite::Result<Progress> {
    let history: Vec<Value> = {
        let mut s = con.prepare("select at, summary from scans order by at")?;
        s.query_map([], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, String>(1)?)))?
            .flatten()
            .filter_map(|(at, j)| {
                let mut v: Value = serde_json::from_str(&j).ok()?;
                v["at"] = json!(at);
                Some(v)
            })
            .collect()
    };
    let mut done: HashMap<String, i64> = HashMap::new();
    {
        let mut s = con.prepare("select at from stage_log where stage='done'")?;
        for at in s.query_map([], |r| r.get::<_, f64>(0))?.flatten() {
            if let Some(d) = chrono::DateTime::from_timestamp(at as i64, 0) {
                *done.entry(d.with_timezone(&chrono::Local).format("%Y-%m").to_string()).or_default() += 1;
            }
        }
    }
    Ok(Progress {
        history,
        done_by_month: sorted(done),
        projection: rv.order.iter().map(|o| o.complete).collect(),
        projection_names: rv.order.iter().map(|o| o.name.clone()).collect(),
        files: rv.files,
        complete_now: rv.complete_now,
    })
}

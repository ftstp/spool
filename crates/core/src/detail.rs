//! One song's sheet: everything the song page shows. Ported from the prototype's
//! views.song_detail.
use crate::keys::{song_key, sure_estimate};
use catalog::{
    Audio, Board, Candidate, MP3_DIR, Rules, list_board_labeled, other_song_prefixes, song_dir_known,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use store::rusqlite::{self, Connection, OptionalExtension};
use ts_rs::TS;

/// The finishing checks shown. Arrangement / mix / master are the stage (chosen at the top of
/// the sheet), "opens with nothing missing" is seen by the app (the Projects card), "released"
/// belongs to the release side (a module of its own, later): only what the stage does not say.
const CHECK_KEYS: [&str; 3] = ["cover", "lyrics", "meta"];

/// One finishing check: what the tool sees, and the user's word over it.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Check {
    pub key: String,
    pub label: String,
    pub done: bool,
    /// Does not apply to this song.
    pub na: bool,
    /// What the tool saw by itself.
    pub auto: bool,
    /// Set by hand.
    pub manual: bool,
    pub why: String,
}

pub fn checklist(checkauto: Option<&str>, checklist: Option<&str>) -> Vec<Check> {
    let auto: Value = serde_json::from_str(checkauto.unwrap_or("{}")).unwrap_or(Value::Null);
    let over: Value = serde_json::from_str(checklist.unwrap_or("{}")).unwrap_or(Value::Null);
    CHECK_KEYS
        .iter()
        .map(|k| {
            let a_done = auto[*k][0].as_bool().unwrap_or(false);
            let why = auto[*k][1].as_str().unwrap_or("").to_string();
            let o = &over[*k];
            let na = o == "na";
            let done = na || o.as_bool().unwrap_or(a_done);
            Check {
                key: k.to_string(),
                label: i18n::tr!(&format!("checks.label.{k}")),
                done,
                na,
                auto: a_done,
                manual: over.get(*k).is_some(),
                why,
            }
        })
        .collect()
}

/// The song's own fields.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SongRow {
    #[ts(type = "number")]
    pub id: i64,
    pub song_dir: Option<String>,
    pub vkey: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub style: Option<String>,
    pub key: Option<String>,
    pub catalog: Option<String>,
    pub ft: Option<String>,
    pub folder_date: Option<String>,
    pub stage: Option<String>,
    pub stage_source: Option<String>,
    pub temoin: Option<String>,
    pub temoin_source: Option<String>,
    pub notes: Option<String>,
    pub notion: Option<String>,
    /// Fields the user wrote: a scan never overwrites them.
    pub user_fields: Vec<String>,
    pub archived: bool,
    pub created: Option<f64>,
    pub updated: Option<f64>,
    pub tempo: Option<f64>,
    pub date: Option<String>,
    pub cover: Option<String>,
    #[ts(type = "number | null")]
    pub rating: Option<i64>,
    pub kinds: Vec<String>,
}

/// A plugin a project cannot load as saved.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Issue {
    pub name: String,
    pub format: String,
    pub status: String,
    #[ts(type = "unknown[]")]
    pub candidates: Vec<Value>,
    pub ignored: bool,
    pub kept: bool,
    /// For a trial version: the plugin it is replaced with.
    pub trial: String,
    /// The preset it was saved with (name, bank, default).
    #[ts(type = "Record<string, unknown> | null")]
    pub preset: Value,
    pub need: String,
    pub need_label: String,
    /// Times it appears in the project.
    #[ts(type = "number")]
    pub n: i64,
}

/// One active project file of the song.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct FileItem {
    pub rel: String,
    pub name: String,
    pub mtime: f64,
    pub ctime: Option<f64>,
    /// The newest active project of the whole song folder.
    pub latest: bool,
    #[ts(type = "number")]
    pub sev: i64,
    pub promoted: bool,
    pub tempo: Option<f64>,
    #[ts(type = "number | null")]
    pub len: Option<i64>,
    /// Live's scale (its default C major left out).
    pub key: Option<String>,
    pub key_est: Option<String>,
    pub key_bass: bool,
    /// The MIDI estimate however unsure, and how sure (0-1): the key field's hover.
    pub key_raw: Option<String>,
    pub key_conf: Option<f64>,
    pub time_sig: Option<String>,
    pub live: Option<String>,
    /// Tracks by kind ("audio", "midi", "return"...).
    #[ts(type = "Record<string, number> | null")]
    pub tracks: Value,
    pub error: Option<String>,
    pub issues: Vec<Issue>,
    pub samples_lost: Vec<String>,
    #[ts(type = "number")]
    pub samples_moved: i64,
    #[ts(type = "number")]
    pub samples_total: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Device {
    pub name: String,
    #[ts(type = "number")]
    pub count: i64,
    /// An Ableton device (always there), else a plugin.
    pub native: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<String>,
    /// The plugin's entry on the Repair page ("format|name|status").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub key: Option<String>,
    /// What is installed instead, when something is ("Kontakt (VST2 v7.10)").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instead: Option<String>,
    /// Set aside on the Repair page (no longer counted).
    #[serde(default)]
    pub ignored: bool,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Devices {
    pub instr: Vec<Device>,
    pub fx: Vec<Device>,
    pub midi: Vec<Device>,
}

/// A project saved close in time to the reference render.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Near {
    pub rel: String,
    pub name: String,
    /// Days from the reference render, negative = before.
    pub days: f64,
    #[ts(type = "number | null")]
    pub song_id: Option<i64>,
    pub title: Option<String>,
    /// A project of this song.
    pub same: bool,
    /// An old version.
    pub old: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct OpenStage {
    #[ts(type = "number")]
    pub id: i64,
    pub value: String,
    pub reasons: Vec<String>,
    pub created: Option<f64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct OpenStyle {
    #[ts(type = "number")]
    pub id: i64,
    pub options: Vec<crate::views::StyleOpt>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Sibling {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Version {
    pub rel: String,
    pub name: String,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Merged {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StageStep {
    pub stage: Option<String>,
    pub at: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SongDetail {
    pub song: SongRow,
    pub board: Board,
    pub checklist: Vec<Check>,
    /// Its folder holds other songs too.
    pub shared: bool,
    /// Backup copies (Ableton's "Backup" folders) under its folder.
    #[ts(type = "number")]
    pub backups: i64,
    pub versions: Vec<Version>,
    pub merged: Vec<Merged>,
    pub files: Vec<FileItem>,
    #[ts(type = "number")]
    pub hidden: i64,
    pub main_chain: Vec<String>,
    pub devices: Devices,
    pub temoins: Vec<Candidate>,
    /// The other audio files of the song's folder: renders the app could not tie to this song
    /// by their name (a folder shared by many songs, an export named otherwise). Any can be
    /// chosen as the reference render.
    pub other_audio: Vec<Candidate>,
    pub suggestions: Vec<OpenStage>,
    pub style_sugg: Option<OpenStyle>,
    pub near_temoin: Vec<Near>,
    pub siblings: Vec<Sibling>,
    pub log: Vec<StageStep>,
}

use plugins::status_severity as severity;

fn setting<T: serde::de::DeserializeOwned>(con: &Connection, k: &str) -> Option<T> {
    let v: String = con.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok()?;
    serde_json::from_str(&v).ok()
}

fn trial_key(fmt: &str, name: &str) -> String {
    if fmt == "vst2" { name.to_lowercase() } else { format!("{fmt}|{}", name.to_lowercase()) }
}

fn base_name(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

fn opt_str(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// (rel, mtime, song id, old version, song title)
type NearRow = (String, f64, Option<i64>, bool, Option<String>);

struct Row {
    rel: String,
    mtime: f64,
    ctime: Option<f64>,
    sev: i64,
    in_version: bool,
    promoted: bool,
    song_dir: Option<String>,
    vkey: Option<String>,
    data: Value,
    plugstate: Value,
}

pub fn song_detail(con: &Connection, sid: i64) -> rusqlite::Result<Option<SongDetail>> {
    let song = con
        .query_row(
            "select id, song_dir, vkey, title, artist, style, key, catalog, ft, folder_date, stage, stage_source, \
             temoin, temoin_source, notes, notion, user_fields, archived, created, updated, tempo, date, cover, \
             checkauto, checklist, rating, kinds from songs where id=?",
            [sid],
            |r| {
                Ok((
                    SongRow {
                        id: r.get(0)?,
                        song_dir: r.get(1)?,
                        vkey: r.get(2)?,
                        title: r.get(3)?,
                        artist: r.get(4)?,
                        style: r.get(5)?,
                        key: r.get(6)?,
                        catalog: r.get(7)?,
                        ft: r.get(8)?,
                        folder_date: r.get(9)?,
                        stage: r.get(10)?,
                        stage_source: r.get(11)?,
                        temoin: r.get(12)?,
                        temoin_source: r.get(13)?,
                        notes: r.get(14)?,
                        notion: r.get(15)?,
                        user_fields: serde_json::from_str(&r.get::<_, Option<String>>(16)?.unwrap_or_default())
                            .unwrap_or_default(),
                        archived: r.get::<_, Option<i64>>(17)?.unwrap_or(0) != 0,
                        created: r.get(18)?,
                        updated: r.get(19)?,
                        tempo: r.get(20)?,
                        date: r.get(21)?,
                        cover: r.get(22)?,
                        rating: r.get(25)?,
                        kinds: r.get::<_, Option<String>>(26)?.and_then(|k| serde_json::from_str(&k).ok()).unwrap_or_default(),
                    },
                    r.get::<_, Option<String>>(23)?,
                    r.get::<_, Option<String>>(24)?,
                ))
            },
        )
        .optional()?;
    let Some((song, checkauto, checks)) = song else { return Ok(None) };
    let song_dir = song.song_dir.clone().unwrap_or_default();
    let rows: Vec<Row> = {
        let mut s = con.prepare(
            "select rel, mtime, ctime, sev, in_version, promoted, song_dir, vkey, data, plugstate \
             from files where song_id=? order by mtime desc",
        )?;
        s.query_map([sid], |r| {
            Ok(Row {
                rel: r.get(0)?,
                mtime: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                ctime: r.get(2)?,
                sev: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                in_version: r.get::<_, Option<i64>>(4)?.unwrap_or(0) != 0,
                promoted: r.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
                song_dir: r.get(6)?,
                vkey: r.get(7)?,
                data: serde_json::from_str(&r.get::<_, Option<String>>(8)?.unwrap_or_default()).unwrap_or(Value::Null),
                plugstate: serde_json::from_str(&r.get::<_, Option<String>>(9)?.unwrap_or_default())
                    .unwrap_or(Value::Null),
            })
        })?
        .collect::<Result<_, _>>()?
    };
    let active: Vec<&Row> = rows.iter().filter(|r| !r.in_version).collect();
    // "the newest": the newest active .als of the whole song folder, whichever song it is
    let newest_rel: Option<String> = con
        .query_row(
            "select rel from files where song_dir=? and in_version=0 order by mtime desc limit 1",
            [&song.song_dir],
            |r| r.get(0),
        )
        .optional()?;
    let lower = |v: Option<Vec<String>>| -> BTreeSet<String> {
        v.unwrap_or_default().into_iter().map(|s| s.to_lowercase()).collect()
    };
    let ign = lower(setting(con, "ignored"));
    let kept = lower(setting(con, "ignored_keep"));
    let trials: HashMap<String, Value> = setting::<Value>(con, "trials")
        .and_then(|v| v.as_object().cloned())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default();
    let mut files = Vec::new();
    for r in &active {
        let d = &r.data;
        let mut issues: Vec<Issue> = Vec::new();
        let refs = d["plugins"].as_array().cloned().unwrap_or_default();
        let sts = r.plugstate.as_array().cloned().unwrap_or_default();
        for (rf, st) in refs.iter().zip(sts.iter()) {
            let status = st["status"].as_str().unwrap_or("");
            if status == "ok" {
                continue;
            }
            let name = rf["name"].as_str().unwrap_or("").to_string();
            let format = rf["format"].as_str().unwrap_or("").to_string();
            if let Some(i) = issues.iter_mut().find(|i| i.format == format && i.name == name) {
                i.n += 1;
                continue;
            }
            let trial = if status == "trial" {
                trials.get(&trial_key(&format, &name)).and_then(|t| t["to"].as_str()).unwrap_or("").to_string()
            } else {
                String::new()
            };
            issues.push(Issue {
                ignored: ign.contains(&name.to_lowercase()),
                kept: kept.contains(&name.to_lowercase()),
                candidates: st["candidates"]
                    .as_array()
                    .map(|a| a.iter().take(2).cloned().collect())
                    .unwrap_or_default(),
                status: status.into(),
                trial,
                preset: rf["preset"].clone(),
                need: st["need"]["version"].as_str().unwrap_or("").into(),
                need_label: st["need"]["label"].as_str().unwrap_or("").into(),
                n: 1,
                name,
                format,
            });
        }
        issues.sort_by_key(|i| -severity(&i.status));
        let (k, src) = song_key(None, d);
        let missing = d["samples_missing"].as_array().cloned().unwrap_or_default();
        let found = |x: &Value| x["found_at"].as_str().is_some_and(|s| !s.is_empty());
        files.push(FileItem {
            rel: r.rel.clone(),
            name: base_name(&r.rel),
            mtime: r.mtime,
            ctime: r.ctime,
            latest: newest_rel.as_deref() == Some(r.rel.as_str()),
            sev: r.sev,
            promoted: r.promoted,
            tempo: d["tempo"].as_f64(),
            len: d["length_s"].as_i64(),
            key: if src == Some("Live") { k } else { None },
            key_est: sure_estimate(d),
            key_bass: d["key_bass"].as_bool().unwrap_or(false),
            key_raw: crate::keys::norm_key(d["key_est"].as_str()),
            key_conf: d["key_conf"].as_f64(),
            time_sig: opt_str(&d["time_sig"]),
            live: opt_str(&d["live_version"]),
            tracks: d["tracks"].clone(),
            error: opt_str(&d["error"]).filter(|e| !e.is_empty()),
            issues,
            samples_lost: missing
                .iter()
                .filter(|x| !found(x))
                .take(30)
                .map(|x| x["path"].as_str().unwrap_or("").to_string())
                .collect(),
            samples_moved: missing.iter().filter(|x| found(x)).count() as i64,
            samples_total: d["samples_total"].as_i64().unwrap_or(0),
        });
    }
    let main = active.first().copied().or(rows.first());
    let mut devices = Devices::default();
    if let Some(m) = main {
        let pick = |devices: &mut Devices, kind: &str, x: Device| match kind {
            "instr" => devices.instr.push(x),
            "midifx" => devices.midi.push(x),
            _ => devices.fx.push(x),
        };
        for x in m.data["devices"].as_array().into_iter().flatten() {
            let dev = Device {
                name: x["name"].as_str().unwrap_or("").into(),
                count: x["count"].as_i64().unwrap_or(0),
                native: true,
                status: None,
                key: None,
                instead: None,
                ignored: false,
            };
            pick(&mut devices, x["kind"].as_str().unwrap_or(""), dev);
        }
        // plugins by use, first seen first among equals (Counter.most_common)
        let mut order: Vec<String> = Vec::new();
        let mut count: HashMap<String, i64> = HashMap::new();
        let mut kinds: HashMap<String, (String, String)> = HashMap::new();
        let mut extra: HashMap<String, (String, Option<String>)> = HashMap::new();
        let ignored: Vec<String> =
            setting::<Vec<String>>(con, "ignored").unwrap_or_default().iter().map(|n| n.to_lowercase()).collect();
        let refs = m.data["plugins"].as_array().cloned().unwrap_or_default();
        let sts = m.plugstate.as_array().cloned().unwrap_or_default();
        for (rf, st) in refs.iter().zip(sts.iter()) {
            let name = rf["name"].as_str().unwrap_or("").to_string();
            if !count.contains_key(&name) {
                order.push(name.clone());
            }
            *count.entry(name.clone()).or_default() += 1;
            let status = st["status"].as_str().unwrap_or("").to_string();
            let fmt = rf["format"].as_str().unwrap_or("").to_string();
            let instead = st["candidates"].as_array().and_then(|a| a.first()).map(|c| {
                let v = c["version"].as_str().filter(|v| !v.is_empty() && *v != "1").map(|v| format!(" v{v}")).unwrap_or_default();
                format!("{} ({}{v})", c["name"].as_str().unwrap_or(""), c["format"].as_str().unwrap_or("").to_uppercase())
            });
            extra.insert(name.clone(), (format!("{fmt}|{name}|{status}"), instead));
            kinds.insert(name, (st["kind"].as_str().unwrap_or("").to_string(), status));
        }
        order.sort_by_key(|n| -count[n]);
        for name in order {
            let (kind, status) = kinds[&name].clone();
            let (key, instead) = extra.remove(&name).unwrap_or_default();
            let dev = Device {
                count: count[&name],
                native: false,
                ignored: ignored.contains(&name.to_lowercase()),
                status: Some(status),
                key: Some(key),
                instead,
                name,
            };
            pick(&mut devices, &kind, dev);
        }
        for v in [&mut devices.instr, &mut devices.fx, &mut devices.midi] {
            v.sort_by_key(|x| -x.count);
        }
    }
    let artists: Vec<String> = setting(con, "artists").unwrap_or_default();
    let words: Vec<String> = setting(con, "work_words").unwrap_or_default();
    let rules = Rules::new(&artists, &words);
    let known: std::collections::HashSet<String> = {
        let mut s = con.prepare("select distinct song_dir from songs where gone=0 and song_dir is not null")?;
        s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    let audio: Vec<(Audio, String)> = {
        let mut s = con.prepare("select rel, size, mtime from audio")?;
        let prefix = format!("{song_dir}/");
        s.query_map([], |r| {
            Ok(Audio {
                rel: r.get(0)?,
                size: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                mtime: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            })
        })?
        .filter_map(|a| match a {
            Ok(a) if a.rel.starts_with(MP3_DIR) || song_dir.is_empty() || a.rel.starts_with(&prefix) => {
                let d = song_dir_known(&a.rel, &rules, &known);
                Some(Ok((a, d)))
            }
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<_, _>>()?
    };
    let shared: i64 =
        con.query_row("select count(*) from songs where song_dir=? and gone=0", [&song.song_dir], |r| r.get(0))?;
    let shared = shared > 1;
    // in a folder shared by several songs: this song's project sub-folders, and the others'
    let own = catalog::own_dirs(&song_dir, &rows.iter().map(|r| r.rel.as_str()).collect::<Vec<_>>());
    let foreign: Vec<String> = {
        let mut s = con.prepare(
            "select f.rel from files f join songs o on o.id = f.song_id where o.song_dir = ? and o.gone = 0 and o.id != ?",
        )?;
        let rels: Vec<String> =
            s.query_map(rusqlite::params![song.song_dir, sid], |r| r.get(0))?.collect::<Result<_, _>>()?;
        catalog::own_dirs(&song_dir, &rels.iter().map(String::as_str).collect::<Vec<_>>())
            .into_iter()
            .filter(|d| !own.contains(d))
            .collect()
    };
    let cands = catalog::temoin_candidates_owned(
        &rules,
        &song_dir,
        song.vkey.as_deref().unwrap_or(""),
        song.title.as_deref().unwrap_or(""),
        &audio,
        main.map(|m| m.mtime).unwrap_or(0.0),
        shared,
        &own,
        &foreign,
    );
    let other_audio: Vec<Candidate> = {
        let mut v: Vec<Candidate> = audio
            .iter()
            // in a shared folder only what lies in this song's own project folders; a lone
            // song's folder is all its own
            .filter(|(a, d)| {
                *d == song_dir
                    && !a.rel.starts_with(MP3_DIR)
                    && !cands.iter().any(|c| c.rel == a.rel)
                    && (!(shared || song_dir.is_empty()) || own.iter().any(|o| a.rel.starts_with(&format!("{o}/"))))
            })
            .map(|(a, _)| Candidate {
                rel: a.rel.clone(),
                place: "dossier".into(),
                mtime: a.mtime,
                size: a.size,
                kind: String::new(),
                pickable: a.size >= 200_000,
                fresh: false,
            })
            .collect();
        v.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(80);
        v
    };
    // the projects saved closest in time to the reference render (old versions and the other songs of the
    // folder included): an old render often matches an old version, not the latest file
    let mut near = Vec::new();
    let tem = song.temoin.as_deref().filter(|t| !t.is_empty()).and_then(|t| {
        let t2 = t.replace('\\', "/");
        cands.iter().find(|c| t == c.rel || t2.ends_with(&format!("/{}", c.rel)))
    });
    if let Some(tem) = tem {
        let scope = if song_dir.is_empty() {
            format!("{}/", tem.rel.rsplit_once('/').map(|(d, _)| d).unwrap_or(&tem.rel))
        } else {
            format!("{song_dir}/")
        };
        let mut s = con.prepare(
            "select f.rel, f.mtime, f.song_id, f.in_version, o.title from files f \
             left join songs o on o.id = f.song_id where substr(f.rel, 1, ?) = ?",
        )?;
        let mut rows2: Vec<NearRow> = s
            .query_map(rusqlite::params![scope.chars().count() as i64, scope], |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    r.get(2)?,
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                    r.get(4)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        rows2.sort_by(|a, b| {
            (a.1 - tem.mtime).abs().partial_cmp(&(b.1 - tem.mtime).abs()).unwrap_or(std::cmp::Ordering::Equal)
        });
        near = rows2
            .into_iter()
            .take(3)
            .map(|(rel, mtime, song_id, old, title)| Near {
                name: base_name(&rel),
                days: format!("{:.1}", (mtime - tem.mtime) / 86400.0).parse().unwrap_or(0.0),
                same: song_id == Some(sid),
                rel,
                song_id,
                title,
                old,
            })
            .collect();
    }
    let suggestions: Vec<OpenStage> = {
        let mut s = con.prepare(
            "select id, value, reasons, created from suggestions where song_id=? and state='open' and kind='stage'",
        )?;
        s.query_map([sid], |r| {
            Ok(OpenStage {
                id: r.get(0)?,
                value: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                reasons: serde_json::from_str(&r.get::<_, Option<String>>(2)?.unwrap_or_default()).unwrap_or_default(),
                created: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?
    };
    let style_sugg = con
        .query_row(
            "select id, reasons from suggestions where song_id=? and state='open' and kind='style' order by id desc limit 1",
            [sid],
            |r| {
                Ok(OpenStyle {
                    id: r.get(0)?,
                    options: serde_json::from_str(&r.get::<_, Option<String>>(1)?.unwrap_or_default())
                        .unwrap_or_default(),
                })
            },
        )
        .optional()?;
    let siblings: Vec<Sibling> = {
        let mut s = con.prepare("select id, title from songs where song_dir=? and gone=0 and id != ?")?;
        s.query_map(rusqlite::params![song.song_dir, sid], |r| Ok(Sibling { id: r.get(0)?, title: r.get(1)? }))?
            .collect::<Result<_, _>>()?
    };
    let backups: i64 = con.query_row(
        "select coalesce(sum(n), 0) from backups where dir=? or dir like ?",
        rusqlite::params![song.song_dir, format!("{song_dir}/%")],
        |r| r.get(0),
    )?;
    let versions: Vec<Version> = rows
        .iter()
        .filter(|r| r.in_version)
        .map(|r| Version { rel: r.rel.clone(), name: base_name(&r.rel), mtime: r.mtime })
        .collect();
    let dirs: Vec<String> = {
        let mut s = con.prepare("select distinct song_dir from songs where gone=0")?;
        s.query_map([], |r| r.get::<_, Option<String>>(0))?
            .map(|r| r.map(Option::unwrap_or_default))
            .collect::<Result<_, _>>()?
    };
    let roots = store::Roots::load(con);
    let board = match roots.split(&song_dir) {
        Some((r, _)) => list_board_labeled(
            Path::new(&r.path),
            &r.label,
            &rules,
            &song_dir,
            song.vkey.as_deref().unwrap_or(""),
            song.title.as_deref().unwrap_or(""),
            shared,
            &other_song_prefixes(&dirs, &song_dir),
        ),
        None => Default::default(),
    };
    // the virtual mood board: files linked to it (where they are), items set aside (hidden)
    let mut board = board;
    let key = format!("{}|{}", song_dir, song.vkey.as_deref().unwrap_or(""));
    let list_of = |k: &str| -> Vec<String> {
        setting::<Value>(con, k)
            .and_then(|v| v.get(&key).and_then(|x| x.as_array().cloned()))
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let hidden = list_of("board_hidden");
    board.items.retain(|i| !hidden.contains(&i.rel));
    board.found.retain(|i| !hidden.contains(&i.rel));
    for rel in list_of("board_links") {
        if board.items.iter().any(|i| i.rel == rel) {
            continue;
        }
        let Some(p) = roots.abs(&rel).filter(|p| p.exists()) else { continue };
        let Ok(m) = std::fs::metadata(&p) else { continue };
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let kind = if m.is_dir() { "dossier" } else { catalog::kind_of(&name) }.to_string();
        let (w, h) = if kind == "image" { catalog::image_size(&p).map(|(w, h)| (Some(w), Some(h))).unwrap_or((None, None)) } else { (None, None) };
        board.items.push(catalog::Item {
            name,
            rel: rel.clone(),
            kind,
            size: m.len(),
            mtime: m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0),
            place: "lien".into(),
            w,
            h,
            text: None,
        });
    }
    let merges: Vec<(String, Value)> = setting::<Value>(con, "merges")
        .and_then(|v| v.as_object().cloned())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default();
    let merged: Vec<Merged> = merges
        .into_iter()
        .filter(|(_, target)| target.as_i64() == Some(sid))
        .map(|(key, _)| {
            let (d, vk) = key.rsplit_once('|').unwrap_or((&key, ""));
            let f = rows.iter().find(|r| r.song_dir.as_deref() == Some(d) && r.vkey.as_deref() == Some(vk));
            let name = f.map(|f| base_name(&f.rel)).unwrap_or_else(|| vk.to_string());
            Merged { key: key.clone(), name }
        })
        .collect();
    let log: Vec<StageStep> = {
        let mut s = con.prepare("select stage, at from stage_log where song_id=? order by at")?;
        s.query_map([sid], |r| Ok(StageStep { stage: r.get(0)?, at: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0) }))?
            .collect::<Result<_, _>>()?
    };
    let main_chain: Vec<String> = main
        .and_then(|m| m.data["main_chain"].as_array().cloned())
        .map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
        .unwrap_or_default();
    Ok(Some(SongDetail {
        checklist: checklist(checkauto.as_deref(), checks.as_deref()),
        board,
        shared,
        backups,
        versions,
        merged,
        hidden: (rows.len() - active.len()) as i64,
        files,
        main_chain,
        devices,
        temoins: cands,
        other_audio,
        suggestions,
        style_sugg,
        near_temoin: near,
        siblings,
        log,
        song,
    }))
}

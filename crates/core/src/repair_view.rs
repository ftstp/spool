//! The Repair page: what is missing across the active projects, what to install first,
//! what the app repairs by itself. Ported from the prototype's views.repair.
use crate::repairs::{Fixable, fixable};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use store::Store;
use store::rusqlite;
use ts_rs::TS;

const YEAR: f64 = 365.25 * 86400.0;

fn stage_weight(s: Option<&str>) -> f64 {
    match s {
        Some("done") | Some("master") => 5.0,
        Some("mix") => 4.0,
        Some("arrangement") => 3.0,
        None => 2.0,
        Some("draft") => 1.0,
        Some("pause") => 0.5,
        Some(_) => 2.0,
    }
}

use plugins::status_severity as severity;

/// A counter keeping first-seen order among equals (Python's Counter.most_common).
#[derive(Default)]
struct Counter(Vec<(String, i64)>);

impl Counter {
    fn add(&mut self, k: &str, n: i64) {
        match self.0.iter_mut().find(|(x, _)| x == k) {
            Some(slot) => slot.1 += n,
            None => self.0.push((k.to_string(), n)),
        }
    }
    fn most_common(&self, n: usize) -> Vec<(String, i64)> {
        let mut v = self.0.clone();
        v.sort_by_key(|(_, c)| -c);
        v.truncate(n);
        v
    }
}

/// One plugin a project cannot load as saved (format + name + status), over all projects.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PluginNeed {
    pub key: String,
    pub name: String,
    pub format: String,
    pub status: String,
    pub kind: String,
    #[ts(type = "unknown[]")]
    pub candidates: Vec<Value>,
    pub vendor: String,
    #[ts(type = "unknown[]")]
    pub old_formats: Vec<Value>,
    pub old_version: String,
    pub old_path: String,
    pub on_disk: bool,
    /// Songs using it.
    #[ts(type = "number[]")]
    pub songs: Vec<i64>,
    /// Projects using it.
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number")]
    pub n: i64,
    #[ts(type = "number")]
    pub instances: i64,
    #[ts(type = "Record<string, number>")]
    pub stages: HashMap<String, i64>,
    /// The versions asked for, most used first: [version, count].
    #[ts(type = "[string, number][]")]
    pub needs: Vec<(String, i64)>,
    pub need_source: String,
    pub filenames: Vec<String>,
    #[ts(type = "unknown[]")]
    pub bundles: Vec<Value>,
    pub any: bool,
    pub need_label: String,
    /// Presets it was saved with: [label, count].
    #[ts(type = "[string, number][]")]
    pub presets: Vec<(String, i64)>,
    #[ts(type = "Record<string, string[]>")]
    pub song_presets: HashMap<String, Vec<String>>,
    /// Mac plugin instances whose state can be carried over to a VST2.
    #[ts(type = "number")]
    pub au_ok: i64,
    /// The installed version chosen to replace it, if any (not for an ignored plugin).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trial: Option<String>,
    /// Installed versions it could be replaced with (not for an ignored plugin).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trial_targets: Option<Vec<String>>,
    /// The installed VST3 it is replaced by in repaired copies (its class id), if chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vst3: Option<String>,
    /// Installed VST3s it could be replaced by (a VST2 not installed, or not in this version).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vst3_targets: Option<Vec<Vst3Target>>,
    /// Projects where an envelope moves it: a switch to the VST3 would lose that automation,
    /// so those keep waiting for the VST2.
    #[ts(type = "number")]
    pub automated: i64,
}

/// An installed VST3 a VST2 can become.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Vst3Target {
    pub name: String,
    /// Its class id, as Live's database writes it.
    pub uid: String,
    pub kind: String,
    /// It declares it replaces the VST2 (class id "VST" + the VST2 id + its name): the
    /// settings come through. Otherwise: to try, and listen.
    pub compatible: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct InstallStep {
    pub key: String,
    pub name: String,
    pub format: String,
    /// Projects needing it.
    #[ts(type = "number")]
    pub files: i64,
    /// Projects complete once it and the ones before are installed.
    #[ts(type = "number")]
    pub complete: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Vendor {
    pub vendor: String,
    #[ts(type = "number")]
    pub plugins: i64,
    #[ts(type = "number")]
    pub to_install: i64,
    #[ts(type = "number")]
    pub songs: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SampleCounts {
    #[ts(type = "number")]
    pub moved: i64,
    #[ts(type = "number")]
    pub by_name: i64,
    #[ts(type = "number")]
    pub lost: i64,
    #[ts(type = "number")]
    pub total: i64,
}

/// A folder lost samples come from.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SampleRoot {
    pub root: String,
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number")]
    pub songs: i64,
    /// The same folder on the previous system's drive (looked at already).
    pub old: Option<String>,
    pub old_exists: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeepMeta {
    pub root: Option<String>,
    pub at: Option<f64>,
    #[ts(type = "number | null")]
    pub files: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct RepairView {
    pub fixable: Vec<Fixable>,
    #[ts(type = "number")]
    pub files: i64,
    /// Projects by severity: opens, repairable, settings lost, something missing.
    #[ts(type = "number[]")]
    pub sev: Vec<i64>,
    #[ts(type = "number")]
    pub complete_now: i64,
    pub plugins: Vec<PluginNeed>,
    pub ignored: Vec<PluginNeed>,
    pub ignored_keep: Vec<String>,
    pub order: Vec<InstallStep>,
    #[ts(type = "number")]
    pub need_any: i64,
    pub vendors: Vec<Vendor>,
    /// [format, loads, count]
    #[ts(type = "[string, boolean, number][]")]
    pub formats: Vec<(String, bool, i64)>,
    pub samples: SampleCounts,
    pub roots: Vec<SampleRoot>,
    pub deep: Option<DeepMeta>,
    #[ts(type = "Record<string, string | null>")]
    pub titles: HashMap<String, Option<String>>,
    #[ts(type = "number | null")]
    pub years: Option<i64>,
    /// Projects by age in whole years: [years, count].
    #[ts(type = "[number, number][]")]
    pub ages: Vec<(i64, i64)>,
    #[ts(type = "number")]
    pub all_files: i64,
}

struct F {
    rel: String,
    song_id: i64,
    mtime: f64,
    sev: i64,
    stage: Option<String>,
    data: Value,
    states: Value,
}

struct Acc {
    pl: PluginNeed,
    songs: BTreeSet<i64>,
    files: HashSet<usize>,
    auto: HashSet<usize>,
    stages: Counter,
    needs: Counter,
    filenames: Counter,
    presets: Counter,
    song_presets: Vec<(i64, BTreeSet<String>)>,
}

fn s(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(t) => !t.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `years`: only projects saved in the last N years, so the install list answers "what to
/// install to get my RECENT projects back". `deep_file`: the old-drive search's result.
pub fn repair_view(st: &Store, years: Option<i64>, deep_file: &Path) -> rusqlite::Result<RepairView> {
    let mut files: Vec<F> = {
        let mut q = st.con.prepare(
            "select f.rel, f.song_id, f.mtime, f.sev, s.stage, f.data, f.plugstate from files f join songs s on s.id = f.song_id \
             where f.in_version = 0 and f.archived = 0 and s.gone = 0 and s.archived = 0",
        )?;
        q.query_map([], |r| {
            Ok(F {
                rel: r.get(0)?,
                song_id: r.get(1)?,
                mtime: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                sev: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                stage: r.get(4)?,
                data: serde_json::from_str(&r.get::<_, Option<String>>(5)?.unwrap_or_default()).unwrap_or(Value::Null),
                states: serde_json::from_str(&r.get::<_, Option<String>>(6)?.unwrap_or_default())
                    .unwrap_or(Value::Null),
            })
        })?
        .collect::<Result<_, _>>()?
    };
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let old_drive: Option<String> =
        st.get::<String>("old_drive").map(|d| d.trim_end_matches(':').to_uppercase()).filter(|d| !d.is_empty());
    let mut ages: Vec<(i64, i64)> = Vec::new();
    for f in &files {
        let a = (((now - f.mtime) / YEAR).floor() as i64).min(30);
        match ages.iter_mut().find(|(x, _)| *x == a) {
            Some(slot) => slot.1 += 1,
            None => ages.push((a, 1)),
        }
    }
    let all_files = files.len() as i64;
    if let Some(y) = years.filter(|y| *y > 0) {
        files.retain(|f| now - f.mtime <= y as f64 * YEAR);
    }
    let low = |k: &str| -> HashSet<String> {
        st.get::<Vec<String>>(k).unwrap_or_default().into_iter().map(|x| x.to_lowercase()).collect()
    };
    let ign = low("ignored");
    let mut plugins: Vec<Acc> = Vec::new();
    let mut ignored_pl: Vec<Acc> = Vec::new();
    let mut formats: Vec<((String, bool), i64)> = Vec::new();
    let mut roots: Vec<(String, BTreeSet<String>, HashSet<i64>)> = Vec::new();
    let mut used_v2: HashSet<String> = HashSet::new();
    // each VST2's unique id (a VST3 that replaces it carries it in its class id)
    let mut v2_id: HashMap<String, i64> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        let refs = f.data["plugins"].as_array().cloned().unwrap_or_default();
        let sts = f.states.as_array().cloned().unwrap_or_default();
        for (r, stv) in refs.iter().zip(sts.iter()) {
            let (fmt, name, status) = (s(&r["format"]), s(&r["name"]), s(&stv["status"]));
            let fk = (fmt.clone(), status == "ok");
            match formats.iter_mut().find(|(k, _)| *k == fk) {
                Some(slot) => slot.1 += 1,
                None => formats.push((fk, 1)),
            }
            if fmt == "vst2" && truthy(&r["id"]) {
                used_v2.insert(s(&r["id"]));
                if let Ok(n) = s(&r["id"]).parse::<i64>() {
                    v2_id.entry(name.to_lowercase()).or_insert(n);
                }
            }
            if status == "ok" {
                continue;
            }
            let key = format!("{fmt}|{name}|{status}");
            let target = if ign.contains(&name.to_lowercase()) { &mut ignored_pl } else { &mut plugins };
            let idx = match target.iter().position(|a| a.pl.key == key) {
                Some(j) => j,
                None => {
                    let old = &stv["old"];
                    let cands = stv["candidates"].as_array().cloned().unwrap_or_default();
                    let vendor = Some(s(&old["vendor"])).filter(|v| !v.is_empty()).unwrap_or_else(|| {
                        cands.iter().map(|c| s(&c["vendor"])).find(|v| !v.is_empty()).unwrap_or_default()
                    });
                    let need = &stv["need"];
                    let old_path = Some(s(&old["path"])).filter(|p| !p.is_empty()).unwrap_or_else(|| s(&r["path"]));
                    target.push(Acc {
                        pl: PluginNeed {
                            key: key.clone(),
                            name: name.clone(),
                            format: fmt.clone(),
                            status: status.clone(),
                            kind: s(&stv["kind"]),
                            candidates: cands.iter().take(3).cloned().collect(),
                            vendor,
                            old_formats: stv["old_formats"].as_array().cloned().unwrap_or_default(),
                            old_version: s(&old["version"]),
                            old_path,
                            on_disk: old["on_disk"].as_bool().unwrap_or(false),
                            songs: Vec::new(),
                            files: 0,
                            n: 0,
                            instances: 0,
                            stages: HashMap::new(),
                            needs: Vec::new(),
                            need_source: String::new(),
                            filenames: Vec::new(),
                            bundles: need["bundles"].as_array().cloned().unwrap_or_default(),
                            any: truthy(&need["any"]),
                            need_label: s(&need["label"]),
                            presets: Vec::new(),
                            song_presets: HashMap::new(),
                            au_ok: 0,
                            trial: None,
                            trial_targets: None,
                            vst3: None,
                            vst3_targets: None,
                            automated: 0,
                        },
                        songs: BTreeSet::new(),
                        files: HashSet::new(),
                        auto: HashSet::new(),
                        stages: Counter::default(),
                        needs: Counter::default(),
                        filenames: Counter::default(),
                        presets: Counter::default(),
                        song_presets: Vec::new(),
                    });
                    target.len() - 1
                }
            };
            let a = &mut target[idx];
            if truthy(&r["au_state"]) {
                a.pl.au_ok += 1;
            }
            let need = &stv["need"];
            if truthy(&need["version"]) {
                a.needs.add(&s(&need["version"]), 1);
                if a.pl.need_source.is_empty() {
                    a.pl.need_source = s(&need["source"]);
                }
            }
            let file = s(&r["file"]);
            if !file.is_empty() {
                // a Mac bundle name stands for its Windows .dll
                let f2 = if file.ends_with(".vst") { file.replace(".vst", ".dll") } else { file.clone() };
                a.filenames.add(&f2, 1);
            }
            a.songs.insert(f.song_id);
            let pr = &r["preset"];
            let sp = match a.song_presets.iter().position(|(k, _)| *k == f.song_id) {
                Some(j) => j,
                None => {
                    a.song_presets.push((f.song_id, BTreeSet::new()));
                    a.song_presets.len() - 1
                }
            };
            if truthy(pr) && !pr["default"].as_bool().unwrap_or(false) {
                let bank = s(&pr["bank"]);
                let label = s(&pr["name"]) + &if bank.is_empty() { String::new() } else { format!(" [{bank}]") };
                a.presets.add(&label, 1);
                a.song_presets[sp].1.insert(label);
            } else if truthy(pr) {
                a.song_presets[sp].1.insert(i18n::tr!("repair.defaultPreset"));
            } else if a.song_presets[sp].1.is_empty() {
                // Python's defaultdict only creates the entry when something is added
                a.song_presets.remove(sp);
            }
            a.files.insert(i);
            if r["automated"].as_bool() == Some(true) {
                a.auto.insert(i);
            }
            a.pl.instances += 1;
        }
        for smp in f.data["samples_missing"].as_array().into_iter().flatten() {
            if truthy(&smp["found_at"]) {
                continue;
            }
            let path = s(&smp["path"]);
            let parts: Vec<&str> = path.split(['/', '\\']).collect();
            let low = path.to_lowercase();
            let root = match low.find("/user library/") {
                Some(i) => path[..i + "/user library".len()].to_string(),
                None if parts.len() > 5 => parts[..5].join("/"),
                None => parts[..parts.len().saturating_sub(1)].join("/"),
            };
            let j = match roots.iter().position(|(r, _, _)| *r == root) {
                Some(j) => j,
                None => {
                    roots.push((root, BTreeSet::new(), HashSet::new()));
                    roots.len() - 1
                }
            };
            roots[j].1.insert(path);
            roots[j].2.insert(f.song_id);
        }
    }
    let stage_of: HashMap<i64, Option<String>> = {
        let mut q = st.con.prepare("select id, stage from songs where gone = 0 and archived = 0")?;
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?
    };
    for a in plugins.iter_mut().chain(ignored_pl.iter_mut()) {
        for sid in &a.songs {
            let k = stage_of.get(sid).cloned().flatten().unwrap_or_else(|| "none".into());
            a.stages.add(&k, 1);
        }
    }

    // a version found nowhere can still be TRIED with an installed VST2 of the same family
    let inst2: Vec<plugins::Installed> = {
        let db = plugins::live_db();
        if db.exists() { plugins::load_installed(&db, true).unwrap_or_default() } else { Vec::new() }
    }
    .into_iter()
    .collect();
    let inst3: Vec<plugins::Installed> = inst2.iter().filter(|x| x.format == "vst3").cloned().collect();
    let inst2: Vec<plugins::Installed> = inst2.into_iter().filter(|x| x.format == "vst2").collect();
    let to_vst3: serde_json::Map<String, Value> =
        st.get::<Value>("to_vst3").and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let trials: serde_json::Map<String, Value> =
        st.get::<Value>("trials").and_then(|v| v.as_object().cloned()).unwrap_or_default();
    if plugins.iter().any(|a| a.pl.format == "au") {
        // old versions count too: any saved VST2 device can serve as the template
        let mut q = st.con.prepare("select data from files where in_version=1")?;
        for d in q.query_map([], |r| r.get::<_, Option<String>>(0))? {
            let d: Value = serde_json::from_str(&d?.unwrap_or_default()).unwrap_or(Value::Null);
            for p in d["plugins"].as_array().into_iter().flatten() {
                if p["format"] == "vst2" && truthy(&p["id"]) {
                    used_v2.insert(s(&p["id"]));
                }
            }
        }
    }
    for a in plugins.iter_mut() {
        let pl = &mut a.pl;
        pl.trial = Some(trials.get(&plugins::trial_key(&pl.format, &pl.name)).map(|t| s(&t["to"])).unwrap_or_default());
        pl.trial_targets = Some(Vec::new());
        pl.vst3 = to_vst3.get(&pl.name.to_lowercase()).map(|t| s(&t["uid"]));
        pl.vst3_targets = Some(Vec::new());
        if pl.format == "vst2" && (severity(&pl.status) >= 2 || pl.status == "vst3") {
            let fam = plugins::family_name(&pl.name);
            let uid2 = v2_id.get(&pl.name.to_lowercase()).copied();
            let mut seen = BTreeSet::new();
            let t: Vec<Vst3Target> = inst3
                .iter()
                .filter(|x| !fam.is_empty() && plugins::family_name(&x.name) == fam && seen.insert(x.id.clone()))
                .map(|x| {
                    let id = repair::vst3::class_id(&x.id).unwrap_or([0; 16]);
                    // its class id is "VST" + this VST2's own unique id
                    let compatible = uid2.is_some_and(|u| repair::vst3::compatible(&id, u));
                    Vst3Target { name: x.name.clone(), uid: x.id.clone(), kind: x.kind.clone(), compatible }
                })
                .filter(|t: &Vst3Target| t.compatible)
                .collect();
            pl.vst3_targets = Some(t);
        }
        if pl.format == "vst2" && (severity(&pl.status) >= 2 || pl.status == "trial") {
            let fam = plugins::family_name(&pl.name);
            let set: BTreeSet<String> = inst2
                .iter()
                .filter(|x| {
                    !fam.is_empty()
                        && plugins::family_name(&x.name) == fam
                        && x.name.to_lowercase() != pl.name.to_lowercase()
                })
                .map(|x| x.name.clone())
                .collect();
            pl.trial_targets = Some(set.into_iter().collect());
        } else if pl.format == "au" && pl.au_ok > 0 && matches!(pl.status.as_str(), "missing" | "trial") {
            let base = strip_fx(&pl.name);
            let fam = plugins::family_name(&base);
            let set: BTreeSet<String> = inst2
                .iter()
                .filter(|x| !fam.is_empty() && plugins::family_name(&x.name) == fam && used_v2.contains(&x.id))
                .map(|x| x.name.clone())
                .collect();
            pl.trial_targets = Some(set.into_iter().collect());
        }
    }
    let finish = |v: Vec<Acc>| -> Vec<PluginNeed> {
        let mut out: Vec<PluginNeed> = v
            .into_iter()
            .map(|a| {
                let mut pl = a.pl;
                pl.n = a.songs.len() as i64;
                pl.songs = a.songs.into_iter().collect();
                pl.files = a.files.len() as i64;
                pl.automated = a.auto.len() as i64;
                pl.stages = a.stages.0.into_iter().collect();
                pl.needs = a.needs.most_common(4);
                pl.filenames = a.filenames.most_common(2).into_iter().map(|(n, _)| n).collect();
                pl.presets = a.presets.most_common(usize::MAX);
                pl.song_presets =
                    a.song_presets.into_iter().map(|(k, v)| (k.to_string(), v.into_iter().collect())).collect();
                pl
            })
            .collect();
        out.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        out
    };

    // install order: greedily the absent plugin that completes the most WEIGHTED files (a file
    // of a song in mix outweighs a draft), ties by files touched
    let need: Vec<(Vec<String>, f64)> = files
        .iter()
        .map(|f| {
            let refs = f.data["plugins"].as_array().cloned().unwrap_or_default();
            let sts = f.states.as_array().cloned().unwrap_or_default();
            let mut ks: Vec<String> = Vec::new();
            for (r, stv) in refs.iter().zip(sts.iter()) {
                let (fmt, name) = (s(&r["format"]), s(&r["name"]));
                if stv["status"] == "missing" && fmt != "au" && !ign.contains(&name.to_lowercase()) {
                    let k = format!("{fmt}|{name}");
                    if !ks.contains(&k) {
                        ks.push(k);
                    }
                }
            }
            (ks, stage_weight(f.stage.as_deref()))
        })
        .collect();
    let mut touched = Counter::default();
    for (ks, _) in &need {
        for k in ks {
            touched.add(k, 1);
        }
    }
    let mut remaining: Vec<HashSet<String>> = need.iter().map(|(k, _)| k.iter().cloned().collect()).collect();
    let weights: Vec<f64> = need.iter().map(|(_, w)| *w).collect();
    let mut have: HashSet<String> = HashSet::new();
    let mut order = Vec::new();
    let mut done = 0;
    while have.len() < touched.0.len() {
        let mut single: HashMap<&str, f64> = HashMap::new();
        for (sset, w) in remaining.iter().zip(&weights) {
            if sset.len() == 1 {
                *single.entry(sset.iter().next().unwrap().as_str()).or_default() += w;
            }
        }
        // max() keeps the first of equals, in the counter's order
        let mut best: Option<(&String, f64, i64)> = None;
        for (k, n) in &touched.0 {
            if have.contains(k) {
                continue;
            }
            let sc = single.get(k.as_str()).copied().unwrap_or(0.0);
            if best.is_none_or(|(_, bs, bn)| (sc, *n) > (bs, bn)) {
                best = Some((k, sc, *n));
            }
        }
        let (k, _, n) = best.unwrap();
        let k = k.clone();
        drop(single);
        have.insert(k.clone());
        for sset in remaining.iter_mut() {
            if sset.remove(&k) && sset.is_empty() {
                done += 1;
            }
        }
        let (fmt, name) = k.split_once('|').unwrap_or(("", &k));
        order.push(InstallStep {
            key: format!("{k}|missing"),
            name: name.into(),
            format: fmt.into(),
            files: n,
            complete: done,
        });
    }

    let mut vendors: Vec<(String, i64, i64, HashSet<i64>)> = Vec::new();
    for a in &plugins {
        let name = if a.pl.vendor.is_empty() { i18n::tr!("repair.unknownVendor") } else { a.pl.vendor.clone() };
        let j = match vendors.iter().position(|v| v.0 == name) {
            Some(j) => j,
            None => {
                vendors.push((name, 0, 0, HashSet::new()));
                vendors.len() - 1
            }
        };
        vendors[j].1 += 1;
        vendors[j].3.extend(a.songs.iter().copied());
        if severity(&a.pl.status) >= 2 {
            vendors[j].2 += 1;
        }
    }
    let mut vendors: Vec<Vendor> = vendors
        .into_iter()
        .map(|(vendor, plugins, to_install, songs)| Vendor { vendor, plugins, to_install, songs: songs.len() as i64 })
        .collect();
    vendors.sort_by_key(|v| -v.songs);
    let sev = (0..4).map(|i| files.iter().filter(|f| f.sev == i).count() as i64).collect();
    let fixable =
        fixable(st)?.into_iter().filter(|x| years.is_none_or(|_| files.iter().any(|f| f.rel == x.rel))).collect();
    let miss = |f: &F| f.data["samples_missing"].as_array().cloned().unwrap_or_default();
    let samples = SampleCounts {
        moved: files.iter().map(|f| miss(f).iter().filter(|x| truthy(&x["found_at"])).count() as i64).sum(),
        by_name: files
            .iter()
            .map(|f| miss(f).iter().filter(|x| truthy(&x["found_at"]) && truthy(&x["by_name"])).count() as i64)
            .sum(),
        lost: files.iter().map(|f| miss(f).iter().filter(|x| !truthy(&x["found_at"])).count() as i64).sum(),
        total: files.iter().map(|f| f.data["samples_total"].as_i64().unwrap_or(0)).sum(),
    };
    let mut roots: Vec<SampleRoot> = roots
        .into_iter()
        .map(|(r, fs, ps)| {
            let old = als::old_path(&r, old_drive.as_deref());
            SampleRoot {
                files: fs.len() as i64,
                songs: ps.len() as i64,
                old_exists: old.as_ref().is_some_and(|p| p.is_dir()),
                old: old.map(|p| p.to_string_lossy().into_owned()),
                root: r,
            }
        })
        .collect();
    roots.sort_by_key(|r| -r.files);
    let deep = std::fs::read_to_string(deep_file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|d| {
        DeepMeta { root: d["root"].as_str().map(str::to_string), at: d["at"].as_f64(), files: d["files"].as_i64() }
    });
    let titles: HashMap<String, Option<String>> = {
        let mut q = st.con.prepare("select id, title from songs where gone = 0 and archived = 0")?;
        q.query_map([], |r| Ok((r.get::<_, i64>(0)?.to_string(), r.get(1)?)))?.collect::<Result<_, _>>()?
    };
    ages.sort();
    Ok(RepairView {
        fixable,
        files: files.len() as i64,
        sev,
        complete_now: need.iter().filter(|(k, _)| k.is_empty()).count() as i64,
        need_any: need.iter().filter(|(k, _)| !k.is_empty()).count() as i64,
        plugins: finish(plugins),
        ignored: finish(ignored_pl),
        ignored_keep: st
            .get::<Vec<String>>("ignored_keep")
            .unwrap_or_default()
            .into_iter()
            .map(|x| x.to_lowercase())
            .collect(),
        order,
        vendors,
        formats: formats.into_iter().map(|((f, ok), n)| (f, ok, n)).collect(),
        samples,
        roots,
        deep,
        titles,
        years: years.filter(|y| *y > 0),
        ages,
        all_files,
    })
}

/// "Guitar Rig 5 FX" -> "Guitar Rig 5": an AU effect's VST2 has no " FX".
fn strip_fx(name: &str) -> String {
    if name.len() > 2 && name.is_char_boundary(name.len() - 2) && name[name.len() - 2..].eq_ignore_ascii_case("fx") {
        let head = &name[..name.len() - 2];
        if head.ends_with(char::is_whitespace) {
            return head.trim_end().to_string();
        }
    }
    name.to_string()
}

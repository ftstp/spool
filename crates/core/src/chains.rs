//! The chains of the user's projects: a chain is a whole track (its instrument, its effects,
//! their presets) as it was used in a song. The same devices in the same order are one chain,
//! seen in several songs. Each is told apart by role (drums, bass, keys...), and drums by
//! part (kick, snare, hats, a whole kit), counted by style, and checked: a chain is only
//! offered when one of its uses still plays here (every plugin installed under the name the
//! set knows, no sample missing).
//!
//! The user's choice per style and part is kept in the setting "chain_prefs":
//! {style: {part: chain key}}. Without a choice, the chain the style uses most is taken.
use crate::insights::Count;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use store::rusqlite::{self, Connection};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChainDevice {
    pub name: String,
    /// instrument / plugin / native / rack / midi
    pub kind: String,
}

/// One track where a chain was used.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChainUse {
    #[ts(type = "number")]
    pub song_id: i64,
    pub title: String,
    pub style: String,
    pub rel: String,
    pub track: String,
    pub track_name: String,
    /// The file format ("12.0_12120") and Live's name for it ("12.1").
    pub format: String,
    pub live: String,
    pub mtime: f64,
    /// Why it would not play here (empty: it plays).
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Chain {
    pub key: String,
    pub role: String,
    /// kick / snare / hats / kit / perc for drums, else the role.
    pub part: String,
    pub devices: Vec<ChainDevice>,
    #[ts(type = "number")]
    pub songs: i64,
    pub styles: Vec<Count>,
    pub uses: Vec<ChainUse>,
    /// At least one use plays here.
    pub playable: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChainCatalogue {
    pub styles: Vec<Count>,
    pub parts: Vec<Count>,
    /// Live versions with the number of playable chains in each ("12.1", n).
    pub versions: Vec<Count>,
    pub chains: Vec<Chain>,
    /// style -> part -> chain key
    #[ts(type = "Record<string, Record<string, string>>")]
    pub prefs: BTreeMap<String, BTreeMap<String, String>>,
}

/// The parts a new project is made of, in track order.
pub const PARTS: [&str; 9] = ["kick", "snare", "hats", "kit", "perc", "bass", "keys", "pad", "lead"];

/// Plugin states Live can load as the set names them (see repair_view): installed, a newer
/// version, a trial. Not: missing, renamed, only its VST3 there.
fn plays(status: &str) -> bool {
    matches!(status, "ok" | "newer" | "trial")
}

static KICK: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(^|[^a-z])(kicks?|kk|bd|bass ?drum)([^a-z]|$)").unwrap());
static SNARE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(^|[^a-z])(snares?|sn\d*|snr|sd|claps?|rims?)([^a-z]|$)").unwrap());
static HATS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)(^|[^a-z])(hats?|hh|hihats?|hi-hats?|ride|cymbals?|cym|tops?|shakers?)([^a-z]|$)").unwrap()
});
static KITS: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)addictive drums|superior drummer|ezdrummer|battery|xo\b|drum ?kit").unwrap());

/// Which part of a new project a track can play.
pub fn part_of(role: &str, name: &str, instrument: &str) -> String {
    match role {
        "drums" | "perc" => {
            let n = name.replace(['_', '-', '.'], " ");
            if KICK.is_match(&n) {
                "kick"
            } else if SNARE.is_match(&n) {
                "snare"
            } else if HATS.is_match(&n) {
                "hats"
            } else if role == "perc" {
                "perc"
            } else if KITS.is_match(instrument) || instrument == "Drum Rack" {
                "kit"
            } else {
                "perc"
            }
        }
        "pluck" => "lead",
        r => r,
    }
    .to_string()
}

/// A song's style: its own field, else the word its folder starts with when that word is a
/// style some song carries ("dnb - Hollow").
pub fn style_of(field: Option<&str>, dir: &str, known: &[String]) -> String {
    if let Some(s) = field.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        return s;
    }
    let leaf = dir.rsplit('/').next().unwrap_or(dir).to_lowercase();
    if let Some((w, _)) = leaf.split_once(" - ") {
        let w = w.trim().to_string();
        if known.contains(&w) {
            return w;
        }
    }
    String::new()
}

fn live_of(info: &Value) -> String {
    info["live_version"].as_str().map(|v| v.split('.').take(2).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// The styles some song carries in its own field (the folder words that count as a style).
pub fn known_styles(con: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut s = con.prepare("select distinct lower(trim(style)) from songs where style is not null and trim(style) <> ''")?;
    s.query_map([], |r| r.get(0))?.collect()
}

pub fn catalogue(con: &Connection, prefs: BTreeMap<String, BTreeMap<String, String>>) -> rusqlite::Result<ChainCatalogue> {
    let known = known_styles(con)?;
    // the newest active project of each song
    let mut s = con.prepare(
        "select f.rel, f.song_id, f.mtime, f.data, f.plugstate, s.title, s.style, s.song_dir from files f \
         join songs s on s.id = f.song_id \
         where f.in_version = 0 and f.archived = 0 and s.archived = 0 and s.gone = 0 and f.rel like '%.als' \
         and f.mtime = (select max(g.mtime) from files g where g.song_id = f.song_id and g.in_version = 0 and g.archived = 0 and g.rel like '%.als')",
    )?;
    type Row = (String, i64, f64, String, String, Option<String>, Option<String>, String);
    let rows: Vec<Row> = s
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .flatten()
        .collect();

    let mut chains: HashMap<String, Chain> = HashMap::new();
    for (rel, song_id, mtime, data, plugstate, title, style, dir) in rows {
        let info: Value = serde_json::from_str(&data).unwrap_or_default();
        let states: Vec<Value> = serde_json::from_str(&plugstate).unwrap_or_default();
        // plugin name -> does it play (a name seen twice plays if one of them does)
        let mut plug_ok: HashMap<String, bool> = HashMap::new();
        for (p, st) in info["plugins"].as_array().into_iter().flatten().zip(states.iter()) {
            let name = p["name"].as_str().unwrap_or("").to_string();
            let ok = plays(st["status"].as_str().unwrap_or(""));
            let e = plug_ok.entry(name).or_insert(false);
            *e = *e || ok;
        }
        let style = style_of(style.as_deref(), &dir, &known);
        let format = info["format"].as_str().unwrap_or("").to_string();
        let live = live_of(&info);
        let title = title.unwrap_or_else(|| dir.rsplit('/').next().unwrap_or(&dir).to_string());
        for t in info["track_info"].as_array().into_iter().flatten() {
            let kind = t["kind"].as_str().unwrap_or("");
            if kind != "midi" {
                continue; // an audio track's sound is its clips; groups and returns are not sounds
            }
            // the sound: the mix tools (EQ, dynamics, gain) are not part of it, and a clean
            // start leaves them out
            let all: Vec<String> = t["chain"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
            let names: Vec<String> = all.iter().filter(|n| !repair::assemble::is_mix_tool(n)).cloned().collect();
            let instrument = t["instrument"].as_str().unwrap_or("").to_string();
            if names.is_empty() || instrument.is_empty() {
                continue;
            }
            let plugins: Vec<String> = t["plugins"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
            let mut problems = Vec::new();
            for p in &plugins {
                // a mix tool's plugin goes with the tool
                if repair::assemble::is_mix_tool(p) && all.contains(p) {
                    continue;
                }
                if !plug_ok.get(p).copied().unwrap_or(false) {
                    problems.push(i18n::tr!("chains.pluginMissing", name = p));
                }
            }
            let missing = t["samples_missing"].as_u64().unwrap_or(0);
            if missing > 0 {
                problems.push(i18n::tr!("chains.samplesOffline", n = missing));
            }
            let role = t["role"].as_str().unwrap_or("other").to_string();
            let tname = t["name"].as_str().unwrap_or("").to_string();
            let part = part_of(&role, &tname, &instrument);
            let key = names.join(" > ");
            let mut seen_inst = false;
            let devices: Vec<ChainDevice> = names
                .iter()
                .map(|n| {
                    let kind = if !seen_inst && *n == instrument {
                        seen_inst = true;
                        "instrument"
                    } else if !seen_inst {
                        "midi"
                    } else if plugins.contains(n) {
                        "plugin"
                    } else if n.ends_with("GroupDevice") || n.contains("Rack") {
                        "rack"
                    } else {
                        "native"
                    };
                    ChainDevice { name: n.clone(), kind: kind.into() }
                })
                .collect();
            let c = chains.entry(format!("{part}|{key}")).or_insert_with(|| Chain {
                key: key.clone(),
                role: role.clone(),
                part: part.clone(),
                devices,
                songs: 0,
                styles: vec![],
                uses: vec![],
                playable: false,
            });
            c.playable |= problems.is_empty();
            c.uses.push(ChainUse {
                song_id,
                title: title.clone(),
                style: style.clone(),
                rel: rel.clone(),
                track: t["id"].as_str().unwrap_or("").to_string(),
                track_name: tname,
                format: format.clone(),
                live: live.clone(),
                mtime,
                problems,
            });
        }
    }

    let mut out: Vec<Chain> = chains
        .into_values()
        .map(|mut c| {
            c.uses.sort_by(|a, b| a.problems.len().cmp(&b.problems.len()).then(b.mtime.total_cmp(&a.mtime)));
            let mut songs: Vec<i64> = c.uses.iter().map(|u| u.song_id).collect();
            songs.sort();
            songs.dedup();
            c.songs = songs.len() as i64;
            let mut st: HashMap<String, i64> = HashMap::new();
            for u in &c.uses {
                if !u.style.is_empty() {
                    *st.entry(u.style.clone()).or_default() += 1;
                }
            }
            c.styles = counts(st);
            c
        })
        .collect();
    out.sort_by(|a, b| b.songs.cmp(&a.songs).then(a.key.cmp(&b.key)));

    let mut styles: HashMap<String, i64> = HashMap::new();
    let mut parts: HashMap<String, i64> = HashMap::new();
    let mut versions: HashMap<String, i64> = HashMap::new();
    for c in &out {
        for s in &c.styles {
            *styles.entry(s.label.clone()).or_default() += s.n;
        }
        *parts.entry(c.part.clone()).or_default() += 1;
        let mut vs: Vec<&str> = c.uses.iter().filter(|u| u.problems.is_empty()).map(|u| u.live.as_str()).collect();
        vs.sort();
        vs.dedup();
        for v in vs {
            *versions.entry(v.to_string()).or_default() += 1;
        }
    }
    let mut versions = counts(versions);
    versions.sort_by(|a, b| b.label.cmp(&a.label));
    Ok(ChainCatalogue { styles: counts(styles), parts: counts(parts), versions, chains: out, prefs })
}

fn counts(m: HashMap<String, i64>) -> Vec<Count> {
    let mut v: Vec<Count> = m.into_iter().map(|(label, n)| Count { label, n }).collect();
    v.sort_by(|a, b| b.n.cmp(&a.n).then(a.label.cmp(&b.label)));
    v
}

/// What will be loaded for one part of a new project.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SlotPick {
    pub part: String,
    pub key: String,
    /// "choice" (the user's), "auto" (the style's most used), "other" (another style's).
    pub why: String,
    pub rel: String,
    pub track: String,
    pub title: String,
    pub track_name: String,
}

/// The chain for each part: the user's choice, else the style's most used, else the most used
/// anywhere; always one that plays here, in the given Live version (a chosen chain is taken from
/// another song of that version when its own song is of another).
pub fn pick_parts(cat: &ChainCatalogue, style: &str, live: &str, parts: &[String]) -> Vec<SlotPick> {
    let usable = |c: &Chain| c.uses.iter().find(|u| u.problems.is_empty() && u.live == live).cloned();
    let mut out = Vec::new();
    for part in parts {
        let pref = cat.prefs.get(style).and_then(|p| p.get(part));
        let of_part: Vec<&Chain> = cat.chains.iter().filter(|c| &c.part == part).collect();
        let found = pref
            .and_then(|k| of_part.iter().find(|c| &c.key == k).and_then(|c| usable(c).map(|u| (*c, u, "choice"))))
            .or_else(|| {
                let mut best: Vec<(&Chain, i64)> = of_part
                    .iter()
                    .map(|c| (*c, c.styles.iter().find(|s| s.label == style).map(|s| s.n).unwrap_or(0)))
                    .filter(|(_, n)| *n > 0)
                    .collect();
                best.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.songs.cmp(&a.0.songs)));
                best.into_iter().find_map(|(c, _)| usable(c).map(|u| (c, u, "auto")))
            })
            .or_else(|| of_part.iter().find_map(|c| usable(c).map(|u| (*c, u, "other"))));
        if let Some((c, u, why)) = found {
            out.push(SlotPick {
                part: part.clone(),
                key: c.key.clone(),
                why: why.into(),
                rel: u.rel,
                track: u.track,
                title: u.title,
                track_name: u.track_name,
            });
        }
    }
    out
}

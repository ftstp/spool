//! A new song started from how the user works: for a style, one chain per part (kick, bass,
//! keys...) taken from their own projects, the structure their songs of that style follow,
//! the style's usual tempo. The new project is written in a NEW folder only (create-only:
//! nothing existing is touched, the source projects are only read).
use crate::chains::{self, Chain, ChainCatalogue, SlotPick};
use crate::insights::Count;
use crate::structure::{self, Structure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use store::rusqlite::Connection;
use ts_rs::TS;

/// Another chain the user can take for a part.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CreateAlt {
    pub key: String,
    /// Its devices, "Serum > OTT > Valhalla Room".
    pub label: String,
    /// Songs it was used in, and how many of them are of the style.
    #[ts(type = "number")]
    pub songs: i64,
    #[ts(type = "number")]
    pub in_style: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CreatePart {
    pub part: String,
    /// What would be loaded (None: no chain of this part plays in this Live version).
    pub pick: Option<SlotPick>,
    pub label: String,
    pub alts: Vec<CreateAlt>,
    /// Chains of this part that load here: what a draw picks among.
    #[ts(type = "number")]
    pub pool: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CreatePlan {
    pub styles: Vec<Count>,
    /// Live versions with the number of chains that play in each.
    pub versions: Vec<Count>,
    pub style: String,
    pub live: String,
    pub parts: Vec<CreatePart>,
    pub structure: Structure,
    /// Where the new project goes: "<main folder>/Nouveaux morceaux".
    pub folder: String,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct CreateReq {
    pub style: Option<String>,
    pub live: Option<String>,
    /// part -> chain key chosen on screen (else the style's usual one).
    #[serde(default)]
    #[ts(type = "Record<string, string>")]
    pub overrides: BTreeMap<String, String>,
    /// The parts to load, in track order (only for the creation).
    #[serde(default)]
    pub parts: Vec<String>,
    #[serde(default)]
    pub tempo: Option<f64>,
    #[serde(default)]
    pub name: String,
    /// A draw: each part not in `overrides` gets a chain drawn among those that load here (the
    /// style's usual ones more often), and the structure is drawn too. None: the usual ones.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub seed: Option<u64>,
    /// The length wanted, in seconds (None: the style's usual one).
    #[serde(default)]
    pub seconds: Option<f64>,
    /// The folder the new "<name> Project" folder goes in (None: "<main folder>/Nouveaux
    /// morceaux"). It must be inside one of the projects folders, or the app would not see it.
    #[serde(default)]
    pub folder: Option<String>,
}

pub const FOLDER: &str = "Nouveaux morceaux";

fn label_of(c: &Chain) -> String {
    // Live's racks under the names its browser gives them
    let shown = |n: &str| match n {
        "InstrumentGroupDevice" => "Instrument Rack",
        "DrumGroupDevice" => "Drum Rack",
        "AudioEffectGroupDevice" => "Audio Effect Rack",
        "MidiEffectGroupDevice" => "MIDI Effect Rack",
        n => n.strip_suffix("_x64").unwrap_or(n),
    }
    .to_string();
    c.devices.iter().map(|d| shown(&d.name)).collect::<Vec<_>>().join(" > ")
}

/// The catalogue with the user's saved choices and this screen's.
pub fn catalogue(con: &Connection, saved: BTreeMap<String, BTreeMap<String, String>>) -> Result<ChainCatalogue, String> {
    chains::catalogue(con, saved).map_err(|e| e.to_string())
}

/// The plan for a style and a Live version (both defaulting to the most used).
pub fn plan(con: &Connection, mut cat: ChainCatalogue, req: &CreateReq, folder: String) -> Result<CreatePlan, String> {
    let style = req
        .style
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| cat.styles.first().map(|c| c.label.clone()))
        .unwrap_or_default();
    // newest Live first: a new song is made in the Live the user has now
    let num = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).take(2).map(|x| x.parse::<i64>().unwrap_or(0)).collect::<Vec<_>>();
    cat.versions.sort_by(|a, b| num(&b.label).cmp(&num(&a.label)).then(a.label.contains('b').cmp(&b.label.contains('b'))));
    let live = req
        .live
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| cat.versions.iter().find(|c| !c.label.contains('b') && c.n > 0).map(|c| c.label.clone()))
        .unwrap_or_default();
    // one file format only: "12.4" covers 12.4.2 and 12.4.5, whose sets differ, and a track
    // is only grafted onto a set of its exact format. The format most chains play in wins.
    let mut formats: BTreeMap<&str, i64> = BTreeMap::new();
    for c in &cat.chains {
        for u in c.uses.iter().filter(|u| u.live == live && u.problems.is_empty()) {
            *formats.entry(u.format.as_str()).or_default() += 1;
        }
    }
    let format = formats.iter().max_by_key(|(f, n)| (**n, (*f).to_string())).map(|(f, _)| f.to_string()).unwrap_or_default();
    for c in cat.chains.iter_mut() {
        c.uses.retain(|u| u.live != live || u.format == format);
    }
    // this screen's choices win over the saved ones, for this plan only
    let p = cat.prefs.entry(style.clone()).or_default();
    for (k, v) in &req.overrides {
        p.insert(k.clone(), v.clone());
    }
    let plays_here = |c: &Chain| c.uses.iter().any(|u| u.problems.is_empty() && u.live == live);
    // a draw: the parts left free get a chain drawn among those that load here
    let mut rng = req.seed.map(structure::Rng::new);
    let mut drawn: Vec<String> = Vec::new();
    if let Some(r) = rng.as_mut() {
        for part in chains::PARTS {
            if req.overrides.contains_key(part) {
                continue;
            }
            let pool: Vec<&Chain> = cat.chains.iter().filter(|c| c.part == part && plays_here(c)).collect();
            let w: Vec<f64> = pool
                .iter()
                .map(|c| 1.0 + c.songs as f64 + 4.0 * c.styles.iter().find(|s| s.label == style).map(|s| s.n).unwrap_or(0) as f64)
                .collect();
            if let Some(i) = r.weighted(&w) {
                let k = pool[i].key.clone();
                cat.prefs.entry(style.clone()).or_default().insert(part.to_string(), k);
                drawn.push(part.to_string());
            }
        }
    }
    let mut parts = Vec::new();
    for part in chains::PARTS {
        let mut pick = chains::pick_parts(&cat, &style, &live, &[part.to_string()]).into_iter().next();
        if let Some(p) = pick.as_mut().filter(|_| drawn.iter().any(|d| d == part)) {
            p.why = "drawn".into();
        }
        let mut alts: Vec<CreateAlt> = cat
            .chains
            .iter()
            .filter(|c| c.part == part && plays_here(c))
            .map(|c| CreateAlt {
                key: c.key.clone(),
                label: label_of(c),
                songs: c.songs,
                in_style: c.styles.iter().find(|s| s.label == style).map(|s| s.n).unwrap_or(0),
            })
            .collect();
        alts.sort_by(|a, b| b.in_style.cmp(&a.in_style).then(b.songs.cmp(&a.songs)));
        let pool = alts.len() as i64;
        // the list offered: the 15 most used, and the one drawn when it is not among them
        let keep = pick.as_ref().and_then(|p| alts.iter().position(|a| a.key == p.key)).filter(|i| *i >= 15);
        if let Some(i) = keep {
            alts.swap(14, i);
        }
        alts.truncate(15);
        let label = pick
            .as_ref()
            .and_then(|p| cat.chains.iter().find(|c| c.key == p.key))
            .map(label_of)
            .unwrap_or_default();
        parts.push(CreatePart { part: part.to_string(), pick, label, alts, pool });
    }
    let known = chains::known_styles(con).map_err(|e| e.to_string())?;
    let st = |f: Option<&str>, d: &str| chains::style_of(f, d, &known);
    let structure = structure::learn(con, &style, req.tempo, req.seconds, req.seed.map(|s| s ^ 0x5EC7), &st).map_err(|e| e.to_string())?;
    Ok(CreatePlan { styles: cat.styles.clone(), versions: cat.versions.clone(), style, live, parts, structure, folder })
}

/// A name that is a valid folder and file name on Windows and macOS.
pub fn clean_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { ' ' } else { c })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches('.').trim().to_string()
}

/// Live's colour index per part.
pub fn color_of(part: &str) -> Option<i64> {
    [("kick", 0), ("snare", 1), ("hats", 2), ("kit", 0), ("perc", 3), ("bass", 9), ("keys", 5), ("pad", 7), ("lead", 11)]
        .iter()
        .find(|c| c.0 == part)
        .map(|c| c.1)
}

/// The French name of a part, used as the track's name.
pub fn part_name(part: &str) -> &str {
    match part {
        "kick" => "Kick",
        "snare" => "Snare",
        "hats" => "Hats",
        "kit" => "Batterie",
        "perc" => "Percus",
        "bass" => "Basse",
        "keys" => "Claviers",
        "pad" => "Nappe",
        "lead" => "Lead",
        p => p,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_are_cleaned() {
        assert_eq!(super::clean_name("  dnb: test/1?  "), "dnb test 1");
        assert_eq!(super::clean_name("..."), "");
    }
}

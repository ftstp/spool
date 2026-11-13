//! The structure the user gives songs of a style, learnt from the locators of their projects:
//! the names are read as sections (intro, buildup, drop, break, outro...; "1", "test" are not
//! sections), the most common order of sections is kept with the median length of each, and
//! the whole is fitted to a target duration at the chosen tempo, in whole phrases.
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use store::rusqlite::{self, Connection};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Section {
    pub name: String,
    /// First bar (from 0) and length in bars.
    pub bar: i64,
    pub bars: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Structure {
    pub style: String,
    /// The usual tempo of the style (median of its projects), when known.
    pub bpm: Option<f64>,
    /// How many projects the order of sections was learnt from (0: a default order).
    #[ts(type = "number")]
    pub learnt_from: i64,
    pub sections: Vec<Section>,
    /// The length it comes to, at the tempo given.
    pub seconds: f64,
    /// How many different orders of sections the user's projects of the style show (a draw
    /// picks among them).
    #[ts(type = "number")]
    pub variants: i64,
}

/// A small seeded generator (splitmix64): the same seed gives the same draw.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// In [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// An index drawn with these weights (None: no weight above 0).
    pub fn weighted(&mut self, w: &[f64]) -> Option<usize> {
        let total: f64 = w.iter().filter(|x| **x > 0.0).sum();
        if total <= 0.0 {
            return None;
        }
        let mut at = self.unit() * total;
        for (i, x) in w.iter().enumerate() {
            if *x <= 0.0 {
                continue;
            }
            if at < *x {
                return Some(i);
            }
            at -= x;
        }
        w.iter().rposition(|x| *x > 0.0)
    }
}

static WORDS: std::sync::LazyLock<Vec<(&'static str, regex::Regex)>> = std::sync::LazyLock::new(|| {
    [
        ("Intro", r"intro|start|d[ée]but|ouverture"),
        ("Buildup", r"build|pre ?drop|pr[ée] ?drop|mont[ée]e|riser|tension|pre ?beat"),
        ("Drop", r"drop|refrain|chorus|hook|beat"),
        ("Break", r"break|pont|bridge|vide|amb|calme|interlude"),
        ("Verse", r"verse|couplet|theme|th[èe]me|melo"),
        ("Outro", r"outro|end|fin\b|final"),
    ]
    .into_iter()
    .map(|(n, w)| (n, regex::Regex::new(&format!(r"(?i)(^|[^a-z])({w})")).unwrap()))
    .collect()
});

fn section_of(name: &str) -> Option<&'static str> {
    WORDS.iter().find(|(_, r)| r.is_match(name)).map(|(n, _)| *n)
}

const DEFAULT: [(&str, i64); 7] = [("Intro", 16), ("Buildup", 16), ("Drop", 32), ("Break", 16), ("Buildup", 8), ("Drop", 32), ("Outro", 16)];

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() { 0.0 } else { v[v.len() / 2] }
}

/// `style_of(song)` decides which songs count (same rule as the chain catalogue).
/// With a `seed`, the order is drawn among all those the user's projects show (the usual one
/// more often), lengths vary a little, and a `seconds` target far from it drops repeated
/// sections or adds a Buildup + Drop rather than stretching every section.
pub fn learn(
    con: &Connection,
    style: &str,
    bpm: Option<f64>,
    seconds: Option<f64>,
    seed: Option<u64>,
    style_of: &dyn Fn(Option<&str>, &str) -> String,
) -> rusqlite::Result<Structure> {
    let mut s = con.prepare(
        "select f.data, s.style, s.song_dir from files f join songs s on s.id = f.song_id \
         where f.in_version = 0 and f.archived = 0 and s.archived = 0 and f.rel like '%.als' \
         and f.mtime = (select max(g.mtime) from files g where g.song_id = f.song_id and g.in_version = 0 and g.archived = 0 and g.rel like '%.als')",
    )?;
    let rows: Vec<(String, Option<String>, String)> =
        s.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?, r.get(2)?)))?.flatten().collect();
    let mut tempos = Vec::new();
    // order of sections -> the lengths (bars) of each, per project
    let mut orders: HashMap<Vec<&'static str>, Vec<Vec<f64>>> = HashMap::new();
    for (data, st, dir) in rows {
        if style_of(st.as_deref(), &dir) != style {
            continue;
        }
        let info: Value = serde_json::from_str(&data).unwrap_or_default();
        if let Some(t) = info["tempo"].as_f64().filter(|t| *t >= 40.0 && *t <= 250.0) {
            tempos.push(t);
        }
        let end_beats = info["length_s"].as_f64().zip(info["tempo"].as_f64()).map(|(s, t)| s * t / 60.0);
        let mut secs: Vec<(&'static str, f64)> = Vec::new();
        for l in info["locators"].as_array().into_iter().flatten() {
            let (Some(beat), Some(name)) = (l[0].as_f64(), l[1].as_str()) else { continue };
            if let Some(sec) = section_of(name) {
                if secs.last().is_none_or(|(n, _)| *n != sec) {
                    secs.push((sec, beat / 4.0));
                }
            }
        }
        if secs.len() < 3 {
            continue;
        }
        let mut lens = Vec::new();
        for i in 0..secs.len() {
            let end = secs.get(i + 1).map(|x| x.1).or(end_beats.map(|b| b / 4.0)).unwrap_or(secs[i].1 + 16.0);
            lens.push((end - secs[i].1).max(1.0));
        }
        orders.entry(secs.iter().map(|x| x.0).collect()).or_default().push(lens);
    }
    let usual_bpm = if tempos.is_empty() { None } else { Some((median(&mut tempos) * 10.0).round() / 10.0) };
    let bpm = bpm.or(usual_bpm).unwrap_or(120.0);

    let medians = |order: &Vec<&'static str>, lens: &Vec<Vec<f64>>| -> Vec<f64> {
        let mut bars: Vec<f64> = (0..order.len()).map(|i| median(&mut lens.iter().map(|l| l[i]).collect::<Vec<_>>())).collect();
        // the last section runs to the end of the set (tails, silence): no longer than the
        // longest of the others
        let longest = bars[..bars.len() - 1].iter().cloned().fold(16.0, f64::max);
        if let Some(l) = bars.last_mut() {
            *l = l.min(longest);
        }
        bars
    };
    let default = || -> (Vec<&'static str>, Vec<f64>, i64) { (DEFAULT.iter().map(|d| d.0).collect(), DEFAULT.iter().map(|d| d.1 as f64).collect(), 0) };
    let variants = orders.len() as i64;
    let mut rng = seed.map(Rng::new);
    let (mut names, mut bars, learnt): (Vec<&'static str>, Vec<f64>, i64) = if let Some(r) = rng.as_mut() {
        // every order seen (the usual ones weigh more), and the plain default
        let mut cands: Vec<(&Vec<&'static str>, &Vec<Vec<f64>>)> = orders.iter().collect();
        cands.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        let mut w: Vec<f64> = cands.iter().map(|c| c.1.len() as f64).collect();
        w.push(0.5);
        match r.weighted(&w) {
            Some(i) if i < cands.len() => {
                let mut bars = medians(cands[i].0, cands[i].1);
                // a little play in each length
                for b in bars.iter_mut() {
                    *b *= 0.8 + r.unit() * 0.45;
                }
                (cands[i].0.clone(), bars, cands[i].1.len() as i64)
            }
            _ => default(),
        }
    } else {
        // the order seen most, in at least 2 projects (one project is a song, not a habit)
        let best = orders.iter().filter(|o| o.1.len() >= 2).max_by(|a, b| a.1.len().cmp(&b.1.len()).then(b.0.len().cmp(&a.0.len())));
        match best {
            Some((order, lens)) => (order.clone(), medians(order, lens), lens.len() as i64),
            None => default(),
        }
    };
    // fitted to the duration, in phrases of 4 bars (8 for the long sections)
    if let Some(sec) = seconds.filter(|s| *s > 0.0) {
        let want = sec * bpm / 240.0;
        let sum = |b: &Vec<f64>| b.iter().sum::<f64>();
        // far too long: a repeated section in the middle goes (the latest first)
        while sum(&bars) > want * 1.3 && names.len() > 4 {
            let Some(i) = (1..names.len() - 1).rev().find(|&i| names[..i].contains(&names[i])) else { break };
            names.remove(i);
            bars.remove(i);
        }
        // far too short: one more Buildup + Drop before the last section
        while sum(&bars) < want * 0.75 && names.len() < 12 {
            let at = names.len() - 1;
            let drop = bars.iter().zip(&names).filter(|(_, n)| **n == "Drop").map(|(b, _)| *b).next().unwrap_or(32.0);
            let build = bars.iter().zip(&names).filter(|(_, n)| **n == "Buildup").map(|(b, _)| *b).next().unwrap_or(16.0);
            names.splice(at..at, ["Buildup", "Drop"]);
            bars.splice(at..at, [build, drop]);
        }
        let have = sum(&bars);
        if have > 0.0 {
            for b in bars.iter_mut() {
                *b *= want / have;
            }
        }
    }
    let mut sections = Vec::new();
    let mut at = 0i64;
    let mut count: HashMap<&str, i64> = HashMap::new();
    for (n, b) in names.iter().zip(bars) {
        let step = if b >= 12.0 { 8.0 } else { 4.0 };
        let len = ((b / step).round() * step).max(4.0) as i64;
        let k = count.entry(n).or_default();
        *k += 1;
        let name = if *k > 1 { format!("{n} {k}") } else { n.to_string() };
        sections.push(Section { name, bar: at, bars: len });
        at += len;
    }
    Ok(Structure { style: style.into(), bpm: Some(bpm), learnt_from: learnt, seconds: at as f64 * 240.0 / bpm, sections, variants })
}

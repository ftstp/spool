//! Style suggestions for the songs whose style nobody wrote down.
//!
//! Every clue gives points to a style; the shares of the total become the percentages
//! shown, and the best two or three are offered to confirm (never applied by themselves):
//! a style word in a file or folder name, a style word inside the project (track, clip,
//! locator, sample names), resemblance with the songs whose style is known (plugins,
//! devices, samples, close tempo), and the tempo alone for drum and bass.
use crate::names::{Rules, STYLE_WORDS, ascii_low, similarity};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Whether a lower-case name is a music style ("dnb", "lofi", "hip hop"): never an artist.
pub fn is_style_name(low: &str) -> bool {
    STYLE_WORDS.contains(&low) || ALIASES.iter().any(|a| a.0 == low) || STYLE_WORDS.contains(&low.replace(' ', "").as_str())
}

const ALIASES: &[(&str, &str)] = &[
    ("drum & bass", "dnb"),
    ("drum and bass", "dnb"),
    ("drum n bass", "dnb"),
    ("drumnbass", "dnb"),
    ("d&b", "dnb"),
    ("dnb", "dnb"),
    ("hip hop", "hiphop"),
    ("hip-hop", "hiphop"),
    ("hiphop", "hiphop"),
    ("lo-fi", "lofi"),
    ("lofi", "lofi"),
    ("electro downtmepo", "downtempo"),
    ("electro downtempo", "downtempo"),
    ("downtempo", "downtempo"),
    ("trip hop", "triphop"),
    ("trip-hop", "triphop"),
    ("triphop", "triphop"),
    ("afro", "afrobeat"),
    ("orc", "orchestral"),
    ("orch", "orchestral"),
    ("uk garage", "garage"),
    ("ukg", "garage"),
    ("jumpup", "dnb"),
    ("jump up", "dnb"),
    ("neurofunk", "dnb"),
    ("liquid funk", "dnb"),
    ("brk", "breakbeat"),
    ("elek", "electro"),
    ("drumstep", "drumstep"),
    ("trap", "trap"),
    ("synthwave", "synthwave"),
];
/// Inside a project these name an instrument or a sound more often than the song's style.
const SOUND_WORDS: &[&str] = &[
    "piano",
    "acoustic",
    "orchestral",
    "orc",
    "orch",
    "acid",
    "dub",
    "pop",
    "rock",
    "soul",
    "funk",
    "jazz",
    "blues",
    "afro",
    "80s",
    "ambient",
    "disco",
    "fanfare",
    "experimental",
    "new wave",
];
const STOP: &[&str] = &[
    "audio", "midi", "track", "group", "return", "main", "master", "copy", "new", "the", "and", "les", "des", "une",
    "pour", "avec",
];
const NEIGHBOURS: usize = 5;
/// Less than that is a hunch, not a suggestion.
const MIN_POINTS: f64 = 2.0;
const SHOW: usize = 3;

static PHRASE: LazyLock<FRegex> = LazyLock::new(|| {
    let mut vocab: Vec<&str> = STYLE_WORDS.iter().copied().chain(ALIASES.iter().map(|a| a.0)).collect();
    vocab.sort();
    vocab.dedup();
    vocab.sort_by_key(|w| std::cmp::Reverse(w.len()));
    let alt: Vec<String> = vocab.iter().map(|w| regex::escape(w)).collect();
    FRegex::new(&format!(r"(?<![a-z0-9])({})(?![a-z0-9])", alt.join("|"))).unwrap()
});
static TEXT_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9&]+").unwrap());
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z][a-z0-9]{2,}").unwrap());

pub fn canon(style: &str) -> String {
    let s = ascii_low(style).trim().to_string();
    ALIASES.iter().find(|a| a.0 == s).map(|a| a.1.to_string()).unwrap_or(s)
}

/// Style words in a name: (canonical, word as written), first occurrence kept.
fn styles_in(s: &str, content: bool) -> Vec<(String, String)> {
    let t = format!(" {} ", TEXT_SEP.replace_all(&ascii_low(s).replace('_', " "), " "));
    let mut out: Vec<(String, String)> = Vec::new();
    for c in PHRASE.captures_iter(&t).flatten() {
        let w = c.get(1).unwrap().as_str();
        if content && SOUND_WORDS.contains(&w) {
            continue;
        }
        let k = canon(w);
        if !out.iter().any(|(c, _)| *c == k) {
            out.push((k, w.to_string()));
        }
    }
    out
}

/// What the reader says about a project, as far as styles care.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StyleData {
    #[serde(default)]
    pub plugins: Vec<NameOnly>,
    #[serde(default)]
    pub devices: Vec<NameOnly>,
    #[serde(default)]
    pub sample_names: Vec<String>,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub tempo: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NameOnly {
    #[serde(default)]
    pub name: String,
}

/// What two projects can share: plugins, Live devices, words of sample and track names.
fn features(d: &StyleData) -> HashSet<String> {
    let mut f: HashSet<String> =
        d.plugins.iter().filter(|p| !p.name.is_empty()).map(|p| format!("p:{}", ascii_low(&p.name))).collect();
    f.extend(d.devices.iter().map(|x| format!("d:{}", x.name)));
    for (prefix, texts) in [("s:", &d.sample_names), ("w:", &d.names)] {
        for t in texts {
            for w in WORDS.find_iter(&ascii_low(t)) {
                if !STOP.contains(&w.as_str()) {
                    f.insert(format!("{prefix}{}", w.as_str()));
                }
            }
        }
    }
    f
}

/// 1 for the same tempo, 0 beyond 10 BPM apart. Half / double tempo count, a bit less.
fn tempo_close(a: Option<f64>, b: Option<f64>) -> f64 {
    let (Some(a), Some(b)) = (a.filter(|x| *x != 0.0), b.filter(|x| *x != 0.0)) else { return 0.0 };
    let dt = (a - b).abs().min((a * 2.0 - b).abs() + 3.0).min((a - b * 2.0).abs() + 3.0);
    (1.0 - dt / 10.0).max(0.0)
}

/// Python 3.12+'s `sum()` of floats: Neumaier compensated summation, so totals (and the
/// percentages and rounded values made from them) come out exactly as in the prototype.
pub fn py_sum(xs: impl IntoIterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in xs {
        let t = s + x;
        if s.abs() >= x.abs() {
            c += (s - t) + x;
        } else {
            c += (x - t) + s;
        }
        s = t;
    }
    if c != 0.0 && c.is_finite() { s + c } else { s }
}

/// Python's `round(x)` (ties to even), as an integer.
fn round_int(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// Python's `round(x, 2)`.
fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// A song as the style deduction sees it.
#[derive(Debug, Clone, Deserialize)]
pub struct StyleSong {
    pub id: i64,
    pub title: Option<String>,
    pub style: Option<String>,
    pub dir: String,
    /// Names of its active .als files, oldest first.
    pub stems: Vec<String>,
    /// Audio file names of its folder (without extension).
    pub audio: Vec<String>,
    /// Its folder is shared with other songs.
    pub shared: bool,
    /// The newest active project.
    pub data: StyleData,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StyleOption {
    pub style: String,
    pub pct: i64,
    pub why: Vec<String>,
}

struct Prepared<'a> {
    s: &'a StyleSong,
    audio: Vec<&'a str>,
    feats: HashSet<String>,
}

/// Points per style in the order they were first given (Python's Counter + most_common).
#[derive(Default)]
struct Points {
    order: Vec<String>,
    pts: HashMap<String, f64>,
    why: HashMap<String, Vec<String>>,
}

impl Points {
    fn add(&mut self, st: &str, p: f64, reason: String) {
        if !self.pts.contains_key(st) {
            self.order.push(st.to_string());
        }
        *self.pts.entry(st.to_string()).or_default() += p;
        self.why.entry(st.to_string()).or_default().push(reason);
    }

    fn ranked(self) -> Vec<(String, f64, Vec<String>)> {
        let mut v: Vec<(String, f64, Vec<String>)> =
            self.order.iter().map(|s| (s.clone(), self.pts[s], self.why.get(s).cloned().unwrap_or_default())).collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); // stable
        v
    }
}

fn guess(song: &Prepared, known: &[(&Prepared, String)]) -> Vec<(String, f64, Vec<String>)> {
    let mut p = Points::default();
    let s = song.s;
    // 1. names: the song's own folder, the folders above, the files
    let parts: Vec<&str> = s.dir.split('/').filter(|x| !x.is_empty()).collect();
    let mut any_folder = false;
    for (i, folder) in parts.iter().rev().enumerate() {
        for (st, _) in styles_in(folder, false) {
            let name = folder.trim_matches(|c| c == ' ' || c == '-' || c == '_');
            p.add(&st, if i == 0 { 5.0 } else { 4.0 }, i18n::tr!("style.folder", name = name));
            any_folder = true;
        }
    }
    let mut named: Vec<(String, String)> = Vec::new();
    for stem in s.stems.iter().map(String::as_str).chain(song.audio.iter().copied()) {
        for (st, _) in styles_in(stem, false) {
            if !named.iter().any(|(k, _)| *k == st) {
                named.push((st, stem.to_string()));
            }
        }
    }
    for (st, stem) in &named {
        p.add(st, 4.0, i18n::tr!("style.file", name = stem));
    }
    // 2. inside the project
    for (texts, samples) in [(&s.data.names, false), (&s.data.sample_names, true)] {
        let mut order: Vec<String> = Vec::new();
        let mut hits: HashMap<String, (usize, String)> = HashMap::new();
        for t in texts {
            for (st, _) in styles_in(t, true) {
                let e = hits.entry(st.clone()).or_insert_with(|| {
                    order.push(st.clone());
                    (0, t.clone())
                });
                e.0 += 1;
            }
        }
        for st in order {
            let (n, ex) = &hits[&st];
            if samples {
                let pts = (1.0 + 0.5 * *n as f64).min(3.0);
                p.add(&st, pts, i18n::tr!("style.samples", n = n, style = st, ex = ex));
            } else {
                let more = if *n > 1 { format!(" (+{})", n - 1) } else { String::new() };
                p.add(&st, 2.0, i18n::tr!("style.inProject", ex = ex, more = more));
            }
        }
    }
    // 3. resemblance with the songs whose style is known
    let tempo = s.data.tempo;
    let mut sims: Vec<(f64, &Prepared, String, HashSet<&String>)> = Vec::new();
    for (k, kstyle) in known {
        if k.s.id == s.id {
            continue;
        }
        let inter: HashSet<&String> = song.feats.intersection(&k.feats).collect();
        let union = song.feats.union(&k.feats).count();
        let sim =
            inter.len() as f64 / if union == 0 { 1.0 } else { union as f64 } + 0.3 * tempo_close(tempo, k.s.data.tempo);
        if sim > 0.15 {
            sims.push((sim, k, kstyle.clone(), inter));
        }
    }
    sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let near = &sims[..sims.len().min(NEIGHBOURS)];
    let total = py_sum(near.iter().map(|x| x.0));
    // A style written in a name is near-certain, a resemblance is a guess: it only weighs in
    // fully when no name says anything.
    let weight = if !named.is_empty() || any_folder { 2.0 } else { 6.0 };
    let mut by_style: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, x) in near.iter().enumerate() {
        match by_style.iter_mut().find(|(st, _)| *st == x.2) {
            Some(e) => e.1.push(i),
            None => by_style.push((x.2.clone(), vec![i])),
        }
    }
    for (st, rows) in by_style {
        let share = py_sum(rows.iter().map(|&i| near[i].0)) / total;
        let (_, k, _, inter) = &near[rows[0]];
        let mut plugs: Vec<String> = inter.iter().filter(|x| x.starts_with("p:")).map(|x| x[2..].to_string()).collect();
        plugs.sort();
        plugs.truncate(2);
        let mut what = Vec::new();
        if !plugs.is_empty() {
            what.push(i18n::tr!("style.samePlugins", list = plugs.join(", ")));
        }
        if tempo_close(tempo, k.s.data.tempo) > 0.5 {
            what.push(i18n::tr!("style.closeTempo", tempo = round_int(k.s.data.tempo.unwrap_or(0.0))));
        }
        let titles: Vec<String> = rows
            .iter()
            .take(2)
            .map(|&i| i18n::tr!("style.quoted", title = near[i].1.s.title.clone().unwrap_or_else(|| "None".into())))
            .collect();
        let tail = if what.is_empty() { String::new() } else { i18n::tr!("style.tail", list = what.join(", ")) };
        p.add(&st, round2(weight * share), i18n::tr!("style.resembles", titles = titles.join(&i18n::tr!("style.and")), tail = tail));
    }
    // 4. tempo, drum and bass only
    if let Some(t) = tempo.filter(|t| *t != 0.0 && (165.0..=180.0).contains(t)) {
        p.add("dnb", 1.5, i18n::tr!("style.dnbTempo", tempo = round_int(t)));
    }
    p.ranked()
}

/// The 2-3 best with their share of all the points, or [] when the clues are too thin.
fn options(ranked: &[(String, f64, Vec<String>)], spelling: &HashMap<String, String>) -> Vec<StyleOption> {
    if ranked.is_empty() || ranked[0].1 < MIN_POINTS {
        return Vec::new();
    }
    let total = py_sum(ranked.iter().map(|r| r.1));
    let mut out: Vec<StyleOption> = Vec::new();
    for (st, p, reasons) in ranked.iter().take(SHOW) {
        let pct = round_int(100.0 * p / total);
        if !out.is_empty() && pct < 8 {
            break;
        }
        out.push(StyleOption {
            style: spelling.get(st).cloned().unwrap_or_else(|| st.clone()),
            pct,
            why: reasons.iter().take(4).cloned().collect(),
        });
    }
    out
}

/// Style options for every song without a style: (song id, options), in the songs' order.
/// `songs` in a stable order (it decides ties between equally close songs).
pub fn deduce(songs: &[StyleSong], rules: &Rules) -> Vec<(i64, Vec<StyleOption>)> {
    let prepared: Vec<Prepared> = songs
        .iter()
        .map(|s| {
            let mut audio: Vec<&str> = s.audio.iter().map(String::as_str).collect();
            if s.shared {
                // A folder shared by several songs: only the renders named like this one are its own.
                let mine: Vec<String> = s
                    .stems
                    .iter()
                    .map(String::as_str)
                    .chain([s.title.as_deref().unwrap_or("")])
                    .map(|x| rules.variant_key(x))
                    .collect();
                audio.retain(|a| {
                    let k = rules.variant_key(a);
                    mine.iter().map(|m| similarity(&k, m, 0.6)).fold(f64::MIN, f64::max) >= 0.6
                });
            }
            audio.truncate(40);
            Prepared { s, audio, feats: features(&s.data) }
        })
        .collect();
    let known: Vec<(&Prepared, String)> =
        prepared.iter().filter_map(|p| p.s.style.as_ref().filter(|x| !x.is_empty()).map(|st| (p, canon(st)))).collect();
    // Shown as the user writes it: "DNB" if that is how most of their songs say it.
    let mut counts: Vec<((String, String), usize)> = Vec::new();
    for p in &prepared {
        if let Some(st) = p.s.style.as_ref().filter(|x| !x.is_empty()) {
            let key = (canon(st), st.clone());
            match counts.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 += 1,
                None => counts.push((key, 1)),
            }
        }
    }
    counts.sort_by_key(|c| std::cmp::Reverse(c.1)); // stable: ties keep first-seen order
    let mut spelling: HashMap<String, String> = HashMap::new();
    for ((c, w), _) in counts {
        spelling.entry(c).or_insert(w);
    }
    prepared
        .iter()
        .filter(|p| p.s.style.as_ref().is_none_or(|x| x.is_empty()))
        .filter_map(|p| {
            let opts = options(&guess(p, &known), &spelling);
            (!opts.is_empty()).then_some((p.s.id, opts))
        })
        .collect()
}

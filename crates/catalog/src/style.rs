//! Style suggestions for the songs whose style nobody wrote down.
//!
//! Every clue gives points to a style; the shares of the total become the percentages
//! shown, and the best two or three are offered to confirm (never applied by themselves):
//! a style word in a file or folder name, a style word inside the project (track, clip,
//! locator, sample names), resemblance with the songs whose style is known (plugins,
//! devices, samples, close tempo), and the tempo alone for drum and bass.
use crate::names::{Rules, ascii_low, similarity};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Whether a name is a music style Spool knows ("dnb", "deep house", "hip-hop"): never an
/// artist. The user's own style words are not in it (see `StyleVocab`).
pub fn is_style_name(name: &str) -> bool {
    let k = key(name);
    BASE.contains(&k) || ALIASES.iter().any(|a| key(a.0) == k) || BASE.contains(&k.replace(' ', ""))
}

/// Discogs' 400 styles, the classes of the style model ("Genre---Style").
const DISCOGS: &str = include_str!("../../audio/styles/discogs400.json");
/// Discogs styles never read in a name: no music style (spoken word, children's, a film's
/// score) or a word that names something else in a folder ("vocal", "instrumental" versions,
/// "swing" and "noise" in production).
const NOT_FROM_DISCOGS: &[&str] = &[
    "vocal", "instrumental", "ballad", "swing", "noise", "beat", "modern", "contemporary", "abstract", "romantic",
    "conscious", "modal", "son", "mod", "bounce", "screw", "ghetto", "gangsta", "surf", "twist", "oi", "glam",
    "ethereal", "freestyle", "acoustic", "marches", "military", "educational", "nursery rhymes", "story", "musical",
    "score", "soundtrack", "theme", "light music", "novelty", "parody", "audiobook", "comedy", "dialogue",
    "education", "field recording", "interview", "monolog", "poetry", "political", "promotional", "radioplay",
    "religious", "spoken word", "cut up dj", "dj battle tool", "space age", "music hall",
];
/// The broad genres, as a folder says them.
const GENRE_WORDS: &[&str] =
    &["blues", "classical", "electronic", "hip hop", "jazz", "latin", "pop", "reggae", "rock", "funk", "soul", "folk", "country"];
/// Styles of today that Discogs' list (2020) lacks.
const RECENT: &[&str] = &[
    "bass house", "future bass", "future house", "melodic techno", "afro house", "amapiano", "phonk", "drill",
    "uk drill", "riddim", "hyperpop", "glitch hop", "moombahton", "jersey club", "neurofunk", "liquid funk",
    "drumstep", "brostep", "organic house", "lofi", "lofi hip hop", "trap", "synthwave", "dubstep", "garage",
];
/// Inside a project (track, clip and sample names) only these count: there "swing",
/// "minimal" or "tribal" name a groove or a sound more often than the song's style.
const CONTENT: &[&str] = &[
    "dnb", "drum & bass", "drum and bass", "drum n bass", "jungle", "electro", "house", "techno", "trance",
    "breakbeat", "dubstep", "garage", "uk garage", "reggae", "lofi", "lo fi", "trip hop", "hip hop", "downtempo",
    "afrobeat", "synthwave", "trap", "drumstep", "glitch", "deep house", "tech house", "hardstyle", "grime",
];
/// Other ways to write a style: (as written, normalised; the style).
const ALIASES: &[(&str, &str)] = &[
    ("drum & bass", "dnb"),
    ("drum and bass", "dnb"),
    ("drum n bass", "dnb"),
    ("drumnbass", "dnb"),
    ("d&b", "dnb"),
    ("dnb", "dnb"),
    ("jump up", "dnb"),
    ("jumpup", "dnb"),
    ("neurofunk", "dnb"),
    ("liquid funk", "dnb"),
    ("hip hop", "hiphop"),
    ("hiphop", "hiphop"),
    ("lo fi", "lofi"),
    ("lofi", "lofi"),
    ("trip hop", "triphop"),
    ("triphop", "triphop"),
    ("afro", "afrobeat"),
    ("uk garage", "garage"),
    ("ukg", "garage"),
];
/// Every style word read in folder and file names (normalised, see `key`).
static BASE: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let off: HashSet<String> = NOT_FROM_DISCOGS.iter().chain(NOT_STYLES).map(|w| key(w)).collect();
    DISCOGS
        .split('"')
        .filter_map(|c| c.split_once("---").map(|x| x.1))
        .chain(GENRE_WORDS.iter().copied())
        .chain(RECENT.iter().copied())
        .chain(CONTENT.iter().copied())
        .chain(ALIASES.iter().map(|a| a.0))
        .map(key)
        .filter(|w| !w.is_empty() && !off.contains(w))
        .collect()
});

/// A name as styles are compared: lower case, no accents, punctuation as spaces ("Hip-Hop"
/// and "hip hop" are one). "&" is kept ("drum & bass").
fn key(s: &str) -> String {
    TEXT_SEP.replace_all(&ascii_low(s).replace('_', " "), " ").split_whitespace().collect::<Vec<_>>().join(" ")
}
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
/// The audio vote: the `SOUND_K` closest renders among the songs with a style, each
/// weighing its similarity; counted only when one style holds `SOUND_SHARE` of the weight.
/// Chosen by leave-one-out on a real catalogue of 110 songs with a style (see
/// crates/audio/README.md): 5 and 80 % gave a vote for 33 % of them, right 83 % of the time
/// (always guessing the commonest style: 22 %).
pub const SOUND_K: usize = 5;
pub const SOUND_SHARE: f64 = 0.8;
/// Points of a unanimous audio vote (MIN_POINTS is 2: alone, it is enough to suggest).
const SOUND_POINTS: f64 = 4.0;
const SHOW: usize = 3;

/// Read in a folder name like a style (so never taken for an artist or a title), but an
/// instrumentation, not a style: never suggested. A style the user wrote is kept as written.
const NOT_STYLES: &[&str] = &["piano", "orchestral"];

/// The style words of folder and file names, and the fewer of a project's content.
static FULL: LazyLock<FRegex> = LazyLock::new(|| phrase(BASE.iter().map(String::as_str), &[]));
static INSIDE: LazyLock<FRegex> = LazyLock::new(|| phrase(inside_words(), &[]));

/// Read inside a project: the common styles, the recent ones and every style of two words or
/// more ("Bass House Drum Loop" is bass house, not house).
fn inside_words() -> impl Iterator<Item = &'static str> {
    CONTENT.iter().copied().chain(RECENT.iter().copied()).chain(BASE.iter().map(String::as_str).filter(|w| w.contains(' ')))
}

/// A regex finding these words, and the user's own, whole, the longest first.
fn phrase<'a>(words: impl Iterator<Item = &'a str>, extra: &[String]) -> FRegex {
    let mut vocab: Vec<String> = words.map(key).chain(extra.iter().map(|e| key(e))).filter(|w| !w.is_empty()).collect();
    vocab.retain(|w| !NOT_STYLES.iter().any(|n| key(n) == *w) || extra.iter().any(|e| key(e) == *w));
    vocab.sort();
    vocab.dedup();
    vocab.sort_by_key(|w| std::cmp::Reverse(w.len()));
    let alt: Vec<String> = vocab.iter().map(|w| regex::escape(w)).collect();
    FRegex::new(&format!(r"(?<![a-z0-9])({})(?![a-z0-9])", alt.join("|"))).unwrap()
}

/// What the user taught about styles: their own style words (found in names like the built-in
/// ones: "jazz fanfare", "neo soul"), and their name for some of the styles Spool hears ("brass
/// band" -> "jazz fanfare": a render heard as Brass Band counts for jazz fanfare).
#[derive(Debug, Clone, Default)]
pub struct StyleVocab {
    /// Lower case.
    pub words: Vec<String>,
    /// (the style heard, lower case, the user's style).
    pub links: Vec<(String, String)>,
    /// Their name for a word of their folders: ("ORC", "orchestral").
    #[allow(clippy::struct_field_names)]
    pub names: Vec<(String, String)>,
}

impl StyleVocab {
    /// The words read in folder and file names, and in a project's content: Spool's and the
    /// user's own.
    fn regexes(&self) -> (FRegex, FRegex) {
        if self.words.is_empty() {
            return (FULL.clone(), INSIDE.clone());
        }
        (phrase(BASE.iter().map(String::as_str), &self.words), phrase(inside_words(), &self.words))
    }

    /// The style a word read in a name counts for: the user's name for it, else its own.
    pub fn canon(&self, word: &str) -> String {
        let k = key(word);
        self.names.iter().find(|(w, _)| key(w) == k).map(|(_, n)| canon(n)).unwrap_or_else(|| canon(word))
    }

    /// One of the user's own style words.
    pub fn has_word(&self, name: &str) -> bool {
        let k = key(name);
        self.words.iter().any(|w| key(w) == k)
    }
}

/// Discogs genres as a folder names them: a style heard ("Hip Hop---Boom Bap") counts for its
/// genre where the song's names say the genre, not the style ("Hip Hop/Rue"). "Electronic"
/// is too broad to agree with anything.
const GENRES: &[(&str, &str)] = &[
    ("hip hop", "hiphop"),
    ("latin", "latin"),
    ("classical", "classical"),
    ("reggae", "reggae"),
    ("jazz", "jazz"),
    ("blues", "blues"),
    ("rock", "rock"),
    ("pop", "pop"),
    ("funk / soul", "soul"),
    ("funk / soul", "funk"),
    ("folk, world, & country", "folk"),
    ("folk, world, & country", "country"),
];

/// The styles a heard class's genre names ("Hip Hop---Boom Bap": hiphop).
pub fn heard_genres(class: &str) -> Vec<&'static str> {
    let genre = key(class.split_once("---").map(|x| x.0).unwrap_or(""));
    GENRES.iter().filter(|(g, _)| key(g) == genre).map(|x| x.1).collect()
}
/// What Spool hears counts from this strength, and a strong hearing at most this much
/// (a style written in a folder name counts 4 to 5).
pub const HEARD_MIN: f32 = 0.1;
const HEARD_POINTS: f64 = 5.0;
const HEARD_MAX: f64 = 3.0;
/// Styles heard from this activation add up for their genre, which counts from this sum.
const GENRE_MIN: f32 = 0.05;
const GENRE_SUM: f64 = 0.25;

/// The style a Discogs class ("Genre---Style") counts for (canonical), and how it is shown:
/// the user's name for it, else its own in lower case like the styles read in names ("deep
/// house"), or the short name Spool gives it ("Drum n Bass" is dnb).
pub fn heard_style(class: &str, vocab: &StyleVocab) -> (String, String) {
    let style = class.split_once("---").map(|x| x.1).unwrap_or(class);
    let low = key(style);
    if let Some((_, mine)) = vocab.links.iter().find(|(d, _)| key(d) == low) {
        return (canon(mine), mine.clone());
    }
    let c = canon(&low);
    (c.clone(), c)
}
static TEXT_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9&]+").unwrap());
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z][a-z0-9]{2,}").unwrap());

pub fn canon(style: &str) -> String {
    let s = key(style);
    ALIASES.iter().find(|a| key(a.0) == s).map(|a| a.1.to_string()).unwrap_or(s)
}

/// The parts of a file or folder name: "6. DNB D# - Lost (Jungle)" is "DNB D#", "Lost",
/// "Jungle" (numbers in front left out).
static PIECES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s-|-\s|[()\[\]{}|]").unwrap());

/// Style words in a name: (canonical, word as written), first occurrence kept.
/// In a file or folder name a style opens one of its parts ("electro - Roasted", "Take Off
/// (DNB)"), so words that only happen to be styles inside a title or a name ("Daft Punk",
/// "lost in time remix breakbeat") are not read. A project's content is read whole.
fn styles_in(s: &str, content: bool, re: &FRegex, vocab: &StyleVocab) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |w: &str| {
        let k = vocab.canon(w);
        if !out.iter().any(|(c, _)| *c == k) {
            out.push((k, w.to_string()));
        }
    };
    if content {
        let t = format!(" {} ", key(s));
        for c in re.captures_iter(&t).flatten() {
            let w = c.get(1).unwrap().as_str();
            if !SOUND_WORDS.contains(&w) {
                add(w);
            }
        }
        return out;
    }
    for piece in PIECES.split(s) {
        let k = key(piece);
        let k = k.split(' ').skip_while(|w| w.chars().all(|c| c.is_ascii_digit())).collect::<Vec<_>>().join(" ");
        let t = format!(" {k} ");
        if let Ok(Some(c)) = re.captures(&t)
            && c.get(1).unwrap().start() == 1
        {
            add(c.get(1).unwrap().as_str());
        }
    }
    out
}

/// The style a song's folders name, the nearest folder first (canonical), with the user's words.
pub fn named_style(dir: &str, vocab: &StyleVocab) -> Option<String> {
    let (re, _) = vocab.regexes();
    dir.split('/').filter(|x| !x.is_empty()).rev().find_map(|f| styles_in(f, false, &re, vocab).into_iter().next().map(|x| x.0))
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
    /// The songs whose reference render sounds closest to this one's (audio analysis,
    /// opt-in): (song id, cosine similarity), closest first, songs of the same folder left
    /// out. Empty when its render was not analysed.
    #[serde(default)]
    pub sound: Vec<(i64, f64)>,
    /// What Spool hears in the render (CLAP, crates/audio's `styles::heard`): (class
    /// "Genre---Style", strength),
    /// strongest first. Empty when it was not heard.
    #[serde(default)]
    pub heard: Vec<(String, f32)>,
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

fn guess(
    song: &Prepared,
    known: &[(&Prepared, String)],
    (re, inside): (&FRegex, &FRegex),
    vocab: &StyleVocab,
    library: &HashSet<String>,
) -> Vec<(String, f64, Vec<String>)> {
    let mut p = Points::default();
    let s = song.s;
    // 1. names: the song's own folder, the folders above, the files
    let parts: Vec<&str> = s.dir.split('/').filter(|x| !x.is_empty()).collect();
    let mut any_folder = false;
    for (i, folder) in parts.iter().rev().enumerate() {
        for (st, _) in styles_in(folder, false, re, vocab) {
            let name = folder.trim_matches(|c| c == ' ' || c == '-' || c == '_');
            p.add(&st, if i == 0 { 5.0 } else { 4.0 }, i18n::tr!("style.folder", name = name));
            any_folder = true;
        }
    }
    let mut named: Vec<(String, String)> = Vec::new();
    for stem in s.stems.iter().map(String::as_str).chain(song.audio.iter().copied()) {
        for (st, _) in styles_in(stem, false, re, vocab) {
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
            for (st, _) in styles_in(t, true, inside, vocab) {
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
    // And one song alike is a weak hint: plugins every project has (a monitoring correction
    // on the master, Kontakt) make one song "like" half the library, and a single answer on
    // the styles page used to give its style to all of them (then, through the second pass,
    // to the songs like those). Full weight from three songs alike.
    let weight = if !named.is_empty() || any_folder { 2.0 } else { 6.0 } * (near.len() as f64 / 3.0).min(1.0);
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
    // 4. the sound of the reference render: a vote of the closest songs with a style
    if let Some((st, share, titles, n)) = sound_vote(&s.sound, known) {
        let titles: Vec<String> = titles.into_iter().take(2).map(|t| i18n::tr!("style.quoted", title = t)).collect();
        p.add(&st, round2(SOUND_POINTS * share), i18n::tr!("style.soundsLike", titles = titles.join(&i18n::tr!("style.and")), n = n));
    }
    // 5. what Spool hears in the render: its three strongest styles, each counting for the
    // style of ours it names (or for itself), as strongly as it is heard
    let mut heard: Vec<(String, f64, Vec<String>)> = Vec::new();
    let named_here: Vec<String> = p.order.clone();
    for (class, act) in s.heard.iter().take(3).filter(|h| h.1 >= HEARD_MIN) {
        let (mut st, _) = heard_style(class, vocab);
        // a style of a genre ("Roots Reggae") counts for the genre where this song's names
        // say it, or where the library's names use the genre and never the style itself
        if !named_here.contains(&st)
            && let Some(g) = heard_genres(class)
                .into_iter()
                .find(|g| named_here.iter().any(|n| n == g) || (!library.contains(&st) && library.contains(*g)))
        {
            st = g.to_string();
        }
        let name = class.rsplit("---").next().unwrap_or(class).to_string();
        let pct = round_int(*act as f64 * 100.0);
        match heard.iter_mut().find(|h| h.0 == st) {
            Some(h) => {
                h.1 += *act as f64;
                h.2.push(i18n::tr!("style.heardOne", name = name, pct = pct));
            }
            None => heard.push((st, *act as f64, vec![i18n::tr!("style.heardOne", name = name, pct = pct)])),
        }
    }
    for (st, act, names) in heard {
        p.add(&st, round2((act * HEARD_POINTS).min(HEARD_MAX)), i18n::tr!("style.heard", list = names.join(", ")));
    }
    // 5b. nothing in the names: the styles heard added up by genre. "Modern", "Romantic" and
    // "Neo-Romantic" each heard weakly say together, clearly, classical.
    if !any_folder && named.is_empty() {
        let mut by: Vec<(&str, f64, Vec<String>)> = Vec::new();
        for (class, act) in s.heard.iter().filter(|h| h.1 >= GENRE_MIN) {
            let Some(g) = heard_genres(class).first().copied() else { continue };
            let one = i18n::tr!("style.heardOne", name = class.rsplit("---").next().unwrap_or(class), pct = round_int(*act as f64 * 100.0));
            match by.iter_mut().find(|x| x.0 == g) {
                Some(x) => {
                    x.1 += *act as f64;
                    x.2.push(one);
                }
                None => by.push((g, *act as f64, vec![one])),
            }
        }
        if let Some((g, sum, names)) = by.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).filter(|x| x.1 >= GENRE_SUM && x.2.len() >= 2) {
            p.add(g, round2((sum * HEARD_POINTS).clamp(MIN_POINTS, HEARD_MAX)), i18n::tr!("style.heardGenre", genre = g, list = names.join(", ")));
        }
    }
    // 6. tempo, drum and bass only
    if let Some(t) = tempo.filter(|t| *t != 0.0 && (165.0..=180.0).contains(t)) {
        p.add("dnb", 1.5, i18n::tr!("style.dnbTempo", tempo = round_int(t)));
    }
    p.ranked()
}

/// The audio vote for a song: (canonical style, its share of the weight, titles of the songs
/// that voted for it, how many), or None when the closest songs do not agree enough.
fn sound_vote(near: &[(i64, f64)], known: &[(&Prepared, String)]) -> Option<(String, f64, Vec<String>, usize)> {
    let voters: Vec<(f64, &Prepared, &String)> = near
        .iter()
        .filter_map(|(id, sim)| known.iter().find(|(k, _)| k.s.id == *id).map(|(k, st)| (sim.max(0.0), *k, st)))
        .take(SOUND_K)
        .collect();
    if voters.len() < SOUND_K.min(3) {
        return None;
    }
    let total: f64 = voters.iter().map(|v| v.0).sum();
    let mut best: Option<(&String, f64)> = None;
    for (_, _, st) in &voters {
        let w: f64 = voters.iter().filter(|v| v.2 == *st).map(|v| v.0).sum();
        if best.is_none_or(|b| w > b.1) {
            best = Some((st, w));
        }
    }
    let (st, w) = best?;
    let share = if total > 0.0 { w / total } else { 0.0 };
    (share >= SOUND_SHARE).then(|| {
        let mine: Vec<&(f64, &Prepared, &String)> = voters.iter().filter(|v| v.2 == st).collect();
        (st.clone(), share, mine.iter().map(|v| v.1.s.title.clone().unwrap_or_default()).collect(), mine.len())
    })
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
/// `deduce` in two passes: the songs whose style the clues make clear (60 % and more) then
/// serve as examples, so a song with nothing but its sound (a render, no telling name) gets
/// the style of the songs it sounds like. Only the user's own styles counted before, and
/// they are few: most songs were left with "no clear clue".
pub fn deduce_with_examples(songs: &[StyleSong], rules: &Rules) -> Vec<(i64, Vec<StyleOption>)> {
    const CLEAR: i64 = 60;
    let first = deduce(songs, rules);
    let clear: std::collections::HashMap<i64, String> =
        first.iter().filter_map(|(id, o)| o.first().filter(|x| x.pct >= CLEAR).map(|x| (*id, x.style.clone()))).collect();
    if clear.is_empty() {
        return first;
    }
    let examples: Vec<StyleSong> = songs
        .iter()
        .map(|s| {
            let mut s = s.clone();
            if let Some(st) = clear.get(&s.id) {
                s.style = Some(st.clone());
            }
            s
        })
        .collect();
    let second: std::collections::HashMap<i64, Vec<StyleOption>> = deduce(&examples, rules).into_iter().collect();
    first
        .into_iter()
        .map(|(id, o)| match second.get(&id) {
            Some(o2) if !clear.contains_key(&id) && !o2.is_empty() => (id, o2.clone()),
            _ => (id, o),
        })
        .chain(second.iter().filter(|(id, o)| !o.is_empty() && !clear.contains_key(id)).map(|(id, o)| (*id, o.clone())))
        .fold(Vec::new(), |mut v: Vec<(i64, Vec<StyleOption>)>, (id, o)| {
            if !v.iter().any(|(x, _)| *x == id) {
                v.push((id, o));
            }
            v
        })
}

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
    let (re, inside) = rules.styles.regexes();
    // the styles the library's names use, and the ones written
    let library: HashSet<String> = prepared
        .iter()
        .flat_map(|p| p.s.dir.split('/').flat_map(|f| styles_in(f, false, &re, &rules.styles)).map(|x| x.0))
        .chain(known.iter().map(|k| k.1.clone()))
        .collect();
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
    // the user's own style words and names as they write them
    for (w, n) in &rules.styles.names {
        spelling.entry(rules.styles.canon(w)).or_insert_with(|| n.trim().to_string());
    }
    for w in &rules.styles.words {
        spelling.entry(canon(w)).or_insert_with(|| w.trim().to_string());
    }
    // the styles heard
    for p in &prepared {
        for (class, _) in &p.s.heard {
            let (c, shown) = heard_style(class, &rules.styles);
            spelling.entry(c).or_insert(shown);
        }
    }
    prepared
        .iter()
        .filter(|p| p.s.style.as_ref().is_none_or(|x| x.is_empty()))
        .filter_map(|p| {
            let opts = options(&guess(p, &known, (&re, &inside), &rules.styles, &library), &spelling);
            (!opts.is_empty()).then_some((p.s.id, opts))
        })
        .collect()
}

/// Every style Spool knows, in lower case, sorted: what the styles page offers as names.
pub fn known_styles() -> Vec<String> {
    let mut v: Vec<String> = BASE.iter().filter(|w| !ALIASES.iter().any(|a| key(a.0) == **w && a.1 != w.as_str())).cloned().collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(s: &str) -> Vec<String> {
        styles_in(s, false, &FULL, &StyleVocab::default()).into_iter().map(|x| x.0).collect()
    }

    #[test]
    fn a_style_inside_a_broader_one_keeps_its_name() {
        assert_eq!(found("Deep House - Night"), ["deep house"]);
        assert_eq!(found("Bass House - Drop"), ["bass house"]);
        // a style of the user's: Spool alone only knows the broader one
        assert_eq!(found("Jazz Fanfare - Brass Day"), ["jazz"]);
        let mine = StyleVocab { words: vec!["Jazz Fanfare".into()], links: vec![], names: vec![] };
        assert_eq!(styles_in("Jazz Fanfare - Brass Day", false, &mine.regexes().0, &StyleVocab::default())[0].0, "jazz fanfare");
    }

    #[test]
    fn the_400_styles_are_read_in_names_however_written() {
        assert_eq!(found("Hip-Hop - Boom Bap - Rue"), ["hiphop", "boom bap"]);
        assert_eq!(found("Musique Concrète"), ["musique concrete"]);
        assert_eq!(found("Lo-Fi beats"), ["lofi"]);
        assert_eq!(found("Drum & Bass"), ["dnb"]);
        // no style: a version, a groove, a word inside a name
        assert!(found("Fit - Instrumental").is_empty());
        assert!(found("Daft Punk - Get Lucky").is_empty());
        assert_eq!(found("6. DNB D# - Lost (Jungle)"), ["dnb", "jungle"]);
        assert_eq!(found("Take Off (DNB)"), ["dnb"]);
        assert!(found("Swing 60").is_empty());
        // inside a project, the common styles only
        assert!(styles_in("Tribal Perc", true, &INSIDE, &StyleVocab::default()).is_empty());
        assert_eq!(styles_in("Trap Kick", true, &INSIDE, &StyleVocab::default())[0].0, "trap");
        assert_eq!(styles_in("MNBOY Bass House Drum Loop 7", true, &INSIDE, &StyleVocab::default())[0].0, "bass house");
        assert!(is_style_name("Deep House") && is_style_name("hip-hop") && !is_style_name("jazz fanfare"));
    }

    #[test]
    fn what_spool_hears_counts_for_our_styles() {
        let none = StyleVocab::default();
        assert_eq!(heard_style("Electronic---Drum n Bass", &none), ("dnb".to_string(), "dnb".to_string()));
        assert_eq!(heard_style("Electronic---Dub", &none).0, "dub");
        // a style of a genre we name
        assert_eq!(heard_style("Hip Hop---Boom Bap", &none).0, "boom bap");
        assert_eq!(heard_genres("Hip Hop---Boom Bap"), ["hiphop"]);
        assert_eq!(heard_genres("Funk / Soul---Neo Soul"), ["soul", "funk"]);
        assert!(heard_genres("Electronic---Deep House").is_empty());
        // none of ours: its own name
        assert_eq!(heard_style("Electronic---Deep House", &none), ("deep house".to_string(), "deep house".to_string()));
        // the user's name for it
        let mine = StyleVocab { words: vec!["jazz fanfare".into()], links: vec![("brass band".into(), "jazz fanfare".into())], names: vec![] };
        assert_eq!(heard_style("Brass & Military---Brass Band", &mine).0, "jazz fanfare");
    }

    #[test]
    fn the_users_own_style_words_are_found_in_names() {
        let mine = StyleVocab { words: vec!["Neo Soul".into()], links: vec![], names: vec![] };
        let (re, _) = mine.regexes();
        assert_eq!(styles_in("Lantern Swing - Velours", false, &re, &StyleVocab::default()).len(), 0);
        let mine = StyleVocab { words: vec!["Lantern Swing".into()], links: vec![], names: vec![] };
        let (re, _) = mine.regexes();
        assert_eq!(styles_in("Lantern Swing - Velours", false, &re, &StyleVocab::default())[0].0, "lantern swing");
        assert!(styles_in("Lantern Swing - Velours", false, &FULL, &StyleVocab::default()).is_empty());
    }

    #[test]
    fn a_word_of_the_users_folders_under_the_name_they_give_it() {
        let v = StyleVocab { words: vec!["ORC".into()], links: vec![], names: vec![("ORC".into(), "Orchestral".into())] };
        let (re, _) = v.regexes();
        assert_eq!(styles_in("ORC - Reborn", false, &re, &v)[0].0, "orchestral");
        assert_eq!(named_style("a/ORC - Reborn", &v).as_deref(), Some("orchestral"));
        assert!(known_styles().contains(&"deep house".to_string()) && !known_styles().contains(&"drum n bass".to_string()));
    }

    #[test]
    fn instrumentations_are_not_suggested() {
        assert!(found("Piano Ballad").is_empty());
        assert!(found("Orchestral - Theme").is_empty());
        // a style named after one is still one
        assert_eq!(found("Piano Blues"), ["piano blues"]);
    }
}

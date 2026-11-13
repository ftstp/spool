//! Names: variant keys, folder names read as title / style / key / artist, titles.
use crate::difflib::SequenceMatcher;
use fancy_regex::Regex as FRegex;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// A path component that only holds superseded versions of the song around it.
static VERSION_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(old|olds|olddies|oldies|old versions?|versions?|backups?|archives?|trash)$").unwrap()
});
static PROJECT_DIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+project(\s+\d+)?$").unwrap());
static LEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\s\-_.]+").unwrap());

pub const STYLE_WORDS: &[&str] = &[
    "dnb",
    "drum & bass",
    "drum and bass",
    "jungle",
    "electro",
    "house",
    "bass house",
    "uk house",
    "techno",
    "breakbeat",
    "breakbeat house",
    "reggae",
    "dub",
    "lofi",
    "lo-fi",
    "jazz",
    "jazz fanfare",
    "fanfare",
    "afrobeat",
    "afro",
    "trip hop",
    "hip hop",
    "hiphop",
    "soul",
    "funk",
    "downtempo",
    "electro downtempo",
    "electro downtmepo",
    "acoustic",
    "orchestral",
    "piano",
    "glitch",
    "dubstep",
    "acid",
    "blues",
    "experimental",
    "80s",
    "new wave",
    "electro & dnb",
    "disco",
    "garage",
    "ambient",
    "pop",
    "rock",
];
static KEY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Ga-g](#|b)?m?$").unwrap());
static CATALOG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z]{2,6}\d{2,5})\s*-\s*").unwrap());
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{2})\.(\d{2})\.(\d{2})\s*-\s*").unwrap());
static FT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\((?:ft|feat)\.?\s+([^)]+)\)").unwrap());
static PART_SPLIT: LazyLock<FRegex> = LazyLock::new(|| FRegex::new(r"\s+-\s+|\s+-(?=\S)|(?<=\S)-\s+").unwrap());
static TRAILING_PAREN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([^)]+)\)\s*$").unwrap());

/// Words that mark a working copy rather than a different song.
const WORK_WORDS: &[&str] = &[
    "wip",
    "final",
    "finale",
    "new",
    "old",
    "test",
    "rough",
    "clean",
    "update",
    "updated",
    "copy",
    "copie",
    "last",
    "real",
    "fix",
    "fixed",
    "maj",
    "majals",
    "ab12",
    "ab11",
    "ab10",
    "w12",
    "draft",
    "version",
    "remaster",
    "night",
    "crash",
    "au",
    "cas",
    "ou",
    "touch",
    "rearrange",
    "vsonarworks",
    "sonarworks",
    "heavy",
    "lighten",
    "bis",
];

/// Stage suggested by a parent folder name, in order.
static STATUS_HINTS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"soon to release|released|release|sortie|masters?\b|mastering", "master"),
        (r"singles? to plan|to plan|mix(ing)?\b", "mix"),
        (r"arrang", "arrangement"),
        (r"draft|idea|sketch|brouillon|vrac", "draft"),
        (r"hold|pause|frozen", "pause"),
    ]
    .into_iter()
    .map(|(r, s)| (Regex::new(r).unwrap(), s))
    .collect()
});

static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.*?\]").unwrap());
static AFTER_VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bv\s?\d.*$").unwrap());
static DOTTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+(\.\d+)+\b").unwrap());
static YEARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());
static AB1X: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.?ab1\d\b").unwrap());
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static PRETTY_AB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.?ab1\d$").unwrap());
static PRETTY_V: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+v\s?\d+[\w.]*.*$").unwrap());
static PRETTY_EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.(mp3|wav|aiff?)$").unwrap());
/// "Name [RCN]", "Name [R] 2", "Name - fixed": copies written by the repair / the treatments.
static PROCESSED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<base>.*?)(?: - fixed| \[(?P<codes>[RCNOPMG]+)\](?: (?P<n>\d+))?)$").unwrap());

/// NFKD, accents dropped, lower case.
pub fn ascii_low(s: &str) -> String {
    s.nfkd().filter(char::is_ascii).collect::<String>().to_lowercase()
}

/// "- 3 ARRANGEMENT" -> "3 arrangement": leading dashes / dots / spaces are sort tricks.
pub fn clean_component(name: &str) -> String {
    LEADING.replace(&ascii_low(name), "").trim().to_string()
}

pub fn is_version_dir(name: &str) -> bool {
    VERSION_DIR.is_match(&clean_component(name))
}

pub fn is_project_dir(name: &str) -> bool {
    PROJECT_DIR.is_match(&ascii_low(name))
}

static PROJECT_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+project(\s+\d+)?$").unwrap());

/// "ID 04.11.2026 Project" -> "ID 04.11.2026" (Live's own suffix, "Project 2" included).
pub fn strip_project_suffix(name: &str) -> &str {
    match PROJECT_SUFFIX.find(name) {
        Some(m) if m.start() > 0 => &name[..m.start()],
        _ => name,
    }
}

/// The user's settings that shape song names: artist / alias names, working-copy words.
#[derive(Debug, Clone, Default)]
pub struct Rules {
    pub artists: Vec<String>,
    /// Artist names as phrases to strip from file names, longest first.
    phrases: Vec<(String, Regex, Regex)>,
    work_words: HashSet<String>,
}

impl Rules {
    pub fn new(artists: &[String], work_words: &[String]) -> Rules {
        let mut set: BTreeSet<String> = BTreeSet::new();
        for a in artists.iter().filter(|a| !a.is_empty()) {
            set.insert(NON_ALNUM.replace_all(&ascii_low(a), " ").trim().to_string());
        }
        let mut phrases: Vec<String> = set.into_iter().collect();
        phrases.sort_by_key(|p| std::cmp::Reverse(p.len())); // stable: equal lengths stay sorted
        let phrases = phrases
            .into_iter()
            .map(|p| {
                let e = regex::escape(&p);
                (p, Regex::new(&format!(r"\b{e}\b")).unwrap(), Regex::new(&format!(r"(?i)^{e}\s*-\s*")).unwrap())
            })
            .collect();
        let mut words: HashSet<String> = WORK_WORDS.iter().map(|w| w.to_string()).collect();
        words.extend(work_words.iter().map(|w| ascii_low(w).trim().to_string()).filter(|w| !w.is_empty()));
        Rules { artists: artists.to_vec(), phrases, work_words: words }
    }

    /// File name reduced to what makes it a different song: versions, dates and working
    /// words stripped. "AFROBEAT - Selo - ORBIT v8.1" and "ORBIT v4.4" differ (prefix),
    /// which is why old files are attached by similarity, not by equality.
    pub fn variant_key(&self, stem: &str) -> String {
        let s = ascii_low(stem);
        let s = BRACKETS.replace_all(&s, " ");
        // What follows a version number is a note about that version, not another song.
        let s = AFTER_VERSION.replace_all(&s, " ");
        let s = DOTTED.replace_all(&s, " ");
        let s = YEARS.replace_all(&s, " ");
        let s = AB1X.replace_all(&s, " ");
        let mut s = NON_ALNUM.replace_all(&s, " ").to_string();
        for (_, rx, _) in &self.phrases {
            s = rx.replace_all(&s, " ").to_string();
        }
        let words: Vec<&str> = s.split_whitespace().filter(|w| !self.work_words.contains(*w)).collect();
        let named: Vec<&str> = words.iter().copied().filter(|w| !w.chars().all(|c| c.is_ascii_digit())).collect();
        // "312 Bbm_104", "318 A Phrygian_115": a number, then only the key and the tempo. The
        // key and the tempo describe the sketch, the number names it: without this, every
        // sketch in B flat minor would be one song.
        if !named.is_empty()
            && named.iter().all(|w| is_musical_key(w))
            && let Some(n) = words.first().filter(|w| w.chars().all(|c| c.is_ascii_digit()))
        {
            return n.to_string();
        }
        named.join(" ")
    }

    /// File name shown as a song title: version and working suffixes removed, case kept.
    pub fn pretty_stem(&self, stem: &str) -> String {
        pretty_stem_with(stem, &self.phrases)
    }
}

fn pretty_stem_with(stem: &str, phrases: &[(String, Regex, Regex)]) -> String {
    let s = BRACKETS.replace_all(stem, "").to_string();
    let s = PRETTY_AB.replace(s.trim(), "").to_string();
    let s = PRETTY_V.replace(&s, "").to_string();
    let s = PRETTY_EXT.replace(&s, "").to_string();
    let mut s = LEADING.replace(&s, "").to_string();
    // "Ivo Sand - Motion" -> "Motion": the artist is a field of its own.
    for (_, _, start) in phrases {
        s = start.replace(&s, "").to_string();
    }
    let t = s.trim_matches(|c| c == ' ' || c == '-' || c == '_');
    if t.is_empty() { stem.to_string() } else { t.to_string() }
}

/// A word of a file name that is a musical key or mode ("bbm", "a", "phrygian", "min").
fn is_musical_key(w: &str) -> bool {
    const MODES: [&str; 13] = [
        "m", "min", "maj", "minor", "major", "dur", "moll", "phrygian", "dorian", "lydian", "mixolydian", "aeolian",
        "locrian",
    ];
    if MODES.contains(&w) {
        return true;
    }
    let mut c = w.chars();
    if !c.next().is_some_and(|n| ('a'..='g').contains(&n)) {
        return false;
    }
    let rest: String = c.collect();
    ["", "m", "b", "bm", "s", "sm", "min", "maj", "bmin", "bmaj", "smin", "smaj"].contains(&rest.as_str())
}

/// `pretty_stem` without artist rules.
pub fn pretty_stem(stem: &str) -> String {
    pretty_stem_with(stem, &[])
}

/// 0..1, how close two variant keys are. Below `floor` the exact value does not matter:
/// a cheap upper bound is returned instead of the full comparison.
pub fn similarity(a: &str, b: &str, floor: f64) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    // Whole words only: "glory" is in "orchestral glory", "under" is NOT in
    // "misunderstanding". The more of the longer name the shorter covers, the closer.
    let (ta, tb): (HashSet<&str>, HashSet<&str>) = (a.split_whitespace().collect(), b.split_whitespace().collect());
    if ta.is_subset(&tb) || tb.is_subset(&ta) {
        return 0.8 + 0.2 * ta.len().min(tb.len()) as f64 / ta.len().max(tb.len()) as f64;
    }
    let sm = SequenceMatcher::new(a, b);
    if floor != 0.0 && (sm.real_quick_ratio() < floor || sm.quick_ratio() < floor) {
        return sm.quick_ratio();
    }
    sm.ratio()
}

/// ("Name", Some("RCN")) for "Name [RCN]", ("Name", Some("R")) for "Name - fixed".
pub fn split_processed(stem: &str) -> (String, Option<String>) {
    match PROCESSED.captures(stem) {
        Some(m) => (m["base"].to_string(), Some(m.name("codes").map(|c| c.as_str()).unwrap_or("R").to_string())),
        None => (stem.to_string(), None),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FolderName {
    pub title: Option<String>,
    pub style: Option<String>,
    pub key: Option<String>,
    pub ft: Option<String>,
    pub catalog: Option<String>,
    pub date: Option<String>,
    pub artist: Option<String>,
}

/// "dnb - D#m - Lantern (ft Ana)" -> style dnb, key D#m, title Lantern, ft Ana.
/// "CAT017 - ivo sand - Satellite (lofi)" -> catalogue, artist, title, style.
/// "- 24.03.09 - electro - Prism" -> date 2024-03-09, style electro, title Prism.
pub fn parse_folder_name(name: &str, known_artists: &[String]) -> FolderName {
    let mut out = FolderName::default();
    let mut s = LEADING.replace(name, "").trim().to_string();
    if let Some(m) = DATE_RE.captures(&s) {
        let (yy, mm, dd) = (m[1].to_string(), m[2].to_string(), m[3].to_string());
        let (mi, di) = (mm.parse::<u32>().unwrap_or(0), dd.parse::<u32>().unwrap_or(0));
        if (1..=12).contains(&mi) && (1..=31).contains(&di) {
            out.date = Some(format!("20{yy}-{mm}-{dd}"));
        }
        s = s[m.get(0).unwrap().end()..].to_string();
    }
    if let Some(m) = CATALOG_RE.captures(&s) {
        out.catalog = Some(m[1].to_string());
        s = s[m.get(0).unwrap().end()..].to_string();
    }
    if let Some(m) = FT_RE.captures(&s) {
        out.ft = Some(m[1].trim().to_string());
        let all = m.get(0).unwrap();
        s = format!("{}{}", &s[..all.start()], &s[all.end()..]).trim().to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut last = 0;
    for m in PART_SPLIT.find_iter(&s).flatten() {
        parts.push(s[last..m.start()].to_string());
        last = m.end();
    }
    parts.push(s[last..].to_string());
    let parts: Vec<String> = parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    let mut artists: HashMap<String, String> = HashMap::new();
    for a in known_artists {
        artists.insert(ascii_low(a), a.clone());
    }
    let mut rest: Vec<String> = Vec::new();
    let n = parts.len();
    for (i, p) in parts.iter().enumerate() {
        let low = ascii_low(p);
        if out.style.is_none() && STYLE_WORDS.contains(&low.as_str()) && n > 1 && i < n - 1 {
            out.style = Some(p.trim().to_string());
        } else if out.key.is_none() && KEY_RE.is_match(p) && n > 1 {
            out.key = Some(p.clone());
        } else if out.artist.is_none() && artists.contains_key(&low) && n > 1 {
            out.artist = Some(artists[&low].clone());
        } else {
            rest.push(p.clone());
        }
    }
    let mut title = rest.join(" - ").trim().to_string();
    if let Some(m) = TRAILING_PAREN.captures(&title)
        && STYLE_WORDS.contains(&ascii_low(&m[1]).as_str())
    {
        if out.style.is_none() {
            out.style = Some(m[1].to_string());
        }
        title = title[..m.get(0).unwrap().start()].trim().to_string();
    }
    out.title = if title.is_empty() { None } else { Some(title) };
    out
}

/// Nearest folder (from the song upward) naming a known artist / alias.
pub fn artist_for(folders: &[&str], known_artists: &[String]) -> Option<String> {
    for f in folders.iter().rev() {
        let low = ascii_low(f);
        for a in known_artists {
            if low.contains(&ascii_low(a)) {
                return Some(a.clone());
            }
        }
    }
    None
}

/// Stage suggested by a parent folder name ("- SOON TO RELEASE" -> master).
pub fn status_hint<'f>(folders: &[&'f str]) -> Option<(&'static str, &'f str)> {
    for f in folders.iter().rev() {
        let low = clean_component(f);
        for (rx, stage) in STATUS_HINTS.iter() {
            if rx.is_match(&low) {
                return Some((stage, f));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_keys() {
        let r = Rules::new(&["Ivo Sand".into()], &[]);
        assert_eq!(r.variant_key("Mirador v2.2 - 128 au lieu de 144"), "mirador");
        assert_eq!(r.variant_key("Ivo Sand - Glass Hour"), "glass hour");
        assert_eq!(r.variant_key("ORBIT v4.4"), "orbit");
        // a numbered sketch named only by its key and tempo is its number
        assert_eq!(r.variant_key("312 Bbm_104"), "312");
        assert_eq!(r.variant_key("318 A Phrygian_115"), "318");
        assert_eq!(r.variant_key("416 Cm_128 v2"), "416");
        // a real name stays the name
        assert_eq!(r.variant_key("12 Bad Habits"), "bad habits");
        assert_eq!(r.variant_key("Em"), "em");
        assert_eq!(r.pretty_stem("Ivo Sand - Motion v3"), "Motion");
    }

    #[test]
    fn folder_names() {
        let f = parse_folder_name("dnb - D#m - Lantern (ft Ana)", &[]);
        assert_eq!(
            (f.style.as_deref(), f.key.as_deref(), f.title.as_deref(), f.ft.as_deref()),
            (Some("dnb"), Some("D#m"), Some("Lantern"), Some("Ana"))
        );
        let f = parse_folder_name("- 24.03.09 - electro - Prism", &[]);
        assert_eq!(
            (f.date.as_deref(), f.style.as_deref(), f.title.as_deref()),
            (Some("2024-03-09"), Some("electro"), Some("Prism"))
        );
        let f = parse_folder_name("CAT017 - ivo sand - Satellite (lofi)", &["Ivo Sand".into()]);
        assert_eq!(
            (f.catalog.as_deref(), f.artist.as_deref(), f.title.as_deref(), f.style.as_deref()),
            (Some("CAT017"), Some("Ivo Sand"), Some("Satellite"), Some("lofi"))
        );
    }

    #[test]
    fn processed_copies() {
        assert_eq!(split_processed("Fit [RCN]"), ("Fit".into(), Some("RCN".into())));
        assert_eq!(split_processed("Fit [R] 2"), ("Fit".into(), Some("R".into())));
        assert_eq!(split_processed("Fit - fixed"), ("Fit".into(), Some("R".into())));
        assert_eq!(split_processed("Fit"), ("Fit".into(), None));
    }
}

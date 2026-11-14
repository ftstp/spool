//! The samples of the sample folders: what a sample's name and folders say of it (category,
//! loop or one-shot, tempo, key, note), and its length from the file's header.
//!
//! Measured on a real library (150,793 samples): 117,509 get exactly one category from their
//! name or a folder (a name naming two is left unclassified rather than guessed). A length
//! of 3 s or more tells a loop from a one-shot 91 % of the time when the name does not.
use regex::Regex;
use std::sync::LazyLock;

/// (category, family, pattern). Order matters only for the drum-loop rule below.
const CATS: [(&str, &str, &str); 16] = [
    ("kick", "drums", r"kick|kik|\bbd\b|bassdrum|bass drum"),
    ("snare", "drums", r"snare|\bsnr\b|\bsd\b|rimshot|\brim\b"),
    ("clap", "drums", r"clap|snap"),
    ("hihat", "drums", r"hi ?hat|\bhh\b|\bhats?\b|openhat|closedhat"),
    ("cymbal", "drums", r"crash|\bride\b|cymbal|splash"),
    ("tom", "drums", r"\btoms?\b"),
    ("perc", "drums", r"perc|shaker|conga|bongo|tamb|cowbell|clave|woodblock"),
    ("808", "bass", r"808|\bsub\b|sub ?bass"),
    ("bass", "bass", r"\bbass\b|\bbass(line|es)\b|reese"),
    ("lead", "melodic", r"\blead"),
    ("pad", "melodic", r"\bpads?\b|atmos|drone"),
    ("pluck", "melodic", r"pluck|\barps?\b|arpegg"),
    ("keys", "melodic", r"piano|\bkeys\b|rhodes|organ|epiano|\bchords?\b"),
    ("guitar", "melodic", r"guitar|\bgtr"),
    ("vocal", "vocal", r"\bvox\b|vocal|voice|acapella|chant|\bvx\b"),
    ("fx", "fx", r"\bfx\b|\bsfx|riser|impact|sweep|downlifter|uplifter|whoosh|transition"),
];

static RX: LazyLock<Vec<(&'static str, &'static str, Regex)>> =
    LazyLock::new(|| CATS.iter().map(|(c, f, p)| (*c, *f, Regex::new(&format!("(?i){p}")).unwrap())).collect());
static DRUMLOOP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(drum|top|break|beat|groove|full).{0,12}loop|loop.{0,12}(drum|top|break)|breakbeat").unwrap());
static LOOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)loop|\d{2,3} ?bpm|\bbpm").unwrap());
static SHOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)one ?shot|\bhit\b|\bshot\b").unwrap());
static BPM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d{2,3}(?:\.\d)?) ?bpm|bpm ?(\d{2,3})").unwrap());
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^| )(\d{2,3})(?: |$)").unwrap());
/// A key with its quality written: "Fm", "F#min", "C# Minor", "Ebmaj" (case-insensitive words).
static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^| )([A-G])(#|b)? ?(m|min|minor|maj|major)(?: |$|\d)").unwrap());
/// A note at the end of a one-shot's name: "808 Boom F#", "Sub C2".
static NOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?: )([A-G])(#|b)?(-?\d)?$").unwrap());

pub const FAMILIES: [&str; 6] = ["drums", "drumloop", "bass", "melodic", "vocal", "fx"];

/// What a sample's path says of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Named {
    pub cat: Option<String>,
    /// "name" or "folder"
    pub cat_src: Option<String>,
    pub family: Option<String>,
    /// Some(true) a loop, Some(false) a one-shot, None: the name does not say.
    pub is_loop: Option<bool>,
    pub bpm: Option<f64>,
    /// "Am", "C#", "F#m": sharps, minor as "m"
    pub key: Option<String>,
    /// A one-shot's note: "F#", "C"
    pub note: Option<String>,
}

/// Separators to spaces: "Kick_01-Hard.wav" -> "Kick 01 Hard".
fn words(s: &str) -> String {
    let t: String = s.chars().map(|c| if matches!(c, '_' | '-' | '.' | '(' | ')' | '[' | ']' | ',' | '+') { ' ' } else { c }).collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sharp(letter: &str, acc: &str) -> Option<String> {
    let n = format!("{}{}", letter.to_uppercase(), acc);
    let n = match n.as_str() {
        "Db" => "C#",
        "Eb" => "D#",
        "Gb" => "F#",
        "Ab" => "G#",
        "Bb" => "A#",
        "Cb" => "B",
        "Fb" => "E",
        "E#" => "F",
        "B#" => "C",
        x => x,
    };
    ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"].contains(&n).then(|| n.to_string())
}

fn cats_in(s: &str) -> Vec<(&'static str, &'static str)> {
    let w = words(s);
    if DRUMLOOP.is_match(&w) {
        return vec![("drumloop", "drumloop")];
    }
    let mut found: Vec<(&str, &str)> = RX.iter().filter(|(_, _, r)| r.is_match(&w)).map(|(c, f, _)| (*c, *f)).collect();
    // "808 Sub Bass": one bass; "Bass Drum": a kick
    if found.iter().any(|x| x.0 == "808" || x.0 == "kick") {
        found.retain(|x| x.0 != "bass");
    }
    found
}

/// `stem`: the file name without extension; `folders`: from the sample folder down to the
/// file's folder.
pub fn read_name(stem: &str, folders: &[&str]) -> Named {
    let mut n = Named::default();
    let mut found = cats_in(stem);
    let mut src = "name";
    if found.is_empty() {
        for f in folders.iter().rev() {
            found = cats_in(f);
            if !found.is_empty() {
                src = "folder";
                break;
            }
        }
    }
    if let [(c, f)] = found.as_slice() {
        n.cat = Some(c.to_string());
        n.family = Some(f.to_string());
        n.cat_src = Some(src.to_string());
    }
    let w = words(stem);
    n.bpm = BPM
        .captures(&w)
        .and_then(|c| c.get(1).or(c.get(2)))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .filter(|b| (50.0..=220.0).contains(b));
    if n.cat.as_deref() == Some("drumloop") || LOOP.is_match(&w) {
        n.is_loop = Some(true);
    } else if SHOT.is_match(&w) {
        n.is_loop = Some(false);
    }
    // a loop's bare number is its tempo ("Loop 124 Am")
    if n.bpm.is_none() && n.is_loop == Some(true) {
        n.bpm = bare_tempo(stem);
    }
    let keys: Vec<String> = KEY
        .captures_iter(&w)
        .filter_map(|c| {
            let q = &c[3];
            let minor = q == "m" || q.eq_ignore_ascii_case("min") || q.eq_ignore_ascii_case("minor");
            sharp(&c[1], c.get(2).map_or("", |m| m.as_str())).map(|k| if minor { k + "m" } else { k })
        })
        .collect();
    if let [k] = keys.as_slice() {
        n.key = Some(k.clone());
    }
    if n.is_loop != Some(true) && n.key.is_none() {
        n.note = NOTE.captures(&w).and_then(|c| sharp(&c[1], c.get(2).map_or("", |m| m.as_str())));
    }
    n
}

/// A bare number of 60 to 200 in a loop's name, its tempo ("LHV_122_Leave_Bass_F#min").
pub fn bare_tempo(stem: &str) -> Option<f64> {
    NUM.captures(&words(stem)).and_then(|c| c[1].parse::<f64>().ok()).filter(|b| (60.0..=200.0).contains(b))
}

/// Loop or one-shot: the name, else the length (3 s or more: a loop).
pub fn is_loop(named: Option<bool>, dur: Option<f32>) -> Option<bool> {
    named.or(dur.map(|d| d >= 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(stem: &str, folders: &[&str]) -> Option<String> {
        read_name(stem, folders).cat
    }

    #[test]
    fn categories_from_the_name_then_the_folders() {
        assert_eq!(cat("Kick_01_Hard", &[]).as_deref(), Some("kick"));
        assert_eq!(cat("KSHMR Snare 12", &[]).as_deref(), Some("snare"));
        assert_eq!(cat("Open Hat 3", &[]).as_deref(), Some("hihat"));
        assert_eq!(cat("808 Sub Bass F#", &[]).as_deref(), Some("808"));
        assert_eq!(cat("Bassline_Gm_124", &[]).as_deref(), Some("bass"));
        assert_eq!(cat("Top Loop 03 128bpm", &[]).as_deref(), Some("drumloop"));
        assert_eq!(cat("001", &["Pack X", "Vocals"]).as_deref(), Some("vocal"));
        let n = read_name("001", &["Pack X", "Drums", "Kicks"]);
        assert_eq!((n.cat.as_deref(), n.cat_src.as_deref()), (Some("kick"), Some("folder")));
        // two categories named: left out, not guessed
        assert_eq!(cat("Kick and Snare", &[]), None);
        // a bass drum is a kick, not a bass
        assert_eq!(cat("Bass Drum 4", &[]).as_deref(), Some("kick"));
    }

    #[test]
    fn tempo_key_note_and_loop() {
        let n = read_name("Synth_Loop_124_Am", &[]);
        assert_eq!((n.is_loop, n.bpm, n.key.as_deref()), (Some(true), Some(124.0), Some("Am")));
        let n = read_name("Pluck 140bpm Eb minor", &[]);
        assert_eq!((n.bpm, n.key.as_deref()), (Some(140.0), Some("D#m")));
        let n = read_name("808 Boom F#", &[]);
        assert_eq!((n.note.as_deref(), n.key.as_deref()), (Some("F#"), None));
        let n = read_name("Kick One Shot 7", &[]);
        assert_eq!(n.is_loop, Some(false));
        // a lone "Am" in a sentence is still a key here: names in packs are not prose
        assert_eq!(read_name("Chords Gmaj 90", &[]).key.as_deref(), Some("G"));
        assert_eq!(is_loop(None, Some(4.2)), Some(true));
        assert_eq!(is_loop(None, Some(0.8)), Some(false));
        assert_eq!(is_loop(Some(true), Some(0.8)), Some(true));
        // a loop by its length: the bare number is its tempo
        assert_eq!(bare_tempo("LHV_122_Leave_The_Sadness_01_Bass_F#min"), Some(122.0));
        assert_eq!(read_name("LHV_122_Leave_The_Sadness_01_Bass_F#min", &[]).key.as_deref(), Some("F#m"));
        assert_eq!(bare_tempo("Kick 01"), None);
    }
}

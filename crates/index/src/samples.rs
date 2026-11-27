//! The samples of the sample folders: what a sample's name and folders say of it (category,
//! loop or one-shot, tempo, key, note), and its length from the file's header.
//!
//! Measured on a real library (150,793 samples): 117,509 get exactly one category from their
//! name or a folder (a name naming two is left unclassified rather than guessed). A length
//! of 3 s or more tells a loop from a one-shot 91 % of the time when the name does not.
use regex::Regex;
use std::sync::LazyLock;

/// (category, family, pattern). Order matters only for the drum-loop rule below. "synth" is
/// the fallback of the melodic ones: said only when nothing more precise is ("Synth Lead" is a
/// lead, "Bass Synth" a bass, "Synth Shot 4" a synth).
const CATS: [(&str, &str, &str); 26] = [
    ("kick", "drums", r"kick|kik|\bkck\b|\bbd\b|bassdrum|bass drum"),
    ("snare", "drums", r"snare|\bsnr\b|\bsd\b|rimshot|\brim\b"),
    ("clap", "drums", r"clap|\bsnaps?\b"),
    ("hihat", "drums", r"hi ?hat|\bhh\b|\bhats?\b|openhat|closedhat"),
    ("cymbal", "drums", r"crash|\bride\b|cymbal|\bcyms?\b|\bcymb|splash"),
    ("tom", "drums", r"\btoms?\b"),
    ("perc", "drums", r"\bpercs?\b|\bperc\d|percussi|shaker|conga|bongo|tamb|maraca|cowbell|clave|woodblock|beatbox"),
    ("808", "bass", r"808"),
    ("sub", "bass", r"\bsub\b|sub ?bass"),
    ("growl", "bass", r"growl ?bass|bass ?growl|wobble|\bwobs?\b|\bwubs?\b|\byoi\b"),
    ("bass", "bass", r"\bbass\b|\bbass(line|es)\b|reese|synth ?bass"),
    ("lead", "melodic", r"\blead"),
    ("pad", "melodic", r"\bpads?\b|atmos|\batmo\b|athmo|ambient|drone"),
    ("pluck", "melodic", r"pluck|\barps?\b|arpegg"),
    ("keys", "melodic", r"piano|\bkeys\b|rhodes|wurli|marimba|mallet|kalimba|vibraphone|xylophone|\borgans?\b|epiano|\bchords?\b"),
    ("guitar", "melodic", r"guitar|sitar|banjo|ukulele|\bkoto\b|\bgtr"),
    ("brass", "melodic", r"brass|trumpet|trombone|\bsax|\bhorns?\b|tuba"),
    ("strings", "melodic", r"\bstrings?\b|violin|viola|cello|pizz|orchestra|\borch\b"),
    ("woodwind", "melodic", r"flute|whistle|clarinet|oboe|recorder|ocarina|bassoon"),
    ("synth", "melodic", r"\bsynths?\b|\bstabs?\b|\briffs?\b"),
    ("melody", "melodic", r"melod|music ?loop|\bmusc\b|\bmel\b"),
    ("vocal", "vocal", r"\bvox\b|vocal|voice|acapella|chant|\bvx\b"),
    ("fx", "fx", r"\bfx\b|\bsfx|riser|impact|sweep|downlifter|uplifter|whoosh|transition|build ?ups?\b|glitch|\bnoise|vinyl|crackle"),
    // a place, a crowd or a gesture recorded, not played: ambiences (an "Atmos" synth stays a pad)
    ("ambience", "ambience", r"field ?rec|location ?rec|\bambiences?\b|\bambiance|room ?tone|soundscape|\bnature\b|\brainfall\b|\bbirds?\b|\bforest\b|\bocean\b|\bcity\b|\bstreet\b|\btraffic\b"),
    ("crowd", "ambience", r"\bcrowds?\b|audience|applau|cheering|\bwalla\b"),
    ("foley", "ambience", r"foley|footsteps?"),
];

static RX: LazyLock<Vec<(&'static str, &'static str, Regex)>> =
    LazyLock::new(|| CATS.iter().map(|(c, f, p)| (*c, *f, Regex::new(&format!("(?i){p}")).unwrap())).collect());
/// Style names that hold a category word ("Bass House Clap 3" is a clap, not a bass):
/// taken out before the categories are looked for.
static GENRES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bass ?house|drum ?(and|&|n) ?bass|drumnbass|future ?bass|bass ?music|uk ?bass|bassline ?house|tech ?house|deep ?house|future ?house|vocal ?house|trap ?vocal|lo ?fi ?keys|wobble ?house)\b").unwrap()
});
static DRUMLOOP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(drum|top|break|beat|groove|full).{0,16}loop|loop.{0,12}(drum|top|break)|breakbeat|\bbreaks?\b|\bfills?\b|\bbeats?\b").unwrap());
/// A pack's demo (the full mix that presents it): its own family, whatever it is made of
/// ("House Bass (Demo)" is a demo, not a bass).
static DEMO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bdemos?\b|\bpreview\b|\bfull ?mix\b").unwrap());
/// "Drums 174 BPM - 9": a drum loop, when nothing more precise is named ("Drums - Hats 172bpm"
/// is a hi-hat loop).
static DRUMS_TEMPO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bdrums?\b.{0,16}\d{2,3} ?bpm\b|\bdrums \d{2,3}\b").unwrap());
/// A loop whose name and folders name no instrument: a whole mix ("VEE Electro Loop 358",
/// "Houseloops 128BPM 005", "Song Starter - Full", a construction kit's full mix).
static FULL_LOOP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)loop|\bfull\b|\bmix\b|construction|song ?starter").unwrap());
static DRUMS_NAMED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bdrums?\b|\bbreaks?\b|break ?beat").unwrap());
static INSTRUMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\binstrument").unwrap());
static LOOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)loop|\d{2,3} ?bpm|\bbpm").unwrap());
static SHOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)one ?shot|\bhit\b|\bshot\b").unwrap());
static BPM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d{2,3}(?:\.\d)?) ?bpm|bpm ?(\d{2,3})").unwrap());
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^| )(\d{2,3})(?: |$)").unwrap());
/// A key with its quality written: "Fm", "F#min", "C# Minor", "Ebmaj" (case-insensitive words).
static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^| )([A-G])(#|b)? ?(m|min|minor|maj|major)(?: |$|\d)").unwrap());
/// A note at the end of a one-shot's name: "808 Boom F#", "Sub C2".
static NOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?: )([A-G])(#|b)?(-?\d)?$").unwrap());

pub const FAMILIES: [&str; 9] = ["drums", "drumloop", "bass", "melodic", "vocal", "fx", "fullloop", "demo", "ambience"];

/// The kinds of effect, found in an effect's name (an effect naming none stays "fx").
pub const FX_CATS: [(&str, &str); 6] = [
    ("riser", r"riser|uplift|build ?ups?\b|\brise\b|\brising"),
    ("downlifter", r"downlift|down ?sweep|\bfalls?\b|\bdrop ?fx"),
    ("impact", r"impact|\bhits?\b|\bboom|\bslam|\bthud|\bsub ?drop"),
    ("sweep", r"sweep|whoosh|swoosh|transition|\bswell"),
    ("noise", r"\bnoise|vinyl|crackle|\bhiss|\bstatic\b|\bwhite\b"),
    ("glitch", r"glitch|stutter|bitcrush|\bglitchy"),
];
/// The kinds of drum loop: a fill, a break, the top (hats, shakers, rides), the percussion,
/// the kick alone; one naming none (or several parts) is a whole drum loop ("drumloop").
pub const DRUMLOOP_CATS: [(&str, &str); 5] = [
    ("fill", r"\bfills?\b"),
    ("break", r"breakbeat|\bbreaks?\b|\bamen\b"),
    ("toploop", r"\btops?\b|\bhats?\b|hi ?hat|\bhh\b|shaker|\bride\b"),
    ("percloop", r"\bpercs?\b|percussi|conga|bongo|\btribal"),
    ("kickloop", r"\bkicks?\b|\bbd\b|low ?end"),
];
static FX_RX: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| FX_CATS.iter().map(|(c, p)| (*c, Regex::new(&format!("(?i){p}")).unwrap())).collect());
static DL_RX: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| DRUMLOOP_CATS.iter().map(|(c, p)| (*c, Regex::new(&format!("(?i){p}")).unwrap())).collect());
/// A drum loop naming the main drums is a whole groove, whatever else it names ("Kick Snare
/// Loop", "Full Drums Top"): not a part.
static DL_WHOLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bsnares?\b|\bclaps?\b|\bfull\b|\bdrums?\b.{0,12}\b(kick|snare)").unwrap());

/// An effect or a drum loop told more precisely by the words around it (the name first,
/// then the folders up): the category it falls in, `cat` unchanged when nothing says.
fn refine(cat: &str, texts: &[String]) -> String {
    for t in texts {
        let w = words(t);
        match cat {
            "fx" => {
                if let Some((c, _)) = FX_RX.iter().find(|(_, r)| r.is_match(&w)) {
                    return c.to_string();
                }
            }
            "drumloop" => {
                if DRUMLOOP_CATS[0].0 == "fill" && DL_RX[0].1.is_match(&w) {
                    return "fill".into();
                }
                if DL_RX[1].1.is_match(&w) {
                    return "break".into();
                }
                if DL_WHOLE.is_match(&w) {
                    return cat.into();
                }
                if let Some((c, _)) = DL_RX[2..].iter().find(|(_, r)| r.is_match(&w)) {
                    return c.to_string();
                }
            }
            _ => return cat.into(),
        }
    }
    cat.into()
}

/// The family a category belongs to ("kick": drums; a drum loop and a demo are their own).
pub fn family_of(cat: &str) -> Option<&'static str> {
    if FX_CATS.iter().any(|c| c.0 == cat) {
        return Some("fx");
    }
    if DRUMLOOP_CATS.iter().any(|c| c.0 == cat) {
        return Some("drumloop");
    }
    match cat {
        "drumloop" => Some("drumloop"),
        "demo" => Some("demo"),
        "fullloop" => Some("fullloop"),
        _ => CATS.iter().find(|c| c.0 == cat).map(|c| c.1),
    }
}

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

/// A kind glued to "loop", "shot" or "hit" ("bassloop", "KICKSHOT").
static GLUED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(bass|kick|snare|clap|hat|perc|drum|synth|pad|lead|vocal|vox|fx|top|beat|groove|break|sub|808|chord|pluck|arp|house|techno|tech)(loops?|shots?|hits?|beats?)\b").unwrap()
});

/// The words of a sound that a name writes glued to the next one with a capital ("BassLoop",
/// "TopPercLoop", "EkaliThreatzHatLoop"). Only these are taken apart: a spoken phrase
/// written the same way ("BreakItUp", "SeenTheLeader") stays one word.
const SOUND_WORDS: &[&str] = &[
    "bass", "basses", "kick", "kicks", "snare", "snares", "clap", "claps", "hat", "hats", "hihat", "perc", "percs", "drum", "drums",
    "synth", "synths", "pad", "pads", "vox", "vocal", "vocals", "fx", "sfx", "sub", "chord", "chords", "pluck", "arp", "piano",
    "keys", "loop", "loops", "shaker", "conga", "bongo", "riser", "wobble", "melody", "fill", "fills", "guitar", "strings", "brass",
    "flute", "stab", "stabs", "ride", "crash", "rim",
];

/// "BassLoop" -> "Bass Loop", "BreakItUp" unchanged (see SOUND_WORDS).
fn camel(token: &str) -> String {
    let c: Vec<char> = token.chars().collect();
    let mut pieces: Vec<String> = vec![String::new()];
    for i in 0..c.len() {
        let cut = i > 0 && c[i - 1].is_lowercase() && c[i].is_uppercase() && c.get(i + 1).is_some_and(|n| n.is_lowercase());
        if cut {
            pieces.push(String::new());
        }
        pieces.last_mut().unwrap().push(c[i]);
    }
    let known = |p: &str| SOUND_WORDS.contains(&p.trim_end_matches(|x: char| x.is_ascii_digit()).to_lowercase().as_str());
    let mut out = String::new();
    for (i, p) in pieces.iter().enumerate() {
        if i > 0 && (known(&pieces[i - 1]) || known(p)) {
            out.push(' ');
        }
        out.push_str(p);
    }
    out
}

/// Separators to spaces ("Kick_01-Hard" -> "Kick 01 Hard"), and glued words apart.
fn words(s: &str) -> String {
    let t: String = s.chars().map(|c| if matches!(c, '_' | '-' | '.' | '(' | ')' | '[' | ']' | ',' | '+') { ' ' } else { c }).collect();
    let t = t.split_whitespace().map(camel).collect::<Vec<_>>().join(" ");
    let t = GLUED.replace_all(&t, "$1 $2");
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

pub fn cats_in(s: &str) -> Vec<(&'static str, &'static str)> {
    let w = GENRES.replace_all(&words(s), " ").into_owned();
    if DEMO.is_match(&w) {
        return vec![("demo", "demo")];
    }
    if DRUMLOOP.is_match(&w) {
        return vec![("drumloop", "drumloop")];
    }
    let mut found: Vec<(&str, &str)> = RX.iter().filter(|(_, _, r)| r.is_match(&w)).map(|(c, f, _)| (*c, *f)).collect();
    // a recording of a place, a crowd, a gesture named with an effect is that recording
    // ("City Ambience FX"); with an instrument, the instrument ("Ocean Synth", "Crowd Clap",
    // "Forest Pad": a sound named after a place)
    if found.iter().any(|x| x.1 == "ambience") {
        if found.iter().all(|x| x.1 == "ambience" || x.1 == "fx") {
            found.retain(|x| x.1 == "ambience");
            found.truncate(1);
        } else {
            found.retain(|x| x.1 != "ambience");
        }
    }
    // "808 Sub Bass": an 808; "Sub Bass", "Growl Bass": the precise one; "Bass Drum": a kick
    if found.is_empty() && DRUMS_TEMPO.is_match(&w) {
        return vec![("drumloop", "drumloop")];
    }
    if found.iter().any(|x| x.0 == "808") {
        found.retain(|x| x.0 != "sub");
    }
    if found.iter().any(|x| matches!(x.0, "808" | "sub" | "growl" | "kick")) {
        found.retain(|x| x.0 != "bass");
    }
    if found.len() > 1 {
        found.retain(|x| x.0 != "synth" && x.0 != "melody");
    }
    // several named: the one the others describe. An effect made of anything ("Vocal FX",
    // "Cowbell Impact") is an effect; a voice played as anything ("Vocal Chop Lead", "Vocal
    // Pad") is a voice; a drum with "sub" in it ("Sub Perc", "BD Sub") is a drum; a bass
    // played as an arp or a pluck is a bass.
    if found.len() > 1 {
        for want in ["fx", "vocal"] {
            if let Some(x) = found.iter().find(|x| x.0 == want).copied() {
                return vec![x];
            }
        }
        if found.iter().any(|x| x.1 == "drums") {
            found.retain(|x| x.0 != "808" && x.0 != "sub");
        }
        if let Some(x) = found.iter().find(|x| x.1 == "bass").copied()
            && found.iter().all(|y| y.0 == x.0 || y.1 == "melodic")
        {
            return vec![x];
        }
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
    // nothing named anywhere, and a loop or a full mix: a whole mix, not one instrument
    // (drums named: a drum loop; an instrument's folder: something melodic, which one unknown)
    if found.is_empty() && (FULL_LOOP.is_match(&words(stem)) || folders.last().is_some_and(|f| FULL_LOOP.is_match(&words(f)))) {
        let near = format!("{} {}", words(stem), folders.iter().rev().take(2).map(|f| words(f)).collect::<Vec<_>>().join(" "));
        if DRUMS_NAMED.is_match(&near) {
            found = vec![("drumloop", "drumloop")];
        } else if INSTRUMENT.is_match(&near) {
            n.family = Some("melodic".into());
            n.cat_src = Some("folder".into());
        } else {
            found = vec![("fullloop", "fullloop")];
        }
    }
    if let [(c, f)] = found.as_slice() {
        // an effect or a drum loop: which kind, when the name or its folders say
        let texts: Vec<String> = std::iter::once(stem.to_string()).chain(folders.iter().rev().take(2).map(|f| f.to_string())).collect();
        n.cat = Some(refine(c, &texts));
        n.family = Some(f.to_string());
        n.cat_src = Some(src.to_string());
    } else if let Some((_, f)) = found.first()
        && found.iter().all(|x| x.1 == *f)
    {
        // two of one family ("Claps & Snares", "Guitar Chords"): the family, no category
        n.family = Some(f.to_string());
        n.cat_src = Some(src.to_string());
    }
    let w = words(stem);
    n.bpm = BPM
        .captures(&w)
        .and_then(|c| c.get(1).or(c.get(2)))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .filter(|b| (50.0..=220.0).contains(b));
    if n.family.as_deref() == Some("drumloop") || LOOP.is_match(&w) {
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
    fn effects_and_drum_loops_told_apart() {
        assert_eq!(cat("Cinematic Riser 04", &[]).as_deref(), Some("riser"));
        assert_eq!(cat("FX Downlifter 2", &[]).as_deref(), Some("downlifter"));
        assert_eq!(cat("Impact_Big_01", &[]).as_deref(), Some("impact"));
        assert_eq!(cat("Whoosh Transition 3", &[]).as_deref(), Some("sweep"));
        assert_eq!(cat("Vinyl Crackle Loop", &[]).as_deref(), Some("noise"));
        assert_eq!(cat("SFX 12", &[]).as_deref(), Some("fx"), "an effect naming no kind stays an effect");
        assert_eq!(cat("012", &["Pack", "FX", "Risers"]).as_deref(), Some("riser"), "the folder says the kind");
        assert_eq!(cat("Top Loop 03 128bpm", &[]).as_deref(), Some("toploop"));
        assert_eq!(cat("Shaker Groove Loop 120", &[]).as_deref(), Some("toploop"));
        assert_eq!(cat("Tribal Groove Loop 7 124bpm", &[]).as_deref(), Some("percloop"));
        assert_eq!(cat("Perc Loop 7 124bpm", &[]).as_deref(), Some("perc"), "a percussion loop stays a percussion (its loop said apart)");
        assert_eq!(cat("MBH_Kit01_Drum_Fill_Build_126_01", &[]).as_deref(), Some("fill"));
        assert_eq!(cat("Amen Break 170", &[]).as_deref(), Some("break"));
        assert_eq!(cat("VP3_172_Drums_KickSnare_Loop_Boom", &[]).as_deref(), Some("drumloop"), "kick and snare: a whole groove");
        assert_eq!(cat("Full Drum Loop 01", &[]).as_deref(), Some("drumloop"));
        let n = read_name("Tribal Groove Loop 7 124bpm", &[]);
        assert_eq!((n.family.as_deref(), n.is_loop), (Some("drumloop"), Some(true)));
        assert_eq!(family_of("riser"), Some("fx"));
        assert_eq!(family_of("fill"), Some("drumloop"));
    }

    #[test]
    fn categories_from_the_name_then_the_folders() {
        assert_eq!(cat("Kick_01_Hard", &[]).as_deref(), Some("kick"));
        assert_eq!(cat("KSHMR Snare 12", &[]).as_deref(), Some("snare"));
        assert_eq!(cat("Open Hat 3", &[]).as_deref(), Some("hihat"));
        assert_eq!(cat("808 Sub Bass F#", &[]).as_deref(), Some("808"));
        assert_eq!(cat("Deep Sub Bass 03", &[]).as_deref(), Some("sub"));
        assert_eq!(cat("Neuro Growl Bass 7", &[]).as_deref(), Some("growl"));
        assert_eq!(cat("Reese Bass 2", &[]).as_deref(), Some("bass"));
        // glued words and capitals inside a word
        assert_eq!(cat("Bpm172_F_BassLoop_11", &[]).as_deref(), Some("bass"));
        assert_eq!(cat("NFK[172]P2_BASSLOOP", &[]).as_deref(), Some("bass"));
        assert_eq!(cat("TopPercLoop 3", &[]).as_deref(), Some("toploop"));
        // a word inside another is not read: "Percy" is no percussion, "organic" no organ
        assert_eq!(cat("CS_House_Bass_Loops_Percy_A_#_125", &[]).as_deref(), Some("bass"));
        assert_eq!(cat("Organic Pad 2", &[]).as_deref(), Some("pad"));
        assert_eq!(cat("VP3_172_Drums_KickSnare_Loop_Boom", &[]).as_deref(), Some("drumloop"));
        // a spoken phrase glued with capitals stays one word; an animal's growl is no bass
        assert_eq!(cat("MD3_BreakItUp", &[]), None);
        assert_eq!(cat("TalkToHim", &["Vox", "TomLaval"]).as_deref(), Some("vocal"));
        assert_eq!(cat("VFH3 128BPM Wobble House Kick", &[]).as_deref(), Some("kick"));
        assert_eq!(cat("VMV1 SP1 SeenTheLeader", &[]), None);
        assert_eq!(cat("KSHMR Animals 29 - Lion Growl", &[]), None);
        assert_eq!(cat("Lev_Bass_Saw_Wobble_Set_F#1", &[]).as_deref(), Some("growl"));
        assert_eq!(read_name("Bpm172_F_BassLoop_11", &[]).is_loop, Some(true));
        assert_eq!(cat("Bassline_Gm_124", &[]).as_deref(), Some("bass"));
        assert_eq!(cat("Top Loop 03 128bpm", &[]).as_deref(), Some("toploop"));
        assert_eq!(cat("001", &["Pack X", "Vocals"]).as_deref(), Some("vocal"));
        let n = read_name("001", &["Pack X", "Drums", "Kicks"]);
        assert_eq!((n.cat.as_deref(), n.cat_src.as_deref()), (Some("kick"), Some("folder")));
        // a style's name is not a category
        assert_eq!(cat("MNBOY Bass House Clap 3", &[]).as_deref(), Some("clap"));
        assert_eq!(cat("Drum and Bass Kick 04", &[]).as_deref(), Some("kick"));
        assert_eq!(cat("Future Bass Lead 2", &[]).as_deref(), Some("lead"));
        assert_eq!(cat("Bass 3", &["Packs", "Deep House"]).as_deref(), Some("bass"));
        // two of one family: the family, no category
        let n = read_name("Kick and Snare", &[]);
        assert_eq!((n.cat.as_deref(), n.family.as_deref()), (None, Some("drums")));
        // several named: the one the others describe
        assert_eq!(cat("LHV_122_Baila_01_Lead_Vocal_Dry_F#min", &[]).as_deref(), Some("vocal"));
        assert_eq!(cat("SPINNIN_transition_one_shot_vocal", &[]).as_deref(), Some("sweep"));
        assert_eq!(cat("VEE2 Sub Perc 001", &[]).as_deref(), Some("perc"));
        assert_eq!(cat("Bass Arp 01 DRY", &[]).as_deref(), Some("bass"));
        // two families with nothing to settle them: left out
        let n = read_name("Snare Guitar 2", &[]);
        assert_eq!((n.cat, n.family), (None, None));
        // words the names use
        assert_eq!(cat("MBH_Kit01_Drum_Fill_Build_126_01", &[]).as_deref(), Some("fill"));
        assert_eq!(cat("TPS - Cosmic - Noise 01", &[]).as_deref(), Some("noise"));
        assert_eq!(cat("Epic Brass Hit 3", &[]).as_deref(), Some("brass"));
        assert_eq!(cat("Violin Staccato C", &[]).as_deref(), Some("strings"));
        assert_eq!(cat("Pan Flute Loop 90", &[]).as_deref(), Some("woodwind"));
        assert_eq!(cat("Melody Loop 07 Am", &[]).as_deref(), Some("melody"));
        // abbreviations, a drum loop by its tempo
        assert_eq!(cat("Cym - 8", &[]).as_deref(), Some("cymbal"));
        assert_eq!(cat("MEL 174 BPM Dm -1", &[]).as_deref(), Some("melody"));
        assert_eq!(cat("Drums 174 BPM - 9_22", &[]).as_deref(), Some("drumloop"));
        assert_eq!(cat("003 Drums - Hats 172bpm Fm", &[]).as_deref(), Some("hihat"));
        assert_eq!(cat("EH_Kick Drum 10", &[]).as_deref(), Some("kick"));
        // a loop naming no instrument, nor its folders: a whole mix
        assert_eq!(cat("VEE Electro Loop 358", &["VEE Electro Loops 128 BPM"]).as_deref(), Some("fullloop"));
        // ambiences: recorded places, crowds, gestures; an "Atmos" synth stays a pad
        assert_eq!(cat("X_Backyard", &["Atmos Pack", "Field Recordings"]).as_deref(), Some("ambience"));
        assert_eq!(cat("Stadium Crowd Cheer 2", &["FX"]).as_deref(), Some("crowd"));
        assert_eq!(cat("Footsteps Gravel", &["Sounds"]).as_deref(), Some("foley"));
        assert_eq!(cat("City Ambience FX", &["Pack"]).as_deref(), Some("ambience"));
        assert_eq!(cat("Atmos Pad Dark", &["Pack"]).as_deref(), Some("pad"));
        assert_eq!(cat("Crowd Clap 3", &["Pack"]).as_deref(), Some("clap"));
        assert_eq!(cat("Ocean_90_C_Synth_SP", &["Retro Keys"]).as_deref(), Some("synth"));
        assert_eq!(cat("Walking in Forest 4", &["LIFE Ambient Recordings", "Walking"]).as_deref(), Some("ambience"));
        assert_eq!(cat("KSHMR Retro SS - Sunset Rendezvous (98, Fm) - Full", &["Retro"]).as_deref(), Some("fullloop"));
        assert_eq!(cat("VEC2 Loops 027", &["Pack", "VEC2 Loops"]).as_deref(), Some("fullloop"));
        // a loop in a folder that names it stays what the folder says
        assert_eq!(cat("Loop 03", &["Bass Loops"]).as_deref(), Some("bass"));
        assert_eq!(cat("VEH2 Housebeats - 01", &[]).as_deref(), Some("drumloop"));
        assert_eq!(cat("120_AnarchyBreak_01_SP", &["AIFF", "Full Drums"]).as_deref(), Some("drumloop"));
        let n = read_name("SO_DU_85_mallet_trooper_Fmaj", &["Splice", "Instrument_Loops"]);
        assert_eq!((n.family.as_deref(), n.cat.as_deref()), (Some("melodic"), Some("keys")));
        let n = read_name("SO_DU_85_pariah_Cmin", &["Splice", "Instrument_Loops"]);
        assert_eq!((n.family.as_deref(), n.cat.as_deref()), (Some("melodic"), None));
        // a one-shot naming nothing stays unsorted
        assert_eq!(cat("Thing 02", &["Pack"]), None);
        // a pack's demo is a demo, whatever it is made of
        assert_eq!(cat("House Bass (Demo)", &[]).as_deref(), Some("demo"));
        assert_eq!(cat("Track 01", &["Catalyst Samples - Ingredient House Bass", "Demo"]).as_deref(), Some("demo"));
        assert_eq!(cat("Pack Preview", &[]).as_deref(), Some("demo"));
        // a bass drum is a kick, not a bass
        assert_eq!(cat("Bass Drum 4", &[]).as_deref(), Some("kick"));
        // a synth is said only when nothing more precise is
        assert_eq!(cat("VMH1 Synth Shot 12", &[]).as_deref(), Some("synth"));
        assert_eq!(cat("Arcade Riff 3", &[]).as_deref(), Some("synth"));
        assert_eq!(cat("Synth Lead 2", &[]).as_deref(), Some("lead"));
        assert_eq!(cat("Chord Stab 7", &[]).as_deref(), Some("keys"));
        assert_eq!(cat("Bass Synth 1", &[]).as_deref(), Some("bass"));
        // a break is a drum loop, a breakdown is not
        assert_eq!(cat("Amen", &["Breaks", "by song"]).as_deref(), Some("break"));
        assert_eq!(cat("300 Drum Breaks 44", &[]).as_deref(), Some("break"));
        assert_ne!(cat("Breakdown FX 2", &[]).as_deref(), Some("drumloop"));
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

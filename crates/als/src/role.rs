//! What a track plays: drums, bass, keys... from its name, devices, samples and notes.
use crate::xml::{N, child, children, iter, iter_tag, or, tag, val};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Drums,
    Perc,
    Bass,
    Keys,
    Pad,
    Pluck,
    Lead,
    Vox,
    Fx,
    Other,
    Return,
}

const ROLE_WORDS: [(Role, &str); 9] = [
    (Role::Vox, r"vox|vocals?|voice|voix|chant|acapell?a|adlibs?|choir|chorus vox|spoken"),
    (Role::Fx, r"s?fx|risers?|uplifter|downlifter|impacts?|sweeps?|whoosh|transitions?|noise|reverse|rev"),
    (Role::Perc, r"percs?|percussions?|shakers?|congas?|bongos?|tamb\w*|rims?|tribal|claves?|cowbell"),
    (
        Role::Drums,
        r"drums?|kicks?|kk|bd|snares?|snr|sd|claps?|hats?|hh|hihats?|cymbals?|cym|ride|crash|breaks?|breakbeat|beat|batterie|toms?|tops?|dnbdr|drumloop",
    ),
    (Role::Bass, r"bass|basse|sub|808|reese|wobble|low ?end"),
    (Role::Pad, r"pads?|nappes?|atmos?|ambient|drones?|textures?|strings?|cordes|choirs?"),
    (Role::Keys, r"pianos?|keys|chords?|accords?|rhodes|organ|orgue|e-?piano|wurli\w*|guit\w*|gtr|ep"),
    (Role::Pluck, r"plucks?|arps?|arpeg\w*|bells?|stabs?|marimba|mallets?|kalimba"),
    (Role::Lead, r"leads?|melod\w*|mélod\w*|hooks?|solo|brass|horns?|trumpets?|flutes?|sax\w*|synth"),
];

/// A role word stands alone: not inside a longer word ("sub" in "subtle" does not count).
static ROLE_RES: LazyLock<Vec<(Role, FRegex)>> = LazyLock::new(|| {
    ROLE_WORDS.iter().map(|(r, w)| (*r, FRegex::new(&format!(r"(?i)(?<![a-z])(?:{w})(?![a-z])")).unwrap())).collect()
});
static SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[_\-.]+").unwrap());
static DRUM_PLUGINS: LazyLock<FRegex> = LazyLock::new(|| {
    FRegex::new(r"(?i)battery|addictive drums|superior drummer|ezdrummer|xo\b|microtonic|drum").unwrap()
});
static BASS_PLUGINS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)bass|modo bass|trilian|sub").unwrap());

fn role_from_text<'s>(texts: impl IntoIterator<Item = &'s str>) -> Option<Role> {
    for t in texts {
        let t = SEPARATORS.replace_all(t, " ");
        for (role, rx) in ROLE_RES.iter() {
            if rx.is_match(&t).unwrap_or(false) {
                return Some(*role);
            }
        }
    }
    None
}

/// The name the user gave the track, else Live's.
pub fn track_name(track: N) -> String {
    let n = child(track, "Name");
    or(val(n, "UserName"), val(n, "EffectiveName")).unwrap_or("").trim().to_string()
}

/// Clips of a track, frozen copies left out. Order: by parent, as the prototype listed them.
pub fn clips<'a, 'i>(track: N<'a, 'i>) -> Vec<N<'a, 'i>> {
    let under_freeze =
        |c: N| c.ancestors().skip(1).take_while(|a| a.id() != track.id()).any(|a| tag(a) == "FreezeSequencer");
    let mut out = Vec::new();
    for parent in iter(track) {
        if tag(parent) == "FreezeSequencer" {
            continue;
        }
        for c in children(parent) {
            if matches!(tag(c), "MidiClip" | "AudioClip") && !under_freeze(c) {
                out.push(c);
            }
        }
    }
    out
}

/// `Path(p).stem` (the last part without its last suffix).
pub fn stem(p: &str) -> String {
    let p = p.replace('\\', "/");
    let name = p.rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

pub fn sample_names(track: N) -> Vec<String> {
    iter_tag(track, "FileRef")
        .filter_map(|fr| or(val(Some(fr), "RelativePath"), val(Some(fr), "Path")).filter(|p| !p.is_empty()))
        .map(stem)
        .collect()
}

/// drums / perc / bass / keys / pad / pluck / lead / vox / fx / other / return.
pub fn track_role(track: N) -> Role {
    if tag(track) == "ReturnTrack" {
        return Role::Return;
    }
    if let Some(r) = role_from_text([track_name(track).as_str()]) {
        return r;
    }
    if iter(track).any(|d| matches!(tag(d), "DrumGroupDevice" | "InstrumentImpulse")) {
        return Role::Drums;
    }
    let plug = iter(track)
        .filter(|i| matches!(tag(*i), "VstPluginInfo" | "Vst3PluginInfo" | "AuPluginInfo"))
        .map(|i| or(val(Some(i), "PlugName"), val(Some(i), "Name")).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    if DRUM_PLUGINS.is_match(&plug).unwrap_or(false) {
        return Role::Drums;
    }
    let mut texts: Vec<String> = sample_names(track).into_iter().take(12).collect();
    texts.extend(clips(track).into_iter().take(12).map(|c| val(Some(c), "Name").unwrap_or("").to_string()));
    if let Some(r) = role_from_text(texts.iter().map(String::as_str)) {
        return r;
    }
    if BASS_PLUGINS.is_match(&plug) {
        return Role::Bass;
    }
    let (mut keys, mut n) = (0i64, 0i64);
    for kt in iter_tag(track, "KeyTrack") {
        let Some(k) = val(Some(kt), "MidiKey") else { continue };
        let digits = k.trim_start_matches('-');
        let cnt = iter_tag(kt, "MidiNoteEvent").count() as i64;
        if !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && cnt > 0
            && let Ok(k) = k.parse::<i64>()
        {
            keys += k * cnt;
            n += cnt;
        }
    }
    if n >= 8 {
        let avg = keys as f64 / n as f64;
        return if avg < 45.0 {
            Role::Bass
        } else if avg > 70.0 {
            Role::Lead
        } else {
            Role::Keys
        };
    }
    Role::Other
}

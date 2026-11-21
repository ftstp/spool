//! Treatments: optional changes of a set, driven by presets. Ported from the prototype's
//! studio.py. Every treatment works on the parsed XML of a COPY: C colours, N names, O order
//! and groups, P cleanup, M markers, G template (tempo, scale, rack on the main track).
//! R (repair) is `fix`. Nothing here changes the sound: no volume, no plugin setting, no note.
//!
//! Tracks are told apart by their `Id` (unique in a set), where the prototype used Python
//! object identity.
use crate::xml::Element;
use fancy_regex::Regex as FRegex;
use md5::{Digest, Md5};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub const ROLES: [&str; 10] = ["drums", "perc", "bass", "keys", "pad", "pluck", "lead", "vox", "fx", "other"];
const ROLE_WORDS: [(&str, &str); 9] = [
    ("vox", r"vox|vocals?|voice|voix|chant|acapell?a|adlibs?|choir|chorus vox|spoken"),
    ("fx", r"s?fx|risers?|uplifter|downlifter|impacts?|sweeps?|whoosh|transitions?|noise|reverse|rev"),
    ("perc", r"percs?|percussions?|shakers?|congas?|bongos?|tamb\w*|rims?|tribal|claves?|cowbell"),
    (
        "drums",
        r"drums?|kicks?|kk|bd|snares?|snr|sd|claps?|hats?|hh|hihats?|cymbals?|cym|ride|crash|breaks?|breakbeat|beat|batterie|toms?|tops?|dnbdr|drumloop",
    ),
    ("bass", r"bass|basse|sub|808|reese|wobble|low ?end"),
    ("pad", r"pads?|nappes?|atmos?|ambient|drones?|textures?|strings?|cordes|choirs?"),
    ("keys", r"pianos?|keys|chords?|accords?|rhodes|organ|orgue|e-?piano|wurli\w*|guit\w*|gtr|ep"),
    ("pluck", r"plucks?|arps?|arpeg\w*|bells?|stabs?|marimba|mallets?|kalimba"),
    ("lead", r"leads?|melod\w*|mélod\w*|hooks?|solo|brass|horns?|trumpets?|flutes?|sax\w*|synth"),
];

/// How track names are matched to roles. `Prototype` keeps the prototype's patterns as they
/// were written: the word list was NOT grouped, so the "not inside a word" guards only bind
/// its first and last words ("Stomper" reads as drums through "tom"). `Words` groups it: a
/// role word must stand alone. The app uses `Starts`; `Prototype` exists for the comparisons.
/// `Starts` (the app): a role word must START a word and may go on ("bassline", "crashs",
/// "cymb", "shakerz", "subadg"), except the 2-letter ones ("ep", "sd", "bd", "hh", "kk", "fx")
/// which must stand alone ("epic" is not keys). Inside a word nothing counts ("marimba" is
/// not perc, "Piste Master" not drums).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleMatch {
    Prototype,
    Words,
    Starts,
}

static ROLE_PROTO: LazyLock<Vec<(&str, FRegex)>> = LazyLock::new(|| {
    ROLE_WORDS.iter().map(|(r, w)| (*r, FRegex::new(&format!("(?i)(?<![a-z]){w}(?![a-z])")).unwrap())).collect()
});
static ROLE_WORDS_RX: LazyLock<Vec<(&str, FRegex)>> = LazyLock::new(|| {
    ROLE_WORDS.iter().map(|(r, w)| (*r, FRegex::new(&format!("(?i)(?<![a-z])(?:{w})(?![a-z])")).unwrap())).collect()
});
static UNDERSCORES: LazyLock<Regex> = LazyLock::new(|| Regex::new("_+").unwrap());
/// Letters of a pattern alternative, its regex syntax left out ("e-?piano" -> "epiano").
fn literal_len(alt: &str) -> usize {
    alt.replace(r"\w*", "").chars().filter(|c| c.is_alphanumeric()).count()
}
static ROLE_STARTS: LazyLock<Vec<(&str, FRegex)>> = LazyLock::new(|| {
    ROLE_WORDS
        .iter()
        .map(|(r, w)| {
            let (short, long): (Vec<&str>, Vec<&str>) = w.split('|').partition(|a| literal_len(a) <= 2);
            let mut alts = long.join("|");
            if !short.is_empty() {
                alts = format!("{alts}{}(?:{})(?![a-z])", if alts.is_empty() { "" } else { "|" }, short.join("|"));
            }
            (*r, FRegex::new(&format!("(?i)(?<![a-z])(?:{alts})")).unwrap())
        })
        .collect()
});
static SEPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[_\-.]+").unwrap());
static DRUM_PLUGINS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)battery|addictive drums|superior drummer|ezdrummer|xo\b|microtonic|drum").unwrap()
});
static BASS_PLUGINS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)bass|modo bass|trilian|sub").unwrap());
static PLUGIN_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\s.(-]*(x64|x86|64[\s-]?bit|32[\s-]?bit|vst3?|\.64|portable|\d+\s*out|\(m\)|\(s\))\)?\s*$")
        .unwrap()
});
static AUTO_TRACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\d+[- ])?(audio|midi|group|groupe|\d+)?( ?\d+)?$|^\d+-").unwrap());
static AUTO_CLIP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+ )?\d+-|^$").unwrap());
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Track\.(-?\d+)/(\w+)").unwrap());

const NATIVE_INSTR: [&str; 12] = [
    "OriginalSimpler",
    "MultiSampler",
    "InstrumentVector",
    "Operator",
    "UltraAnalog",
    "LoungeLizard",
    "StringStudio",
    "Collision",
    "DrumGroupDevice",
    "InstrumentMeld",
    "Drift",
    "InstrumentImpulse",
];

/// The labels used for track names and new groups, by role.
pub fn default_labels() -> Map<String, Value> {
    serde_json::from_value(json!({"drums": "Drums", "perc": "Perc", "bass": "Bass", "keys": "Keys", "pad": "Pad",
        "pluck": "Pluck", "lead": "Lead", "vox": "Vox", "fx": "FX", "other": "Other"}))
    .unwrap()
}

/// The preset the treatments start with ("Complet").
pub fn default_preset() -> Value {
    json!({
        "name": "Complet",
        "R": true,
        "C": {"on": true, "clips": true,
              "hue": {"drums": 0, "perc": 1, "bass": 9, "keys": 5, "pad": 7, "pluck": 3, "lead": 11,
                      "vox": 12, "fx": 13, "other": 2, "return": 13},
              "track_shade": 1},
        "N": {"on": true, "tracks": true, "clips": true, "scenes": true, "only_auto": true, "labels": default_labels()},
        "O": {"on": true, "order": ROLES, "inside_groups": true, "make_groups": false},
        "P": {"on": false, "empty_tracks": true, "disabled_devices": true, "unused_returns": true, "simplify_groups": false},
        "M": {"on": true, "min_bars": 8, "only_if_none": true},
        "G": {"on": false, "tempo": null, "scale": null, "main_rack": ""},
    })
}

pub const CODES: &str = "RCNOPMG";

/// The suffix letters a preset produces, in the fixed order.
pub fn codes_of(preset: &Value) -> String {
    CODES.chars().filter(|c| on(&preset[c.to_string().as_str()])).collect()
}

/// `true`, or an object with "on": true.
fn on(v: &Value) -> bool {
    v.as_bool() == Some(true) || v.get("on").is_some_and(truthy)
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// An option of a treatment: its value, or `default` when absent (Python's `opt.get(k, d)`).
fn opt_bool(o: &Value, k: &str, default: bool) -> bool {
    o.get(k).map(truthy).unwrap_or(default)
}

// ---------------------------------------------------------------- small helpers

fn v<'a>(e: &'a Element, path: &str) -> Option<&'a str> {
    e.find(path).and_then(|x| x.get("Value"))
}

/// An element other parts of a set point at by its Id.
pub fn is_pointee(tag: &str) -> bool {
    pointee(tag)
}

/// The first pointee id free in a set.
pub fn next_pointee_id(root: &Element) -> i64 {
    next_pointee(root)
}

fn pointee(tag: &str) -> bool {
    tag.ends_with("Target") || tag.starts_with("ControllerTargets.") || tag == "Pointee"
}

fn tid(t: &Element) -> String {
    t.get("Id").unwrap_or("").to_string()
}

fn group_of(t: &Element) -> String {
    v(t, "TrackGroupId").unwrap_or("-1").to_string()
}

/// Python's `str(float)` for the values the treatments write.
fn py_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 { format!("{f:.1}") } else { format!("{f}") }
}

fn live_set(root: &Element) -> &Element {
    root.child("LiveSet").expect("LiveSet")
}

fn tracks(root: &Element) -> &Vec<Element> {
    &live_set(root).child("Tracks").expect("Tracks").children
}

fn tracks_mut(root: &mut Element) -> &mut Vec<Element> {
    &mut root.child_mut("LiveSet").unwrap().child_mut("Tracks").unwrap().children
}

/// Paths (from `track`) of its clips, frozen copies left out; `arrangement_only`: only
/// those in the arrangement.
fn clip_paths(track: &Element, arrangement_only: bool) -> Vec<Vec<usize>> {
    fn walk(e: &Element, path: &mut Vec<usize>, frozen: bool, arr: bool, only_arr: bool, out: &mut Vec<Vec<usize>>) {
        let frozen_here = frozen || e.tag == "FreezeSequencer";
        let arr_here = arr || e.tag == "ArrangerAutomation";
        if e.tag != "FreezeSequencer" {
            for (i, c) in e.children.iter().enumerate() {
                if matches!(c.tag.as_str(), "MidiClip" | "AudioClip") && !frozen_here && (!only_arr || arr_here) {
                    path.push(i);
                    out.push(path.clone());
                    path.pop();
                }
            }
        }
        for (i, c) in e.children.iter().enumerate() {
            path.push(i);
            walk(c, path, frozen_here, arr_here, only_arr, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(track, &mut Vec::new(), false, false, arrangement_only, &mut out);
    out
}

/// Same content -> same signature (colour shade, letter, sections).
fn clip_signature(clip: &Element) -> String {
    let mut h = Md5::new();
    let val = |p: &str| v(clip, p).unwrap_or("").to_string();
    if clip.tag == "MidiClip" {
        for kt in clip.descendants().skip(1).filter(|e| e.tag == "KeyTrack") {
            let key = v(kt, "MidiKey").unwrap_or("");
            for n in kt.descendants().skip(1).filter(|e| e.tag == "MidiNoteEvent") {
                h.update(
                    format!("{key}:{}:{};", n.get("Time").unwrap_or("None"), n.get("Duration").unwrap_or("None"))
                        .as_bytes(),
                );
            }
        }
        h.update(format!("{}/{}", val("Loop/LoopEnd"), val("Loop/LoopStart")).as_bytes());
    } else {
        let rel = Some(val(".//SampleRef/FileRef/RelativePath"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| val(".//SampleRef/FileRef/Path"));
        h.update(rel.as_bytes());
        h.update(format!("{}/{}", val("Loop/LoopStart"), val("Loop/LoopEnd")).as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Python's `Path(p).stem`.
fn stem(p: &str) -> String {
    let name = p.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => name[..i].to_string(),
        _ => name.to_string(),
    }
}

fn sample_names(track: &Element) -> Vec<String> {
    track
        .descendants()
        .skip(1)
        .filter(|e| e.tag == "FileRef")
        .filter_map(|fr| {
            let p = v(fr, "RelativePath").filter(|s| !s.is_empty()).or(v(fr, "Path")).unwrap_or("");
            (!p.is_empty()).then(|| stem(&p.replace('\\', "/")))
        })
        .collect()
}

/// "Kontakt 5 Portable x64 64out" -> "Kontakt 5", "Serum_x64" -> "Serum".
pub fn clean_plugin(n: &str) -> String {
    let mut n = UNDERSCORES.replace_all(n, " ").to_string();
    loop {
        let next = PLUGIN_NOISE.replace(&n, "").trim().to_string();
        if next == n {
            return n;
        }
        n = next;
    }
}

/// Display name of the main instrument / plugin of a track.
fn instrument(track: &Element) -> String {
    for dev in track.descendants() {
        match dev.tag.as_str() {
            "PluginDevice" => {
                if let Some(n) = v(dev, ".//VstPluginInfo/PlugName")
                    .filter(|s| !s.is_empty())
                    .or(v(dev, ".//Vst3PluginInfo/Name"))
                    .filter(|s| !s.is_empty())
                {
                    return clean_plugin(n);
                }
            }
            "AuPluginDevice" => {
                if let Some(n) = v(dev, ".//AuPluginInfo/Name").filter(|s| !s.is_empty()) {
                    return clean_plugin(n);
                }
            }
            t if NATIVE_INSTR.contains(&t) => return als::native::native_name(t).to_string(),
            _ => {}
        }
    }
    String::new()
}

pub fn track_name(t: &Element) -> String {
    let n = t.child("Name");
    let user = n.and_then(|n| v(n, "UserName")).filter(|s| !s.is_empty());
    user.or(n.and_then(|n| v(n, "EffectiveName"))).unwrap_or("").trim().to_string()
}

fn set_track_name(t: &mut Element, text: &str) {
    if let Some(n) = t.child_mut("Name") {
        n.set_value("UserName", text);
        n.set_value("EffectiveName", text);
    }
}

fn role_from_text(texts: &[String], m: RoleMatch) -> Option<&'static str> {
    let rx = match m {
        RoleMatch::Prototype => &*ROLE_PROTO,
        RoleMatch::Words => &*ROLE_WORDS_RX,
        RoleMatch::Starts => &*ROLE_STARTS,
    };
    for t in texts {
        let t = SEPS.replace_all(t, " ");
        for (role, r) in rx.iter() {
            if r.is_match(&t).unwrap_or(false) {
                return Some(role);
            }
        }
    }
    None
}

/// drums / perc / bass / keys / pad / pluck / lead / vox / fx / other, "return" for a return.
pub fn track_role(t: &Element, m: RoleMatch) -> &'static str {
    if t.tag == "ReturnTrack" {
        return "return";
    }
    if let Some(r) = role_from_text(&[track_name(t)], m) {
        return r;
    }
    if t.descendants().any(|d| matches!(d.tag.as_str(), "DrumGroupDevice" | "InstrumentImpulse")) {
        return "drums";
    }
    let plug: Vec<&str> = t
        .descendants()
        .filter(|i| matches!(i.tag.as_str(), "VstPluginInfo" | "Vst3PluginInfo" | "AuPluginInfo"))
        .map(|i| v(i, "PlugName").filter(|s| !s.is_empty()).or(v(i, "Name")).unwrap_or(""))
        .collect();
    let plug = plug.join(" ");
    if DRUM_PLUGINS.is_match(&plug) {
        return "drums";
    }
    let mut texts: Vec<String> = sample_names(t).into_iter().take(12).collect();
    texts.extend(clip_paths(t, false).iter().take(12).map(|p| v(t.at(p), "Name").unwrap_or("").to_string()));
    if let Some(r) = role_from_text(&texts, m) {
        return r;
    }
    if BASS_PLUGINS.is_match(&plug) {
        return "bass";
    }
    let (mut keys, mut n) = (0i64, 0i64);
    for kt in t.descendants().filter(|e| e.tag == "KeyTrack") {
        let k = v(kt, "MidiKey");
        let cnt = kt.descendants().filter(|e| e.tag == "MidiNoteEvent").count() as i64;
        if let Some(k) = k.filter(|k| {
            let d = k.trim_start_matches('-');
            !d.is_empty() && d.chars().all(|c| c.is_ascii_digit())
        }) && cnt > 0
        {
            keys += k.parse::<i64>().unwrap_or(0) * cnt;
            n += cnt;
        }
    }
    if n >= 8 {
        let avg = keys as f64 / n as f64;
        return if avg < 45.0 {
            "bass"
        } else if avg > 70.0 {
            "lead"
        } else {
            "keys"
        };
    }
    "other"
}

/// Group id -> the ids of its direct child tracks, in order.
fn children_of<'a>(ts: impl Iterator<Item = &'a Element>) -> HashMap<String, Vec<String>> {
    let mut kids: HashMap<String, Vec<String>> = HashMap::new();
    for t in ts {
        kids.entry(group_of(t)).or_default().push(tid(t));
    }
    kids
}

pub type Roles = HashMap<String, String>;

/// Track id -> role; a group takes the role most of its tracks have.
pub fn roles_of(root: &Element, m: RoleMatch) -> Roles {
    let ts = tracks(root);
    let mut roles: Roles = HashMap::new();
    for t in ts.iter().filter(|t| t.tag != "GroupTrack") {
        roles.insert(tid(t), track_role(t, m).to_string());
    }
    let kids = children_of(ts.iter());
    let by_id: HashMap<String, &Element> = ts.iter().map(|t| (tid(t), t)).collect();
    fn group_role(
        g: &Element,
        kids: &HashMap<String, Vec<String>>,
        by_id: &HashMap<String, &Element>,
        roles: &Roles,
        m: RoleMatch,
    ) -> String {
        if let Some(r) = role_from_text(&[track_name(g)], m) {
            return r.into();
        }
        let mut c: Vec<(String, i64)> = Vec::new();
        for k in kids.get(&tid(g)).into_iter().flatten() {
            let Some(kt) = by_id.get(k) else { continue };
            let r = if kt.tag == "GroupTrack" {
                group_role(kt, kids, by_id, roles, m)
            } else {
                roles.get(k).cloned().unwrap_or_default()
            };
            match c.iter_mut().find(|(x, _)| *x == r) {
                Some(slot) => slot.1 += 1,
                None => c.push((r, 1)),
            }
        }
        c.retain(|(r, _)| r != "other");
        // most_common(1): the highest count, the first seen among equals
        let mut best: Option<&(String, i64)> = None;
        for x in &c {
            if best.is_none_or(|b| x.1 > b.1) {
                best = Some(x);
            }
        }
        best.map(|b| b.0.clone()).unwrap_or_else(|| "other".into())
    }
    for t in ts.iter().filter(|t| t.tag == "GroupTrack") {
        let r = group_role(t, &kids, &by_id, &roles, m);
        roles.insert(tid(t), r);
    }
    roles
}

fn role<'a>(roles: &'a Roles, t: &Element) -> &'a str {
    roles.get(&tid(t)).map(String::as_str).unwrap_or("other")
}

// ---------------------------------------------------------------- C: colours

pub fn colours(root: &mut Element, opt: &Value, roles: &Roles) -> Value {
    let hue = opt.get("hue").cloned().unwrap_or(json!({}));
    let shade = opt.get("track_shade").and_then(Value::as_i64).unwrap_or(1);
    let clips = opt_bool(opt, "clips", true);
    let (mut nt, mut nc) = (0, 0);
    let other_h = hue.get("other").and_then(Value::as_i64).unwrap_or(2);
    for t in tracks_mut(root).iter_mut() {
        let h = hue.get(role(roles, t)).and_then(Value::as_i64).unwrap_or(other_h);
        if t.set_value("Color", (shade * 14 + h).to_string()) {
            nt += 1;
        }
        if !clips {
            continue;
        }
        let order = [shade, 0, 2, 3, 4];
        let mut variant: Vec<(String, i64)> = Vec::new();
        for p in clip_paths(t, false) {
            let sig = clip_signature(t.at(&p));
            let shade_of = match variant.iter().find(|(s, _)| *s == sig) {
                Some((_, x)) => *x,
                None => {
                    let x = order[variant.len() % order.len()];
                    variant.push((sig, x));
                    x
                }
            };
            if t.at_mut(&p).set_value("Color", (shade_of * 14 + h).to_string()) {
                nc += 1;
            }
        }
    }
    json!({"tracks": nt, "clips": nc})
}

// ---------------------------------------------------------------- N: names

fn auto_track_name(t: &Element) -> bool {
    let user = t.child("Name").and_then(|n| v(n, "UserName")).unwrap_or("");
    user.is_empty() || AUTO_TRACK.is_match(user.trim())
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
}

pub fn names(root: &mut Element, opt: &Value, roles: &Roles) -> Value {
    let labels = opt.get("labels").and_then(Value::as_object).cloned().unwrap_or_else(default_labels);
    let label_of = |r: &str| labels.get(r).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| title_case(r));
    let only_auto = opt_bool(opt, "only_auto", true);
    let (mut nt, mut nc, mut ns) = (0, 0, 0);
    if opt_bool(opt, "tracks", true) {
        let mut planned: Vec<(usize, String)> = Vec::new();
        for (i, t) in tracks(root).iter().enumerate() {
            if t.tag == "ReturnTrack" || (only_auto && !auto_track_name(t)) {
                continue;
            }
            let r = role(roles, t);
            if r == "other" {
                continue; // nothing reliable to say: its name stays
            }
            let label = label_of(r);
            if t.tag == "GroupTrack" {
                planned.push((i, label));
                continue;
            }
            let mut instr = instrument(t);
            if instr.is_empty() && t.tag == "AudioTrack" {
                instr = sample_names(t)
                    .first()
                    .map(|s| UNDERSCORES.replace_all(s, " ").chars().take(24).collect::<String>().trim().to_string())
                    .unwrap_or_default();
            }
            let n = if !instr.is_empty() && instr.to_lowercase() != label.to_lowercase() {
                format!("{label} - {instr}")
            } else {
                label
            };
            planned.push((i, n));
        }
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (_, n) in &planned {
            *seen.entry(n.clone()).or_default() += 1;
        }
        let mut count: HashMap<String, usize> = HashMap::new();
        let ts = tracks_mut(root);
        for (i, n) in planned {
            let c = count.entry(n.clone()).or_default();
            *c += 1;
            let fin = if seen[&n] > 1 { format!("{n} {c}") } else { n };
            if fin != track_name(&ts[i]) {
                set_track_name(&mut ts[i], &fin);
                nt += 1;
            }
        }
    }
    if opt_bool(opt, "clips", true) {
        let rs: Vec<String> = tracks(root).iter().map(|t| role(roles, t).to_string()).collect();
        for (i, t) in tracks_mut(root).iter_mut().enumerate() {
            if t.tag == "ReturnTrack" {
                continue;
            }
            let label = labels.get(&rs[i]).and_then(Value::as_str).unwrap_or("Clip").to_string();
            let mut letters: Vec<(String, char)> = Vec::new();
            for p in clip_paths(t, false) {
                let c = t.at(&p);
                if c.tag != "MidiClip" {
                    continue;
                }
                if only_auto && !AUTO_CLIP.is_match(v(c, "Name").unwrap_or("")) {
                    continue;
                }
                let sig = clip_signature(c);
                let letter = match letters.iter().find(|(s, _)| *s == sig) {
                    Some((_, l)) => *l,
                    None => {
                        let l = (b'A' + (letters.len() % 26) as u8) as char;
                        letters.push((sig, l));
                        l
                    }
                };
                if t.at_mut(&p).set_value("Name", format!("{label} {letter}")) {
                    nc += 1;
                }
            }
        }
    }
    if opt_bool(opt, "scenes", true)
        && let Some(scenes) = root.child_mut("LiveSet").and_then(|l| l.child_mut("Scenes"))
    {
        let mut i = 0;
        for sc in scenes.children.iter_mut().filter(|c| c.tag == "Scene") {
            i += 1;
            if v(sc, "Name").unwrap_or("").is_empty() {
                sc.set_value("Name", format!("Scène {i}"));
                ns += 1;
            }
        }
    }
    json!({"tracks": nt, "clips": nc, "scenes": ns})
}

// ---------------------------------------------------------------- O: order and groups

/// Top-level blocks (a group with all its tracks, or one track) sorted by role; the tracks
/// inside a group sorted the same way. Returns stay last, in their order.
pub fn order(root: &mut Element, opt: &Value, roles: &mut Roles) -> Value {
    let rank: HashMap<String, usize> = match opt.get("order").and_then(Value::as_array) {
        Some(a) => a.iter().enumerate().filter_map(|(i, r)| r.as_str().map(|r| (r.to_string(), i))).collect(),
        None => ROLES.iter().enumerate().map(|(i, r)| (r.to_string(), i)).collect(),
    };
    let rank_of =
        |roles: &Roles, id: &str| rank.get(roles.get(id).map(String::as_str).unwrap_or("other")).copied().unwrap_or(99);
    let others_ids: Vec<String> = tracks(root).iter().filter(|t| t.tag != "ReturnTrack").map(tid).collect();
    let mut kids = children_of(tracks(root).iter().filter(|t| t.tag != "ReturnTrack"));
    let mut top: Vec<String> =
        tracks(root).iter().filter(|t| t.tag != "ReturnTrack" && group_of(t) == "-1").map(tid).collect();
    let mut made = 0;
    if opt_bool(opt, "make_groups", false) {
        made = make_groups(root, &mut top, roles, &mut kids);
    }
    let is_group: HashSet<String> = tracks(root).iter().filter(|t| t.tag == "GroupTrack").map(tid).collect();
    top.sort_by_key(|id| rank_of(roles, id));
    let inside = opt_bool(opt, "inside_groups", true);
    fn block(
        id: &str,
        inside: bool,
        kids: &HashMap<String, Vec<String>>,
        is_group: &HashSet<String>,
        rank_of: &dyn Fn(&str) -> usize,
        out: &mut Vec<String>,
    ) {
        out.push(id.to_string());
        if is_group.contains(id) {
            let mut inner = kids.get(id).cloned().unwrap_or_default();
            if inside {
                inner.sort_by_key(|k| rank_of(k));
            }
            for k in inner {
                block(&k, inside, kids, is_group, rank_of, out);
            }
        }
    }
    let rk = |id: &str| rank_of(roles, id);
    let mut new: Vec<String> = Vec::new();
    for t in &top {
        block(t, inside, &kids, &is_group, &rk, &mut new);
    }
    let seen: HashSet<String> = new.iter().cloned().collect();
    // compared with the tracks as they were before any group was made (as the prototype)
    new.extend(others_ids.iter().filter(|id| !seen.contains(*id)).cloned()); // orphans, if any: kept, at the end
    let moved = new.iter().zip(others_ids.iter()).filter(|(a, b)| a != b).count();
    // rebuild: the new order, then the returns
    let ts = tracks_mut(root);
    let all: Vec<Element> = std::mem::take(ts);
    let (returns, mut rest): (Vec<Element>, Vec<Element>) = all.into_iter().partition(|t| t.tag == "ReturnTrack");
    let mut out: Vec<Element> = Vec::new();
    for id in &new {
        if let Some(i) = rest.iter().position(|t| tid(t) == *id) {
            out.push(rest.remove(i));
        }
    }
    out.extend(rest);
    out.extend(returns);
    *ts = out;
    json!({"moved": moved, "groups": made})
}

const SEND_OFF: &str = "0.0003162277571";

/// A group that changes nothing: unity volume, centre, sends off, no delay, on, straight to
/// the main output.
fn neutral(g: &mut Element) {
    if let Some(mixer) = g.find_mut("DeviceChain/Mixer") {
        mixer.set_value("Volume/Manual", "1");
        mixer.set_value("Pan/Manual", "0");
        mixer.set_value("Speaker/Manual", "true");
        mixer.set_value("SplitStereoPanL/Manual", "-1");
        mixer.set_value("SplitStereoPanR/Manual", "1");
        mixer.set_value("PanMode", "0");
        if let Some(sends) = mixer.child_mut("Sends") {
            for h in sends.children.iter_mut().filter(|h| h.tag == "TrackSendHolder") {
                h.set_value("Send/Manual", SEND_OFF);
            }
        }
    }
    g.set_value("TrackDelay/Value", "0");
    g.set_value("DeviceChain/AudioOutputRouting/Target", "AudioOut/Main");
    g.set_value("DeviceChain/AudioOutputRouting/UpperDisplayString", "Master");
    g.set_value("DeviceChain/AudioOutputRouting/LowerDisplayString", "");
}

fn next_pointee(root: &Element) -> i64 {
    let mut n = v(live_set(root), "NextPointeeId").and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    for e in root.descendants() {
        if pointee(&e.tag)
            && let Some(id) = e.get("Id").filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
        {
            n = n.max(id.parse::<i64>().unwrap_or(0) + 1);
        }
    }
    n
}

fn renumber(e: &mut Element, nxt: &mut i64) {
    e.walk_mut(&mut |x| {
        if pointee(&x.tag) && x.get("Id").is_some() {
            x.set("Id", nxt.to_string());
            *nxt += 1;
        }
    });
}

/// Ungrouped tracks of the same role (2 or more) go into a new NEUTRAL group (0 dB, centre,
/// no send, no delay, no device): the sound is the same, the mix is tidier. Only tracks going
/// straight to the main output, only when the set already has a group to copy.
fn make_groups(
    root: &mut Element,
    top: &mut Vec<String>,
    roles: &mut Roles,
    kids: &mut HashMap<String, Vec<String>>,
) -> usize {
    let Some(model) = tracks(root).iter().find(|t| t.tag == "GroupTrack").cloned() else { return 0 };
    let refs = track_refs(root);
    let mut by_role: Vec<(String, Vec<String>)> = Vec::new();
    for id in top.iter() {
        let Some(t) = tracks(root).iter().find(|t| tid(t) == *id) else { continue };
        if matches!(t.tag.as_str(), "GroupTrack" | "ReturnTrack") || t.find("DeviceChain/AudioOutputRouting").is_none()
        {
            continue;
        }
        if !matches!(v(t, "DeviceChain/AudioOutputRouting/Target"), Some("AudioOut/Main") | Some("AudioOut/Master")) {
            continue; // going elsewhere: left alone
        }
        if refs.get(id).is_some_and(|r| !r.is_empty()) {
            continue; // something listens to it: left alone
        }
        let r = role(roles, t).to_string();
        match by_role.iter_mut().find(|(x, _)| *x == r) {
            Some(slot) => slot.1.push(id.clone()),
            None => by_role.push((r, vec![id.clone()])),
        }
    }
    let mut nxt = next_pointee(root);
    let mut next_id = tracks(root)
        .iter()
        .filter_map(|t| {
            t.get("Id")
                .filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|i| i.parse::<i64>().ok())
        })
        .max()
        .unwrap_or(0)
        + 1;
    let labels = default_labels();
    let mut made = 0;
    for (r, members) in by_role {
        if members.len() < 2 || r == "other" || r == "return" {
            continue;
        }
        let mut g = model.clone();
        g.walk_mut(&mut |e| {
            if e.tag == "Devices" || e.tag == "Envelopes" {
                e.children.clear();
            }
        });
        neutral(&mut g);
        renumber(&mut g, &mut nxt);
        g.set("Id", next_id.to_string());
        g.set_value("TrackGroupId", "-1");
        g.set_value("TrackUnfolded", "true");
        set_track_name(
            &mut g,
            labels.get(&r).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| title_case(&r)).as_str(),
        );
        let ts = tracks_mut(root);
        for m in &members {
            if let Some(t) = ts.iter_mut().find(|t| tid(t) == *m) {
                t.set_value("TrackGroupId", next_id.to_string());
                t.set_value("DeviceChain/AudioOutputRouting/Target", "AudioOut/GroupTrack");
                t.set_value("DeviceChain/AudioOutputRouting/UpperDisplayString", "Group");
                t.set_value("DeviceChain/AudioOutputRouting/LowerDisplayString", "");
            }
        }
        roles.insert(next_id.to_string(), r.clone());
        let at = ts.iter().position(|t| tid(t) == members[0]).unwrap_or(ts.len());
        ts.insert(at, g);
        kids.insert(next_id.to_string(), members.clone());
        top.retain(|t| !members.contains(t));
        top.push(next_id.to_string());
        next_id += 1;
        made += 1;
    }
    if let Some(l) = root.child_mut("LiveSet") {
        l.set_value("NextPointeeId", nxt.to_string());
    }
    made
}

// ---------------------------------------------------------------- safety

/// Track id -> the ways other parts of the set point at it ("TrackOut", "DeviceIn"...).
pub fn track_refs(root: &Element) -> HashMap<String, HashSet<String>> {
    let mut refs: HashMap<String, HashSet<String>> = HashMap::new();
    for e in root.descendants().filter(|e| e.tag == "Target") {
        for c in REF.captures_iter(e.get("Value").unwrap_or("")) {
            refs.entry(c[1].to_string()).or_default().insert(c[2].to_string());
        }
    }
    refs
}

fn positional(refs: &HashMap<String, HashSet<String>>, id: &str) -> bool {
    refs.get(id).is_some_and(|k| k.iter().any(|k| k.starts_with("Device")))
}

/// Pointee ids something points at: automation, clip envelopes, modulation, mapping.
fn referenced_ids(root: &Element) -> HashSet<String> {
    let mut out: HashSet<String> = root
        .descendants()
        .filter(|e| e.tag == "PointeeId")
        .map(|e| e.get("Value").unwrap_or("None").to_string())
        .collect();
    for sm in root.descendants().filter(|e| e.tag == "SignalModulations") {
        for e in sm.descendants() {
            if let Some(val) = e.get("Value").filter(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit())) {
                out.insert(val.to_string());
            }
        }
    }
    out
}

fn ids_in(e: &Element) -> HashSet<String> {
    e.descendants()
        .filter(|x| pointee(&x.tag))
        .filter_map(|x| x.get("Id").filter(|i| !i.is_empty()).map(str::to_string))
        .collect()
}

fn automated(root: &Element) -> HashSet<String> {
    root.descendants()
        .filter(|e| e.tag == "AutomationEnvelope")
        .map(|e| v(e, "EnvelopeTarget/PointeeId").unwrap_or("None").to_string())
        .collect()
}

fn drop_envelopes(root: &mut Element, ids: &HashSet<String>) {
    root.walk_mut(&mut |e| {
        if e.tag == "Envelopes" {
            e.children.retain(|env| env.value("EnvelopeTarget/PointeeId").is_none_or(|x| !ids.contains(x)));
        }
    });
}

/// Python's `float()` on a Live value: None when it is not a number.
fn pyf(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

fn sends_active(t: &Element, auto: &HashSet<String>) -> bool {
    for h in t.find_all("DeviceChain/Mixer/Sends/TrackSendHolder") {
        let s = h.child("Send");
        let raw = s.and_then(|s| v(s, "Manual")).filter(|x| !x.is_empty()).unwrap_or("0");
        let Some(val) = pyf(raw) else { return true };
        let tgt = s.and_then(|s| s.child("AutomationTarget"));
        if val > 0.00032 || tgt.is_some_and(|t| auto.contains(t.get("Id").unwrap_or("None"))) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------- P: cleanup

/// Removes only what provably plays no part.
pub fn cleanup(root: &mut Element, opt: &Value) -> Value {
    let refs = track_refs(root);
    let used = referenced_ids(root);
    let (mut nt, mut nd, mut nr, mut ns) = (0, 0, 0, 0);
    if opt_bool(opt, "disabled_devices", true) {
        // a switched-off device passes the sound through: removing it changes nothing, unless
        // something points at it or at the POSITION of the devices after it. Only the track's
        // own chain: a device inside a rack may be macro-mapped.
        let ls = root.child_mut("LiveSet").unwrap();
        let main_tag = if ls.child("MainTrack").is_some() { "MainTrack" } else { "MasterTrack" };
        let mut clean = |t: &mut Element, id: &str| {
            if positional(&refs, id) {
                return;
            }
            let Some(chain) = t.find_mut("DeviceChain/DeviceChain/Devices") else { return };
            chain.children.retain(|d| {
                if v(d, "On/Manual") != Some("false") {
                    return true;
                }
                if !ids_in(d).is_disjoint(&used) {
                    return true;
                }
                nd += 1;
                false
            });
        };
        for t in ls.child_mut("Tracks").unwrap().children.iter_mut() {
            let id = tid(t);
            clean(t, &id);
        }
        if let Some(m) = ls.child_mut(main_tag) {
            clean(m, "-1");
        }
    }
    if opt_bool(opt, "empty_tracks", true) {
        // no clip, no device, nothing listening to it, not a group: it makes no sound
        let group_ids: HashSet<String> = tracks(root).iter().map(group_of).collect();
        let keep: Vec<bool> = tracks(root)
            .iter()
            .map(|t| {
                if !matches!(t.tag.as_str(), "MidiTrack" | "AudioTrack") {
                    return true;
                }
                if !clip_paths(t, false).is_empty()
                    || t.descendants().any(|d| d.tag == "Devices" && !d.children.is_empty())
                {
                    return true;
                }
                let id = tid(t);
                refs.get(&id).is_some_and(|r| !r.is_empty()) || group_ids.contains(&id) || !ids_in(t).is_disjoint(&used)
            })
            .collect();
        let ts = tracks_mut(root);
        let mut i = 0;
        ts.retain(|_| {
            let k = keep[i];
            i += 1;
            k
        });
        nt = keep.iter().filter(|k| !**k).count();
    }
    if opt_bool(opt, "unused_returns", true) {
        nr = drop_unused_returns(root, &refs);
    }
    if opt_bool(opt, "simplify_groups", false) {
        ns = simplify_groups(root, &refs, &used);
    }
    json!({"tracks": nt, "devices": nd, "returns": nr, "simplified": ns})
}

/// A return nobody sends to (every send at its minimum, never automated) and nothing else
/// points at makes no sound: removed, with its send slot on every track.
fn drop_unused_returns(root: &mut Element, refs: &HashMap<String, HashSet<String>>) -> usize {
    let auto = automated(root);
    let ts = tracks(root);
    let rets: Vec<String> = ts.iter().filter(|t| t.tag == "ReturnTrack").map(tid).collect();
    let mut unused = Vec::new();
    for (i, r) in rets.iter().enumerate() {
        if refs.get(r).is_some_and(|x| !x.is_empty()) {
            continue;
        }
        let mut used = false;
        for t in ts {
            let holders = t.find_all("DeviceChain/Mixer/Sends/TrackSendHolder");
            let Some(h) = holders.get(i) else { continue };
            let s = h.child("Send");
            let raw = s.and_then(|s| v(s, "Manual")).filter(|x| !x.is_empty()).unwrap_or("0");
            let Some(val) = pyf(raw) else {
                used = true;
                break;
            };
            let tgt = s.and_then(|s| s.child("AutomationTarget"));
            if val > 0.00032 || tgt.is_some_and(|t| auto.contains(t.get("Id").unwrap_or("None"))) {
                used = true;
                break;
            }
        }
        if !used {
            unused.push(i);
        }
    }
    let mut gone: HashSet<String> = HashSet::new();
    // the tracks as they were: a return removed earlier still gives up its send slot's ids
    let mut removed: Vec<Element> = Vec::new();
    for &i in unused.iter().rev() {
        let rid = rets[i].clone();
        let ts = tracks_mut(root);
        let Some(pos) = ts.iter().position(|t| tid(t) == rid) else { continue };
        let ret = ts.remove(pos);
        gone.extend(ids_in(&ret));
        for t in ts.iter_mut().chain(removed.iter_mut()) {
            if let Some(sends) = t.find_mut("DeviceChain/Mixer/Sends")
                && i < sends.children.len()
            {
                let h = sends.children.remove(i);
                gone.extend(ids_in(&h));
            }
        }
        removed.push(ret);
        if let Some(pre) = root.child_mut("LiveSet").and_then(|l| l.child_mut("SendsPre"))
            && i < pre.children.len()
        {
            pre.children.remove(i);
        }
    }
    drop_envelopes(root, &gone);
    unused.len()
}

/// Devices whose effect on a sum equals the sum of their effects AND that treat left and
/// right alike, so they commute with a track's volume and pan.
fn linear_ok(d: &Element) -> bool {
    match d.tag.as_str() {
        "FilterEQ3" => true,
        "Eq8" => matches!(v(d, "Mode"), Some("0") | Some("1")),
        "StereoGain" => {
            v(d, "Source/Manual").unwrap_or("3") == "3"
                && pyf(Some(v(d, "StereoSeparation/Manual").unwrap_or("1")).filter(|x| !x.is_empty()).unwrap_or("1"))
                    == Some(1.0)
                && v(d, "BassMono/Manual").unwrap_or("false") == "false"
                && v(d, "Mono/Manual").unwrap_or("false") == "false"
        }
        _ => false,
    }
}

const SKIP_IN_COMPARE: [&str; 23] = [
    "LomId",
    "LomIdView",
    "IsExpanded",
    "BreakoutIsExpanded",
    "ViewData",
    "IsFolded",
    "ShouldShowPresetName",
    "LastSelectedTimeableIndex",
    "LastSelectedClipEnvelopeIndex",
    "LastPresetRef",
    "UserName",
    "Annotation",
    "SourceContext",
    "SpectrumAnalyzer",
    "SelectedBand",
    "EditMode",
    "ModulationSourceCount",
    "ParametersListWrapper",
    "LockedScripts",
    "Pointee",
    "AutomationTarget",
    "ModulationTarget",
    "AuditionOnOff",
];

/// What decides a device's sound: its type and its parameter values.
fn sound_key(d: &Element) -> String {
    let kids: String =
        d.children.iter().filter(|c| !SKIP_IN_COMPARE.contains(&c.tag.as_str())).map(sound_key).collect();
    format!("<{} {}>{kids}</{}>", d.tag, d.get("Value").unwrap_or(""), d.tag)
}

/// When every track of a group ends with the SAME linear devices (same settings), one copy at
/// the start of the group's chain sounds identical: they are moved there.
fn simplify_groups(root: &mut Element, refs: &HashMap<String, HashSet<String>>, used: &HashSet<String>) -> usize {
    let auto = automated(root);
    let groups: Vec<String> = tracks(root).iter().filter(|t| t.tag == "GroupTrack").map(tid).collect();
    let mut moved = 0;
    for gid in groups {
        let kids = children_of(tracks(root).iter());
        let members = kids.get(&gid).cloned().unwrap_or_default();
        let ts = tracks(root);
        let Some(g) = ts.iter().find(|t| tid(t) == gid) else { continue };
        if members.len() < 2
            || g.find("DeviceChain/DeviceChain/Devices").is_none()
            || refs.get(&gid).is_some_and(|r| !r.is_empty())
        {
            continue;
        }
        let mts: Vec<&Element> = members.iter().filter_map(|m| ts.iter().find(|t| tid(t) == *m)).collect();
        if mts.iter().any(|m| !matches!(m.tag.as_str(), "MidiTrack" | "AudioTrack")) {
            continue;
        }
        let ok = mts.iter().all(|m| {
            v(m, "DeviceChain/AudioOutputRouting/Target") == Some("AudioOut/GroupTrack")
                && !refs.get(&tid(m)).is_some_and(|r| !r.is_empty())
                && v(m, "Freeze") != Some("true")
                && !sends_active(m, &auto)
                && m.find("DeviceChain/DeviceChain/Devices").is_some()
        });
        if !ok {
            continue;
        }
        let chains: Vec<&Vec<Element>> =
            mts.iter().map(|m| &m.find("DeviceChain/DeviceChain/Devices").unwrap().children).collect();
        let mut n = 0;
        while chains.iter().all(|c| c.len() > n) {
            let ds: Vec<&Element> = chains.iter().map(|c| &c[c.len() - 1 - n]).collect();
            if !ds.iter().all(|d| linear_ok(d) && v(d, "On/Manual") != Some("false") && ids_in(d).is_disjoint(used)) {
                break;
            }
            let keys: HashSet<String> = ds.iter().map(|d| sound_key(d)).collect();
            if keys.len() != 1 {
                break;
            }
            n += 1;
        }
        if n == 0 {
            continue;
        }
        let common: Vec<Element> = chains[0][chains[0].len() - n..].to_vec();
        let mut nxt = next_pointee(root);
        let gchain_len_ids: i64 = {
            let gch = &tracks(root)
                .iter()
                .find(|t| tid(t) == gid)
                .unwrap()
                .find("DeviceChain/DeviceChain/Devices")
                .unwrap()
                .children;
            gch.iter()
                .filter_map(|d| {
                    d.get("Id")
                        .filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
                        .and_then(|i| i.parse::<i64>().ok())
                })
                .max()
                .unwrap_or(-1)
                + 1
        };
        let mut copies = Vec::new();
        for (i, d) in common.iter().enumerate() {
            let mut c = d.clone();
            renumber(&mut c, &mut nxt);
            c.set("Id", (gchain_len_ids + i as i64).to_string());
            copies.push(c);
        }
        let ts = tracks_mut(root);
        if let Some(g) = ts.iter_mut().find(|t| tid(t) == gid) {
            let gch = g.find_mut("DeviceChain/DeviceChain/Devices").unwrap();
            for (i, c) in copies.into_iter().enumerate() {
                gch.children.insert(i, c); // first in the group: it processes the sum, as before
            }
        }
        for m in &members {
            if let Some(t) = ts.iter_mut().find(|t| tid(t) == *m) {
                let ch = t.find_mut("DeviceChain/DeviceChain/Devices").unwrap();
                let l = ch.children.len();
                ch.children.truncate(l - n);
            }
        }
        if let Some(l) = root.child_mut("LiveSet") {
            l.set_value("NextPointeeId", nxt.to_string());
        }
        moved += n * members.len();
    }
    moved
}

// ---------------------------------------------------------------- M: markers

pub fn markers(root: &mut Element, opt: &Value, time_sig: i64) -> Value {
    let Some(holder_len) = live_set(root).find("Locators/Locators").map(|h| h.children.len()) else {
        return json!({"markers": 0, "skipped": i18n::tr!("repair.noLocators")});
    };
    if opt_bool(opt, "only_if_none", true) && holder_len > 0 {
        return json!({"markers": 0, "skipped": i18n::tr!("repair.hasLocators")});
    }
    let bar = time_sig as f64;
    let mut spans: Vec<(f64, f64, String)> = Vec::new();
    for t in tracks(root) {
        if t.tag == "GroupTrack" || t.tag == "ReturnTrack" {
            continue;
        }
        for p in clip_paths(t, true) {
            let c = t.at(&p);
            if v(c, "Disabled") == Some("true") {
                continue;
            }
            let (Some(s), Some(e)) = (v(c, "CurrentStart").and_then(pyf), v(c, "CurrentEnd").and_then(pyf)) else {
                continue;
            };
            // a section changes when what plays changes: the track AND its clip's content
            spans.push((s, e, format!("{}:{}", tid(t), &clip_signature(c)[..10])));
        }
    }
    if spans.is_empty() {
        return json!({"markers": 0, "skipped": i18n::tr!("repair.emptyArrangement")});
    }
    let end = spans.iter().map(|x| x.1).fold(f64::NEG_INFINITY, f64::max);
    let nbars = (end / bar).floor() as i64 + 1;
    let mut playing: Vec<Vec<String>> = Vec::new();
    for b in 0..nbars {
        let (a, z) = (b as f64 * bar, (b + 1) as f64 * bar);
        let mut set: Vec<String> = spans.iter().filter(|(s, e, _)| *s < z && *e > a).map(|x| x.2.clone()).collect();
        set.sort();
        set.dedup();
        playing.push(set);
    }
    let mut runs: Vec<(i64, Vec<String>, i64)> = Vec::new();
    for (b, p) in playing.into_iter().enumerate() {
        match runs.last_mut() {
            Some(r) if r.1 == p => r.2 += 1,
            _ => runs.push((b as i64, p, 1)),
        }
    }
    let minb = opt.get("min_bars").and_then(Value::as_i64).unwrap_or(8);
    let mut merged: Vec<(i64, Vec<String>, i64)> = Vec::new();
    for r in runs {
        match merged.last_mut() {
            Some(m) if r.2 < minb || m.2 < minb => m.2 += r.2,
            _ => merged.push(r),
        }
    }
    merged.retain(|r| !r.1.is_empty());
    let mut letters: Vec<(Vec<String>, char)> = Vec::new();
    let mut locs = Vec::new();
    for (start, combo, _) in &merged {
        let l = match letters.iter().find(|(c, _)| c == combo) {
            Some((_, l)) => *l,
            None => {
                let l = (b'A' + (letters.len() % 26) as u8) as char;
                letters.push((combo.clone(), l));
                l
            }
        };
        let mut loc = Element::new("Locator");
        for (tag, val) in [
            ("LomId", "0".to_string()),
            ("Time", (start * time_sig).to_string()),
            ("Name", l.to_string()),
            ("Annotation", String::new()),
            ("IsSongStart", "false".to_string()),
        ] {
            let mut e = Element::new(tag);
            e.set("Value", val);
            loc.children.push(e);
        }
        locs.push(loc);
    }
    let n = locs.len();
    root.child_mut("LiveSet").unwrap().find_mut("Locators/Locators").unwrap().children.extend(locs);
    json!({"markers": n})
}

// ---------------------------------------------------------------- G: template

fn note(n: &str) -> Option<i64> {
    Some(match n {
        "C" => 0,
        "C#" | "DB" => 1,
        "D" => 2,
        "D#" | "EB" => 3,
        "E" => 4,
        "F" => 5,
        "F#" | "GB" => 6,
        "G" => 7,
        "G#" | "AB" => 8,
        "A" => 9,
        "A#" | "BB" => 10,
        "B" => 11,
        _ => return None,
    })
}

static SCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([A-G][#b]?)\s*(m|min|minor|maj|major)?$").unwrap());

/// The device inside an .adg rack preset (gzip XML).
pub fn load_rack(path: &std::path::Path) -> Option<Element> {
    let set = crate::xml::read_set(path).ok()?;
    let holder = set.root.find(".//GroupDevicePreset/Device")?;
    holder.children.first().cloned()
}

pub fn template(root: &mut Element, opt: &Value) -> Value {
    let mut out = Map::new();
    let main_tag = if live_set(root).child("MainTrack").is_some() { "MainTrack" } else { "MasterTrack" };
    if let Some(t) = opt.get("tempo").filter(|t| truthy(t)) {
        let f = t.as_f64().or_else(|| t.as_str().and_then(pyf)).unwrap_or(0.0);
        if root
            .child_mut("LiveSet")
            .and_then(|l| l.child_mut(main_tag))
            .is_some_and(|m| m.set_value("DeviceChain/Mixer/Tempo/Manual", py_float(f)))
        {
            out.insert("tempo".into(), json!(f));
        }
    }
    let sc = opt.get("scale").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if let Some(m) = SCALE.captures(&sc) {
        let ls = root.child_mut("LiveSet").unwrap();
        let minor = matches!(m.get(2).map(|x| x.as_str().to_lowercase()).as_deref(), Some("m" | "min" | "minor"));
        let tonic = note(&m[1].to_uppercase()).unwrap_or(0).to_string();
        // Live 12.1+: Root + the scale's index; Live 11 and 12.0: RootNote + the scale's name
        let new_style = ls.child("ScaleInformation").and_then(|s| s.child("Root")).is_some();
        let old_style = ls.child("ScaleInformation").and_then(|s| s.child("RootNote")).is_some();
        if new_style || old_style {
            let si = ls.child_mut("ScaleInformation").unwrap();
            if new_style {
                si.set_value("Root", tonic);
                si.set_value("Name", if minor { "1" } else { "0" });
            } else {
                si.set_value("RootNote", tonic);
                si.set_value("Name", if minor { "Minor" } else { "Major" });
            }
            ls.set_value("InKey", "true");
            out.insert("scale".into(), json!(sc));
        }
    }
    let rack = opt.get("main_rack").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if !rack.is_empty() {
        let dev = load_rack(std::path::Path::new(&rack));
        let has_chain =
            live_set(root).child(main_tag).and_then(|m| m.find("DeviceChain/DeviceChain/Devices")).is_some();
        match (dev, has_chain) {
            (Some(mut dev), true) => {
                let mut nxt = next_pointee(root);
                renumber(&mut dev, &mut nxt);
                let devs = root
                    .child_mut("LiveSet")
                    .unwrap()
                    .child_mut(main_tag)
                    .unwrap()
                    .find_mut("DeviceChain/DeviceChain/Devices")
                    .unwrap();
                let id = devs
                    .children
                    .iter()
                    .filter_map(|d| {
                        d.get("Id")
                            .filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
                            .and_then(|i| i.parse::<i64>().ok())
                    })
                    .max()
                    .unwrap_or(-1)
                    + 1;
                dev.set("Id", id.to_string());
                devs.children.push(dev);
                root.child_mut("LiveSet").unwrap().set_value("NextPointeeId", nxt.to_string());
                out.insert("rack".into(), json!(stem(&rack.replace('\\', "/"))));
            }
            _ => {
                out.insert("rack_error".into(), json!(i18n::tr!("repair.rackError")));
            }
        }
    }
    Value::Object(out)
}

// ---------------------------------------------------------------- one pass

/// Applies the preset's treatments (except R) to a parsed set. Returns a report.
pub fn treat(root: &mut Element, preset: &Value, m: RoleMatch) -> Value {
    let mut rep = Map::new();
    let ts = live_set(root)
        .find(".//MainTrack/DeviceChain/Mixer/TimeSignature/Manual")
        .and_then(|e| e.get("Value"))
        .filter(|x| !x.is_empty())
        .unwrap_or("201")
        .trim()
        .parse::<i64>()
        .map(|v| v.rem_euclid(99) + 1)
        .unwrap_or(4);
    if on(&preset["P"]) {
        rep.insert("P".into(), cleanup(root, &preset["P"]));
    }
    let mut roles = roles_of(root, m);
    let mut counts: Map<String, Value> = Map::new();
    for t in tracks(root) {
        let r = role(&roles, t).to_string();
        let c = counts.get(&r).and_then(Value::as_i64).unwrap_or(0);
        counts.insert(r, json!(c + 1));
    }
    rep.insert("roles".into(), Value::Object(counts));
    if on(&preset["O"]) {
        rep.insert("O".into(), order(root, &preset["O"], &mut roles));
    }
    if on(&preset["C"]) {
        rep.insert("C".into(), colours(root, &preset["C"], &roles));
    }
    if on(&preset["N"]) {
        rep.insert("N".into(), names(root, &preset["N"], &roles));
    }
    if on(&preset["M"]) {
        rep.insert("M".into(), markers(root, &preset["M"], ts));
    }
    if on(&preset["G"]) {
        rep.insert("G".into(), template(root, &preset["G"]));
    }
    Value::Object(rep)
}

/// What the preview shows: each track with its name, role, colour, group depth.
pub fn track_list(root: &Element, m: RoleMatch) -> Vec<Value> {
    let roles = roles_of(root, m);
    let mut depth: HashMap<String, i64> = HashMap::new();
    let mut out = Vec::new();
    for t in tracks(root) {
        let g = group_of(t);
        let d = if g != "-1" { depth.get(&g).copied().unwrap_or(-1) + 1 } else { 0 };
        depth.insert(tid(t), d);
        out.push(json!({
            "name": track_name(t), "kind": t.tag.replace("Track", ""), "role": role(&roles, t),
            "color": v(t, "Color").filter(|x| !x.is_empty()).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0), "depth": d,
        }));
    }
    out
}

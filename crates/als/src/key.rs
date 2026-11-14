//! Key and mode guessed from the MIDI notes.
use crate::NOTE_NAMES;
use crate::role::{Role, track_role};
use crate::xml::{N, child, iter, iter_tag, py_sum, round2, tag, val};
use regex::Regex;
use std::sync::LazyLock;

/// Key profiles: Faraldo's "bgate" (electronic dance music; Essentia's default), the same as
/// crates/midi. A project's notes come by clip, not in playing order, so the bass is the low
/// register (below E3) rather than the lowest note at each moment. On 2,507 sample-pack MIDI
/// files whose name gives the key (packs not used to choose): Krumhansl-Kessler 39 % exact
/// (what this was), bgate 50 %, bgate + the low register 78 %.
const MAJOR: [f64; 12] = [1.00, 0.00, 0.42, 0.00, 0.53, 0.37, 0.00, 0.77, 0.00, 0.38, 0.21, 0.30];
const MINOR: [f64; 12] = [1.00, 0.00, 0.36, 0.39, 0.00, 0.38, 0.00, 0.74, 0.27, 0.00, 0.42, 0.23];
/// Notes below this one (E3) are the bass.
const LOW_BELOW: i64 = 52;
const LOW_WEIGHT: f64 = 1.0;

static DRUM_TRACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)kick|\bkk\b|snare|\bsd\b|hat|\bhh\b|perc|drum|clap|\btoms?\b|cym|ride|shaker|break|\brim|batt?erie|\bbeat|\bfx\b|\bsfx\b",
    )
    .unwrap()
});

/// Modes, read from the note that tells them apart from plain major / minor (semitones
/// above the tonic): (mode, its note, the major / minor note it replaces). Live's names.
const MAJOR_MODES: [(&str, usize, usize); 2] = [("Lydian", 6, 5), ("Mixolydian", 10, 11)];
const MINOR_MODES: [(&str, usize, usize); 3] = [("Dorian", 9, 8), ("Phrygian", 1, 2), ("Harmonic Minor", 11, 10)];
/// The mode's note must outweigh the usual one 3 to 1, and the pair be heard at all.
const MODE_SHARE: f64 = 0.75;
const MODE_MASS: f64 = 0.03;

/// "Dorian", "Lydian"... when the notes clearly say so, else None (plain major / minor).
/// Two modes found at once contradict each other: None.
pub fn refine_mode(hist: &[f64; 12], tonic: usize, minor: bool) -> Option<&'static str> {
    let total = match py_sum(hist.iter().copied()) {
        0.0 => 1.0,
        t => t,
    };
    let modes: &[(&str, usize, usize)] = if minor { &MINOR_MODES } else { &MAJOR_MODES };
    let found: Vec<&str> = modes
        .iter()
        .filter(|(_, note, usual)| {
            let (a, b) = (hist[(tonic + note) % 12], hist[(tonic + usual) % 12]);
            let pair = if a + b == 0.0 { 1.0 } else { a + b };
            (a + b) / total >= MODE_MASS && a / pair >= MODE_SHARE
        })
        .map(|m| m.0)
        .collect();
    if found.len() == 1 { Some(found[0]) } else { None }
}

fn corr(x: &[f64; 12], y: &[f64; 12]) -> f64 {
    let (mx, my) = (py_sum(x.iter().copied()) / 12.0, py_sum(y.iter().copied()) / 12.0);
    let num = py_sum(x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)));
    let den = (py_sum(x.iter().map(|a| (a - mx).powi(2))) * py_sum(y.iter().map(|b| (b - my).powi(2)))).powf(0.5);
    if den != 0.0 { num / den } else { 0.0 }
}

/// The bass's main note is the tonic.
fn bass_on_tonic(bass: &[f64; 12], tonic: usize) -> bool {
    py_sum(bass.iter().copied()) > 0.0 && (0..12).fold(0, |best, i| if bass[i] > bass[best] { i } else { best }) == tonic
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyEstimate {
    pub key: Option<String>,
    pub conf: f64,
    /// The bass confirms the tonic: makes an estimate under 0.85 worth showing.
    pub bass: bool,
}

/// Key guessed from the MIDI notes (drum tracks left out), weighted by duration.
pub fn estimate_key(tracks: N) -> KeyEstimate {
    let (mut hist, mut bass, mut low, mut n) = ([0.0f64; 12], [0.0f64; 12], [0.0f64; 12], 0usize);
    for t in iter_tag(tracks, "MidiTrack") {
        let name = val(child(t, "Name"), "EffectiveName").unwrap_or("");
        if DRUM_TRACK.is_match(name) || iter(t).any(|d| matches!(tag(d), "DrumGroupDevice" | "InstrumentImpulse")) {
            continue;
        }
        let is_bass = track_role(t) == Role::Bass;
        for kt in iter_tag(t, "KeyTrack") {
            let Some(pitch) = val(Some(kt), "MidiKey").and_then(|v| v.trim().parse::<i64>().ok()) else { continue };
            let pc = pitch.rem_euclid(12) as usize;
            for ev in iter_tag(kt, "MidiNoteEvent") {
                if ev.attribute("IsEnabled") == Some("false") {
                    continue;
                }
                let raw = ev.attribute("Duration").filter(|s| !s.is_empty()).unwrap_or("0");
                let Ok(d) = raw.trim().parse::<f64>() else { continue };
                let d = d.min(8.0);
                hist[pc] += d;
                if pitch < LOW_BELOW {
                    low[pc] += d;
                }
                if is_bass {
                    bass[pc] += d;
                }
                n += 1;
            }
        }
    }
    if n < 30 {
        return KeyEstimate { key: None, conf: 0.0, bass: false };
    }
    let has_low = py_sum(low.iter().copied()) > 0.0;
    // (combined score, the notes' correlation alone, tonic, minor)
    let (mut top, mut score, mut tonic, mut minor) = (f64::MIN, -2.0f64, 0usize, false);
    for t in 0..12 {
        for (prof, m) in [(&MAJOR, false), (&MINOR, true)] {
            let rotated: [f64; 12] = std::array::from_fn(|i| prof[(i + 12 - t) % 12]);
            let c = corr(&hist, &rotated);
            let all = c + if has_low { LOW_WEIGHT * corr(&low, &rotated) } else { 0.0 };
            if all > top {
                (top, score, tonic, minor) = (all, c, t, m);
            }
        }
    }
    if score < 0.6 {
        return KeyEstimate { key: None, conf: round2(score), bass: false };
    }
    let best = format!("{}{}", NOTE_NAMES[tonic], if minor { "m" } else { "" });
    let key = match refine_mode(&hist, tonic, minor) {
        Some(mode) => format!("{} {}", NOTE_NAMES[tonic], mode),
        None => best,
    };
    KeyEstimate { key: Some(key), conf: round2(score), bass: bass_on_tonic(&bass, tonic) || bass_on_tonic(&low, tonic) }
}

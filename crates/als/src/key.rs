//! Key and mode guessed from the MIDI notes.
use crate::NOTE_NAMES;
use crate::role::{Role, track_role};
use crate::xml::{N, child, iter, iter_tag, py_sum, round2, tag, val};
use regex::Regex;
use std::sync::LazyLock;

/// Krumhansl-Kessler key profiles: how much each scale degree "belongs" to a major / minor key.
const MAJOR: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

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

/// The bass's main note is the tonic: the quality then comes from the third heard over it.
/// None when the bass points elsewhere.
fn bass_confirms(hist: &[f64; 12], bass: &[f64; 12], tonic: usize) -> Option<bool> {
    if py_sum(bass.iter().copied()) == 0.0 {
        return None;
    }
    let top = (0..12).fold(0, |best, i| if bass[i] > bass[best] { i } else { best });
    if top != tonic {
        return None;
    }
    let (mut m3, mut maj3) = (hist[(tonic + 3) % 12], hist[(tonic + 4) % 12]);
    if m3 == maj3 {
        m3 = hist[(tonic + 8) % 12] + hist[(tonic + 10) % 12];
        maj3 = hist[(tonic + 9) % 12] + hist[(tonic + 11) % 12];
    }
    Some(m3 > maj3)
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
    let (mut hist, mut bass, mut n) = ([0.0f64; 12], [0.0f64; 12], 0usize);
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
    let (mut best, mut score, mut tonic, mut minor) = (String::new(), -2.0f64, 0usize, false);
    for t in 0..12 {
        for (prof, suffix) in [(&MAJOR, ""), (&MINOR, "m")] {
            let rotated: [f64; 12] = std::array::from_fn(|i| prof[(i + 12 - t) % 12]);
            let c = corr(&hist, &rotated);
            if c > score {
                best = format!("{}{}", NOTE_NAMES[t], suffix);
                score = c;
                tonic = t;
                minor = !suffix.is_empty();
            }
        }
    }
    if score < 0.6 {
        return KeyEstimate { key: None, conf: round2(score), bass: false };
    }
    let confirmed = bass_confirms(&hist, &bass, tonic);
    if let Some(m) = confirmed
        && m != minor
    {
        minor = m;
        best = format!("{}{}", NOTE_NAMES[tonic], if m { "m" } else { "" });
    }
    let key = match refine_mode(&hist, tonic, minor) {
        Some(mode) => format!("{} {}", NOTE_NAMES[tonic], mode),
        None => best,
    };
    KeyEstimate { key: Some(key), conf: round2(score), bass: confirmed.is_some() }
}

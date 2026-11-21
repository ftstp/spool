//! What a MIDI file holds, read for the project generator: each part (track) said to be
//! drums, chords, bass or melody; the key (Krumhansl-Kessler on note durations), the tempo
//! and metre, the length in bars, and for harmony the chord of each bar ("Am F C G", with
//! its degrees "i VI III VII").
//!
//! The guesses lean on the notes first (channel 10 = drums, notes sounding together =
//! chords, low and one at a time = bass), then on the file and track names ("bass", "arp",
//! "drum"...), which producers name well.
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Part {
    pub name: String,
    /// "drums", "chords", "bass", "melody".
    pub kind: String,
    #[ts(type = "number")]
    pub notes: usize,
    /// Lowest and highest note (MIDI numbers).
    pub low: u8,
    pub high: u8,
    /// Notes sounding together, on average at each note start.
    pub poly: f32,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct MidiInfo {
    /// The file's main kind ("drums", "chords", "bass", "melody"), "multi" for a full
    /// arrangement (several kinds of parts), "" when it has no note.
    pub kind: String,
    pub parts: Vec<Part>,
    /// "Am", "F#", None for drums only or too few notes.
    pub key: Option<String>,
    /// How clearly the notes say the key, 0..1 (correlation).
    pub key_strength: f32,
    pub bpm: Option<f32>,
    pub meter: String,
    pub bars: f32,
    #[ts(type = "number")]
    pub notes: usize,
    /// One chord per bar, repeats folded ("Am", "F", "C", "G").
    pub chords: Vec<String>,
    /// The same as degrees of the key ("i", "VI", "III", "VII").
    pub degrees: Vec<String>,
    pub error: Option<String>,
}

const NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const FLATS: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const SHARPS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// The note names a key is written with: flats for F, Bb, Eb, Ab, Db (and their relative
/// minors), sharps for G, D, A, E, B, F# (and theirs).
fn spelling(tonic: usize, minor: bool) -> &'static [&'static str; 12] {
    let major = if minor { (tonic + 3) % 12 } else { tonic };
    if [5, 10, 3, 8, 1].contains(&major) { &FLATS } else if major == 0 { &NAMES } else { &SHARPS }
}

/// "C#m", "Db", "Ebm": the tonic as musicians write that key.
pub fn key_name(tonic: usize, minor: bool) -> String {
    let n = match (tonic, minor) {
        (1, true) => "C#",
        (1, false) => "Db",
        (3, _) => "Eb",
        (6, _) => "F#",
        (8, true) => "G#",
        (8, false) => "Ab",
        (10, _) => "Bb",
        _ => NAMES[tonic],
    };
    format!("{n}{}", if minor { "m" } else { "" })
}
/// Key profiles: Faraldo's "bgate" (electronic dance music, Beatport; Essentia's default).
/// On 2,507 sample-pack MIDI files whose name gives the key, scored on packs not used to
/// choose them: Krumhansl-Kessler alone 39 % exact, bgate alone 50 %, bgate with the bass
/// line and the first / last bass note (below) 86 % (Spool 0.1.1: 44 %).
pub const MAJOR: [f32; 12] = [1.00, 0.00, 0.42, 0.00, 0.53, 0.37, 0.00, 0.77, 0.00, 0.38, 0.21, 0.30];
pub const MINOR: [f32; 12] = [1.00, 0.00, 0.36, 0.39, 0.00, 0.38, 0.00, 0.74, 0.27, 0.00, 0.42, 0.23];
/// The bass line's weight next to all the notes', and the bonus of a key whose tonic is the
/// first or the last bass note (each).
const BASS_WEIGHT: f32 = 0.5;
const END_BONUS: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
struct Note {
    start: u64,
    end: u64,
    key: u8,
    ch: u8,
    track: usize,
}

fn corr(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        num += (a[i] - ma) * (b[i] - mb);
        da += (a[i] - ma).powi(2);
        db += (b[i] - mb).powi(2);
    }
    if da == 0.0 || db == 0.0 { 0.0 } else { num / (da * db).sqrt() }
}

/// Every key's score: the notes' correlation with its profile, plus the bass line's (the
/// lowest note sounding at each note start), plus a bonus when the music starts or ends on
/// its tonic in the bass: (tonic, minor, score, the notes' correlation alone), best first.
pub fn rank_keys(h: &[f32; 12], bass: &[f32; 12], first: Option<usize>, last: Option<usize>) -> Vec<(usize, bool, f32, f32)> {
    let notes = keys_of(h);
    let lows = keys_of(bass);
    let mut all: Vec<(usize, bool, f32, f32)> = notes
        .iter()
        .map(|&(t, m, c)| {
            let b = lows.iter().find(|k| k.0 == t && k.1 == m).map(|k| k.2).unwrap_or(0.0);
            let ends = END_BONUS * ((first == Some(t)) as u8 + (last == Some(t)) as u8) as f32;
            (t, m, c + BASS_WEIGHT * b + ends, c)
        })
        .collect();
    all.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    all
}

/// Every key's correlation with a pitch-class histogram: (tonic, minor, strength), best
/// first.
fn keys_of(h: &[f32; 12]) -> Vec<(usize, bool, f32)> {
    if h.iter().sum::<f32>() <= 0.0 {
        return Vec::new();
    }
    let mut all = Vec::new();
    for t in 0..12 {
        let rot: [f32; 12] = std::array::from_fn(|i| h[(i + t) % 12]);
        for (minor, prof) in [(false, &MAJOR), (true, &MINOR)] {
            all.push((t, minor, corr(&rot, prof)));
        }
    }
    all.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    all
}

/// Chord templates: (suffix, intervals).
const CHORDS: [(&str, &[usize]); 8] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("dim", &[0, 3, 6]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
];

/// The chord a pitch-class weight vector sounds like, with the bass note as a hint.
fn chord_of(w: &[f32; 12], bass: Option<usize>) -> Option<(usize, &'static str)> {
    let total: f32 = w.iter().sum();
    if total <= 0.0 || w.iter().filter(|x| **x > 0.0).count() < 2 {
        return None;
    }
    let mut best: Option<(usize, &str, f32)> = None;
    for root in 0..12 {
        for (suf, iv) in CHORDS {
            let inside: f32 = iv.iter().map(|i| w[(root + i) % 12]).sum();
            let outside = total - inside;
            // every chord tone counts, missing tones cost, a bass on the root helps
            let missing = iv.iter().filter(|i| w[(root + **i) % 12] == 0.0).count() as f32;
            let mut score = inside - outside * 0.8 - missing * total * 0.15 - (iv.len() as f32 - 3.0) * total * 0.05;
            if bass == Some(root) {
                score += total * 0.1;
            }
            if best.is_none_or(|b| score > b.2) {
                best = Some((root, suf, score));
            }
        }
    }
    best.filter(|b| b.2 > 0.0).map(|b| (b.0, b.1))
}

/// The chord of a stretch of a song, in its key: the simplest chord that says the notes.
/// A triad unless the added tone is really heard (a jazz line passing over a chord does
/// not make it a maj7 or a sus), and among chords that fit as well, the key's own (an A
/// and an E with no third, in A minor, is Am).
fn chord_in(w: &[f32; 12], bass: Option<usize>, scale: Option<&[bool; 12]>) -> Option<(usize, &'static str)> {
    let total: f32 = w.iter().sum();
    if total <= 0.0 || w.iter().filter(|x| **x > 0.0).count() < 2 {
        return None;
    }
    let mut best: Option<(usize, &str, f32)> = None;
    for root in 0..12 {
        for (suf, iv) in CHORDS {
            let at = |i: usize| w[(root + i) % 12];
            let inside: f32 = iv.iter().map(|i| at(*i)).sum();
            let outside = total - inside;
            // a richer chord must be earned: its added tone really heard (a sus with no third
            // at all), not a line passing on it
            let earned = match suf {
                // a sus is a chord on its own root: over another bass it is a line passing by
                "sus4" => at(5) >= total * 0.15 && at(3) + at(4) < total * 0.05 && bass.is_none_or(|b| b == root),
                "sus2" => at(2) >= total * 0.15 && at(3) + at(4) < total * 0.05 && bass.is_none_or(|b| b == root),
                "7" | "m7" => at(10) >= total * 0.12,
                "maj7" => at(11) >= total * 0.12,
                "dim" => at(6) >= total * 0.12,
                _ => true,
            };
            if !earned {
                continue;
            }
            // a tone missing costs, but an open fifth (root and fifth, no third) is the plain
            // chord: the key says which
            let open_fifth = matches!(suf, "" | "m") && at(0) > 0.0 && at(7) > 0.0;
            let missing: f32 = iv
                .iter()
                .filter(|i| at(**i) < total * 0.02)
                .map(|i| if open_fifth && matches!(*i, 3 | 4) { 0.05 } else { 0.25 })
                .sum();
            let extra = match suf {
                "" | "m" => 0.0,
                "dim" => 0.15,
                "sus4" | "sus2" => 0.18,
                _ => 0.2,
            };
            let mut score = inside - outside * 0.8 - missing * total - extra * total;
            if bass == Some(root) {
                score += total * 0.15;
            }
            if scale.is_some_and(|s| iv.iter().all(|i| s[(root + i) % 12])) {
                score += total * 0.12;
            }
            if best.is_none_or(|b| score > b.2) {
                best = Some((root, suf, score));
            }
        }
    }
    best.filter(|b| b.2 > 0.0).map(|b| (b.0, b.1))
}

/// The notes of a key: the major scale, or the minor with its raised seventh (the V of a
/// minor key is major).
fn scale_of(tonic: usize, minor: bool) -> [bool; 12] {
    let iv: &[usize] = if minor { &[0, 2, 3, 5, 7, 8, 10, 11] } else { &[0, 2, 4, 5, 7, 9, 11] };
    let mut s = [false; 12];
    for i in iv {
        s[(tonic + i) % 12] = true;
    }
    s
}

fn roman(degree: usize, minor_chord: bool) -> String {
    const R: [&str; 12] = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"];
    let r = R[degree % 12];
    if minor_chord { r.to_lowercase() } else { r.to_string() }
}

fn hint(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| n.contains(w));
    if has(&["drum", "kick", "snare", "hat", "perc", "beat", "groove", "break", "clap", "tom", "batterie"]) {
        Some("drums")
    } else if has(&["bass", "sub", "basse", "808"]) {
        Some("bass")
    } else if has(&["chord", "accord", "harmon", "pad", "key", "piano", "prog", "stab"]) {
        Some("chords")
    } else if has(&["lead", "melod", "arp", "pluck", "hook", "topline", "vocal", "voix", "riff"]) {
        Some("melody")
    } else {
        None
    }
}

/// Reads a MIDI file. `name`: its name, or the end of its path (the words of the file and of
/// its folders help say what it is: "Drums/Breaks/b01.mid").
pub fn analyse(bytes: &[u8], name: &str) -> MidiInfo {
    let smf = match midly::Smf::parse(bytes) {
        Ok(s) => s,
        Err(e) => return MidiInfo { error: Some(i18n::tr!("midi.unreadable", e = e)), ..Default::default() },
    };
    let tpq = match smf.header.timing {
        midly::Timing::Metrical(t) => t.as_int().max(1) as u64,
        midly::Timing::Timecode(..) => 480,
    };
    let (mut bpm, mut meter) = (None::<f32>, (4u8, 4u8));
    let mut notes: Vec<Note> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut t = 0u64;
        let mut open: std::collections::HashMap<(u8, u8), u64> = std::collections::HashMap::new();
        let mut tname = String::new();
        for ev in track {
            t += ev.delta.as_int() as u64;
            match ev.kind {
                midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(us)) if bpm.is_none() => {
                    bpm = Some((60_000_000.0 / us.as_int().max(1) as f64) as f32);
                }
                midly::TrackEventKind::Meta(midly::MetaMessage::TimeSignature(n, d, ..)) if ti == 0 || meter == (4, 4) => {
                    meter = (n, 1u8 << d.min(6));
                }
                midly::TrackEventKind::Meta(midly::MetaMessage::TrackName(b)) => {
                    tname = String::from_utf8_lossy(b).trim().to_string();
                }
                midly::TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            open.insert((ch, key.as_int()), t);
                        }
                        midly::MidiMessage::NoteOn { key, .. } | midly::MidiMessage::NoteOff { key, .. } => {
                            if let Some(s) = open.remove(&(ch, key.as_int())) {
                                notes.push(Note { start: s, end: t.max(s + 1), key: key.as_int(), ch, track: ti });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        for ((ch, key), s) in open {
            notes.push(Note { start: s, end: t.max(s + tpq / 4), key, ch, track: ti });
        }
        names.push(tname);
    }
    let mut info = MidiInfo {
        bpm: bpm.map(|b| (b * 10.0).round() / 10.0),
        meter: format!("{}/{}", meter.0, meter.1),
        notes: notes.len(),
        ..Default::default()
    };
    if notes.is_empty() {
        return info;
    }
    notes.sort_by_key(|n| (n.start, n.key));
    let beats_per_bar = meter.0 as f32 * 4.0 / meter.1 as f32;
    let end = notes.iter().map(|n| n.end).max().unwrap_or(0);
    info.bars = ((end as f32 / tpq as f32 / beats_per_bar) * 4.0).ceil() / 4.0;
    let file_hint = hint(name);

    // parts: one per track (a single-track file: one per channel)
    let tracks: std::collections::BTreeSet<usize> = notes.iter().map(|n| n.track).collect();
    let by_channel = tracks.len() == 1;
    let mut groups: std::collections::BTreeMap<(usize, u8), Vec<Note>> = Default::default();
    for n in &notes {
        groups.entry(if by_channel { (n.track, n.ch) } else { (n.track, 0) }).or_default().push(*n);
    }
    let mut tonal_hist = [0f32; 12];
    for ((ti, ch), ns) in &groups {
        let pname = names.get(*ti).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| name.to_string());
        let drums_ch = ns.iter().filter(|n| n.ch == 9).count() * 2 > ns.len();
        // notes sounding together at each note start (a sweep: notes come sorted by start)
        let poly: f32 = {
            let mut ends = std::collections::BinaryHeap::new();
            let mut total = 0usize;
            for n in ns {
                while ends.peek().is_some_and(|std::cmp::Reverse(e)| *e <= n.start) {
                    ends.pop();
                }
                ends.push(std::cmp::Reverse(n.end));
                total += ends.len();
            }
            total as f32 / ns.len() as f32
        };
        let mut keys: Vec<u8> = ns.iter().map(|n| n.key).collect();
        keys.sort_unstable();
        let median = keys[keys.len() / 2];
        let kind = if drums_ch || (*ch == 9 && by_channel) {
            "drums"
        } else if let Some(h) = hint(&pname).or(if groups.len() == 1 { file_hint } else { None }) {
            h
        } else if poly >= 2.3 {
            "chords"
        } else if median < 52 {
            "bass"
        } else {
            "melody"
        };
        if kind != "drums" {
            for n in ns {
                tonal_hist[(n.key % 12) as usize] += (n.end - n.start) as f32;
            }
        }
        info.parts.push(Part { name: pname, kind: kind.into(), notes: ns.len(), low: keys[0], high: *keys.last().unwrap(), poly: (poly * 10.0).round() / 10.0 });
    }
    let kinds: std::collections::BTreeSet<&str> = info.parts.iter().map(|p| p.kind.as_str()).collect();
    info.kind = if kinds.len() == 1 { kinds.iter().next().unwrap().to_string() } else { "multi".into() };

    // chords, one per bar, from the parts that make harmony (chords and bass; a melody alone
    // gives none), then the key: the notes' profile, and between a key and its relative
    // (A minor / C major share their notes) the chord the music starts or ends on
    // the bass line: at each note start, the lowest note sounding (drums left out)
    let tonal: Vec<&Note> = notes
        .iter()
        .filter(|n| {
            let g = if by_channel { (n.track, n.ch) } else { (n.track, 0) };
            info.parts.iter().zip(groups.keys()).find(|(_, kk)| **kk == g).is_some_and(|(p, _)| p.kind != "drums")
        })
        .collect();
    let mut bass_hist = [0f32; 12];
    let (mut first, mut last) = (None, None);
    {
        let mut starts: Vec<u64> = tonal.iter().map(|n| n.start).collect();
        starts.dedup();
        for (i, &t) in starts.iter().enumerate().take(4000) {
            if let Some(low) = tonal.iter().filter(|n| n.start <= t && n.end > t).map(|n| n.key).min() {
                bass_hist[(low % 12) as usize] += 1.0;
                if i == 0 {
                    first = Some((low % 12) as usize);
                }
            }
        }
        if let Some(&t) = starts.last() {
            last = tonal.iter().filter(|n| n.start == t).map(|n| n.key).min().map(|k| (k % 12) as usize);
        }
    }
    let ranked = rank_keys(&tonal_hist, &bass_hist, first, last);
    let mut found: Vec<(usize, &'static str)> = Vec::new();
    {
        let harmonic: Vec<&Note> = notes
            .iter()
            .filter(|n| {
                let g = if by_channel { (n.track, n.ch) } else { (n.track, 0) };
                let k = info.parts.iter().zip(groups.keys()).find(|(_, kk)| **kk == g).map(|(p, _)| p.kind.as_str());
                matches!(k, Some("chords") | Some("bass"))
            })
            .collect();
        // a bass line alone says no chord: only files with a chords part get a progression
        if harmonic.len() >= 3 && info.parts.iter().any(|p| p.kind == "chords") {
            let bar = (tpq as f32 * beats_per_bar) as u64;
            let nbars = (end / bar.max(1) + 1).min(256);
            for b in 0..nbars {
                let (s, e) = (b * bar, (b + 1) * bar);
                let mut w = [0f32; 12];
                let mut bass: Option<(u8, usize)> = None;
                for n in harmonic.iter().filter(|n| n.start < e && n.end > s) {
                    w[(n.key % 12) as usize] += (n.end.min(e) - n.start.max(s)) as f32;
                    if bass.is_none_or(|x| n.key < x.0) {
                        bass = Some((n.key, (n.key % 12) as usize));
                    }
                }
                if let Some(c) = chord_of(&w, bass.map(|b| b.1))
                    && found.last() != Some(&c)
                {
                    found.push(c);
                }
                if found.len() >= 16 {
                    break;
                }
            }
        }
    }
    // (the bass line and the first / last bass note settle a key against its relative, which
    // the chord at either end used to do: measured, the bass does it better)
    if let Some(&(tonic, minor, _, strength)) = ranked.first().filter(|k| k.3 > 0.3) {
        info.key = Some(key_name(tonic, minor));
        info.key_strength = (strength * 100.0).round() / 100.0;
        let names = spelling(tonic, minor);
        info.chords = found.iter().map(|(r, suf)| format!("{}{}", names[*r], suf)).collect();
        info.degrees = found
            .iter()
            .map(|(r, suf)| roman((r + 12 - tonic) % 12, (suf.starts_with('m') && !suf.starts_with("maj")) || *suf == "dim"))
            .collect();
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny MIDI file: format 0, 96 ticks a beat, the given (key, start beat, beats) notes.
    fn smf(notes: &[(u8, u32, u32)], ch: u8) -> Vec<u8> {
        let mut ev: Vec<(u32, [u8; 3])> = Vec::new();
        for &(k, s, d) in notes {
            ev.push((s * 96, [0x90 | ch, k, 100]));
            ev.push(((s + d) * 96, [0x80 | ch, k, 0]));
        }
        ev.sort_by_key(|e| (e.0, e.1[0] & 0xF0 == 0x90));
        let mut trk = Vec::new();
        let mut t = 0;
        for (at, m) in ev {
            let d = at - t;
            t = at;
            // variable-length delta
            let mut v = vec![(d & 0x7F) as u8];
            let mut x = d >> 7;
            while x > 0 {
                v.insert(0, ((x & 0x7F) as u8) | 0x80);
                x >>= 7;
            }
            trk.extend(v);
            trk.extend(m);
        }
        trk.extend([0x00, 0xFF, 0x2F, 0x00]);
        let mut out = b"MThd".to_vec();
        out.extend([0, 0, 0, 6, 0, 0, 0, 1, 0, 96]);
        out.extend(b"MTrk");
        out.extend((trk.len() as u32).to_be_bytes());
        out.extend(trk);
        out
    }

    #[test]
    fn a_minor_progression() {
        // Am F C G, one bar each (4 beats), triads
        let mut n = Vec::new();
        for (i, ch) in [[57u8, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]].iter().enumerate() {
            for k in ch {
                n.push((*k, i as u32 * 4, 4));
            }
        }
        let info = analyse(&smf(&n, 0), "chords.mid");
        assert_eq!(info.kind, "chords");
        assert_eq!(info.chords, vec!["Am", "F", "C", "G"]);
        assert_eq!(info.key.as_deref(), Some("Am"));
        assert_eq!(info.degrees, vec!["i", "bVI", "bIII", "bVII"]);
        assert_eq!(info.bars, 4.0);
    }

    #[test]
    fn drums_and_bass() {
        let d = analyse(&smf(&[(36, 0, 1), (38, 1, 1), (42, 0, 1), (42, 2, 1)], 9), "loop.mid");
        assert_eq!(d.kind, "drums");
        let b = analyse(&smf(&[(33, 0, 2), (36, 2, 2), (31, 4, 2), (33, 6, 2)], 0), "x.mid");
        assert_eq!(b.kind, "bass");
    }
}

/// A Standard MIDI File (one track, 480 ticks a beat) of notes given in beats: `(pitch,
/// start, length, velocity)`, at `bpm`, 4/4, on channel 10 for drums. `len` (beats) closes
/// the track, so a loop keeps its length.
pub fn write_smf(name: &str, notes: &[(u8, f64, f64, u8)], len: f64, bpm: f64, drums: bool) -> Vec<u8> {
    const TPQ: f64 = 480.0;
    let ch = if drums { 9u8 } else { 0u8 };
    let tick = |b: f64| (b.max(0.0) * TPQ).round() as u64;
    // (tick, order: offs before ons at the same tick, bytes)
    let mut ev: Vec<(u64, u8, [u8; 3])> = Vec::with_capacity(notes.len() * 2);
    for &(p, s, l, v) in notes {
        let p = p.min(127);
        ev.push((tick(s), 1, [0x90 | ch, p, v.clamp(1, 127)]));
        ev.push((tick(s + l.max(0.01)).max(tick(s) + 1), 0, [0x80 | ch, p, 0]));
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let mut t: Vec<u8> = Vec::new();
    let vlq = |out: &mut Vec<u8>, mut v: u64| {
        let mut b = vec![(v & 0x7f) as u8];
        v >>= 7;
        while v > 0 {
            b.push(((v & 0x7f) as u8) | 0x80);
            v >>= 7;
        }
        b.reverse();
        out.extend(b);
    };
    let nm = name.as_bytes();
    let nm = &nm[..nm.len().min(120)];
    t.extend([0x00, 0xff, 0x03]);
    vlq(&mut t, nm.len() as u64);
    t.extend(nm);
    let us = (60_000_000.0 / bpm.clamp(20.0, 999.0)).round() as u32;
    t.extend([0x00, 0xff, 0x51, 0x03, (us >> 16) as u8, (us >> 8) as u8, us as u8]);
    t.extend([0x00, 0xff, 0x58, 0x04, 4, 2, 24, 8]);
    let mut at = 0u64;
    for (tk, _, b) in &ev {
        vlq(&mut t, tk - at);
        at = *tk;
        t.extend(b);
    }
    vlq(&mut t, tick(len).saturating_sub(at));
    t.extend([0xff, 0x2f, 0x00]);
    let mut f = b"MThd\0\0\0\x06\0\0\0\x01".to_vec();
    f.extend((TPQ as u16).to_be_bytes());
    f.extend(b"MTrk");
    f.extend((t.len() as u32).to_be_bytes());
    f.extend(t);
    f
}

#[cfg(test)]
mod write_tests {
    #[test]
    fn a_written_file_reads_back() {
        let notes = [(57u8, 0.0, 1.0, 100u8), (60, 0.0, 1.0, 100), (64, 0.0, 1.0, 100), (53, 4.0, 1.0, 90), (57, 4.0, 1.0, 90), (60, 4.0, 1.0, 90)];
        let b = super::write_smf("Keys / Chords", &notes, 8.0, 174.0, false);
        let i = super::analyse(&b, "Keys/Chords");
        assert!(i.error.is_none());
        assert_eq!(i.notes, 6);
        assert_eq!(i.bpm.map(|b| b.round()), Some(174.0));
        assert!(i.bars > 1.0, "the analysis measures up to the last note");
    }
}

/// A song's harmony, bar by bar: the key, and the chord of each bar where the harmonic parts
/// (not drums) say one.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Harmony {
    /// "Am", None when the notes say no key clearly.
    pub key: Option<String>,
    /// The key's tonic (0 = C) and mode.
    pub tonic: Option<u8>,
    pub minor: bool,
    pub bars: Vec<BarChord>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BarChord {
    /// From 0.
    pub bar: u32,
    /// Where in the bar it starts, and how long it lasts (beats): a bar can hold two chords
    /// (a half each) or more (a beat each).
    pub at: f64,
    pub len: f64,
    /// The bass note when it is not the root ("A" in "F#m/A"), spelled in the key.
    pub bass: Option<String>,
    /// "Am7", spelled in the key.
    pub name: String,
    /// "i", "VI", "bVII"... in the key ("" without one).
    pub degree: String,
    pub root: u8,
    /// "", "m", "7", "maj7", "m7", "dim", "sus4", "sus2"
    pub quality: String,
}

/// The harmony of notes given in beats: (pitch, start, length, played by a bass). `bars` bars
/// of `beats_per_bar` beats.
pub fn harmony_of(notes: &[(u8, f64, f64, bool)], beats_per_bar: f64, bars: usize) -> Harmony {
    harmony_with(notes, beats_per_bar, bars, None)
}

/// A key written as the catalogue keeps it ("Cm", "F#", "Bb minor", "Do mineur", "Sol#m"):
/// (tonic, minor), None when it is not one.
pub fn parse_key(s: &str) -> Option<(usize, bool)> {
    let s = s.trim();
    let low = s.to_lowercase();
    const LATIN: [(&str, usize); 7] = [("do", 0), ("ré", 2), ("re", 2), ("mi", 4), ("fa", 5), ("sol", 7), ("la", 9)];
    let (mut tonic, mut rest) = if let Some((n, t)) = LATIN.iter().find(|(n, _)| low.starts_with(n)) {
        (*t, &low[n.len()..])
    } else if low.starts_with("si") {
        (11, &low[2..])
    } else {
        let c = low.chars().next()?;
        let t = match c {
            'c' => 0, 'd' => 2, 'e' => 4, 'f' => 5, 'g' => 7, 'a' => 9, 'b' => 11,
            _ => return None,
        };
        (t, &low[1..])
    };
    for (acc, d) in [("#", 1i32), ("♯", 1), ("b", -1), ("♭", -1)] {
        if let Some(r) = rest.strip_prefix(acc) {
            tonic = ((tonic as i32 + d).rem_euclid(12)) as usize;
            rest = r;
            break;
        }
    }
    let rest = rest.trim();
    let minor = rest.starts_with('m') && !rest.starts_with("maj") || rest.starts_with("min");
    let major = rest.is_empty() || rest.starts_with("maj") || rest.starts_with("ma") || rest.starts_with("dur");
    (minor || major).then_some((tonic, minor))
}

/// As `harmony_of`, with the key the song is said to be in (its catalogue key: the folder's
/// name, or set by the user). The notes alone often hesitate between keys sharing a scale (F
/// minor and C minor, a minor key and its relative major): the one said wins when the notes
/// fit its scale at least as well as they fit the key found. Notes that do not fit it keep
/// the key found (a key said wrongly is not followed).
pub fn harmony_with(notes: &[(u8, f64, f64, bool)], beats_per_bar: f64, bars: usize, said: Option<(usize, bool)>) -> Harmony {
    let mut h = [0f32; 12];
    for &(p, _, l, _) in notes {
        h[(p % 12) as usize] += l as f32;
    }
    // the bass line as the key finder of `analyse` reads it: the lowest note at each note
    // start, and the first and last of them
    let mut bass_h = [0f32; 12];
    let mut starts: Vec<f64> = notes.iter().map(|n| n.1).collect();
    starts.sort_by(|a, b| a.total_cmp(b));
    starts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let low_at = |t: f64| notes.iter().filter(|n| n.1 <= t + 1e-6 && n.1 + n.2 > t + 1e-6).map(|n| n.0).min();
    for &t in starts.iter().take(4000) {
        if let Some(l) = low_at(t) {
            bass_h[(l % 12) as usize] += 1.0;
        }
    }
    let first = starts.first().and_then(|&t| low_at(t)).map(|l| (l % 12) as usize);
    let last = starts.last().and_then(|&t| low_at(t)).map(|l| (l % 12) as usize);
    let mut out = Harmony::default();
    let mut key = rank_keys(&h, &bass_h, first, last).into_iter().next().filter(|k| k.3 > 0.3);
    if let (Some(k), Some((st, sm))) = (key, said) {
        let total: f32 = h.iter().sum();
        // the share of the notes inside a key's own scale (natural minor)
        let fit = |t: usize, m: bool| -> f32 {
            let iv: &[usize] = if m { &[0, 2, 3, 5, 7, 8, 10] } else { &[0, 2, 4, 5, 7, 9, 11] };
            iv.iter().map(|i| h[(t + i) % 12]).sum::<f32>() / total.max(1e-6)
        };
        if (k.0, k.1) != (st % 12, sm) && fit(st % 12, sm) >= 0.95 && fit(st % 12, sm) + 0.005 >= fit(k.0, k.1) {
            key = Some((st % 12, sm, k.2, k.3));
        }
    }
    if let Some((t, m, _, _)) = key {
        out.key = Some(key_name(t, m));
        out.tonic = Some(t as u8);
        out.minor = m;
    }
    let names = match key {
        Some((t, m, _, _)) => spelling(t, m),
        None => &NAMES,
    };
    let scale = key.map(|(t, m, _, _)| scale_of(t, m));
    // what each note is: struck with others (a chord, comping) or alone and short (a line
    // played over the chords: a melody's passing notes say little of the chord). `group`:
    // how many notes start with it (a block chord is three or more)
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&a, &b| notes[a].1.total_cmp(&notes[b].1));
    let mut group = vec![1usize; notes.len()];
    let mut i = 0;
    while i < order.len() {
        let t0 = notes[order[i]].1;
        let mut j = i;
        while j < order.len() && notes[order[j]].1 - t0 < 0.06 {
            j += 1;
        }
        let n = order[i..j].iter().filter(|&&k| !notes[k].3).count();
        for &k in &order[i..j] {
            group[k] = n;
        }
        i = j;
    }
    let weight = |k: usize| -> f32 {
        let (_, _, l, is_bass) = notes[k];
        if is_bass || group[k] >= 2 || l >= 1.0 { 1.0 } else { 0.4 }
    };
    // a stretch of time: its chord, its bass (the bass part's lowest note heard at its start,
    // else the lowest note), the largest chord struck inside it, and whether the bass part
    // holds that bass (a slash bass is said only then: a walking line passing on the fifth
    // is not an inversion)
    let seg = |s: f64, e: f64| -> Option<(usize, &'static str, usize, usize, bool)> {
        let mut w = [0f32; 12];
        let (mut low, mut low_bass): (Option<u8>, Option<u8>) = (None, None);
        let mut bass_time = [0f64; 12];
        let mut struck = 0usize;
        for k in (0..notes.len()).filter(|&k| notes[k].1 < e && notes[k].1 + notes[k].2 > s) {
            let (p, ns, l, is_bass) = notes[k];
            let d = e.min(ns + l) - s.max(ns);
            // a bass line walking on after the chord's start says less than the note it lands on
            let walk = if is_bass && ns > s + 0.25 { 0.5 } else { 1.0 };
            w[(p % 12) as usize] += d as f32 * weight(k) * walk;
            if low.is_none_or(|x| p < x) {
                low = Some(p);
            }
            if is_bass {
                bass_time[(p % 12) as usize] += d;
                if ns <= s + (e - s) * 0.5 && low_bass.is_none_or(|x| p < x) {
                    low_bass = Some(p);
                }
            }
            if ns >= s - 1e-6 && !is_bass {
                struck = struck.max(group[k]);
            }
        }
        let bass = low_bass.or(low).map(|p| (p % 12) as usize)?;
        let held: f64 = bass_time.iter().sum();
        let holds = low_bass.is_some() && held > 0.0 && bass_time[bass] >= (e - s) * 0.75;
        let (r, suf) = chord_in(&w, Some(bass), scale.as_ref())?;
        Some((r, suf, bass, struck, holds))
    };
    let push = |out: &mut Harmony, b: usize, at: f64, len: f64, c: (usize, &'static str, usize, usize, bool)| {
        let (r, suf, bass, _, holds) = c;
        // a bass under another note than the root: said when the bass part holds a chord tone
        // there (an inversion), not for a line passing by
        let iv = (bass + 12 - r) % 12;
        let tones = CHORDS.iter().find(|c| c.0 == suf).map(|c| c.1).unwrap_or(&[]);
        let slash = bass != r && tones.contains(&iv) && holds;
        let minor_chord = (suf.starts_with('m') && !suf.starts_with("maj")) || suf == "dim";
        out.bars.push(BarChord {
            bar: b as u32,
            at,
            len,
            bass: slash.then(|| names[bass].to_string()),
            name: format!("{}{}", names[r], suf),
            degree: key.map(|(t, _, _, _)| roman((r + 12 - t) % 12, minor_chord)).unwrap_or_default(),
            root: r as u8,
            quality: suf.to_string(),
        });
    };
    // two stretches say another chord when the root moves (Am then Am7, or Am then Am/E, is
    // the same chord going on)
    let same = |a: &(usize, &str, usize, usize, bool), b: &(usize, &str, usize, usize, bool)| a.0 == b.0;
    for b in 0..bars.min(1024) {
        let s = b as f64 * beats_per_bar;
        let half = beats_per_bar / 2.0;
        // a bar splits in two when each half says its own chord; a half splits in beats when
        // each beat strikes a chord of its own (three notes or more): a quick change, not a
        // passing note
        let halves = if beats_per_bar >= 2.0 && beats_per_bar.fract() == 0.0 && (beats_per_bar as i64) % 2 == 0 {
            match (seg(s, s + half), seg(s + half, s + beats_per_bar)) {
                (Some(x), Some(y)) if !same(&x, &y) => Some([x, y]),
                _ => None,
            }
        } else {
            None
        };
        match halves {
            Some(hs) => {
                for (k, h) in hs.into_iter().enumerate() {
                    let hs0 = s + k as f64 * half;
                    let beats = if half == 2.0 { (seg(hs0, hs0 + 1.0), seg(hs0 + 1.0, hs0 + 2.0)) } else { (None, None) };
                    match beats {
                        (Some(x), Some(y)) if !same(&x, &y) && x.3 >= 3 && y.3 >= 3 => {
                            push(&mut out, b, k as f64 * half, 1.0, x);
                            push(&mut out, b, k as f64 * half + 1.0, 1.0, y);
                        }
                        _ => push(&mut out, b, k as f64 * half, half, h),
                    }
                }
            }
            None => {
                if let Some(c) = seg(s, s + beats_per_bar) {
                    push(&mut out, b, 0.0, beats_per_bar, c);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod harmony_tests {
    #[test]
    fn a_progression_bar_by_bar() {
        // Am | F | C | G, one bar each, as `analyse`'s own test plays it
        let mut n = Vec::new();
        for (i, ch) in [[57u8, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]].iter().enumerate() {
            for k in ch {
                n.push((*k, i as f64 * 4.0, 4.0, false));
            }
        }
        let h = super::harmony_of(&n, 4.0, 5);
        assert_eq!(h.key.as_deref(), Some("Am"));
        assert_eq!(h.bars.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["Am", "F", "C", "G"]);
        assert_eq!(h.bars.iter().map(|b| b.degree.as_str()).collect::<Vec<_>>(), ["i", "bVI", "bIII", "bVII"]);
        assert_eq!(h.bars.last().map(|b| b.bar), Some(3), "an empty bar has no chord");
    }

    #[test]
    fn open_fifths_under_a_jazz_line_are_the_simple_chord() {
        // "cafard": the keys play A-E (no third) on the beats, a line wanders E D C over it,
        // the bass plays A and walks to E at the end of the bar; then the same over F
        let mut n = Vec::new();
        for (bar, root) in [(0.0, 45u8), (4.0, 45), (8.0, 41), (12.0, 45)] {
            n.push((root - 12, bar, 0.5, true));
            n.push((root - 12, bar + 2.0, 0.5, true));
            n.push((root - 5, bar + 3.0, 0.5, true));
            for at in [0.5, 1.5, 2.5, 3.5] {
                n.push((root, bar + at, 0.15, false));
                n.push((root + 7, bar + at, 0.15, false));
            }
            for (k, p) in [64u8, 62, 60, 62, 64, 59].iter().enumerate() {
                n.push((*p, bar + 0.25 + k as f64 * 0.6, 0.3, false));
            }
        }
        let h = super::harmony_of(&n, 4.0, 4);
        assert_eq!(h.key.as_deref(), Some("Am"));
        let got: Vec<String> = h.bars.iter().map(|b| format!("{}{}", b.name, b.bass.as_deref().map(|x| format!("/{x}")).unwrap_or_default())).collect();
        assert_eq!(got, ["Am", "Am", "F", "Am"], "one simple chord a bar, no slash for a walking fifth");
    }

    #[test]
    fn two_chords_in_a_bar_and_a_bass_not_on_the_root() {
        // | C  G/B | Am : two halves, then a whole bar; the bass part plays C, B, A
        let mut n = Vec::new();
        for (at, ch, bass) in [(0.0, [60u8, 64, 67], 48u8), (2.0, [62, 67, 71], 47), (4.0, [57, 60, 64], 45)] {
            let len = if at < 4.0 { 2.0 } else { 4.0 };
            for k in ch {
                n.push((k, at, len, false));
            }
            n.push((bass, at, len, true));
        }
        let h = super::harmony_of(&n, 4.0, 2);
        let got: Vec<(u32, f64, String, Option<String>)> = h.bars.iter().map(|b| (b.bar, b.at, b.name.clone(), b.bass.clone())).collect();
        assert_eq!(got, [(0, 0.0, "C".to_string(), None), (0, 2.0, "G".to_string(), Some("B".to_string())), (1, 0.0, "Am".to_string(), None)]);
    }

    /// A real song's notes (exported from the app, kept outside the repository): prints its
    /// chords. `SPOOL_HARMONY_JSON=<file> cargo test -p midi real_song -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_song() {
        let Ok(f) = std::env::var("SPOOL_HARMONY_JSON") else { return };
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        let n: Vec<(u8, f64, f64, bool)> = v["notes"].as_array().unwrap().iter().map(|x| (x[0].as_u64().unwrap() as u8, x[1].as_f64().unwrap(), x[2].as_f64().unwrap(), x[3].as_bool().unwrap())).collect();
        let h = super::harmony_of(&n, 4.0, v["bars"].as_u64().unwrap() as usize);
        let mut line = String::new();
        let mut last = u32::MAX;
        for b in &h.bars {
            if b.bar != last {
                line.push_str(&format!(" | {}:", b.bar));
                last = b.bar;
            }
            line.push_str(&format!(" {}{}", b.name, b.bass.as_deref().map(|x| format!("/{x}")).unwrap_or_default()));
        }
        println!("{:?}{line}", h.key);
    }

    #[test]
    fn a_catalogue_key_is_read_in_any_spelling() {
        use super::parse_key;
        assert_eq!(parse_key("Cm"), Some((0, true)));
        assert_eq!(parse_key("F#"), Some((6, false)));
        assert_eq!(parse_key("Bb minor"), Some((10, true)));
        assert_eq!(parse_key("Do mineur"), Some((0, true)));
        assert_eq!(parse_key("Sol#m"), Some((8, true)));
        assert_eq!(parse_key("Si"), Some((11, false)));
        assert_eq!(parse_key("Ebmaj"), Some((3, false)));
        assert_eq!(parse_key("x"), None);
    }

    #[test]
    fn the_key_said_wins_when_the_notes_fit_its_scale() {
        // "Intro Fm", in C minor by ear: F, C and Eb most played, F lowest, a D, no Db
        let mut n: Vec<(u8, f64, f64, bool)> = Vec::new();
        let mut t = 0.0;
        for (p, len) in [(53u8, 28.5), (60, 23.7), (63, 19.9), (56, 12.0), (55, 10.0), (58, 3.8), (62, 0.7)] {
            n.push((p, t, len, p == 53));
            t += len;
        }
        let found = super::harmony_of(&n, 4.0, 40);
        assert_eq!(found.key.as_deref(), Some("Fm"));
        let said = super::harmony_with(&n, 4.0, 40, Some((0, true)));
        assert_eq!(said.key.as_deref(), Some("Cm"));
        // a key said that the notes do not fit is not followed
        let wrong = super::harmony_with(&n, 4.0, 40, Some((2, false)));
        assert_eq!(wrong.key.as_deref(), Some("Fm"));
    }
}

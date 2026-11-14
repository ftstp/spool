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
fn key_name(tonic: usize, minor: bool) -> String {
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

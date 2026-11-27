//! The MIDI clips of a set: the chord progressions, basslines and melodies written in it,
//! each as its notes in beats over the clip's loop. Read-only, like the rest of the crate.
use crate::read::open_doc;
use crate::role::{Role, clips, track_name, track_role};
use crate::samples::{Env, candidates, is_file, project_root};
use crate::xml::{N, children, child, iter_tag, path, tag, val};
use serde::Serialize;
use std::path::Path;
use ts_rs::TS;

/// One note: MIDI pitch, start and length in beats (from the clip's loop start), velocity.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipNote {
    pub pitch: u8,
    pub start: f64,
    pub len: f64,
    pub vel: u8,
}

#[derive(Debug, Clone)]
pub struct MidiClipInfo {
    /// The track's name, and the clip's own ("" when Live left it unnamed).
    pub track: String,
    pub name: String,
    /// The track holds a drum rack or says it is drums: its notes are drum hits.
    pub drums: bool,
    /// The loop's length, in beats.
    pub len: f64,
    pub notes: Vec<ClipNote>,
}

/// The set's tempo and its MIDI clips with two notes or more. A set that cannot be read
/// gives none.
pub fn midi_clips(p: &Path) -> (Option<f64>, Vec<MidiClipInfo>) {
    let Some(text) = open_doc(p) else { return (None, Vec::new()) };
    let opts = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX };
    let Ok(doc) = roxmltree::Document::parse_with_options(&text, opts) else { return (None, Vec::new()) };
    let Some(ls) = child(doc.root_element(), "LiveSet") else { return (None, Vec::new()) };
    let main = child(ls, "MainTrack").or_else(|| child(ls, "MasterTrack"));
    let tempo = main
        .and_then(|m| iter_tag(m, "Tempo").next())
        .and_then(|t| val(Some(t), "Manual"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|t| *t > 20.0 && *t < 999.0);
    let mut out = Vec::new();
    let Some(tracks) = child(ls, "Tracks") else { return (tempo, out) };
    for t in children(tracks).filter(|t| tag(*t) == "MidiTrack") {
        let tname = track_name(t);
        let drums = matches!(track_role(t), Role::Drums);
        for c in clips(t).into_iter().filter(|c| tag(*c) == "MidiClip") {
            if let Some(clip) = read_clip(c, &tname, drums) {
                out.push(clip);
            }
        }
    }
    (tempo, out)
}

/// A clip's identity by its notes (FNV-1a, 64 bits, stable across Rust versions): the same
/// clip copied into ten versions of a song is one. The MIDI page names a clip "clip:<this>".
pub fn clip_hash(c: &MidiClipInfo) -> String {
    let q = |x: f64| (x * 480.0).round() as i64;
    let bytes = c
        .notes
        .iter()
        .flat_map(|n| [n.pitch as i64, q(n.start), q(n.len), n.vel as i64])
        .chain([q(c.len), c.drums as i64])
        .flat_map(|v| v.to_le_bytes());
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// A set's arrangement as a picture: its tracks, the clips placed on each (in beats), the
/// locators. Session clips (not placed in time) are left out.
#[derive(Debug, Clone, Serialize, TS, Default)]
#[ts(export)]
pub struct Arrangement {
    pub tempo: Option<f64>,
    /// Beats per bar (4 in 4/4).
    pub beats_per_bar: f64,
    /// Where the last clip ends, in beats.
    pub len: f64,
    pub locators: Vec<Locator>,
    pub tracks: Vec<ArrTrack>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Locator {
    pub at: f64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArrTrack {
    pub name: String,
    /// "audio", "midi", "group"
    pub kind: String,
    /// As the catalogue names roles: "drums", "bass", "keys", "pad", "lead", "vox"...
    pub role: String,
    /// Live's colour index.
    #[ts(type = "number")]
    pub color: i64,
    /// Inside a group track.
    pub grouped: bool,
    /// Live's id of the track, and of the group holding it (-1: none): groups nest.
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub group: i64,
    /// Silent in Live: its own activator off, or a group holding it off.
    pub muted: bool,
    /// What it sounds like in Live (MIDI tracks): instrument, preset, effects, glide, samples.
    pub sound: Option<crate::sound::TrackSound>,
    /// Its fader, Utilities, clear cuts and their automation, for playback (mixdown).
    pub mix: crate::mixdown::TrackMix,
    pub clips: Vec<ArrClip>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArrClip {
    pub start: f64,
    pub end: f64,
    pub name: String,
    #[ts(type = "number")]
    pub color: i64,
    /// Turned off in Live.
    pub muted: bool,
    /// An audio clip's own gain, linear (1: 0 dB).
    pub gain: f64,
    /// A MIDI clip: its identity (clip_hash) and its notes, (pitch, start, length) in beats
    /// from the clip's start, at most 400.
    pub hash: Option<String>,
    #[ts(type = "Array<[number, number, number]>")]
    pub notes: Vec<(u8, f64, f64)>,
    /// Each note's velocity (1 to 127), in the order of `notes`.
    pub vels: Vec<u8>,
    /// The clip's pitch bend: (beats from the clip's start, semitones), as it sounds in the
    /// arrangement (the loop repeated); empty when the clip has none.
    #[ts(type = "Array<[number, number]>")]
    pub bend: Vec<(f64, f64)>,
    /// An audio clip: its sample, where it is on this machine (None: missing).
    pub sample: Option<String>,
    /// An audio clip as it sounds: pieces (start, length) in beats from the clip's start,
    /// each read from `offset` seconds into the sample at `secs` seconds of sample per beat
    /// (warp markers and the clip's loop applied), at most 128.
    #[ts(type = "Array<[number, number, number, number]>")]
    pub segs: Vec<[f64; 4]>,
}

/// The arrangement of the set at `p` (None: not readable).
pub fn arrangement(p: &Path, env: &Env) -> Option<Arrangement> {
    let text = open_doc(p)?;
    let opts = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX };
    let doc = roxmltree::Document::parse_with_options(&text, opts).ok()?;
    let ls = child(doc.root_element(), "LiveSet")?;
    let main = child(ls, "MainTrack").or_else(|| child(ls, "MasterTrack"));
    let tempo = main
        .and_then(|m| iter_tag(m, "Tempo").next())
        .and_then(|t| val(Some(t), "Manual"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|t| *t > 20.0 && *t < 999.0);
    let num_den = main.and_then(|m| iter_tag(m, "TimeSignature").next()).and_then(|t| val(Some(t), "Manual")).and_then(|v| v.trim().parse::<i64>().ok());
    // Live encodes it as 99 * log2(denominator) + numerator - 1 (4/4 -> 201)
    let beats_per_bar = num_den
        .map(|v| {
            let (n, d) = ((v.rem_euclid(99) + 1) as f64, 2f64.powi(v.div_euclid(99) as i32));
            n * 4.0 / d
        })
        .filter(|b| *b > 0.0 && *b <= 16.0)
        .unwrap_or(4.0);
    let mut out = Arrangement { tempo, beats_per_bar, ..Default::default() };
    if let Some(locs) = path(ls, "Locators/Locators") {
        out.locators = children(locs)
            .filter(|l| tag(*l) == "Locator")
            .filter_map(|l| Some(Locator { at: num(Some(l), "Time")?, name: val(Some(l), "Name").unwrap_or("").trim().to_string() }))
            .collect();
        out.locators.sort_by(|a, b| a.at.total_cmp(&b.at));
    }
    let proj = project_root(p);
    let tracks = child(ls, "Tracks")?;
    // each track's own activator, and the group it sits in (by Live's track id): a track is
    // silent when it, or any group above it, is off
    let mut own: std::collections::HashMap<String, (bool, Option<String>)> = std::collections::HashMap::new();
    for t in children(tracks) {
        if let Some(id) = t.attribute("Id") {
            let off = val(path(t, "DeviceChain/Mixer/Speaker"), "Manual") == Some("false");
            let group = val(Some(t), "TrackGroupId").filter(|g| *g != "-1").map(String::from);
            own.insert(id.to_string(), (off, group));
        }
    }
    let silent = |id: Option<&str>| -> bool {
        let mut cur = id.map(String::from);
        for _ in 0..32 {
            let Some((off, up)) = cur.as_ref().and_then(|c| own.get(c)) else { return false };
            if *off {
                return true;
            }
            cur = up.clone();
        }
        false
    };
    for t in children(tracks) {
        let kind = match tag(t) {
            "AudioTrack" => "audio",
            "MidiTrack" => "midi",
            "GroupTrack" => "group",
            _ => continue,
        };
        let role = serde_json::to_value(track_role(t)).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        let drums = matches!(track_role(t), Role::Drums);
        let tname = track_name(t);
        // a Pitch MIDI effect in front of the instrument: the notes as Live plays them
        let shift = if kind == "midi" { crate::sound::midi_shift(t) } else { 0 };
        // the track's pitch bend target (for its clips' bend envelopes)
        let pb: Option<String> = t.descendants().find(|c| c.is_element() && tag(*c) == "ControllerTargets.0").and_then(|c| c.attribute("Id")).map(String::from);
        let mut clips_out = Vec::new();
        for aa in iter_tag(t, "ArrangerAutomation") {
            if aa.ancestors().any(|a| tag(a) == "FreezeSequencer") {
                continue;
            }
            for c in path(aa, "Events").into_iter().flat_map(children) {
                let (start, end) = (num(Some(c), "CurrentStart"), num(Some(c), "CurrentEnd"));
                let (Some(start), Some(end)) = (start, end) else { continue };
                if end <= start {
                    continue;
                }
                let muted = val(Some(c), "Disabled") == Some("true");
                let name = val(Some(c), "Name").unwrap_or("").trim().to_string();
                let color = val(Some(c), "Color").and_then(|v| v.trim().parse().ok()).unwrap_or(-1);
                let mut clip = ArrClip { start, end, name, color, muted, gain: 1.0, hash: None, notes: Vec::new(), vels: Vec::new(), bend: Vec::new(), sample: None, segs: Vec::new() };
                match tag(c) {
                    "MidiClip" => {
                        if let Some(m) = read_clip(c, &tname, drums) {
                            clip.hash = Some(clip_hash(&m));
                            // the notes as they sound in the arrangement: the loop repeated
                            let mut at = 0.0;
                            while at < end - start && clip.notes.len() < 400 && m.len > 0.0 {
                                for n in &m.notes {
                                    if at + n.start >= end - start || clip.notes.len() >= 400 {
                                        break;
                                    }
                                    clip.notes.push(((n.pitch as i32 + shift).clamp(0, 127) as u8, at + n.start, n.len));
                                    clip.vels.push(n.vel);
                                }
                                at += m.len;
                            }
                            // its pitch bend, read where its notes are (from the loop's start)
                            if let Some(pb) = &pb {
                                let ls = num(child(c, "Loop"), "LoopStart").unwrap_or(0.0);
                                let pts: Vec<(f64, f64)> = crate::sound::clip_bend(c, pb).into_iter().map(|(tm, v)| (tm - ls, v)).filter(|p| p.0 >= 0.0).collect();
                                if pts.iter().any(|p| p.1.abs() > 0.01) && m.len > 0.0 {
                                    let mut at = 0.0;
                                    while at < end - start && clip.bend.len() < 512 {
                                        for &(tm, v) in &pts {
                                            if at + tm < end - start && tm < m.len {
                                                clip.bend.push((at + tm, v));
                                            }
                                        }
                                        at += m.len;
                                    }
                                }
                            }
                        }
                    }
                    "AudioClip" => {
                        if let Some(fr) = path(c, "SampleRef/FileRef") {
                            let (shown, cands) = candidates(fr, &proj, env);
                            clip.sample = cands.into_iter().find(|x| is_file(x)).map(|x| x.to_string_lossy().replace('\\', "/"));
                            let _ = shown;
                        }
                        clip.gain = num(Some(c), "SampleVolume").filter(|g| g.is_finite() && *g >= 0.0).unwrap_or(1.0);
                        if clip.sample.is_some() {
                            clip.segs = audio_segs(c, end - start, 60.0 / tempo.unwrap_or(120.0));
                        }
                    }
                    _ => continue,
                }
                out.len = out.len.max(end);
                clips_out.push(clip);
            }
        }
        clips_out.sort_by(|a, b| a.start.total_cmp(&b.start));
        out.tracks.push(ArrTrack {
            name: tname,
            kind: kind.into(),
            role,
            color: val(Some(t), "Color").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            grouped: val(Some(t), "TrackGroupId").is_some_and(|g| g != "-1"),
            id: t.attribute("Id").and_then(|v| v.parse().ok()).unwrap_or(-1),
            group: val(Some(t), "TrackGroupId").and_then(|v| v.trim().parse().ok()).unwrap_or(-1),
            muted: silent(t.attribute("Id")),
            sound: (kind == "midi").then(|| crate::sound::track_sound(t, &proj, env)),
            mix: crate::mixdown::track_mix(t),
            clips: clips_out,
        });
    }
    Some(out)
}

/// Where an audio clip `dur` beats long reads its sample: a warped clip counts in beats and
/// its warp markers turn a beat into a second of the sample; an unwarped one counts in
/// seconds, at the set's tempo (`spb` seconds per beat). A looped clip starts at its start
/// marker and goes round its loop.
fn audio_segs(c: N, dur: f64, spb: f64) -> Vec<[f64; 4]> {
    let warped = val(Some(c), "IsWarped") == Some("true");
    let lp = child(c, "Loop");
    let (ls, le) = (num(lp, "LoopStart").unwrap_or(0.0), num(lp, "LoopEnd").unwrap_or(0.0));
    let looped = val(lp, "LoopOn") == Some("true") && le > ls;
    let mut marks: Vec<(f64, f64)> = iter_tag(c, "WarpMarker")
        .filter_map(|m| {
            let f = |a: &str| m.attribute(a).and_then(|v| v.trim().parse::<f64>().ok());
            Some((f("BeatTime")?, f("SecTime")?))
        })
        .collect();
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    marks.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    // a beat of the clip -> a second of the sample (straight lines between markers, the first
    // and last carried on)
    let to_sec = |u: f64| -> f64 {
        if !warped {
            return u;
        }
        match marks.len() {
            0 => u * spb,
            1 => marks[0].1 + (u - marks[0].0) * spb,
            n => {
                let i = marks.iter().rposition(|m| m.0 <= u).unwrap_or(0).min(n - 2);
                let (a, b) = (marks[i], marks[i + 1]);
                a.1 + (u - a.0) * (b.1 - a.1) / (b.0 - a.0)
            }
        }
    };
    let per_beat = if warped { 1.0 } else { spb };
    let mut u = ls + if looped { num(lp, "StartRelative").unwrap_or(0.0) } else { 0.0 };
    if looped && u >= le {
        u = ls;
    }
    let (mut at, mut out) = (0.0, Vec::new());
    while at < dur - 1e-6 && out.len() < 128 {
        let mut to = u + (dur - at) * per_beat;
        if looped {
            to = to.min(le);
        }
        if warped {
            if let Some(m) = marks.iter().map(|m| m.0).find(|b| *b > u + 1e-6 && *b < to) {
                to = m;
            }
        }
        if to - u < 1e-6 {
            break;
        }
        let len = (to - u) / per_beat;
        let (s0, s1) = (to_sec(u), to_sec(to));
        out.push([at, len, s0, (s1 - s0) / len]);
        at += len;
        u = to;
        if looped && u >= le - 1e-6 {
            u = ls;
        }
    }
    out
}

fn num(n: Option<N>, p: &str) -> Option<f64> {
    val(n, p).and_then(|v| v.trim().parse().ok())
}

fn read_clip(c: N, track: &str, drums: bool) -> Option<MidiClipInfo> {
    let lp = child(c, "Loop");
    let (ls, le) = (num(lp, "LoopStart").unwrap_or(0.0), num(lp, "LoopEnd").unwrap_or(0.0));
    let mut all: Vec<ClipNote> = Vec::new();
    for kt in path(c, "Notes/KeyTracks").into_iter().flat_map(children) {
        let Some(pitch) = val(Some(kt), "MidiKey").and_then(|v| v.trim().parse::<i64>().ok()).filter(|p| (0..128).contains(p)) else { continue };
        for ev in iter_tag(kt, "MidiNoteEvent") {
            if ev.attribute("IsEnabled") == Some("false") {
                continue;
            }
            let f = |a: &str| ev.attribute(a).and_then(|v| v.trim().parse::<f64>().ok());
            let (Some(start), Some(len)) = (f("Time"), f("Duration")) else { continue };
            let vel = f("Velocity").unwrap_or(100.0).round().clamp(1.0, 127.0) as u8;
            all.push(ClipNote { pitch: pitch as u8, start, len: len.max(0.01), vel });
        }
    }
    // the loop's notes, from its start; a clip whose notes lie outside it keeps them all
    let (mut notes, mut len): (Vec<ClipNote>, f64) = if le > ls {
        let inside: Vec<ClipNote> = all
            .iter()
            .filter(|n| n.start >= ls - 1e-6 && n.start < le - 1e-6)
            .map(|n| ClipNote { start: n.start - ls, len: n.len.min(le - n.start), ..n.clone() })
            .collect();
        if inside.len() >= 2 { (inside, le - ls) } else { (Vec::new(), 0.0) }
    } else {
        (Vec::new(), 0.0)
    };
    if notes.is_empty() {
        let first = all.iter().map(|n| n.start).fold(f64::INFINITY, f64::min);
        if !first.is_finite() {
            return None;
        }
        let from = (first / 4.0).floor() * 4.0;
        notes = all.iter().map(|n| ClipNote { start: n.start - from, ..n.clone() }).collect();
        len = notes.iter().map(|n| n.start + n.len).fold(0.0, f64::max);
        len = (len / 4.0).ceil().max(1.0) * 4.0;
    }
    if notes.len() < 2 {
        return None;
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    Some(MidiClipInfo {
        track: track.to_string(),
        name: val(Some(c), "Name").unwrap_or("").trim().to_string(),
        drums,
        len: len.min(4.0 * 128.0),
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn segs_of(xml: &str, dur: f64) -> Vec<[f64; 4]> {
        let doc = roxmltree::Document::parse(xml).unwrap();
        audio_segs(doc.root_element(), dur, 0.5)
    }
    #[test]
    fn an_audio_clip_reads_its_sample_where_live_does() {
        // a 2-beat loop from beat 4, started 1 beat in, sample warped 1 s for 2 beats
        let xml = r#"<AudioClip><IsWarped Value="true"/><Loop><LoopStart Value="4"/><LoopEnd Value="6"/>
            <StartRelative Value="1"/><LoopOn Value="true"/></Loop>
            <WarpMarkers><WarpMarker SecTime="0" BeatTime="0"/><WarpMarker SecTime="4" BeatTime="8"/></WarpMarkers></AudioClip>"#;
        let s = segs_of(xml, 4.0);
        // beat 5 -> 6, then round the loop 4 -> 6, then 4 -> 5
        assert_eq!(s.len(), 3);
        assert_eq!(s[0], [0.0, 1.0, 2.5, 0.5]);
        assert_eq!(s[1], [1.0, 2.0, 2.0, 0.5]);
        assert_eq!(s[2], [3.0, 1.0, 2.0, 0.5]);
        // unwarped: seconds at the set's tempo, read straight
        let xml = r#"<AudioClip><IsWarped Value="false"/><Loop><LoopStart Value="3"/><LoopEnd Value="10"/><LoopOn Value="false"/></Loop></AudioClip>"#;
        assert_eq!(segs_of(xml, 4.0), vec![[0.0, 4.0, 3.0, 0.5]]);
    }
    #[test]
    fn a_clip_hash_never_changes() {
        // the MIDI page names clips by it: a change would list every clip twice
        let c = MidiClipInfo {
            track: "Keys".into(),
            name: "".into(),
            drums: false,
            len: 4.0,
            notes: vec![ClipNote { pitch: 60, start: 0.0, len: 1.0, vel: 100 }, ClipNote { pitch: 64, start: 1.0, len: 0.5, vel: 90 }],
        };
        assert_eq!(clip_hash(&c), clip_hash(&c.clone()));
        assert_eq!(clip_hash(&c), "4e1ce026b6c9de8e");
    }
}

//! What a MIDI track sounds like in Live, as far as the set says: its instrument (a Live
//! device or a plugin's name), its preset, the effects after it, whether it glides, and the
//! samples it plays when it is a Simpler, a Sampler or a Drum Rack. The interface's small
//! synth plays closer to the real thing with it: the real samples when it has them, else a
//! sound of the same family.
use crate::samples::{Env, candidates, is_file};
use crate::xml::{N, child, children, path, tag, val};
use serde::Serialize;
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct TrackSound {
    /// "Operator", "Wavetable", "Simpler", "Drum Rack"... or a plugin's name ("Serum").
    pub instrument: String,
    /// The preset loaded in it, or the name given to the device ("" when none says).
    pub preset: String,
    /// The devices after it ("Saturator", "Auto Filter", "OTT"...).
    pub fx: Vec<String>,
    /// Glide (portamento) on in the instrument: notes slide into each other.
    pub glide: bool,
    /// The samples it plays: a Drum Rack's pads (each on its note), a Simpler's or a
    /// Sampler's zones (played across the keyboard from their root). At most 128.
    pub zones: Vec<Zone>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Zone {
    /// The note a pad answers to; -1: any note (a sampler's zone, between `lo` and `hi`).
    pub note: i16,
    pub lo: u8,
    pub hi: u8,
    /// The note at which the sample plays as recorded.
    pub root: u8,
    /// The file, where it is on this computer.
    pub path: String,
    /// The part of the file played, in seconds: a sliced loop's pads ("Slice to new Drum
    /// Rack") all hold the whole loop, each from its own start. `end` 0: to the file's end.
    pub start: f64,
    pub end: f64,
}

/// Live's devices by their tag, as Live names them.
fn device_name(n: N) -> Option<String> {
    let t = tag(n);
    let named = match t {
        "PluginDevice" | "AuPluginDevice" => {
            // its plugin's name, as its info says (a VST2's PlugName, a VST3's or an AU's
            // Name): the first `Name` anywhere inside was often a parameter's, empty
            if let Some((info, _)) = plugin_info(n) {
                let v = val(Some(info), "PlugName").or_else(|| val(Some(info), "Name")).unwrap_or("").trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
            return n.descendants().find(|c| c.is_element() && tag(*c) == "PlugName").and_then(|c| c.attribute("Value")).map(|s| s.trim().to_string())
                .or_else(|| n.descendants().filter(|c| c.is_element() && tag(*c) == "Name").filter_map(|c| c.attribute("Value")).find(|v| !v.trim().is_empty()).map(String::from));
        }
        "InstrumentVector" => "Wavetable",
        "UltraAnalog" => "Analog",
        "OriginalSimpler" => "Simpler",
        "MultiSampler" => "Sampler",
        "DrumGroupDevice" => "Drum Rack",
        "InstrumentGroupDevice" => "Instrument Rack",
        "AudioEffectGroupDevice" => "Audio Effect Rack",
        "LoungeLizard" => "Electric",
        "StringStudio" => "Tension",
        "InstrumentMeld" => "Meld",
        "AutoFilter" | "AutoFilter2" => "Auto Filter",
        "Eq8" => "EQ Eight",
        "Compressor2" => "Compressor",
        "Reverb" | "Reverb2" => "Reverb",
        "Delay" | "PingPongDelay" | "FilterDelay" | "Echo" => "Delay",
        "Chorus" | "Chorus2" => "Chorus",
        other => other,
    };
    Some(named.to_string())
}

const INSTRUMENTS: &[&str] = &[
    "Operator", "InstrumentVector", "UltraAnalog", "Drift", "OriginalSimpler", "MultiSampler", "DrumGroupDevice", "InstrumentGroupDevice",
    "LoungeLizard", "StringStudio", "Collision", "InstrumentMeld", "InstrumentImpulse", "Tension",
];

/// A plugin device's info element and its format ("vst2", "vst3", "au").
fn plugin_info<'a, 'i>(dev: N<'a, 'i>) -> Option<(N<'a, 'i>, &'static str)> {
    dev.descendants().find_map(|c| match c.is_element().then(|| tag(c)) {
        Some("VstPluginInfo") => Some((c, "vst2")),
        Some("Vst3PluginInfo") => Some((c, "vst3")),
        Some("AuPluginInfo") => Some((c, "au")),
        _ => None,
    })
}

fn preset_of(dev: N) -> String {
    if let Some(u) = val(Some(dev), "UserName").filter(|s| !s.trim().is_empty()) {
        return u.trim().to_string();
    }
    let p = child(dev, "LastPresetRef")
        .and_then(|r| r.descendants().find(|c| c.is_element() && (tag(*c) == "Path" || tag(*c) == "RelativePath") && c.attribute("Value").is_some_and(|v| !v.is_empty())))
        .and_then(|c| c.attribute("Value"))
        .unwrap_or("");
    let stem = p.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = stem.rsplit_once('.').map(|s| s.0).unwrap_or(stem);
    // a plugin's own file is not a preset
    if !p.is_empty() && !(p.to_lowercase().ends_with(".dll") || p.to_lowercase().ends_with(".vst3")) {
        return stem.to_string();
    }
    // a plugin: the preset its own state names (a Kontakt's instrument, a Serum's patch)
    if let Some((info, fmt)) = plugin_info(dev) {
        let name = val(Some(info), "PlugName").or_else(|| val(Some(info), "Name")).unwrap_or("");
        if let Some(pr) = crate::presets::preset_of(info, fmt, name).filter(|p| !p.default) {
            return if pr.bank.is_empty() { pr.name } else { format!("{} {}", pr.bank, pr.name) };
        }
    }
    String::new()
}

/// A switch or time that says the notes glide: Operator's and Analog's portamento, Simpler's
/// glide mode, Drift's and Wavetable's glide.
fn glides(dev: N) -> bool {
    dev.descendants().filter(|c| c.is_element()).any(|c| {
        let t = tag(c);
        if !matches!(t, "PortamentoOn" | "GlideOn" | "GlideMode" | "Glide" | "Legato" | "GlideToggle") {
            return false;
        }
        match val(Some(c), "Manual") {
            Some("true") => true,
            Some(v) => v.parse::<f64>().is_ok_and(|x| x > 0.0) && t == "GlideMode",
            None => false,
        }
    })
}

fn zones_of(dev: N, proj: &Path, env: &Env, note: i16, out: &mut Vec<Zone>) {
    for part in dev.descendants().filter(|c| c.is_element() && tag(*c) == "MultiSamplePart") {
        if out.len() >= 128 {
            return;
        }
        let Some(fr) = path(part, "SampleRef/FileRef") else { continue };
        let (_, cands) = candidates(fr, proj, env);
        let Some(p) = cands.into_iter().find(|x| is_file(x)) else { continue };
        let num = |k: &str| val(Some(part), k).and_then(|v| v.trim().parse::<i64>().ok());
        let root = num("RootKey").unwrap_or(60).clamp(0, 127) as u8;
        let lo = path(part, "KeyRange").and_then(|k| val(Some(k), "Min")).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0).clamp(0, 127) as u8;
        let hi = path(part, "KeyRange").and_then(|k| val(Some(k), "Max")).and_then(|v| v.parse::<i64>().ok()).unwrap_or(127).clamp(0, 127) as u8;
        // Live counts the part in frames of the file
        let rate = path(part, "SampleRef/DefaultSampleRate").and_then(|r| r.attribute("Value")).and_then(|v| v.parse::<f64>().ok()).filter(|r| *r > 0.0).unwrap_or(44100.0);
        let (s0, s1) = (num("SampleStart").unwrap_or(0).max(0) as f64, num("SampleEnd").unwrap_or(0).max(0) as f64);
        let start = s0 / rate;
        let end = if s1 > s0 { s1 / rate } else { 0.0 };
        out.push(Zone { note, lo, hi, root, path: p.to_string_lossy().replace('\\', "/"), start, end });
        if note >= 0 {
            // a pad: its first sample
            return;
        }
    }
}

/// The sound of track `t` (a MidiTrack element).
/// Semitones a MIDI track's notes are moved by before its instrument: the Pitch MIDI effects
/// switched on in front of it (Live plays a note written C as the Pitch says). 0 when none.
pub fn midi_shift(t: N) -> i32 {
    let Some(devs) = path(t, "DeviceChain/DeviceChain/Devices") else { return 0 };
    children(devs)
        .take_while(|d| tag(*d).starts_with("Midi"))
        .filter(|d| tag(*d) == "MidiPitcher" && val(child(*d, "On"), "Manual") != Some("false"))
        .filter_map(|d| val(child(d, "Pitch"), "Manual").and_then(|v| v.trim().parse::<f64>().ok()))
        .map(|v| v.round() as i32)
        .sum::<i32>()
        .clamp(-48, 48)
}

pub fn track_sound(t: N, proj: &Path, env: &Env) -> TrackSound {
    let mut s = TrackSound::default();
    let Some(devs) = path(t, "DeviceChain/DeviceChain/Devices") else { return s };
    let list: Vec<N> = children(devs).collect();
    // the MIDI effects in front (Pitch, Arpeggiator...) are not the sound
    let at = list.iter().position(|d| !tag(*d).starts_with("Midi"));
    let Some(at) = at else { return s };
    let inst = list[at];
    // no instrument (frozen, or only effects left): the effects are not the sound
    if !(INSTRUMENTS.contains(&tag(inst)) || matches!(tag(inst), "PluginDevice" | "AuPluginDevice")) {
        s.fx = list[at..].iter().filter_map(|d| device_name(*d)).take(12).collect();
        return s;
    }
    // a rack: the instrument inside it (its first chain's), the rack's own name as preset
    let inner = if tag(inst) == "InstrumentGroupDevice" {
        inst.descendants().skip(1).find(|c| c.is_element() && (INSTRUMENTS.contains(&tag(*c)) || tag(*c) == "PluginDevice") && tag(*c) != "InstrumentGroupDevice")
    } else {
        None
    };
    let core = inner.unwrap_or(inst);
    s.instrument = device_name(core).unwrap_or_default();
    s.preset = preset_of(inst);
    if s.preset.is_empty() && inner.is_some() {
        s.preset = preset_of(core);
    }
    s.glide = glides(core);
    s.fx = list[at + 1..].iter().filter_map(|d| device_name(*d)).take(12).collect();
    match tag(core) {
        "DrumGroupDevice" => {
            for b in core.descendants().filter(|c| c.is_element() && tag(*c) == "DrumBranch") {
                // Live writes a pad's note as 128 minus it (C1, 36, is 92)
                let rn = b.descendants().find(|c| c.is_element() && tag(*c) == "ReceivingNote").and_then(|c| c.attribute("Value")).and_then(|v| v.parse::<i16>().ok());
                let Some(rn) = rn else { continue };
                zones_of(b, proj, env, (128 - rn).clamp(0, 127), &mut s.zones);
            }
        }
        "OriginalSimpler" | "MultiSampler" => zones_of(core, proj, env, -1, &mut s.zones),
        _ => {}
    }
    s
}

/// The pitch bend a MIDI clip writes in its own envelope: (beats into the clip's content,
/// semitones at a bend range of 2), for the track's pitch bend target `pb` (its
/// ControllerTargets.0 id).
pub fn clip_bend(c: N, pb: &str) -> Vec<(f64, f64)> {
    let Some(envs) = path(c, "Envelopes/Envelopes") else { return Vec::new() };
    for e in children(envs) {
        if val(path(e, "EnvelopeTarget"), "PointeeId") != Some(pb) {
            continue;
        }
        let Some(evs) = path(e, "Automation/Events") else { continue };
        return children(evs)
            .filter_map(|ev| {
                let tm = ev.attribute("Time")?.parse::<f64>().ok()?;
                let v = ev.attribute("Value")?.parse::<f64>().ok()?;
                Some((tm.max(0.0), (v / 8192.0 * 2.0).clamp(-2.0, 2.0)))
            })
            .collect();
    }
    Vec::new()
}

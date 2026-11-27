//! What a track holds, as Live's device view shows it: its devices in order, each rack opened
//! on its chains (a Drum Rack on its pads), a sampler with its sample's name. Read only.
use crate::assemble::device_name;
use crate::xml::{Element, read_set};
use serde::Serialize;
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DevView {
    pub name: String,
    /// instrument / effect / midi / rack / drums
    pub kind: String,
    /// "VST2", "VST3", "AU" for a plugin, else None (Live's own device).
    pub plugin: Option<String>,
    pub on: bool,
    /// A Simpler's or Sampler's sample, by its file name.
    pub sample: Option<String>,
    /// A rack's chains (a Drum Rack's pads).
    pub chains: Vec<ChainView>,
    /// Left out of a new song (a mix tool or a device switched off, on the track itself).
    pub dropped: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChainView {
    pub name: String,
    /// A pad's note ("C1"), for a Drum Rack.
    pub note: Option<String>,
    pub devices: Vec<DevView>,
}

const INSTRUMENTS: [&str; 13] = [
    "OriginalSimpler", "MultiSampler", "InstrumentVector", "Operator", "UltraAnalog", "LoungeLizard", "StringStudio", "Collision",
    "InstrumentMeld", "Drift", "InstrumentImpulse", "InstrumentGroupDevice", "DrumGroupDevice",
];

fn note_name(n: i64) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[(n.rem_euclid(12)) as usize], n.div_euclid(12) - 2)
}

/// The devices of a chain element (a track's or a branch's DeviceChain).
fn devices_of(chain: &Element, audio: bool) -> Vec<DevView> {
    // a track: DeviceChain/DeviceChain/Devices; a branch: DeviceChain/<Midi|Audio>To...DeviceChain/Devices
    let list = chain
        .children
        .iter()
        .find(|c| c.tag.ends_with("DeviceChain") && c.child("Devices").is_some())
        .and_then(|c| c.child("Devices"));
    let mut seen_instrument = audio;
    list.map(|l| l.children.iter().map(|d| one(d, &mut seen_instrument)).collect()).unwrap_or_default()
}

fn one(d: &Element, seen_instrument: &mut bool) -> DevView {
    let plugin = match d.tag.as_str() {
        "PluginDevice" if d.find("PluginDesc/Vst3PluginInfo").is_some() => Some("VST3"),
        "PluginDevice" => Some("VST2"),
        "AuPluginDevice" => Some("AU"),
        _ => None,
    };
    let is_midi = d.tag.starts_with("Midi") || d.tag == "MxDeviceMidiEffect";
    let kind = if d.tag == "DrumGroupDevice" {
        *seen_instrument = true;
        "drums"
    } else if d.tag.ends_with("GroupDevice") {
        if d.tag == "InstrumentGroupDevice" {
            *seen_instrument = true;
        }
        "rack"
    } else if is_midi && !*seen_instrument {
        "midi"
    } else if !*seen_instrument && (INSTRUMENTS.contains(&d.tag.as_str()) || plugin.is_some() || d.tag == "MxDeviceInstrument") {
        *seen_instrument = true;
        "instrument"
    } else {
        "effect"
    };
    let sample = if matches!(d.tag.as_str(), "OriginalSimpler" | "MultiSampler") {
        // the file a SampleRef points to (other FileRefs, a preset's, are often empty)
        d.descendants()
            .filter(|e| e.tag == "SampleRef")
            .filter_map(|e| e.child("FileRef"))
            .find_map(|f| ["Path", "RelativePath", "Name"].iter().find_map(|k| f.value(k).filter(|p| !p.is_empty())))
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string())
    } else {
        None
    };
    let mut chains = Vec::new();
    if let Some(b) = d.child("Branches") {
        for br in &b.children {
            let audio = br.tag == "AudioEffectBranch";
            let name = br.value("Name/EffectiveName").or_else(|| br.value("Name/UserName")).unwrap_or("").replace('\t', " ").trim().to_string();
            let note = br
                .value("BranchInfo/ReceivingNote")
                .or_else(|| br.value("ZoneSettings/ReceivingNote"))
                .and_then(|n| n.parse::<i64>().ok())
                .filter(|_| br.tag == "DrumBranch")
                // Live counts pads from the top: 92 is C1 (MIDI 36)
                .map(|n| note_name(128 - n));
            let devices = br.child("DeviceChain").map(|c| devices_of(c, audio)).unwrap_or_default();
            chains.push(ChainView { name, note, devices });
        }
    }
    let name = match d.tag.as_str() {
        "InstrumentGroupDevice" | "AudioEffectGroupDevice" | "MidiEffectGroupDevice" | "DrumGroupDevice"
            if d.value("UserName").is_none_or(|u| u.trim().is_empty()) =>
        {
            match d.tag.as_str() {
                "InstrumentGroupDevice" => "Instrument Rack",
                "AudioEffectGroupDevice" => "Audio Effect Rack",
                "MidiEffectGroupDevice" => "MIDI Effect Rack",
                _ => "Drum Rack",
            }
            .to_string()
        }
        _ => device_name(d),
    };
    DevView {
        dropped: false,
        name,
        kind: kind.into(),
        plugin: plugin.map(String::from),
        on: d.find("On/Manual").and_then(|m| m.get("Value")) != Some("false"),
        sample,
        chains,
    }
}

/// The devices of the track `id` of a set.
pub fn track_view(set: &Path, id: &str) -> Result<Vec<DevView>, String> {
    let s = read_set(set)?;
    let ls = s.root.child("LiveSet").ok_or_else(|| i18n::tr!("repair.unreadableProject"))?;
    let t = ls
        .child("Tracks")
        .and_then(|t| t.children.iter().find(|t| t.get("Id") == Some(id)))
        .ok_or_else(|| i18n::tr!("repair.trackNotFound"))?;
    let mut v = t.child("DeviceChain").map(|c| devices_of(c, t.tag == "AudioTrack")).unwrap_or_default();
    // what a new song leaves out (crate::assemble): the track's own mix tools and devices off
    for d in v.iter_mut() {
        d.dropped = !d.on || crate::assemble::is_mix_tool(&d.name);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    #[test]
    fn notes() {
        assert_eq!(super::note_name(36), "C1");
        assert_eq!(super::note_name(42), "F#1");
    }
}

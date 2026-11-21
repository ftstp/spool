//! A VST2 plugin replaced by its VST3 in a repaired copy, its settings carried over.
//!
//! How Live keeps the two (read from real projects):
//! - VST2: `PluginDesc/VstPluginInfo` (PlugName, UniqueId...) with `Preset/VstPreset`: the
//!   plugin's opaque state in `Buffer` (hex) and its `Type`: "FBCh" a bank chunk, "FPCh" a
//!   program chunk ("FxBk" / "FxCk" are parameter lists, not handled here);
//! - VST3: `PluginDesc/Vst3PluginInfo` (Name, Uid as four signed 32-bit fields, DeviceType)
//!   with `Preset/Vst3Preset`: the processor's state in `ProcessorState` (hex).
//!
//! The VST3 of a plugin made to REPLACE its VST2 (Steinberg's convention, followed by most
//! vendors) has a class id made of "VST" + the VST2 unique id + the start of its name, and
//! reads a state in the VST2 form: "VstW" (header size 8, version 1, bypass 0) then the VST2
//! preset file ("CcnK" size "FBCh" version 2, id, plugin version, 0, 128 zeros, chunk size,
//! chunk; checked byte for byte on five plugins of the reference corpus). That is what this writes. For a
//! VST3 without that class id the same state may or may not be understood: the user tries,
//! listens, and decides (see `compatible`).
use crate::xml::Element;

/// The VST3 a VST2 becomes: its 16-byte class id, name, and Live's device type (1 an
/// instrument, 2 an audio effect, 4 a MIDI effect).
#[derive(Debug, Clone)]
pub struct Target {
    pub vst2_name: String,
    pub class_id: [u8; 16],
    pub name: String,
    pub device_type: i64,
}

/// "56535453-766c-3373-7761-6d2076696f6c" (Live's database) -> 16 bytes.
pub fn class_id(uuid: &str) -> Option<[u8; 16]> {
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Whether a VST3 declares it replaces this VST2: its class id is "VST" + the VST2 unique id.
pub fn compatible(class_id: &[u8; 16], vst2_unique_id: i64) -> bool {
    &class_id[..3] == b"VST" && class_id[3..7] == (vst2_unique_id as u32).to_be_bytes()
}

/// Live's device type for a plugin kind ("instr" / "audiofx" / "midifx").
pub fn device_type(kind: &str) -> i64 {
    match kind {
        "instr" => 1,
        "midifx" => 4,
        _ => 2,
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len() / 2).map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

const FBCH: u32 = u32::from_be_bytes(*b"FBCh");
const FPCH: u32 = u32::from_be_bytes(*b"FPCh");

/// The VST3 processor state for a VST2 preset (None: a parameter list, not a chunk).
pub fn state_from_vst2(kind: u32, chunk: &[u8], unique_id: i64, plugin_version: i64, programs: i64, params: i64) -> Option<Vec<u8>> {
    let mut fx = Vec::new();
    let mut body = Vec::new();
    match kind {
        FBCH => {
            // as the VST3s that replace their VST2 write it (read from real projects: Blackhole,
            // Lindell 80, MODO BASS, StandardCLIP, Ghz): format version 2, no program count,
            // the current program (0) then 124 bytes of zeros
            let _ = programs;
            body.extend_from_slice(b"FBCh");
            body.extend_from_slice(&2u32.to_be_bytes());
            body.extend_from_slice(&(unique_id as u32).to_be_bytes());
            body.extend_from_slice(&(plugin_version as u32).to_be_bytes());
            body.extend_from_slice(&0u32.to_be_bytes());
            body.extend_from_slice(&[0u8; 128]);
        }
        FPCH => {
            body.extend_from_slice(b"FPCh");
            body.extend_from_slice(&1u32.to_be_bytes());
            body.extend_from_slice(&(unique_id as u32).to_be_bytes());
            body.extend_from_slice(&(plugin_version as u32).to_be_bytes());
            body.extend_from_slice(&(params.max(0) as u32).to_be_bytes());
            body.extend_from_slice(&[0u8; 28]); // program name
        }
        _ => return None,
    }
    body.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
    body.extend_from_slice(chunk);
    fx.extend_from_slice(b"CcnK");
    fx.extend_from_slice(&(body.len() as u32).to_be_bytes());
    fx.extend_from_slice(&body);
    let mut out = Vec::with_capacity(fx.len() + 16);
    out.extend_from_slice(b"VstW");
    out.extend_from_slice(&8u32.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&fx);
    Some(out)
}

fn leaf(tag: &str, value: &str) -> Element {
    let mut e = Element::new(tag);
    e.set("Value", value);
    e
}

fn uid(class_id: &[u8; 16]) -> Element {
    let mut u = Element::new("Uid");
    for i in 0..4 {
        let v = i32::from_be_bytes(class_id[i * 4..i * 4 + 4].try_into().unwrap());
        u.children.push(leaf(&format!("Fields.{i}"), &v.to_string()));
    }
    u
}

/// The fields every Live preset has (on/off, folding, macro mapping...), copied as they were.
const COMMON: [&str; 11] = [
    "OverwriteProtectionNumber",
    "ParameterSettings",
    "IsOn",
    "PowerMacroControlIndex",
    "PowerMacroMappingRange",
    "IsFolded",
    "StoredAllParameters",
    "DeviceLomId",
    "DeviceViewLomId",
    "IsOnLomId",
    "ParametersListWrapperLomId",
];

/// Replaces, in a whole set, every VST2 device of `targets` by its VST3. Returns (devices
/// replaced, devices left: their VST2 state is a parameter list).
pub fn replace(tree: &mut Element, targets: &[Target]) -> (usize, usize) {
    let (mut done, mut left) = (0, 0);
    tree.walk_mut(&mut |e| {
        if e.tag != "PluginDesc" {
            return;
        }
        let Some(info) = e.child("VstPluginInfo") else { return };
        let name = info.child("PlugName").and_then(|n| n.get("Value")).unwrap_or("").to_lowercase();
        let Some(t) = targets.iter().find(|t| t.vst2_name.to_lowercase() == name) else { return };
        let Some(preset) = info.find("Preset/VstPreset") else {
            left += 1;
            return;
        };
        let num = |e: &Element, tag: &str| e.child(tag).and_then(|x| x.get("Value")).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let kind = num(preset, "Type") as u32;
        let chunk = preset.child("Buffer").and_then(|b| b.text.as_deref()).and_then(unhex).unwrap_or_default();
        let uid2 = num(info, "UniqueId");
        let Some(state) = state_from_vst2(kind, &chunk, uid2, num(preset, "PluginVersion"), num(preset, "ProgramCount"), num(preset, "ParameterCount")) else {
            left += 1;
            return;
        };
        let mut p3 = Element::new("Vst3Preset");
        if let Some(id) = preset.get("Id") {
            p3.set("Id", id);
        }
        for tag in COMMON {
            if let Some(c) = preset.child(tag) {
                p3.children.push(c.clone());
            }
        }
        p3.children.push(uid(&t.class_id));
        p3.children.push(leaf("DeviceType", &t.device_type.to_string()));
        let mut ps = Element::new("ProcessorState");
        ps.text = Some(hex(&state));
        p3.children.push(ps);
        p3.children.push(Element::new("ControllerState"));
        p3.children.push(leaf("Name", ""));
        p3.children.push(Element::new("PresetRef"));
        let mut pr = Element::new("Preset");
        pr.children.push(p3);
        let mut i3 = Element::new("Vst3PluginInfo");
        i3.set("Id", info.get("Id").unwrap_or("0"));
        for tag in ["WinPosX", "WinPosY"] {
            if let Some(c) = info.child(tag) {
                i3.children.push(c.clone());
            }
        }
        i3.children.push(pr);
        i3.children.push(leaf("Name", &t.name));
        i3.children.push(uid(&t.class_id));
        i3.children.push(leaf("DeviceType", &t.device_type.to_string()));
        if let Some(slot) = e.children.iter_mut().find(|c| c.tag == "VstPluginInfo") {
            i3.tail = slot.tail.clone();
            *slot = i3;
            done += 1;
        }
    });
    (done, left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ids_and_compat() {
        let id = class_id("56535453-766c-3373-7761-6d2076696f6c").unwrap();
        assert_eq!(&id, b"VSTSvl3swam viol");
        assert!(compatible(&id, i64::from(u32::from_be_bytes(*b"Svl3"))));
        let melda = class_id("4d656c64-6170-726f-4d41-41614d414161").unwrap();
        assert!(!compatible(&melda, i64::from(u32::from_be_bytes(*b"MAAa"))));
    }

    #[test]
    fn state_has_the_vst2_preset_inside() {
        let s = state_from_vst2(FBCH, &[1, 2, 3], 0x4C383063, 7, 1, 0).unwrap();
        assert_eq!(&s[..4], b"VstW");
        assert_eq!(&s[16..20], b"CcnK");
        assert_eq!(&s[24..28], b"FBCh");
        let size = u32::from_be_bytes(s[20..24].try_into().unwrap()) as usize;
        assert_eq!(size, s.len() - 24);
        assert_eq!(&s[s.len() - 3..], &[1, 2, 3]);
        assert!(state_from_vst2(u32::from_be_bytes(*b"FxBk"), &[], 1, 1, 1, 1).is_none());
    }
}

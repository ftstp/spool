use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Everything read from one set. Field names match the Python prototype's JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AlsInfo {
    pub error: Option<String>,
    pub live_version: Option<String>,
    pub tempo: Option<f64>,
    pub time_sig: Option<String>,
    pub length_s: Option<i64>,
    /// The scale set in Live ("C Major", "D Dorian"), when its scale mode is on.
    pub key: Option<String>,
    /// Track counts by kind: audio, midi, groupes, retours.
    pub tracks: BTreeMap<String, u32>,
    pub devices: Vec<Device>,
    pub main_chain: Vec<String>,
    pub plugins: Vec<PluginRef>,
    pub samples_total: u32,
    pub samples_missing: Vec<MissingSample>,
    pub names: Vec<String>,
    pub sample_names: Vec<String>,
    /// Key guessed from the MIDI notes ("Fm", "G Phrygian"); only when the set has tracks.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub key_est: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub key_conf: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub key_bass: Option<bool>,
    /// The file format, exact ("12.0_12120/7"): Live 12.0 and 12.1 both say "12.0" in front.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub format: Option<String>,
    /// Locators: (beat, name), in time order.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub locators: Vec<(f64, String)>,
    /// Each track: what it is made of (for the project assembly).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub track_info: Vec<TrackInfo>,
}

/// One track of a set, as a sound that could go into another project.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrackInfo {
    /// Live's track id (unique in the set).
    pub id: String,
    /// midi / audio / group / return
    pub kind: String,
    pub name: String,
    pub role: String,
    /// The id of the group it is in ("" at the top).
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub color: i64,
    /// Its devices in order (a rack by its name).
    #[serde(default)]
    pub chain: Vec<String>,
    /// Every plugin in it, racks included.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// The first device of a MIDI track: what makes the sound.
    #[serde(default)]
    pub instrument: Option<String>,
    /// Samples its devices load (not its clips), and how many are missing.
    #[serde(default)]
    pub samples: u32,
    #[serde(default)]
    pub samples_missing: u32,
    #[serde(default)]
    pub volume: Option<f64>,
    /// MIDI notes and audio clips it plays (frozen copies left out).
    #[serde(default)]
    pub notes: u32,
    #[serde(default)]
    pub audio_clips: u32,
    /// A device listens to another track (a sidechain).
    #[serde(default)]
    pub sidechain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub name: String,
    pub native: bool,
    /// instr / midifx / audiofx
    pub kind: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginRef {
    /// vst2 / vst3 / au
    pub format: String,
    pub name: String,
    pub id: String,
    pub path: String,
    /// What the plugin itself reported when the set was saved (VST2 only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file: Option<String>,
    pub preset: Option<Preset>,
    /// AU only: how its state can become the VST2's (None: it cannot).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub au_state: Option<Option<String>>,
    /// A parameter of this device moved by an envelope (automation or clip modulation): a
    /// switch to another plugin (VST3) would lose it.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub automated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    pub bank: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissingSample {
    pub path: String,
    pub size: Option<i64>,
    /// The same path on the previous system's drive, when it is there.
    pub found_at: Option<String>,
}

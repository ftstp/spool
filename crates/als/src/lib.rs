//! Read one Ableton Live set (`.als`, gzip XML). Read-only: never writes to a project.
//!
//! Returns musical facts (tempo, length, key, tracks), the devices used (native and
//! plugins), the main track's chain, the words the user typed (track, clip and locator
//! names) and every sample reference with whether it exists.
//!
//! Ported from the Python prototype (`apf/als.py`), field for field: the output
//! serialises to the same JSON so both can be compared on a real corpus.

mod au;
mod key;
mod model;
pub mod native;
mod presets;
mod read;
mod role;
mod risks;
mod samples;
mod xml;

pub use key::{KeyEstimate, estimate_key, refine_mode};
pub use model::{AlsInfo, Device, MissingSample, PluginRef, Preset, TrackInfo};
pub use read::{read_als, read_als_bytes};
pub use role::{Role, track_role};
pub use risks::{move_risks, project_files};
pub use samples::{Env, is_file, old_path, path_candidates, project_root};

/// Audio (and Max for Live) files a set may reference.
pub const SAMPLE_EXT: [&str; 8] = [".wav", ".aif", ".aiff", ".mp3", ".flac", ".ogg", ".m4a", ".amxd"];

/// Note names, C = 0.
pub const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

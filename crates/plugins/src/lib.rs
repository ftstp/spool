//! What plugins this machine (and the previous Windows) has, and how a project's plugin
//! reference matches them. Reads Live's own plugin database: what Live has actually scanned.
//!
//! Ported from the prototype (`apf/plugins.py`); output serialises to the same JSON.

mod installed;
mod matcher;
mod names;
mod version;

pub use installed::{Installed, live_db, load_installed, old_system_dbs};
pub use matcher::{Candidate, Describe, Matcher, Need, Old, Ref, Status, classify, status_severity};
pub use names::{family_name, norm_name};
pub use version::{decode_plugin_version, file_version};

/// Key of a plugin in the "trials" setting: the bare name for VST2 (as first stored),
/// "au|name" for a Mac plugin (Kontakt 5 exists as both).
pub fn trial_key(fmt: &str, name: &str) -> String {
    if fmt == "vst2" { name.to_lowercase() } else { format!("{fmt}|{}", name.to_lowercase()) }
}

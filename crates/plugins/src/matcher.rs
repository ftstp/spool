//! Classify one plugin reference of a project against what is installed.
use crate::installed::{Installed, live_db, load_installed, old_system_dbs};
use crate::names::{family_name, norm_name};
use crate::version::{decode_plugin_version, file_version};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// A plugin as a project references it (from the .als reader).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Ref {
    pub format: String,
    pub name: String,
    #[serde(default)]
    pub id: String,
    /// VST2: the version code the plugin wrote in the project.
    #[serde(default)]
    pub version: Option<String>,
    /// AU: how its state can become the VST2's.
    #[serde(default)]
    pub au_state: Option<String>,
    /// Moved by an envelope in the project (als::PluginRef::automated).
    #[serde(default)]
    pub automated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Same format and id (and name for VST2) installed.
    Ok,
    /// VST2: the id is installed under another name (x64 / portable variant).
    Renamed,
    /// VST2 missing, a VST2 with the same normalised name exists (other id).
    OtherVst2,
    /// VST2 missing, only a VST3 of the same name exists (state may not carry).
    ToVst3,
    /// VST3 missing, another plugin of the same name exists (another version).
    OtherVst3,
    /// Only another version of the product is installed (Pro-Q 2 -> Pro-Q 4).
    Newer,
    /// Nothing installed resembles it.
    Missing,
    /// Replaced by an installed version chosen by the user (the Repair page).
    Trial,
}

/// The severity of each state, the ONE table: web/src/lib/severity.json, read here and by the
/// interface. 0 opens, 1 the app repairs it (settings kept), 2 loads but settings lost, 3 not
/// installed.
static SEVERITY: LazyLock<HashMap<String, i64>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../../web/src/lib/severity.json")).expect("severity.json"));

/// A plugin state's severity (an unknown state counts as missing).
pub fn status_severity(status: &str) -> i64 {
    SEVERITY.get(status).copied().unwrap_or(3)
}

impl Status {
    /// 0 fine, 1 fixable with settings kept, 2 loads but settings lost, 3 nothing installed.
    /// Must agree with the table (a test checks every state).
    pub fn severity(self) -> u8 {
        match self {
            Status::Ok => 0,
            Status::Renamed | Status::OtherVst2 | Status::Trial => 1,
            Status::Newer | Status::ToVst3 | Status::OtherVst3 => 2,
            Status::Missing => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    pub format: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Old {
    pub vendor: String,
    pub version: String,
    pub path: String,
    pub on_disk: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Need {
    pub version: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundles: Option<Vec<String>>,
}

/// Status, candidates and what the previous system had, for one reference.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Describe {
    pub status: Status,
    pub candidates: Vec<Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Old>,
    pub old_formats: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub need: Option<Need>,
}

type Key = (String, String);

/// Everything needed to classify references, built once per state of Live's database.
pub struct Matcher {
    old_drive: Option<String>,
    pub installed: Vec<Installed>,
    by_id: HashMap<Key, Vec<usize>>,
    by_norm: HashMap<String, Vec<usize>>,
    by_family: HashMap<String, Vec<usize>>,
    old_installed: Vec<Installed>,
    old_by_id: HashMap<Key, usize>,
    old_by_family: HashMap<String, Vec<usize>>,
    bundles: Mutex<HashMap<String, Vec<String>>>,
}

static NOT_FAMILY_V2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-_ ]v?\d+(\.\d+)+$").unwrap());
static NEED_NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s_.(-]*(x64|x86|64[\s-]?bit|32[\s-]?bit|\(?mono\)?|\(?stereo\)?)\)?").unwrap());
static GENERATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d|\s[vV]$").unwrap());
static FIRST_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s,]+").unwrap());

/// Plugin ids whose product version is known for sure: the vendor changed the id with a
/// version, so the id alone says which version a project needs.
fn known_id(fmt: &str, id: &str) -> Option<String> {
    match (fmt, id) {
        ("vst2", "1399875954") => Some(i18n::tr!("plugins.need.spire10")),
        _ => None,
    }
}

impl Matcher {
    /// This machine's database (plugins whose file is gone are dropped) and the previous
    /// system's ones.
    pub fn load(old_drive: Option<&str>) -> Matcher {
        let db = live_db();
        let installed = if db.exists() { load_installed(&db, true).unwrap_or_default() } else { Vec::new() };
        let mut old = Vec::new();
        for d in old_system_dbs(old_drive) {
            old.extend(load_installed(&d, false).unwrap_or_default());
        }
        Matcher::new(installed, old, old_drive)
    }

    pub fn new(installed: Vec<Installed>, old_installed: Vec<Installed>, old_drive: Option<&str>) -> Matcher {
        let mut by_id: HashMap<Key, Vec<usize>> = HashMap::new();
        let mut by_norm: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_family: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, pl) in installed.iter().enumerate() {
            by_id.entry((pl.format.clone(), pl.id.clone())).or_default().push(i);
            for key in BTreeSet::from([norm_name(&pl.name), norm_name(&pl.file)]) {
                by_norm.entry(key).or_default().push(i);
            }
            for key in BTreeSet::from([family_name(&pl.name), family_name(&pl.file)]) {
                if key.len() >= 4 {
                    by_family.entry(key).or_default().push(i);
                }
            }
        }
        let mut old_by_id = HashMap::new();
        let mut old_by_family: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in old_installed.iter().enumerate() {
            old_by_id.insert((p.format.clone(), p.id.clone()), i); // the last one wins
            old_by_family.entry(family_name(&p.name)).or_default().push(i);
        }
        Matcher {
            old_drive: old_drive.filter(|d| !d.is_empty()).map(str::to_string),
            installed,
            by_id,
            by_norm,
            by_family,
            old_installed,
            old_by_id,
            old_by_family,
            bundles: Mutex::new(HashMap::new()),
        }
    }

    fn get<'s>(&'s self, ix: &[usize]) -> Vec<&'s Installed> {
        ix.iter().map(|&i| &self.installed[i]).collect()
    }

    /// instr / audiofx / midifx, from this PC's or the old PC's Live database.
    pub fn kind(&self, r: &Ref) -> String {
        let key = (r.format.clone(), r.id.clone());
        if let Some(i) = self.by_id.get(&key).and_then(|v| v.first()) {
            return self.installed[*i].kind.clone();
        }
        self.old_by_id.get(&key).map(|&i| self.old_installed[i].kind.clone()).unwrap_or_default()
    }

    pub fn describe(&self, r: &Ref) -> Describe {
        let (status, cands) = classify(r, self);
        let mut out = Describe {
            status,
            candidates: cands
                .iter()
                .take(5)
                .map(|c| Candidate {
                    format: c.format.clone(),
                    name: c.name.clone(),
                    vendor: c.vendor.clone(),
                    version: c.version.clone(),
                    path: c.path.clone(),
                })
                .collect(),
            old: None,
            old_formats: Vec::new(),
            need: None,
        };
        let fam: Vec<&Installed> = self
            .old_by_family
            .get(&family_name(&r.name))
            .map(|v| v.iter().map(|&i| &self.old_installed[i]).collect())
            .unwrap_or_default();
        if let Some(&i) = self.old_by_id.get(&(r.format.clone(), r.id.clone())) {
            let old = &self.old_installed[i];
            let op = old_path(&old.path, self.old_drive.as_deref());
            let on_disk = op.as_ref().is_some_and(|p| p.exists());
            out.old = Some(Old {
                vendor: old.vendor.clone(),
                version: old.version.clone(),
                path: old.path.clone(),
                on_disk,
                file_version: Some(if on_disk { file_version(op.as_ref().unwrap()) } else { String::new() }),
            });
        } else if let Some(f) = fam.first() {
            out.old = Some(Old {
                vendor: f.vendor.clone(),
                version: String::new(),
                path: String::new(),
                on_disk: false,
                file_version: None,
            });
        }
        out.old_formats = fam.iter().map(|p| p.format.clone()).collect::<BTreeSet<_>>().into_iter().collect();
        if status != Status::Ok {
            let mut need = needed_version(r, &out);
            if need.version.is_empty()
                && let Some(vendor) = out.old.as_ref().map(|o| o.vendor.clone()).filter(|v| !v.is_empty())
            {
                let b = self.old_bundles(&vendor);
                if !b.is_empty() {
                    need.bundles = Some(b);
                }
            }
            out.need = Some(need);
        }
        out
    }

    /// Product folders the previous Windows had for this vendor ("V Collection 7", "FX
    /// Collection 2"): when a plugin does not record its version, the bundle it came from
    /// tells what to reinstall.
    fn old_bundles(&self, vendor: &str) -> Vec<String> {
        let Some(drive) = &self.old_drive else { return Vec::new() };
        let key = vendor.to_lowercase();
        if let Some(b) = self.bundles.lock().unwrap().get(&key) {
            return b.clone();
        }
        let first = FIRST_WORD.split(vendor.trim()).next().unwrap_or("").to_string();
        let mut names = BTreeSet::new();
        for pf in ["Program Files", "Program Files (x86)"] {
            let base = PathBuf::from(format!("{drive}:/{pf}"));
            for d in [base.join(vendor), base.join(&first)] {
                if d.is_dir() {
                    for c in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                        let n = c.file_name().to_string_lossy().to_string();
                        let low = n.to_lowercase();
                        if c.path().is_dir() && !low.starts_with("common") && !low.starts_with("uninst") {
                            names.insert(n);
                        }
                    }
                }
            }
        }
        let b: Vec<String> = names.into_iter().take(8).collect();
        self.bundles.lock().unwrap().insert(key, b.clone());
        b
    }
}

/// The same path on the previous system's drive (dual boot: its C: is now that drive).
fn old_path(path: &str, old_drive: Option<&str>) -> Option<PathBuf> {
    let d = old_drive?;
    let b = path.as_bytes();
    if b.len() >= 3 && b[0].eq_ignore_ascii_case(&b'c') && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\') {
        Some(PathBuf::from(format!("{d}:{}", &path[2..])))
    } else {
        None
    }
}

/// The exact version to install, from the most reliable source available.
fn needed_version(r: &Ref, d: &Describe) -> Need {
    let need =
        |version: String, source: &str| Need { version, source: source.into(), label: None, any: None, bundles: None };
    if let Some(old) = &d.old {
        if let Some(fv) = old.file_version.as_ref().filter(|v| !v.is_empty()) {
            return need(fv.clone(), &i18n::tr!("plugins.need.oldFile"));
        }
        if !old.version.is_empty() && old.version != "1" {
            return need(old.version.clone(), &i18n::tr!("plugins.need.oldList"));
        }
    }
    let v = decode_plugin_version(r.version.as_deref());
    if !v.is_empty() {
        let code = r.version.clone().unwrap_or_default();
        return need(v, &i18n::tr!("plugins.need.inProject", code = code));
    }
    // No number anywhere: the PRODUCT to install is still known, and within one product
    // line (same plugin id) Live loads any update and newer updates read older states.
    let name = NEED_NOISE.replace_all(&r.name, "").trim().to_string();
    let label = |text: String, source: &str, any: bool| Need {
        version: String::new(),
        source: source.into(),
        label: Some(text),
        any: any.then_some(true),
        bundles: None,
    };
    let base_source = &i18n::tr!("plugins.need.notWritten");
    if let Some(k) = known_id(&r.format, &r.id) {
        return label(k, &i18n::tr!("plugins.need.knownId"), false);
    }
    if r.format == "au" {
        return label(
            i18n::tr!("plugins.need.au", name = name),
            base_source,
            true,
        );
    }
    if matches!(d.status, Status::Newer | Status::OtherVst3)
        && let Some(first) = d.candidates.first()
    {
        return label(
            i18n::tr!("plugins.need.itself", name = name, other = first.name),
            base_source,
            true,
        );
    }
    if d.status == Status::ToVst3 {
        return label(i18n::tr!("plugins.need.vst2", name = name), base_source, true);
    }
    let text = if GENERATION.is_match(&name) {
        i18n::tr!("plugins.need.anyUpdate", name = name)
    } else {
        i18n::tr!("plugins.need.anyRecent", name = name)
    };
    label(text, base_source, true)
}

fn dedupe(cands: Vec<&Installed>) -> Vec<&Installed> {
    let mut seen = std::collections::HashSet::new();
    cands.into_iter().filter(|c| seen.insert((c.format.clone(), c.id.clone()))).collect()
}

/// (status, candidates) for one reference.
pub fn classify<'m>(r: &Ref, m: &'m Matcher) -> (Status, Vec<&'m Installed>) {
    let (status, cands) = classify_exact(r, m);
    let fam = family_name(&r.name);
    if status == Status::Missing
        && r.format != "au"
        && fam.len() >= 4
        && let Some(ix) = m.by_family.get(&fam)
    {
        return (Status::Newer, dedupe(m.get(ix)));
    }
    if status == Status::ToVst3 {
        // The same name with a version tacked on is that product's VST2 under another id:
        // Spire (1.0) vs Spire-1.5. Installing "the VST2" would not help.
        let want = r.name.to_lowercase();
        let other_v2: Vec<&Installed> = m
            .by_family
            .get(&fam)
            .map(|ix| m.get(ix))
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.format == "vst2" && NOT_FAMILY_V2.replace(&c.name, "").to_lowercase() == want)
            .collect();
        if !other_v2.is_empty() {
            return (Status::Newer, dedupe(other_v2.into_iter().chain(cands).collect()));
        }
    }
    (status, dedupe(cands))
}

fn classify_exact<'m>(r: &Ref, m: &'m Matcher) -> (Status, Vec<&'m Installed>) {
    if r.format == "au" {
        return (Status::Missing, Vec::new());
    }
    let hits = m.by_id.get(&(r.format.clone(), r.id.clone())).map(|ix| m.get(ix)).unwrap_or_default();
    if r.format == "vst2" && !hits.is_empty() {
        if hits.iter().any(|h| h.name == r.name) {
            return (Status::Ok, Vec::new());
        }
        return (Status::Renamed, hits);
    }
    if r.format == "vst3" && !hits.is_empty() {
        return (Status::Ok, Vec::new());
    }
    let same: Vec<&Installed> = m.by_norm.get(&norm_name(&r.name)).map(|ix| m.get(ix)).unwrap_or_default();
    let of = |f: &str| same.iter().copied().filter(|c| c.format == f).collect::<Vec<_>>();
    let (first, second, s1, s2) = if r.format == "vst2" {
        ("vst2", "vst3", Status::OtherVst2, Status::ToVst3)
    } else {
        ("vst3", "vst2", Status::OtherVst3, Status::OtherVst3)
    };
    let a = of(first);
    if !a.is_empty() {
        return (s1, a);
    }
    let b = of(second);
    if !b.is_empty() {
        return (s2, b);
    }
    (Status::Missing, Vec::new())
}

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn every_state_agrees_with_the_one_table() {
        for s in [Status::Ok, Status::Renamed, Status::OtherVst2, Status::ToVst3, Status::OtherVst3, Status::Newer, Status::Missing, Status::Trial] {
            let name = serde_json::to_value(s).unwrap();
            assert_eq!(s.severity() as i64, status_severity(name.as_str().unwrap()), "{name}");
        }
        // set by the app on top of the matcher's states
        assert_eq!(status_severity("vst3"), 1);
        // an unknown state never passes for a working one
        assert_eq!(status_severity("something new"), 3);
    }
}

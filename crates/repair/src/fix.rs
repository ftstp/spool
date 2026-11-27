//! The repair (R): what a copy fixes, and the changes on a parsed set. Ported from the
//! prototype's repair.py. Nothing here touches the original; writing the copy is `write`.
//!
//! - VST2 plugins installed under another name (same id): the reference is renamed, the
//!   saved state is untouched.
//! - Samples found elsewhere (same name AND size): copied into the project's
//!   Samples/Imported folder and referenced from there, like Live's "Collect All and Save".
//! - Plugins the user no longer uses: removed (device and its automation), unless kept to
//!   be replaced by hand.
//! - A version that cannot be found anywhere, replaced anyway by an installed one (the
//!   user's choice, per plugin): same saved state. A Mac (AU) plugin becomes its Windows VST2,
//!   its state converted (auconv).
//!   What it cannot fix (missing plugins, other versions, lost samples) is reported only.
use crate::auconv::{self, Target};
use crate::xml::Element;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const IMPORTED: &str = "Samples/Imported";

/// Key of a plugin in the "trials" setting: the bare name for VST2 (as first stored),
/// "au|name" for a Mac plugin (Kontakt 5 exists as both).
pub fn trial_key(fmt: &str, name: &str) -> String {
    if fmt == "vst2" { name.to_lowercase() } else { format!("{fmt}|{}", name.to_lowercase()) }
}

#[derive(Debug, Clone, Serialize)]
pub struct Rename {
    pub id: String,
    pub from: String,
    pub to: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Named {
    pub format: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Unfixable {
    pub name: String,
    pub status: String,
}

/// What a repair of one project would do. Pure: from the base only.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub rel: String,
    pub renames: Vec<Rename>,
    /// Missing samples found elsewhere: (as saved, where found).
    pub copies: Vec<Value>,
    pub lost: Vec<String>,
    pub unfixable: Vec<Unfixable>,
    pub removes: Vec<Named>,
    /// Left in the copy, to be replaced by hand.
    pub manual: Vec<Named>,
    /// The trial entry, with "ref_id", "from", "format" added.
    pub trials: Vec<Map<String, Value>>,
    /// VST2 plugins replaced by their VST3: {from, name, uid, kind}.
    pub vst3: Vec<Map<String, Value>>,
    pub useful: bool,
    pub complete: bool,
}

/// What the user decided, from the settings.
#[derive(Debug, Clone, Default)]
pub struct Choices {
    pub ignored: HashSet<String>,
    pub kept: HashSet<String>,
    pub trials: Map<String, Value>,
    /// VST2 plugins to replace by their VST3 (lowercase name -> {name, uid, kind}).
    pub to_vst3: Map<String, Value>,
}

fn s(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `data`: the reader's output for the file, `states`: its plugin states (plugstate).
pub fn plan(rel: &str, data: &Value, states: &Value, ch: &Choices) -> Plan {
    let mut renames: Vec<((String, String), Rename)> = Vec::new();
    type Keyed<T> = Vec<((String, String), T)>;
    let mut keep: Vec<(String, String)> = Vec::new();
    let (mut removes, mut manual): (Vec<Named>, Vec<Named>) = Default::default();
    let mut trials: Keyed<Map<String, Value>> = Vec::new();
    let mut vst3: Vec<Map<String, Value>> = Vec::new();
    let refs = data["plugins"].as_array().cloned().unwrap_or_default();
    let sts = states.as_array().cloned().unwrap_or_default();
    for (r, st) in refs.iter().zip(sts.iter()) {
        let status = st["status"].as_str().unwrap_or("");
        let (name, fmt, id) = (s(&r["name"]), s(&r["format"]), s(&r["id"]));
        let low = name.to_lowercase();
        let first = st["candidates"].as_array().and_then(|a| a.first()).filter(|_| status == "renamed");
        if let Some(c) = first {
            let file =
                Path::new(&s(&c["path"])).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let k = (id.clone(), name.clone());
            let v = Rename { id: id.clone(), from: name.clone(), to: s(&c["name"]), file };
            // a dict: the last one for a key wins, at the place of the first
            match renames.iter_mut().find(|(kk, _)| *kk == k) {
                Some(slot) => slot.1 = v,
                None => renames.push((k, v)),
            }
        } else if status != "ok" && ch.ignored.contains(&low) {
            let n = Named { format: fmt.clone(), name: name.clone() };
            let list = if ch.kept.contains(&low) { &mut manual } else { &mut removes };
            if !list.contains(&n) {
                list.push(n);
            }
        } else if status == "trial"
            && ch.trials.get(&trial_key(&fmt, &name)).is_some_and(|t| !t.is_null() && t != &Value::Object(Map::new()))
            && (!id.is_empty() || fmt == "au")
        {
            let k = if fmt == "au" { ("au".to_string(), name.clone()) } else { (id.clone(), name.clone()) };
            let mut t = ch.trials[&trial_key(&fmt, &name)].as_object().cloned().unwrap_or_default();
            t.insert("ref_id".into(), Value::String(k.0.clone()));
            t.insert("from".into(), Value::String(name.clone()));
            t.insert("format".into(), Value::String(fmt.clone()));
            match trials.iter_mut().find(|(kk, _)| *kk == k) {
                Some(slot) => slot.1 = t,
                None => trials.push((k, t)),
            }
        } else if fmt == "vst2"
            && r["automated"].as_bool() != Some(true)
            && matches!(status, "to_vst3" | "missing" | "other_vst3" | "vst3")
            && ch.to_vst3.contains_key(&low)
        {
            if !vst3.iter().any(|v| v["from"] == name.as_str()) {
                let mut t = ch.to_vst3[&low].as_object().cloned().unwrap_or_default();
                t.insert("from".into(), Value::String(name.clone()));
                vst3.push(t);
            }
        } else if status != "ok" && !keep.iter().any(|(n, _)| *n == name) {
            keep.push((name.clone(), status.to_string()));
        }
    }
    let missing = data["samples_missing"].as_array().cloned().unwrap_or_default();
    let found = |x: &Value| x["found_at"].as_str().is_some_and(|f| !f.is_empty());
    let copies: Vec<Value> = missing.iter().filter(|x| found(x)).cloned().collect();
    let lost: Vec<String> = missing.iter().filter(|x| !found(x)).map(|x| s(&x["path"])).collect();
    let useful = !renames.is_empty() || !copies.is_empty() || !removes.is_empty() || !trials.is_empty() || !vst3.is_empty();
    let complete = keep.is_empty() && lost.is_empty() && manual.is_empty();
    Plan {
        rel: rel.into(),
        renames: renames.into_iter().map(|(_, v)| v).collect(),
        copies,
        lost,
        unfixable: keep.into_iter().map(|(name, status)| Unfixable { name, status }).collect(),
        removes,
        manual,
        trials: trials.into_iter().map(|(_, v)| v).collect(),
        vst3,
        useful,
        complete,
    }
}

/// (format, lowercase name) of a plugin device.
fn device_ref(dev: &Element) -> Option<(&'static str, String)> {
    for (info, name, fmt) in
        [("VstPluginInfo", "PlugName", "vst2"), ("Vst3PluginInfo", "Name", "vst3"), ("AuPluginInfo", "Name", "au")]
    {
        if let Some(el) = dev.find(&format!(".//{info}/{name}")) {
            return Some((fmt, el.get("Value").unwrap_or("").to_lowercase()));
        }
    }
    None
}

/// Elements whose Id lives in the set-wide id space that automation points into.
fn pointee(tag: &str) -> bool {
    tag.ends_with("Target") || tag.starts_with("ControllerTargets.") || tag == "Pointee"
}

/// Removes the envelopes (automation, clip envelopes) pointing at `gone` ids.
fn drop_envelopes(tree: &mut Element, gone: &HashSet<String>) {
    if gone.is_empty() {
        return;
    }
    tree.walk_mut(&mut |e| {
        if e.tag == "Envelopes" {
            e.children.retain(|env| env.value("EnvelopeTarget/PointeeId").is_none_or(|v| !gone.contains(v)));
        }
    });
}

/// Removes the plugin devices whose (format, lowercase name) is in `wanted`, with the
/// automation and clip envelopes that pointed at their parameters. Returns how many.
pub fn remove_plugins(tree: &mut Element, wanted: &HashSet<(String, String)>) -> usize {
    let mut gone: HashSet<String> = HashSet::new();
    let mut n = 0;
    tree.walk_mut(&mut |e| {
        if e.tag != "Devices" {
            return;
        }
        e.children.retain(|dev| {
            let hit = matches!(dev.tag.as_str(), "PluginDevice" | "AuPluginDevice")
                && device_ref(dev).is_some_and(|(f, nm)| wanted.contains(&(f.to_string(), nm)));
            if hit {
                for x in dev.descendants() {
                    if matches!(x.tag.as_str(), "AutomationTarget" | "ModulationTarget")
                        && let Some(id) = x.get("Id").filter(|i| !i.is_empty())
                    {
                        gone.insert(id.to_string());
                    }
                }
                n += 1;
            }
            !hit
        });
    });
    drop_envelopes(tree, &gone);
    n
}

fn vst_key(info: &Element) -> (String, Option<String>) {
    (
        info.child("UniqueId").and_then(|u| u.get("Value")).unwrap_or("").to_lowercase(),
        info.child("PlugName").and_then(|n| n.get("Value")).map(str::to_string),
    )
}

/// Where Mac plugins' VST2 templates are kept (one real device per plugin id).
pub fn template_file(dir: &Path, vst_id: &str) -> PathBuf {
    dir.join(format!("{vst_id}.xml"))
}

pub fn load_template(dir: &Path, vst_id: &str) -> Option<String> {
    std::fs::read_to_string(template_file(dir, vst_id)).ok()
}

/// A trial version replaced IN PLACE: a VST2 renamed (same saved state), a Mac plugin turned
/// into its VST2 with the state converted (its automation cannot follow and is dropped).
/// `templates`: AU name -> template XML. Returns (devices replaced, AU devices left as
/// they were: no state to carry over).
pub fn replace_trials(
    tree: &mut Element,
    trials: &[Map<String, Value>],
    templates: &HashMap<String, String>,
) -> (usize, usize) {
    if tree.child("LiveSet").is_none() || trials.is_empty() {
        return (0, 0);
    }
    let target = |t: &Map<String, Value>| Target {
        to: s(&t.get("to").cloned().unwrap_or_default()),
        file: s(&t.get("file").cloned().unwrap_or_default()),
        id: s(&t.get("id").cloned().unwrap_or_default()),
    };
    let mut vst: HashMap<(String, String), Target> = HashMap::new();
    let mut au: HashMap<String, (Target, Option<String>)> = HashMap::new();
    for t in trials {
        let (rid, from) = (s(&t["ref_id"]).to_lowercase(), s(&t["from"]));
        if t.get("format").and_then(Value::as_str) == Some("au") {
            au.insert(from.clone(), (target(t), templates.get(&from).cloned()));
        } else {
            vst.insert((rid, from), target(t));
        }
    }
    let (mut done, mut skipped) = (0, 0);
    tree.walk_mut(&mut |e| {
        if e.tag != "VstPluginInfo" {
            return;
        }
        let (id, name) = vst_key(e);
        let Some(t) = name.and_then(|n| vst.get(&(id, n))) else { return };
        e.child_mut("PlugName").unwrap().set("Value", t.to.clone());
        if let Some(f) = e.child_mut("FileName") {
            f.set("Value", t.file.clone());
        }
        if let Some(u) = e.child_mut("UniqueId") {
            u.set("Value", t.id.clone());
        }
        done += 1;
    });
    if au.is_empty() {
        return (done, skipped);
    }
    let next_el =
        tree.find("LiveSet/NextPointeeId").and_then(|e| e.get("Value")).map(|v| v.parse::<i64>().unwrap_or(0));
    let mut nxt = next_el.unwrap_or(0);
    for e in tree.descendants() {
        if pointee(&e.tag)
            && let Some(id) = e.get("Id").filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
        {
            nxt = nxt.max(id.parse::<i64>().unwrap_or(0) + 1);
        }
    }
    let mut gone: HashSet<String> = HashSet::new();
    tree.walk_mut(&mut |e| {
        if e.tag != "Devices" {
            return;
        }
        for i in 0..e.children.len() {
            let dev = &e.children[i];
            if dev.tag != "AuPluginDevice" {
                continue;
            }
            let Some(name) = dev.find(".//AuPluginInfo").and_then(|inf| inf.value("Name")).map(str::to_string) else {
                continue;
            };
            let Some((t, tpl)) = au.get(&name) else { continue };
            let new = tpl.as_ref().and_then(|x| auconv::convert(dev, &name, x, t));
            let Some(mut new) = new else {
                skipped += 1; // no state to carry over: left as it is
                continue;
            };
            for x in dev.descendants() {
                if pointee(&x.tag)
                    && let Some(id) = x.get("Id").filter(|i| !i.is_empty())
                {
                    gone.insert(id.to_string());
                }
            }
            // the template's ids come from another project
            new.walk_mut(&mut |x| {
                if pointee(&x.tag) && x.get("Id").is_some() {
                    x.set("Id", nxt.to_string());
                    nxt += 1;
                }
            });
            e.children[i] = new;
            done += 1;
        }
    });
    drop_envelopes(tree, &gone);
    if next_el.is_some()
        && let Some(n) = tree.find_mut("LiveSet/NextPointeeId")
    {
        n.set("Value", nxt.to_string());
    }
    (done, skipped)
}

/// How the samples are put into the project: really copied, or only planned (a dry run,
/// and the comparisons with the prototype).
pub trait Copier {
    fn exists(&self, p: &Path) -> bool;
    fn size(&self, p: &Path) -> Option<u64>;
    fn copy(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
}

/// Copies for real, the modification date kept (as Python's shutil.copy2).
pub struct Disk;

impl Copier for Disk {
    fn exists(&self, p: &Path) -> bool {
        p.exists()
    }
    fn size(&self, p: &Path) -> Option<u64> {
        std::fs::metadata(p).ok().map(|m| m.len())
    }
    fn copy(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        crate::safe::copy_new(from, to)
    }
}

/// Plans the copies only: nothing is written.
pub struct DryRun;

impl Copier for DryRun {
    fn exists(&self, p: &Path) -> bool {
        p.exists()
    }
    fn size(&self, p: &Path) -> Option<u64> {
        std::fs::metadata(p).ok().map(|m| m.len())
    }
    fn copy(&mut self, _: &Path, _: &Path) -> std::io::Result<()> {
        Ok(())
    }
}

/// What a repair did.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Done {
    pub renamed: usize,
    pub removed: usize,
    pub tried: usize,
    pub tried_skipped: usize,
    pub samples: usize,
    pub copied: usize,
    /// VST2 devices replaced by their VST3, and those left (a parameter-list state).
    pub vst3: usize,
    pub vst3_left: usize,
}

fn posix(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The repair on a parsed set (`src`: the set's path, for its project folder).
pub fn repair_tree(
    p: &Plan,
    tree: &mut Element,
    src: &Path,
    templates_dir: &Path,
    copier: &mut dyn Copier,
) -> Result<Done, String> {
    // 1. installed under another name: same id, so the saved state still applies. Matched on
    // id AND name: two plugins can share a name and only one may be renamed.
    let by: HashMap<(String, String), &Rename> =
        p.renames.iter().map(|r| ((r.id.clone(), r.from.clone()), r)).collect();
    let mut renamed = 0;
    tree.walk_mut(&mut |e| {
        if e.tag != "VstPluginInfo" {
            return;
        }
        let (id, name) = vst_key(e);
        let Some(r) = name.and_then(|n| by.get(&(id, n))) else { return };
        e.child_mut("PlugName").unwrap().set("Value", r.to.clone());
        if let Some(f) = e.child_mut("FileName") {
            f.set("Value", r.file.clone());
        }
        renamed += 1;
    });
    // 1b. plugins no longer used: removed (unless kept to be replaced by hand)
    let wanted: HashSet<(String, String)> =
        p.removes.iter().map(|r| (r.format.clone(), r.name.to_lowercase())).collect();
    let removed = remove_plugins(tree, &wanted);
    // 1c. a version that cannot be found, replaced in place by an installed one
    let mut templates = HashMap::new();
    for t in p.trials.iter().filter(|t| t.get("format").and_then(Value::as_str) == Some("au")) {
        if let Some(x) = load_template(templates_dir, &s(&t["id"])) {
            templates.insert(s(&t["from"]), x);
        }
    }
    let (tried, tried_skipped) = replace_trials(tree, &p.trials, &templates);
    // 1d. a VST2 replaced by its installed VST3, its state carried over
    let targets: Vec<crate::vst3::Target> = p
        .vst3
        .iter()
        .filter_map(|t| {
            Some(crate::vst3::Target {
                vst2_name: s(t.get("from")?),
                class_id: crate::vst3::class_id(&s(t.get("uid")?))?,
                name: s(t.get("name")?),
                device_type: crate::vst3::device_type(&s(t.get("kind").unwrap_or(&Value::Null))),
            })
        })
        .collect();
    let (vst3, vst3_left) = crate::vst3::replace(tree, &targets);
    // 2. samples found elsewhere: copied into the project, referenced relatively
    let proj = als::project_root(src);
    let dest_dir = proj.join(IMPORTED);
    let found: HashMap<String, PathBuf> =
        p.copies.iter().map(|c| (s(&c["path"]), PathBuf::from(s(&c["found_at"])))).collect();
    let mut done: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut copied = 0;
    let mut err = None;
    tree.walk_mut(&mut |fr| {
        if fr.tag != "FileRef" || err.is_some() {
            return;
        }
        let path = fr.value("Path").unwrap_or("").replace('\\', "/");
        let shown = if path.is_empty() { fr.value("RelativePath").unwrap_or("").replace('\\', "/") } else { path };
        let Some(source) = found.get(&shown) else { return };
        if !done.contains_key(&shown) {
            let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let (stem, ext) = match name.rsplit_once('.') {
                Some((a, b)) if !a.is_empty() => (a.to_string(), format!(".{b}")),
                _ => (name.clone(), String::new()),
            };
            let mut target = dest_dir.join(&name);
            let mut i = 2;
            while copier.exists(&target) && copier.size(&target) != copier.size(source).or(Some(u64::MAX)) {
                target = dest_dir.join(format!("{stem} ({i}){ext}"));
                i += 1;
            }
            if !copier.exists(&target) {
                if let Err(e) = copier.copy(source, &target) {
                    err = Some(i18n::tr!("repair.copyFailed", name = name, e = e));
                    return;
                }
                copied += 1;
            }
            done.insert(shown.clone(), target);
        }
        let target = &done[&shown];
        let rel = target.strip_prefix(&proj).map(posix).unwrap_or_default();
        for (tag, val) in [("RelativePathType", "3".to_string()), ("RelativePath", rel), ("Path", posix(target))] {
            if let Some(el) = fr.child_mut(tag) {
                el.set("Value", val);
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(Done { renamed, removed, tried, tried_skipped, samples: done.len(), copied, vst3, vst3_left })
}

/// Keeps a real VST2 device of this plugin (taken from one of the user's projects, `sets`)
/// to build converted Mac devices on. True when one is stored.
pub fn ensure_template(dir: &Path, vst_id: &str, sets: &[PathBuf]) -> bool {
    let f = template_file(dir, vst_id);
    if f.exists() {
        return true;
    }
    let vid = vst_id.to_lowercase();
    for p in sets {
        let Ok(set) = crate::xml::read_set(p) else { continue };
        let found = set.root.descendants().find(|dev| {
            dev.tag == "PluginDevice"
                && dev.find(".//VstPluginInfo").is_some_and(|info| vst_key(info).0 == vid)
                && dev.find(".//VstPreset/Buffer").is_some()
        });
        if let Some(dev) = found {
            let mut s = String::new();
            crate::xml::serialize(dev, &mut s);
            return std::fs::create_dir_all(dir).is_ok() && std::fs::write(&f, s).is_ok();
        }
    }
    false
}

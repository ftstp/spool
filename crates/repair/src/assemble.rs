//! A new set made of tracks taken from the user's projects: each comes with its instrument,
//! its effects and their presets ("a chain"), nothing else. It is a clean start, not a song:
//! no clip, no automation, no group, no return; the mixer flat (heard, not soloed, centred,
//! one level for all); a colour per role; the main track with a few plain devices; locators
//! for the structure. The set that gives the frame lends its file format and settings only.
//!
//! What tied a track to its old set is cut or rewired: it plays to the main output, a
//! sidechain listening to another track listens to nothing, a device switched off is gone,
//! and the samples its devices load are copied into the new project (like Live's "Collect
//! All and Save"): the new set stands on its own and the source projects are never touched.
use crate::studio;
use crate::xml::{Element, Set, read_set, write_set};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One track of one set.
#[derive(Debug, Clone, Default)]
pub struct Pick {
    pub set: PathBuf,
    pub track: String,
    /// Its name in the new set (else its own).
    pub name: Option<String>,
    /// Live's colour index (else its own).
    pub color: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Order {
    /// The set that lends its format (its tracks, returns and devices all go).
    pub base: PathBuf,
    pub picks: Vec<Pick>,
    /// Native devices for the main track, in order ("StereoGain", "Limiter"): copied from the
    /// frame or from a set of the same format in `donors` (the picks' sets are donors too).
    pub main_devices: Vec<String>,
    pub donors: Vec<PathBuf>,
    pub tempo: Option<f64>,
    /// "F minor", "Am", "C"...
    pub scale: Option<String>,
    /// (beat, name)
    pub markers: Vec<(f64, String)>,
    /// The level of every track, in dB.
    pub volume_db: f64,
    /// Keep the mix tools of the tracks (EQ, dynamics, gain): off for a clean start.
    pub keep_mix: bool,
    /// The .als to write (its folder is the new project).
    pub out: PathBuf,
}

const SAMPLE_EXT: [&str; 8] = [".wav", ".aif", ".aiff", ".mp3", ".flac", ".ogg", ".m4a", ".amxd"];

/// The set's file format: "12.0_12049" (Live 12.0) and "12.0_12120" (Live 12.1) differ although
/// both say 12.0, so the whole MinorVersion counts, with the SchemaChangeCount. A track is only
/// grafted onto a set of the same format: Live converts whole sets only, and devices change
/// between versions (Live 12.1 made the Saturator's PostClip a number: a 12.0 Saturator in a
/// 12.1 set is a corrupt file).
pub fn generation(root: &Element) -> String {
    let v = root.get("MinorVersion").unwrap_or("");
    match root.get("SchemaChangeCount") {
        Some(c) => format!("{v}/{c}"),
        None => v.to_string(),
    }
}

/// "Live 12.1", for messages.
pub fn live_name(root: &Element) -> String {
    root.get("Creator").map(|c| c.replace("Ableton ", "")).unwrap_or_else(|| format!("Live {}", generation(root)))
}

fn ls(root: &Element) -> &Element {
    root.child("LiveSet").expect("LiveSet")
}

fn tracks_mut(root: &mut Element) -> &mut Vec<Element> {
    &mut root.child_mut("LiveSet").unwrap().child_mut("Tracks").unwrap().children
}

fn tid(t: &Element) -> String {
    t.get("Id").unwrap_or("").to_string()
}

fn main_tag(root: &Element) -> &'static str {
    if ls(root).child("MainTrack").is_some() { "MainTrack" } else { "MasterTrack" }
}

/// New pointee ids for everything in `e`; the automation pointing at them follows.
fn renumber(e: &mut Element, nxt: &mut i64) {
    let mut map: HashMap<String, String> = HashMap::new();
    e.walk_mut(&mut |x| {
        if studio::is_pointee(&x.tag)
            && let Some(old) = x.get("Id").map(str::to_string)
        {
            map.insert(old, nxt.to_string());
            x.set("Id", nxt.to_string());
            *nxt += 1;
        }
    });
    e.walk_mut(&mut |x| {
        if x.tag == "PointeeId"
            && let Some(n) = x.get("Value").and_then(|v| map.get(v)).cloned()
        {
            x.set("Value", n);
        }
    });
}

/// Clips (arrangement, session, takes, frozen), automation: gone. The freeze is off.
fn strip_playing(t: &mut Element) -> usize {
    let n = t.remove_where(&mut |_, c| matches!(c.tag.as_str(), "MidiClip" | "AudioClip"));
    t.walk_mut(&mut |x| {
        if x.tag == "AutomationEnvelopes"
            && let Some(env) = x.child_mut("Envelopes")
        {
            env.children.clear();
        }
    });
    t.set_value("Freeze", "false");
    n
}

/// Every list of scene slots gets the new set's number of scenes: a track's ClipSlotList, a
/// group's Slots (one GroupTrackSlot per scene). One slot too many or too few and Live
/// crashes on opening ("a serious program error").
fn fit_slots(t: &mut Element, scenes: usize) {
    t.walk_mut(&mut |x| {
        if !is_slot_list(x) || x.children.is_empty() {
            return;
        }
        let model = x.children[0].clone();
        x.children.truncate(scenes);
        while x.children.len() < scenes {
            x.children.push(model.clone());
        }
        for (i, s) in x.children.iter_mut().enumerate() {
            if s.get("Id").is_some() {
                s.set("Id", i.to_string());
            }
        }
    });
}

pub fn is_slot_list(x: &Element) -> bool {
    x.tag == "ClipSlotList" || (x.tag == "Slots" && x.children.iter().all(|c| c.tag == "GroupTrackSlot"))
}

/// Where a routing points, as Live writes it: "AudioIn/Track.12/PostFxOut" -> 12.
fn routed_track(v: &str) -> Option<&str> {
    let i = v.find("/Track.")? + 7;
    let rest = &v[i..];
    Some(rest.split('/').next().unwrap_or(rest))
}

/// Copies what the frame's own tracks used for this routing element, else a neutral value.
fn reset_routing(t: &mut Element, name: &str, model: Option<&Element>, fallback: (&str, &str, &str)) {
    let Some(dc) = t.child_mut("DeviceChain") else { return };
    let Some(i) = dc.children.iter().position(|c| c.tag == name) else { return };
    match model {
        Some(m) => {
            let tail = dc.children[i].tail.clone();
            dc.children[i] = m.clone();
            dc.children[i].tail = tail;
        }
        None => {
            let r = &mut dc.children[i];
            r.set_value("Target", fallback.0);
            r.set_value("UpperDisplayString", fallback.1);
            r.set_value("LowerDisplayString", fallback.2);
        }
    }
}

/// The files a track's devices load. Those inside the source project's folder are copied into
/// the new project at the same place (Samples/..., Presets/.../Imported/...), where Live looks
/// for a project's own files. Everything else is left as the source set has it: Live finds a
/// pack or library file by itself (a copy of this machine's version of it is refused: not the
/// file the set knew), and a sample library on another drive stays where it is.
struct Collect {
    env: als::Env,
    proj: PathBuf,
    /// source file -> its place in the new project
    done: HashMap<PathBuf, String>,
    copied: usize,
    missing: Vec<String>,
}

impl Collect {
    fn fix(&mut self, e: &mut Element, src_set: &Path) {
        let src_proj = als::project_root(src_set);
        let src_dir = src_set.parent().map(Path::to_path_buf).unwrap_or_default();
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let src_root = canon(&src_proj);
        e.walk_mut(&mut |fr| {
            if fr.tag != "FileRef" {
                return;
            }
            let path = fr.value("Path").unwrap_or("").to_string();
            let rel = fr.value("RelativePath").unwrap_or("").replace('\\', "/");
            let shown = if path.is_empty() { rel.clone() } else { path.clone() };
            let low = shown.to_lowercase();
            if shown.is_empty() || !SAMPLE_EXT.iter().any(|x| low.ends_with(x)) {
                return;
            }
            let (_, mut cands) = als::path_candidates(&path, &rel, fr.value("RelativePathType"), &src_proj, &self.env);
            if !rel.is_empty() {
                cands.push(src_dir.join(&rel));
            }
            let Some(src) = cands.into_iter().find(|c| !c.as_os_str().is_empty() && als::is_file(c)) else {
                if !self.missing.contains(&shown) {
                    self.missing.push(shown);
                }
                return;
            };
            // outside the source project: Live finds it as it found it there
            let Ok(inside) = canon(&src).strip_prefix(&src_root).map(Path::to_path_buf) else { return };
            if !self.done.contains_key(&src) {
                let inside_s = inside.to_string_lossy().replace('\\', "/");
                let (dir, name) = match inside_s.rsplit_once('/') {
                    Some((d, n)) => (d.to_string(), n.to_string()),
                    None => (String::new(), inside_s.clone()),
                };
                let (stem, ext) = match name.rsplit_once('.') {
                    Some((a, b)) if !a.is_empty() => (a.to_string(), format!(".{b}")),
                    _ => (name.clone(), String::new()),
                };
                let size = std::fs::metadata(&src).map(|m| m.len()).ok();
                let at = |n: &str| if dir.is_empty() { n.to_string() } else { format!("{dir}/{n}") };
                let mut target_rel = at(&name);
                let mut i = 2;
                while self.proj.join(&target_rel).exists() && std::fs::metadata(self.proj.join(&target_rel)).map(|m| m.len()).ok() != size {
                    target_rel = at(&format!("{stem} ({i}){ext}"));
                    i += 1;
                }
                let target = self.proj.join(&target_rel);
                if !target.exists() {
                    let _ = std::fs::create_dir_all(target.parent().unwrap_or(&self.proj));
                    if crate::safe::copy_new(&src, &target).is_err() {
                        self.missing.push(shown);
                        return;
                    }
                    self.copied += 1;
                }
                self.done.insert(src.clone(), target_rel);
            }
            let rel = self.done[&src].clone();
            let abs = self.proj.join(&rel).to_string_lossy().replace('\\', "/");
            // relative to the set's own folder (1), not to "the project" (3): Live takes a
            // folder for a project only once it holds "Ableton Project Info", which a new set
            // does not have yet, and then does not find a project-relative file (it said
            // "could not be opened" for every copy)
            for (tag, val) in [("RelativePathType", "1".to_string()), ("RelativePath", rel), ("Path", abs)] {
                if let Some(el) = fr.child_mut(tag) {
                    el.set("Value", val);
                }
            }
        });
    }
}

/// A device's name as the user sees it: the name given, else the plugin's, else Live's.
pub fn device_name(d: &Element) -> String {
    if let Some(u) = d.value("UserName").filter(|u| !u.trim().is_empty()) {
        return u.trim().to_string();
    }
    for p in ["PluginDesc/VstPluginInfo/PlugName", "PluginDesc/Vst3PluginInfo/Name", "PluginDesc/AuPluginInfo/Name"] {
        if let Some(n) = d.value(p).filter(|n| !n.is_empty()) {
            return n.to_string();
        }
    }
    als::native::native_name(&d.tag).to_string()
}

static MIX: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)^(eq eight|eq three|channel eq|compressor|glue compressor|limiter|gate|utility|multiband dynamics|spectrum|tuner)$|pro-?q|pro-?c\b|pro-?l\b|pro-?mb|pro-?g\b|pro-?ds|pultec|\b4k\b|harrison|\bssl\b|neve|1176|la-?2a|channel ?strip|tonal balance|vision 4x|soundid|sonarworks|reference ?4|levels|ozone|standardclip|clipper|gullfoss|soothe|\bspan\b|insight|youlean|metric ab|\beq\b|equali[sz]er|limiter|compressor",
    )
    .unwrap()
});

/// A tool of a particular mix (EQ, dynamics, gain, metering), not part of the sound: left out
/// of a clean start. A device the user renamed ("OTT" on a Multiband Dynamics) is a sound.
pub fn is_mix_tool(name: &str) -> bool {
    MIX.is_match(name.trim())
}

/// A device switched off ("On" off) plays no part in the sound.
fn is_off(d: &Element) -> bool {
    d.find("On/Manual").and_then(|m| m.get("Value")) == Some("false")
}

/// Finds a native device by its tag: in the main chains first, then on the tracks.
fn find_device(sets: &[&Set], tag: &str) -> Option<Element> {
    let usable = |ds: &Element| ds.children.iter().find(|d| d.tag == tag && !is_off(d)).cloned();
    for s in sets {
        let mt = main_tag(&s.root);
        if let Some(d) = ls(&s.root).child(mt).and_then(|m| m.find("DeviceChain/DeviceChain/Devices")).and_then(usable) {
            return Some(d);
        }
    }
    for s in sets {
        for t in ls(&s.root).child("Tracks").map(|t| t.children.as_slice()).unwrap_or(&[]) {
            if let Some(d) = t.find("DeviceChain/DeviceChain/Devices").and_then(usable) {
                return Some(d);
            }
        }
    }
    None
}

/// The devices' own settings back to neutral where it is safe to say what neutral is: no
/// gain added (Utility, Limiter), the Utility not muting, at full width.
fn neutral_device(d: &mut Element) {
    for (p, v) in [("Gain/Manual", "0"), ("Mute/Manual", "false"), ("StereoWidth/Manual", "1"), ("On/Manual", "true")] {
        d.set_value(p, v);
    }
}

fn db_to_live(db: f64) -> String {
    format!("{}", (10f64.powf(db / 20.0) * 1e10).round() / 1e10)
}

/// Builds the set. Returns a report; nothing is written when it fails.
pub fn assemble(o: &Order) -> Result<Value, String> {
    if o.out.exists() {
        return Err(i18n::tr!("repair.exists", name = o.out.display()));
    }
    let mut base = read_set(&o.base)?;
    let live_gen = generation(&base.root);
    let base_name = live_name(&base.root);
    let mut sets: HashMap<PathBuf, Set> = HashMap::new();
    let load = |p: &Path, sets: &mut HashMap<PathBuf, Set>| -> Result<(), String> {
        if !sets.contains_key(p) {
            let s = read_set(p)?;
            if generation(&s.root) != live_gen {
                return Err(i18n::tr!(
                    "repair.otherLive",
                    name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    live = live_name(&s.root),
                    base = base_name
                ));
            }
            sets.insert(p.to_path_buf(), s);
        }
        Ok(())
    };
    for p in &o.picks {
        load(&p.set, &mut sets)?;
    }
    let mut donor_order: Vec<PathBuf> = Vec::new();
    for d in o.donors.iter().chain(o.picks.iter().map(|p| &p.set)) {
        if load(d, &mut sets).is_ok() && !donor_order.contains(d) {
            donor_order.push(d.clone());
        }
    }

    // what the frame shows: its routings, its number of scenes
    let old: Vec<Element> = ls(&base.root).child("Tracks").map(|t| t.children.clone()).unwrap_or_default();
    let plain = |tag: &str| old.iter().find(|t| t.tag == tag).cloned();
    let out_routing = old
        .iter()
        .filter(|t| t.tag != "ReturnTrack")
        .filter_map(|t| t.find("DeviceChain/AudioOutputRouting"))
        .find(|r| matches!(r.value("Target"), Some("AudioOut/Main" | "AudioOut/Master")))
        .cloned();
    let midi_in = plain("MidiTrack").and_then(|t| t.find("DeviceChain/MidiInputRouting").cloned());
    let audio_in = plain("AudioTrack").and_then(|t| t.find("DeviceChain/AudioInputRouting").cloned());
    let scenes = ls(&base.root).child("Scenes").map(|s| s.children.len()).unwrap_or(0);
    let main_out = if main_tag(&base.root) == "MainTrack" { ("AudioOut/Main", "Main") } else { ("AudioOut/Master", "Master") };
    let locator_model = ls(&base.root)
        .find("Locators/Locators")
        .and_then(|l| l.children.first())
        .cloned()
        .or_else(|| sets.values().find_map(|s| ls(&s.root).find("Locators/Locators").and_then(|l| l.children.first()).cloned()));

    // the main track's devices, found before the frame is emptied (it is a donor too)
    let mut main_devs: Vec<Element> = Vec::new();
    let mut main_missing: Vec<String> = Vec::new();
    {
        let mut donors: Vec<&Set> = vec![&base];
        donors.extend(donor_order.iter().map(|p| &sets[p]));
        for tag in &o.main_devices {
            match find_device(&donors, tag) {
                Some(mut d) => {
                    neutral_device(&mut d);
                    main_devs.push(d);
                }
                None => main_missing.push(tag.clone()),
            }
        }
    }

    // the frame: no track, no return, no locator, scenes without name or tempo, the main
    // track without devices or automation
    let mut next_track = old.iter().filter_map(|t| tid(t).parse::<i64>().ok()).max().unwrap_or(0) + 1;
    {
        let l = base.root.child_mut("LiveSet").unwrap();
        if let Some(t) = l.child_mut("Tracks") {
            t.children.clear();
        }
        if let Some(sp) = l.child_mut("SendsPre") {
            sp.children.clear();
        }
        if let Some(loc) = l.find_mut("Locators/Locators") {
            loc.children.clear();
        }
        if let Some(sc) = l.child_mut("Scenes") {
            for s in &mut sc.children {
                s.set_value("Name", "");
                s.set_value("IsTempoEnabled", "false");
                s.set_value("IsTimeSignatureEnabled", "false");
            }
        }
        for tr in ["MainTrack", "MasterTrack", "PreHearTrack"] {
            if let Some(t) = l.child_mut(tr) {
                if let Some(s) = t.find_mut("DeviceChain/Mixer/Sends") {
                    s.children.clear();
                }
                if let Some(e) = t.find_mut("AutomationEnvelopes/Envelopes") {
                    e.children.clear();
                }
            }
        }
        l.set_value("Transport/LoopOn", "false");
        l.set_value("Transport/CurrentTime", "0");
    }
    let mut nxt = studio::next_pointee_id(&base.root);
    let proj = o.out.parent().ok_or_else(|| i18n::tr!("repair.badOutDir"))?.to_path_buf();
    let mut col = Collect { env: als::Env::detect(None), proj: proj.clone(), done: HashMap::new(), copied: 0, missing: Vec::new() };
    let (mut added, mut clips, mut cut, mut off, mut mix) = (Vec::<Value>::new(), 0usize, 0usize, 0usize, 0usize);
    let mut new_tracks: Vec<Element> = Vec::new();
    let volume = db_to_live(o.volume_db);

    for p in &o.picks {
        let src = &sets[&p.set];
        let mut t = ls(&src.root)
            .child("Tracks")
            .and_then(|ts| ts.children.iter().find(|t| tid(t) == p.track))
            .cloned()
            .ok_or_else(|| i18n::tr!("repair.trackNotFoundIn", track = p.track, set = p.set.display()))?;
        if !matches!(t.tag.as_str(), "MidiTrack" | "AudioTrack") {
            return Err(i18n::tr!("repair.groupOrReturn", name = studio::track_name(&t)));
        }
        clips += strip_playing(&mut t);
        fit_slots(&mut t, scenes);
        if let Some(s) = t.find_mut("DeviceChain/Mixer/Sends") {
            s.children.clear();
        }
        if let Some(ds) = t.find_mut("DeviceChain/DeviceChain/Devices") {
            let before = ds.children.len();
            ds.children.retain(|d| !is_off(d));
            off += before - ds.children.len();
            if !o.keep_mix {
                let before = ds.children.len();
                ds.children.retain(|d| !is_mix_tool(&device_name(d)));
                mix += before - ds.children.len();
            }
        }
        t.set("Id", next_track.to_string());
        next_track += 1;
        t.set_value("TrackGroupId", "-1");
        t.set_value("LinkedTrackGroupId", "-1");
        // the mixer flat
        t.set_value("DeviceChain/Mixer/Speaker/Manual", "true");
        t.set_value("DeviceChain/Mixer/SoloSink", "false");
        t.set_value("DeviceChain/Mixer/Pan/Manual", "0");
        t.set_value("DeviceChain/Mixer/Volume/Manual", volume.clone());
        if let Some(n) = p.name.as_deref().filter(|n| !n.trim().is_empty()) {
            t.set_value("Name/UserName", n.trim());
            t.set_value("Name/EffectiveName", n.trim());
        }
        if let Some(c) = p.color {
            t.set_value("Color", c.to_string());
        }
        reset_routing(&mut t, "AudioOutputRouting", out_routing.as_ref(), (main_out.0, main_out.1, ""));
        for (name, model, fb) in [
            ("MidiInputRouting", midi_in.as_ref(), ("MidiIn/External.All/-1", "All Ins", "")),
            ("AudioInputRouting", audio_in.as_ref(), ("AudioIn/External/S0", "Ext. In", "1/2")),
        ] {
            let from_track =
                t.find(&format!("DeviceChain/{name}/Target")).and_then(|x| x.get("Value")).is_some_and(|v| routed_track(v).is_some());
            if from_track {
                reset_routing(&mut t, name, model, fb);
            }
        }
        // a sidechain listened to a track of the old set
        t.walk_mut(&mut |x| {
            if x.tag == "Target" && x.get("Value").is_some_and(|v| v.starts_with("AudioIn/") && routed_track(v).is_some()) {
                x.set("Value", "AudioIn/None");
                cut += 1;
            }
        });
        renumber(&mut t, &mut nxt);
        col.fix(&mut t, &p.set);
        added.push(json!({"name": studio::track_name(&t), "from": p.set.file_stem().map(|s| s.to_string_lossy().into_owned())}));
        new_tracks.push(t);
    }
    tracks_mut(&mut base.root).extend(new_tracks);

    // the main track's devices
    {
        let bt = main_tag(&base.root);
        let devs = base
            .root
            .child_mut("LiveSet")
            .unwrap()
            .child_mut(bt)
            .and_then(|t| t.find_mut("DeviceChain/DeviceChain/Devices"))
            .ok_or_else(|| i18n::tr!("repair.noBaseMasterChain"))?;
        devs.children.clear();
        for (i, mut d) in main_devs.into_iter().enumerate() {
            renumber(&mut d, &mut nxt);
            d.set("Id", i.to_string());
            devs.children.push(d);
        }
    }

    // locators
    let mut placed = 0;
    if let Some(model) = locator_model {
        let l = base.root.child_mut("LiveSet").unwrap().find_mut("Locators/Locators").ok_or_else(|| i18n::tr!("repair.noLocators"))?;
        for (i, (beat, name)) in o.markers.iter().enumerate() {
            let mut m = model.clone();
            m.set("Id", i.to_string());
            m.set_value("Time", format!("{beat}"));
            m.set_value("Name", name.clone());
            m.set_value("Annotation", "");
            m.set_value("IsSongStart", "false");
            l.children.push(m);
            placed += 1;
        }
    }
    base.root.child_mut("LiveSet").unwrap().set_value("NextPointeeId", nxt.to_string());

    let mut opt = serde_json::Map::new();
    if let Some(t) = o.tempo {
        opt.insert("tempo".into(), json!(t));
    }
    if let Some(s) = &o.scale {
        opt.insert("scale".into(), json!(s));
    }
    let tpl = studio::template(&mut base.root, &Value::Object(opt));

    std::fs::create_dir_all(&proj).map_err(|e| e.to_string())?;
    write_set(&base, &o.out)?;
    Ok(json!({
        "out": o.out.to_string_lossy(),
        "live": base_name,
        "added": added,
        "clips_left": clips,
        "devices_off_removed": off,
        "mix_tools_removed": mix,
        "sidechains_cut": cut,
        "main_missing": main_missing,
        "markers": placed,
        "samples_copied": col.copied,
        "samples_missing": col.missing,
        "template": tpl,
    }))
}

#[cfg(test)]
mod tests {
    use super::is_mix_tool;

    #[test]
    fn mix_tools_and_sounds() {
        for m in ["EQ Eight", "Utility", "Glue Compressor", "FabFilter Pro-Q 3", "UAD 4K Channel Strip", "UAD Harrison 32C SE", "UAD Pultec-Pro Legacy", "StandardCLIP", "Limiter", "Tonal Balance Control 2"] {
            assert!(is_mix_tool(m), "{m}");
        }
        for s in ["Serum_x64", "Drum Rack", "OTT", "Saturator", "Drum Buss", "Reverb", "Delay", "FabFilter Saturn", "Roar", "Faturator", "Addictive Drums 2", "InstrumentGroupDevice", "Arpeggiator", "Spire-1.5", "The Drop 64", "Convolution Reverb"] {
            assert!(!is_mix_tool(s), "{s}");
        }
    }
}

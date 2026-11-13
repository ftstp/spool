//! The reader: one .als file -> AlsInfo.
use crate::au::state_kind;
use crate::key::estimate_key;
use crate::model::{AlsInfo, Device, MissingSample, PluginRef, TrackInfo};
use crate::native::{is_container, is_native_instrument, native_name};
use crate::presets::preset_of;
use crate::samples::{Env, candidates, is_file, old_path, project_root};
use crate::xml::{N, attr, child, children, find_desc, find_desc_path, iter, iter_tag, or, path, round2, tag, val};
use crate::{NOTE_NAMES, SAMPLE_EXT};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

/// Live 12 saves the scale as an index into its scale list (the first ones are certain).
const LIVE12_SCALES: [&str; 7] = ["Major", "Minor", "Dorian", "Mixolydian", "Lydian", "Phrygian", "Locrian"];

/// Reads a set from disk. Never fails: a broken file comes back with `error` set.
pub fn read_als(p: &Path, env: &Env) -> AlsInfo {
    let mut raw = Vec::new();
    let res = std::fs::File::open(p).and_then(|f| flate2::read::MultiGzDecoder::new(f).read_to_end(&mut raw));
    match res {
        Ok(_) => read_als_bytes(&raw, p, env),
        Err(e) => AlsInfo { error: Some(truncate(&format!("{e}"), 200)), ..Default::default() },
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Reads an already decompressed set. `p` is where it lives (for project-relative samples).
pub fn read_als_bytes(xml: &[u8], p: &Path, env: &Env) -> AlsInfo {
    let mut out = AlsInfo::default();
    let text = match std::str::from_utf8(xml) {
        Ok(t) => t,
        Err(e) => return AlsInfo { error: Some(truncate(&format!("not UTF-8: {e}"), 200)), ..Default::default() },
    };
    let opts = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX };
    let doc = match roxmltree::Document::parse_with_options(text, opts) {
        Ok(d) => d,
        Err(e) => return AlsInfo { error: Some(truncate(&format!("{e}"), 200)), ..Default::default() },
    };
    let root = doc.root_element();
    out.live_version = attr(root, "Creator").map(|c| c.replace("Ableton Live ", "")).filter(|s| !s.is_empty());
    out.format = attr(root, "MinorVersion").map(|v| match attr(root, "SchemaChangeCount") {
        Some(c) => format!("{v}/{c}"),
        None => v.to_string(),
    });
    let Some(ls) = child(root, "LiveSet") else {
        out.error = Some(i18n::tr!("als.noLiveSet"));
        return out;
    };
    let main = child(ls, "MainTrack").or_else(|| child(ls, "MasterTrack"));
    if let Some(locs) = path(ls, "Locators/Locators") {
        out.locators = children(locs)
            .filter(|l| tag(*l) == "Locator")
            .filter_map(|l| Some((val(Some(l), "Time")?.trim().parse::<f64>().ok()?, val(Some(l), "Name").unwrap_or("").trim().to_string())))
            .collect();
        out.locators.sort_by(|a, b| a.0.total_cmp(&b.0));
    }

    let tempo = val(main.and_then(|m| find_desc_path(m, "Mixer", "Tempo")), "Manual");
    if let Some(t) = tempo.filter(|t| !t.is_empty()) {
        out.tempo = t.trim().parse::<f64>().ok().map(round2);
    }
    out.time_sig = time_signature(val(main.and_then(|m| find_desc_path(m, "Mixer", "TimeSignature")), "Manual"));

    if val(Some(ls), "InKey") == Some("true") {
        // Live 11: RootNote + scale name; Live 12: Root + scale index
        let si = child(ls, "ScaleInformation");
        let root_note = val(si, "RootNote").or_else(|| val(si, "Root"));
        let mut name = val(si, "Name").unwrap_or("").to_string();
        if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
            name = name.parse::<usize>().ok().and_then(|i| LIVE12_SCALES.get(i)).copied().unwrap_or("").to_string();
        }
        if let Some(r) = root_note {
            let digits = r.trim_start_matches('-');
            if !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
                && let Ok(n) = r.parse::<i64>()
            {
                out.key = Some(format!("{} {}", NOTE_NAMES[n.rem_euclid(12) as usize], name).trim().to_string());
            }
        }
    }

    if let Some(tracks) = child(ls, "Tracks") {
        for t in children(tracks) {
            let k = match tag(t) {
                "AudioTrack" => "audio",
                "MidiTrack" => "midi",
                "GroupTrack" => "groupes",
                "ReturnTrack" => "retours",
                _ => continue,
            };
            *out.tracks.entry(k.to_string()).or_insert(0) += 1;
        }
        // Arrangement length: end of the last clip placed in the arrangement, in beats.
        let mut end = 0.0f64;
        for arr in iter_tag(tracks, "ArrangerAutomation") {
            for ev in iter_tag(arr, "CurrentEnd") {
                if let Some(v) = ev.attribute("Value").and_then(|v| v.trim().parse::<f64>().ok()) {
                    end = end.max(v);
                }
            }
        }
        if end != 0.0
            && let Some(t) = out.tempo.filter(|t| *t != 0.0)
        {
            out.length_s = Some((end * 60.0 / t).round_ties_even() as i64);
        }
        let proj = project_root(p);
        out.track_info = children(tracks).filter_map(|t| track_info(t, &proj, env)).collect();
        let k = estimate_key(tracks);
        out.key_est = Some(k.key);
        out.key_conf = Some(k.conf);
        out.key_bass = Some(k.bass);
    }

    // Devices: natives by tag, plugins by their info block. Counted once per instance.
    let mut devs: Vec<Device> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for chain in iter_tag(ls, "Devices") {
        for d in children(chain) {
            let t = tag(d);
            if is_container(t) {
                continue;
            }
            let name = native_name(t).to_string();
            let kind = if is_native_instrument(t) {
                "instr"
            } else if t.starts_with("Midi") {
                "midifx"
            } else {
                "audiofx"
            };
            match at.get(&name) {
                Some(&i) => devs[i].count += 1,
                None => {
                    at.insert(name.clone(), devs.len());
                    devs.push(Device { name, native: true, kind: kind.into(), count: 1 });
                }
            }
        }
    }
    devs.sort_by_key(|d| std::cmp::Reverse(d.count)); // stable: ties keep their first-seen order
    out.devices = devs;
    // the envelopes' targets, to tell which plugins are automated
    let moved: HashSet<&str> = iter_tag(ls, "EnvelopeTarget").filter_map(|t| val(Some(t), "PointeeId")).collect();
    out.plugins = plugin_refs(ls);
    let devs: Vec<N> = iter(ls).filter(|n| matches!(tag(*n), "PluginDevice" | "AuPluginDevice")).collect();
    // plugin_refs lists VST2s, then VST3s, then AUs, each in document order: so do these
    let order = |d: &N| match find_desc(*d, "VstPluginInfo").map(|_| 0).or(find_desc(*d, "Vst3PluginInfo").map(|_| 1)) {
        Some(k) if tag(*d) == "PluginDevice" => k,
        _ => 2,
    };
    let mut sorted = devs.clone();
    sorted.sort_by_key(order); // stable
    if sorted.len() == out.plugins.len() && !moved.is_empty() {
        for (r, d) in out.plugins.iter_mut().zip(&sorted) {
            r.automated = iter(*d)
                .filter(|n| matches!(tag(*n), "AutomationTarget" | "ModulationTarget"))
                .filter_map(|n| attr(n, "Id"))
                .any(|id| moved.contains(id));
        }
    }

    if let Some(chain) = main.and_then(|m| child(m, "DeviceChain")) {
        let mut names: Vec<String> = plugin_refs(chain).into_iter().map(|r| r.name).collect();
        for devices in iter_tag(chain, "Devices") {
            names.extend(children(devices).filter(|d| !is_container(tag(*d))).map(|d| native_name(tag(d)).to_string()));
        }
        out.main_chain = names;
    }

    // Words the user typed (track, clip and locator names, info texts): clues for the style.
    let mut names: Vec<Option<&str>> = Vec::new();
    for el in iter(ls) {
        match tag(el) {
            "AudioTrack" | "MidiTrack" | "GroupTrack" | "ReturnTrack" => {
                let n = child(el, "Name");
                names.push(val(n, "UserName"));
                names.push(val(n, "Annotation"));
            }
            "AudioClip" | "MidiClip" | "Locator" => {
                names.push(val(Some(el), "Name"));
                names.push(val(Some(el), "Annotation"));
            }
            _ => {}
        }
    }
    names.push(val(Some(ls), "Annotation"));
    let mut seen = HashSet::new();
    out.names = names
        .into_iter()
        .flatten()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
        .map(|n| n.chars().take(80).collect::<String>())
        .filter(|n| seen.insert(n.clone()))
        .take(150)
        .collect();

    // Samples and Max for Live devices are FileRefs. Preset refs (.adg/.adv/.alc) are only
    // "last loaded preset" breadcrumbs, a missing one changes nothing, so skipped.
    let proj = project_root(p);
    let mut seen = HashSet::new();
    for fr in iter_tag(root, "FileRef") {
        let (shown, cands) = candidates(fr, &proj, env);
        let low = shown.to_lowercase();
        if shown.is_empty() || seen.contains(&shown) || !SAMPLE_EXT.iter().any(|e| low.ends_with(e)) {
            continue;
        }
        seen.insert(shown.clone());
        out.samples_total += 1;
        if out.sample_names.len() < 150 {
            let file = shown.rsplit(['\\', '/']).next().unwrap_or("");
            let stem = file.rsplit_once('.').map(|(a, _)| a).unwrap_or(file);
            out.sample_names.push(stem.to_string());
        }
        if !cands.iter().any(|c| is_file(c)) {
            let size = val(Some(fr), "OriginalFileSize").filter(|s| !s.is_empty()).and_then(|s| s.trim().parse().ok());
            let found_at = old_path(&shown, env.old_drive.as_deref())
                .filter(|o| is_file(o))
                .map(|o| o.to_string_lossy().to_string());
            out.samples_missing.push(MissingSample { path: shown, size, found_at });
        }
    }
    out
}

/// What one track is made of. Its devices only: Track/DeviceChain/DeviceChain/Devices.
fn track_info(t: N, proj: &Path, env: &Env) -> Option<TrackInfo> {
    let kind = match tag(t) {
        "AudioTrack" => "audio",
        "MidiTrack" => "midi",
        "GroupTrack" => "group",
        "ReturnTrack" => "return",
        _ => return None,
    };
    let devices = path(t, "DeviceChain/DeviceChain/Devices");
    let dev_name = |d: N| -> String {
        let user = val(Some(d), "UserName").filter(|s| !s.trim().is_empty());
        if let Some(u) = user {
            return u.trim().to_string();
        }
        if let Some(r) = plugin_refs(d).into_iter().next().filter(|_| matches!(tag(d), "PluginDevice" | "AuPluginDevice")) {
            return r.name;
        }
        native_name(tag(d)).to_string()
    };
    let chain: Vec<N> = devices.map(|ds| children(ds).collect()).unwrap_or_default();
    let mut plugins: Vec<String> = Vec::new();
    let (mut samples, mut missing, mut sidechain) = (0u32, 0u32, false);
    if let Some(ds) = devices {
        for r in plugin_refs(ds) {
            if !plugins.contains(&r.name) {
                plugins.push(r.name);
            }
        }
        let mut seen = HashSet::new();
        for fr in iter_tag(ds, "FileRef") {
            let (shown, cands) = candidates(fr, proj, env);
            let low = shown.to_lowercase();
            if shown.is_empty() || !SAMPLE_EXT.iter().any(|e| low.ends_with(e)) || !seen.insert(shown) {
                continue;
            }
            samples += 1;
            if !cands.iter().any(|c| is_file(c)) {
                missing += 1;
            }
        }
        sidechain = iter_tag(ds, "Target").any(|x| x.attribute("Value").is_some_and(|v| v.contains("/Track.")));
    }
    let instrument = if kind == "midi" {
        // MIDI effects come first; the instrument is the first device that is not one
        chain.iter().find(|d| !tag(**d).starts_with("Midi")).map(|d| dev_name(*d))
    } else {
        None
    };
    let mut notes = 0u32;
    let mut audio_clips = 0u32;
    for c in crate::role::clips(t) {
        if tag(c) == "AudioClip" {
            audio_clips += 1;
        } else {
            notes += iter_tag(c, "MidiNoteEvent").count() as u32;
        }
    }
    Some(TrackInfo {
        id: attr(t, "Id").unwrap_or("").to_string(),
        kind: kind.into(),
        name: crate::role::track_name(t),
        role: serde_json::to_value(crate::role::track_role(t)).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
        group: val(Some(t), "TrackGroupId").filter(|g| *g != "-1").unwrap_or("").to_string(),
        color: val(Some(t), "Color").and_then(|c| c.trim().parse().ok()).unwrap_or(0),
        chain: chain.iter().map(|d| dev_name(*d)).collect(),
        plugins,
        instrument,
        samples,
        samples_missing: missing,
        volume: val(path(t, "DeviceChain/Mixer/Volume"), "Manual").and_then(|v| v.trim().parse().ok()),
        notes,
        audio_clips,
        sidechain,
    })
}

fn time_signature(v: Option<&str>) -> Option<String> {
    // Live encodes it as 99 * log2(denominator) + numerator - 1 (4/4 -> 201).
    let v: i64 = v?.trim().parse().ok()?;
    Some(format!("{}/{}", v.rem_euclid(99) + 1, 2i64.checked_pow(v.div_euclid(99) as u32)?))
}

fn vst3_uid(info: N) -> Option<String> {
    let uid = path(info, "Preset/Vst3Preset/Uid").or_else(|| child(info, "Uid"))?;
    let fields: Vec<u32> = children(uid)
        .map(|f| f.attribute("Value").and_then(|v| v.trim().parse::<i64>().ok()).map(|v| (v & 0xFFFF_FFFF) as u32))
        .collect::<Option<Vec<_>>>()?;
    if fields.len() != 4 {
        return None;
    }
    let h: String = fields.iter().map(|x| format!("{x:08x}")).collect();
    Some(format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
}

fn plugin_refs(el: N) -> Vec<PluginRef> {
    let mut refs = Vec::new();
    for info in iter_tag(el, "VstPluginInfo") {
        let name = val(Some(info), "PlugName").unwrap_or("").to_string();
        refs.push(PluginRef {
            format: "vst2".into(),
            id: val(Some(info), "UniqueId").unwrap_or("").to_lowercase(),
            path: or(val(Some(info), "Path"), val(Some(info), "Dir")).unwrap_or("").to_string(),
            version: Some(val(Some(info), "Version").unwrap_or("").to_string()),
            file: Some(val(Some(info), "FileName").unwrap_or("").to_string()),
            preset: preset_of(info, "vst2", &name),
            au_state: None,
            automated: false,
            name,
        });
    }
    for info in iter_tag(el, "Vst3PluginInfo") {
        let name = val(Some(info), "Name").unwrap_or("").to_string();
        refs.push(PluginRef {
            format: "vst3".into(),
            id: vst3_uid(info).unwrap_or_default(),
            path: String::new(),
            version: None,
            file: None,
            preset: preset_of(info, "vst3", &name),
            au_state: None,
            automated: false,
            name,
        });
    }
    for dev in iter_tag(el, "AuPluginDevice") {
        let Some(info) = find_desc(dev, "AuPluginInfo") else { continue };
        let name = val(Some(info), "Name").unwrap_or("").to_string();
        refs.push(PluginRef {
            format: "au".into(),
            id: String::new(),
            path: String::new(),
            version: None,
            file: None,
            preset: preset_of(info, "au", &name),
            au_state: Some(state_kind(info, &name, Some(dev))),
            automated: false,
            name,
        });
    }
    refs
}

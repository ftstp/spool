//! Mac (AU) plugin devices turned into the VST2 of the same plugin, for repaired copies.
//! Ported from the prototype's auconv.py.
//!
//! Windows has no AU. Live saves an AU's state as a plist, and most plugins put inside it
//! the very blob their VST2 saves (each vendor under its own key), so the VST2 can be loaded
//! with the same settings. The VST2 device itself is a TEMPLATE: a real device of that plugin
//! taken from one of the user's Windows projects, so its structure is exactly what Live
//! writes, only its state is swapped. Klanghelm (SDRR, DC8C2) keeps no blob in the AU: their
//! VST2 bank is rebuilt from the parameter values Live stored for the AU.
use crate::xml::{self, Element};
use plist::Value as P;

/// Plist keys under which a plugin family keeps its VST2 state inside the AU state.
const BLOB_KEYS: [&str; 5] = ["vstdata", "sylenth1-data", "soundtoys-data", "AM_STATE", "jucePluginState"];
const FPCH: i64 = 0x4650_4368; // Live's VstPreset Type: program chunk
const FBCH: i64 = 0x4642_4368; // bank chunk

/// Klanghelm VST2 chunk = a bank of programs, each a 28-byte name + N float32 (0..1).
/// Enumerated parameters: number of steps (the AU stores the index, the VST2 index/(steps-1)).
fn klanghelm(name: &str) -> Option<(usize, &'static [(usize, u32)])> {
    match name {
        "sdrr" => Some((
            42,
            &[
                (0, 4),
                (3, 3),
                (10, 3),
                (16, 2),
                (19, 3),
                (26, 3),
                (31, 2),
                (32, 2),
                (33, 2),
                (36, 3),
                (37, 3),
                (40, 3),
                (41, 2),
            ],
        )),
        "dc8c2" => Some((
            37,
            &[
                (1, 3),
                (3, 2),
                (12, 3),
                (18, 3),
                (19, 2),
                (21, 2),
                (22, 2),
                (23, 2),
                (24, 2),
                (25, 2),
                (26, 2),
                (33, 2),
                (34, 4),
                (35, 2),
                (36, 2),
            ],
        )),
        _ => None,
    }
}

fn unhex(e: Option<&Element>) -> Vec<u8> {
    let s: String = e.and_then(|e| e.text.as_deref()).unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Vec::new();
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// The AU state plist, None when unreadable.
pub fn au_plist(dev: &Element) -> Option<plist::Dictionary> {
    let raw = unhex(dev.find(".//AuPreset/Buffer"));
    if raw.is_empty() {
        return None;
    }
    plist::Value::from_reader(std::io::Cursor::new(raw)).ok()?.into_dictionary()
}

/// Live writes a parameter's id (and so its value) only for some parameters.
fn has_param_values(dev: &Element) -> bool {
    dev.descendants().find(|e| e.tag == "ParameterList").is_some_and(|pl| {
        pl.children.iter().any(|q| q.value("ParameterId").and_then(|v| v.parse::<i64>().ok()).unwrap_or(-1) >= 0)
    })
}

fn truthy(v: Option<&P>) -> bool {
    match v {
        None => false,
        Some(P::Boolean(b)) => *b,
        Some(P::Data(d)) => !d.is_empty(),
        Some(P::String(s)) => !s.is_empty(),
        Some(P::Integer(i)) => i.as_signed() != Some(0),
        Some(P::Real(r)) => *r != 0.0,
        Some(P::Array(a)) => !a.is_empty(),
        Some(P::Dictionary(d)) => !d.is_empty(),
        Some(_) => true,
    }
}

/// Which way this AU's state can become its VST2 state.
pub fn state_kind(pl: &plist::Dictionary, name: &str, dev: Option<&Element>) -> Option<&'static str> {
    if truthy(pl.get("data_is_vst_chunk")) && truthy(pl.get("data")) {
        return Some("chunk");
    }
    if let Some(k) = BLOB_KEYS.iter().find(|k| truthy(pl.get(k))) {
        return Some(k);
    }
    (klanghelm(&name.to_lowercase()).is_some() && dev.is_none_or(has_param_values)).then_some("klanghelm")
}

/// vstdata saved as a whole .fxp / .fxb file (Serum): the chunk inside it, its type.
fn unwrap_fxp(b: Vec<u8>) -> (Vec<u8>, Option<i64>) {
    if b.len() < 60 || &b[..4] != b"CcnK" {
        return (b, None);
    }
    let size = |at: usize| i32::from_be_bytes(b[at..at + 4].try_into().unwrap()).max(0) as usize;
    match &b[8..12] {
        b"FPCh" => {
            let n = size(56);
            (b[60..(60 + n).min(b.len())].to_vec(), Some(FPCH))
        }
        b"FBCh" if b.len() >= 160 => {
            let n = size(156);
            (b[160..(160 + n).min(b.len())].to_vec(), Some(FBCH))
        }
        _ => (b, None),
    }
}

fn f(e: Option<&Element>) -> Option<f64> {
    e.and_then(|e| e.get("Value")).and_then(|v| v.parse().ok())
}

fn klanghelm_chunk(name: &str, au: &Element, tpl: &Element) -> Option<Vec<u8>> {
    let lname = name.to_lowercase();
    let (n, enums) = klanghelm(&lname)?;
    let mut chunk = unhex(tpl.find(".//VstPreset/Buffer"));
    let size = 28 + 4 * n;
    let pn = tpl
        .find(".//VstPreset/ProgramNumber")
        .and_then(|e| e.get("Value"))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if chunk.len() < (pn + 1) * size {
        return None;
    }
    let off = pn * size + 28;
    let mut vals: Vec<f32> =
        (0..n).map(|i| f32::from_le_bytes(chunk[off + 4 * i..off + 4 * i + 4].try_into().unwrap())).collect();
    let mut found = 0;
    if let Some(pl) = au.descendants().find(|e| e.tag == "ParameterList") {
        for q in &pl.children {
            let (Some(pid), Some(man)) = (q.find("ParameterId"), q.find("ParameterValue/Manual")) else { continue };
            let Some(i) = pid.get("Value").and_then(|v| v.parse::<i64>().ok()) else { continue };
            if !(0..n as i64).contains(&i) {
                continue;
            }
            let i = i as usize;
            let mut v: f64 = man.get("Value").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            if let Some((_, steps)) = enums.iter().find(|(k, _)| *k == i) {
                v /= (*steps - 1) as f64;
            } else {
                let (mut lo, mut hi) = match (
                    f(q.find("ParameterValue/MidiControllerRange/Min")),
                    f(q.find("ParameterValue/MidiControllerRange/Max")),
                ) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (0.0, 1.0),
                };
                if hi <= lo {
                    (lo, hi) = if lname == "sdrr" && i == 39 { (-24.0, 0.0) } else { (0.0, 1.0) };
                }
                v = (v - lo) / (hi - lo);
            }
            vals[i] = v.clamp(0.0, 1.0) as f32;
            found += 1;
        }
    }
    if found == 0 {
        return None;
    }
    for (i, v) in vals.iter().enumerate() {
        chunk[off + 4 * i..off + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    Some(chunk)
}

/// (chunk, program number or None to keep the template's, chunk type or None).
type State = (Vec<u8>, Option<i64>, Option<i64>);

fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

fn bytes_of(v: &P) -> Vec<u8> {
    match v {
        P::String(s) => latin1(s),
        P::Data(d) => d.clone(),
        _ => Vec::new(),
    }
}

pub fn vst_state(au: &Element, name: &str, tpl: &Element) -> Option<State> {
    let pl = au_plist(au).unwrap_or_default();
    let kind = state_kind(&pl, name, Some(au))?;
    let prog = pl
        .get("program_num")
        .or_else(|| pl.get("ProgramNumber"))
        .and_then(|v| match v {
            P::Integer(i) => i.as_signed(),
            _ => None,
        })
        .unwrap_or(0);
    match kind {
        "chunk" => Some((bytes_of(&pl["data"]), Some(prog), None)),
        "klanghelm" => klanghelm_chunk(name, au, tpl).map(|c| (c, None, None)),
        "soundtoys-data" => {
            // same text, the VST2 writes Windows-style line ends
            let t: String = match &pl[kind] {
                P::String(s) => s.clone(),
                other => bytes_of(other).iter().map(|b| *b as char).collect(),
            };
            Some((latin1(&t.replace("\r\n", "\r").replace('\n', "\r")), Some(prog), None))
        }
        _ => {
            let v = bytes_of(&pl[kind]);
            let mut prog = prog;
            if kind == "sylenth1-data" && v.len() >= 20 && &v[..4] == b"1lys" {
                prog = i32::from_le_bytes(v[16..20].try_into().unwrap()) as i64; // its bank says which program is current
            }
            if kind == "vstdata" {
                let (v, typ) = unwrap_fxp(v);
                return Some((v, Some(prog), typ));
            }
            Some((v, Some(prog), None))
        }
    }
}

/// Writes `data` the way Live does: upper-case hex, 80 per line, indented like before.
fn set_hex(el: &mut Element, data: &[u8]) {
    let old = el.text.as_deref().unwrap_or("");
    let trimmed = old.trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
    let indent = trimmed
        .strip_prefix('\n')
        .map(|r| r.chars().take_while(|c| *c == ' ' || *c == '\t').collect::<String>())
        .unwrap_or_else(|| "\t".repeat(12));
    let h: String = data.iter().map(|b| format!("{b:02X}")).collect();
    let lines: Vec<&str> = (0..h.len()).step_by(80).map(|i| &h[i..(i + 80).min(h.len())]).collect();
    el.text = Some(if lines.is_empty() {
        String::new()
    } else {
        let mut ind = indent.clone();
        ind.pop();
        format!("\n{indent}{}\n{ind}", lines.join(&format!("\n{indent}")))
    });
}

/// The VST2 a Mac plugin becomes: its name, file and id.
#[derive(Debug, Clone)]
pub struct Target {
    pub to: String,
    pub file: String,
    pub id: String,
}

/// A PluginDevice carrying the AU's state, built on the template device; None when the state
/// cannot be carried over.
pub fn convert(au: &Element, name: &str, tpl_xml: &str, target: &Target) -> Option<Element> {
    let mut new = xml::parse(tpl_xml).ok()?;
    let (chunk, prog, typ) = vst_state(au, name, &new)?;
    if chunk.is_empty() {
        return None;
    }
    let id = au.get("Id").or(new.get("Id")).unwrap_or("0").to_string();
    new.set("Id", id);
    {
        let info = find_desc_mut(&mut new, "VstPluginInfo")?;
        info.child_mut("PlugName")?.set("Value", target.to.clone());
        if let Some(fe) = info.child_mut("FileName") {
            fe.set("Value", target.file.clone());
        }
        info.child_mut("UniqueId")?.set("Value", target.id.clone());
    }
    {
        let pre = find_desc_mut(&mut new, "VstPreset")?;
        set_hex(pre.child_mut("Buffer")?, &chunk);
        if let Some(u) = pre.child_mut("UniqueId") {
            u.set("Value", target.id.clone());
        }
        if let Some(p) = prog
            && let Some(pe) = pre.child_mut("ProgramNumber")
        {
            pe.set("Value", p.to_string());
        }
        if let Some(t) = typ
            && let Some(te) = pre.child_mut("Type")
        {
            te.set("Value", t.to_string());
        }
    }
    // the template's parameter values belong to another project: let Live read them back from
    // the plugin rather than risk them being applied over the state
    for pl in new.children.iter_mut().filter(|c| c.tag == "ParameterList") {
        pl.children.clear();
    }
    for tag in ["On/Manual", "UserName", "IsFolded"] {
        if let Some(v) = au.find(tag).and_then(|e| e.get("Value")).map(str::to_string)
            && let Some(dst) = new.find_mut(tag)
        {
            dst.set("Value", v);
        }
    }
    Some(new)
}

/// ET's `find(".//tag")`, mutable: the first descendant with this tag, in document order.
pub fn find_desc_mut<'a>(e: &'a mut Element, tag: &str) -> Option<&'a mut Element> {
    for c in e.children.iter_mut() {
        if c.tag == tag {
            return Some(c);
        }
        if let Some(found) = find_desc_mut(c, tag) {
            return Some(found);
        }
    }
    None
}

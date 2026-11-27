//! Mac (AU) plugins: which way their saved state can become the VST2's.
use crate::presets::unhex;
use crate::xml::{N, child, find_desc_path, iter_tag, text};
use plist::Value;

/// Plist keys under which plugin families keep their VST2 state inside the AU state.
const BLOB_KEYS: [&str; 5] = ["vstdata", "sylenth1-data", "soundtoys-data", "AM_STATE", "jucePluginState"];
/// Klanghelm keeps no blob: its VST2 bank is rebuilt from the parameter values Live stored.
const KLANGHELM: [&str; 2] = ["sdrr", "dc8c2"];

fn plist_of(info_or_device: N) -> Option<plist::Dictionary> {
    let buf = find_desc_path(info_or_device, "AuPreset", "Buffer")?;
    let s: String = text(buf).unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
    let raw = unhex(&s).filter(|r| !r.is_empty())?;
    match Value::from_reader(std::io::Cursor::new(raw)).ok()? {
        Value::Dictionary(d) => Some(d),
        _ => None,
    }
}

/// Python truthiness of a plist value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Boolean(b)) => *b,
        Some(Value::Integer(i)) => i.as_signed().map(|x| x != 0).unwrap_or(true),
        Some(Value::Real(r)) => *r != 0.0,
        Some(Value::Data(d)) => !d.is_empty(),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Dictionary(d)) => !d.is_empty(),
        Some(_) => true,
    }
}

/// Live writes a parameter's id (and so its value) only for some parameters.
fn has_param_values(dev: N) -> bool {
    let Some(list) = iter_tag(dev, "ParameterList").next() else { return false };
    crate::xml::children(list).any(|q| {
        child(q, "ParameterId")
            .and_then(|p| p.attribute("Value"))
            .unwrap_or("-1")
            .parse::<i64>()
            .map(|v| v >= 0)
            .unwrap_or(false)
    })
}

/// chunk / one of the blob keys / klanghelm, or None when the state cannot be converted.
pub fn state_kind(info: N, name: &str, dev: Option<N>) -> Option<String> {
    let pl = plist_of(info).unwrap_or_default();
    if truthy(pl.get("data_is_vst_chunk")) && truthy(pl.get("data")) {
        return Some("chunk".into());
    }
    for k in BLOB_KEYS {
        if truthy(pl.get(k)) {
            return Some(k.into());
        }
    }
    if KLANGHELM.contains(&name.to_lowercase().as_str()) && dev.map(has_param_values).unwrap_or(true) {
        return Some("klanghelm".into());
    }
    None
}

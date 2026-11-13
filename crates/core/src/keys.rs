//! Keys as shown: one spelling ("Fm", "G phrygien"), and which source wins.
use serde_json::Value;

/// Modes shown in French, typed in French or English (Live's names are English).
fn mode_fr(rest: &str) -> Option<&'static str> {
    Some(match rest {
        "dorian" | "dorien" => "dorien",
        "phrygian" | "phrygien" => "phrygien",
        "lydian" | "lydien" => "lydien",
        "mixolydian" | "mixolydien" => "mixolydien",
        "locrian" | "locrien" => "locrien",
        "harmonic minor" | "mineur harmonique" | "m harmonique" => "mineur harmonique",
        "melodic minor" | "mineur melodique" | "mineur mélodique" => "mineur mélodique",
        _ => return None,
    })
}

/// "C Major" / "f minor" / "F#m" / "D#" -> "C" / "Fm" / "F#m" / "D#"; "G Phrygian" ->
/// "G phrygien"; "A aeolian" -> "Am".
pub fn norm_key(k: Option<&str>) -> Option<String> {
    let k = k.filter(|k| !k.is_empty())?;
    let t = k.trim_start();
    let mut chars = t.chars();
    let note = match chars.next() {
        Some(c) if matches!(c.to_ascii_uppercase(), 'A'..='G') => c.to_ascii_uppercase(),
        _ => return Some(k.to_string()),
    };
    let rest = chars.as_str().trim_start();
    let (acc, rest) = match rest.chars().next() {
        Some(c @ ('#' | 'b')) => (c.to_string(), rest[1..].to_string()),
        _ => (String::new(), rest.to_string()),
    };
    let mut rest = rest.trim().to_lowercase();
    if let Some(r) = rest.strip_prefix("m ")
        && mode_fr(r.trim()).is_some()
    {
        rest = r.trim().to_string(); // "Dm dorien"
    }
    if matches!(rest.as_str(), "" | "maj" | "major" | "majeur" | "ionian" | "ionien") {
        return Some(format!("{note}{acc}"));
    }
    if matches!(rest.as_str(), "m" | "min" | "minor" | "mineur" | "aeolian" | "eolien" | "éolien") {
        return Some(format!("{note}{acc}m"));
    }
    Some(format!("{note}{acc} {}", mode_fr(&rest).map(str::to_string).unwrap_or(rest)))
}

/// "C" for "C", "C lydien", "C mixolydien": the keys a C-major-looking set fits.
fn tonic_major(k: &str) -> Option<String> {
    let n = norm_key(Some(k)).unwrap_or_default();
    let parts: Vec<&str> = n.split(' ').collect();
    let minorish = parts[0].ends_with('m')
        || parts[1..].iter().any(|w| matches!(*w, "dorien" | "phrygien" | "locrien" | "mineur"));
    if minorish || parts[0].is_empty() { None } else { Some(parts[0].to_string()) }
}

/// MIDI key estimates below 0.85 matched hand-written keys too rarely to be shown, unless
/// the bass confirms the tonic (from 0.6).
pub fn sure_estimate(d: &Value) -> Option<String> {
    let c = d["key_conf"].as_f64().unwrap_or(0.0);
    let bass = d["key_bass"].as_bool().unwrap_or(false);
    if c >= 0.85 || (c >= 0.6 && bass) { norm_key(d["key_est"].as_str()) } else { None }
}

/// (key, source): what the user or the folder name says, else Live's scale, else a sure MIDI
/// estimate. Live saves C major as soon as its scale mode is switched on: a Live "C major"
/// counts only when the notes agree.
pub fn song_key(user_key: Option<&str>, d: &Value) -> (Option<String>, Option<&'static str>) {
    if let Some(k) = user_key.filter(|k| !k.is_empty()) {
        return (norm_key(Some(k)), Some("noté"));
    }
    let sure = sure_estimate(d);
    if let Some(live) = d["key"].as_str().filter(|k| !k.is_empty()) {
        let n = norm_key(Some(live));
        if n.as_deref() != Some("C") || sure.as_deref().and_then(tonic_major).as_deref() == Some("C") {
            return (n, Some("Live"));
        }
    }
    match sure {
        Some(s) => (Some(s), Some("estimé")),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn norm() {
        assert_eq!(norm_key(Some("G Phrygian")).as_deref(), Some("G phrygien"));
        assert_eq!(norm_key(Some("Dm dorien")).as_deref(), Some("D dorien"));
        assert_eq!(norm_key(Some("D# Harmonic Minor")).as_deref(), Some("D# mineur harmonique"));
        assert_eq!(norm_key(Some("A aeolian")).as_deref(), Some("Am"));
        assert_eq!(norm_key(Some("f minor")).as_deref(), Some("Fm"));
    }

    #[test]
    fn live_default_c_major_needs_the_notes() {
        assert_eq!(
            song_key(None, &json!({"key": "C Major", "key_est": "Em", "key_conf": 0.9})),
            (Some("Em".into()), Some("estimé"))
        );
        assert_eq!(song_key(None, &json!({"key": "C Major"})), (None, None));
        assert_eq!(
            song_key(None, &json!({"key": "C Major", "key_est": "C", "key_conf": 0.9})),
            (Some("C".into()), Some("Live"))
        );
        assert_eq!(song_key(None, &json!({"key": "D Dorian"})), (Some("D dorien".into()), Some("Live")));
        assert_eq!(song_key(Some("Fm"), &json!({"key": "C Major"})), (Some("Fm".into()), Some("noté")));
    }
}

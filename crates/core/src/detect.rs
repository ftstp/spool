//! A song's style, key and tempo as shown: what the user wrote, else what Spool detects by
//! itself, the project first and the audio of the reference render to back it or fill in.
//! Every field says where its value comes from, the other candidates (offered on hover) and
//! whether the sources disagree. Nothing here is written: a detected value stays detected
//! until the user types or picks one.
//!
//! Measured (crates/midi, als, audio; plans in the commit history): the key from a project's
//! MIDI notes is exact ~78 % of the time, from the audio ~40 % (short loops; full mixes do
//! better), the project's tempo is the tempo, unless left at Live's default 120.
use crate::keys::norm_key;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// One value and where it comes from: "noté" (the user), "Live" (the project's scale),
/// "estimé" (the project's MIDI notes), "projet" (the project's tempo), "audio" (the
/// reference render), "auto" (the style clues: names, plugins, similar songs).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Guess {
    pub value: String,
    pub src: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Field {
    /// What is shown (the user's value if any).
    pub shown: Option<Guess>,
    /// The other candidates, best first, never the shown value again.
    pub alts: Vec<Guess>,
    /// The detections disagree: worth a look.
    pub doubt: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Detected {
    pub style: Field,
    pub key: Field,
    pub tempo: Field,
}

/// What the reference render's analysis measured (audio analysis, opt-in).
#[derive(Debug, Clone, Default)]
pub struct AudioMeasure {
    pub bpm: Option<f64>,
    /// "F# minor", "C major"
    pub key: Option<String>,
}

fn g(value: impl Into<String>, src: &str) -> Guess {
    Guess { value: value.into(), src: src.to_string() }
}

/// (pitch class, minor) of a shown key ("Fm", "C#", "G dorien" -> its tonic, minor-ish).
pub(crate) fn pitch_class(k: &str) -> Option<(usize, bool)> {
    pc(k)
}

fn pc(k: &str) -> Option<(usize, bool)> {
    let n = norm_key(Some(k))?;
    let head = n.split(' ').next()?;
    let minor = head.ends_with('m') || n.contains("dorien") || n.contains("phrygien") || n.contains("mineur") || n.contains("locrien");
    let root = head.trim_end_matches('m');
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let alt = match root {
        "Db" => "C#",
        "Eb" => "D#",
        "Gb" => "F#",
        "Ab" => "G#",
        "Bb" => "A#",
        r => r,
    };
    N.iter().position(|x| *x == alt).map(|i| (i, minor))
}

/// The same key, or its relative (A minor / C major: the same notes, often mistaken).
fn same_notes(a: &str, b: &str) -> bool {
    match (pc(a), pc(b)) {
        (Some(x), Some(y)) => x == y || (x.1 != y.1 && (if x.1 { (x.0 + 3) % 12 } else { (y.0 + 3) % 12 }) == if x.1 { y.0 } else { x.0 }),
        _ => false,
    }
}

/// Tempos that are the same groove: equal within 3 %, or half / double.
fn same_groove(a: f64, b: f64) -> bool {
    [1.0, 2.0, 0.5].iter().any(|m| ((b * m) - a).abs() / a <= 0.03)
}

fn push_alt(f: &mut Field, x: Guess) {
    let dup = f.shown.as_ref().is_some_and(|s| s.value == x.value) || f.alts.iter().any(|a| a.value == x.value);
    if !dup {
        f.alts.push(x);
    }
}

/// The key: the user's, else Live's scale (a "C major" only when the notes agree: Live saves
/// it as soon as its scale mode is on), else the MIDI notes, else the audio.
pub fn key(user: Option<&str>, d: &Value, audio: &AudioMeasure) -> Field {
    let mut f = Field::default();
    let est = d["key_est"].as_str().filter(|_| d["key_conf"].as_f64().unwrap_or(0.0) >= 0.6).and_then(|k| norm_key(Some(k)));
    let live = d["key"].as_str().filter(|k| !k.is_empty()).and_then(|k| norm_key(Some(k)));
    let live = live.filter(|l| l != "C" || est.as_deref().is_some_and(|e| pc(e) == Some((0, false))));
    let heard = audio.key.as_deref().and_then(|k| norm_key(Some(k)));
    let mut cands: Vec<Guess> = Vec::new();
    if let Some(u) = user.filter(|u| !u.trim().is_empty()) {
        cands.push(g(norm_key(Some(u)).unwrap_or_else(|| u.to_string()), "noté"));
    }
    if let Some(l) = live {
        cands.push(g(l, "Live"));
    }
    if let Some(e) = &est {
        cands.push(g(e.clone(), "estimé"));
    }
    if let Some(h) = &heard {
        cands.push(g(h.clone(), "audio"));
    }
    let mut it = cands.into_iter();
    f.shown = it.next();
    for c in it {
        push_alt(&mut f, c);
    }
    // the project's notes and the audio say different things (not just a relative key), and
    // the bass does not back the notes: the audio is right far less often than the notes
    // (~40 % against ~78 %), so a disagreement alone is mostly the audio's mistake
    if let (Some(e), Some(h)) = (&est, &heard) {
        let backed = d["key_bass"].as_bool().unwrap_or(false);
        f.doubt = !same_notes(e, h) && !backed && user.is_none_or(|u| u.trim().is_empty());
    }
    f
}

/// The tempo: the user's, else the project's, unless it sits at Live's default 120 while the
/// render says otherwise (a project nobody set). The audio stays a candidate when it is a
/// different groove.
pub fn tempo(user: Option<f64>, d: &Value, audio: &AudioMeasure) -> Field {
    let mut f = Field::default();
    let fmt = |t: f64| format!("{}", (t * 10.0).round() / 10.0);
    let project = d["tempo"].as_f64().filter(|t| *t > 0.0);
    let heard = audio.bpm.filter(|b| *b > 0.0);
    let user = user.filter(|t| *t > 0.0);
    let default_120 = project.is_some_and(|p| (p - 120.0).abs() < 0.01);
    let mut cands: Vec<Guess> = Vec::new();
    if let Some(u) = user {
        cands.push(g(fmt(u), "noté"));
    }
    match (project, heard) {
        (Some(p), Some(h)) if default_120 && !same_groove(p, h) => {
            cands.push(g(fmt(h), "audio"));
            cands.push(g(fmt(p), "projet"));
        }
        (Some(p), h) => {
            cands.push(g(fmt(p), "projet"));
            if let Some(h) = h.filter(|h| !same_groove(p, *h)) {
                cands.push(g(fmt(h), "audio"));
                f.doubt = user.is_none();
            }
        }
        (None, Some(h)) => cands.push(g(fmt(h), "audio")),
        (None, None) => {}
    }
    let mut it = cands.into_iter();
    f.shown = it.next();
    for c in it {
        push_alt(&mut f, c);
    }
    f
}

/// The style: the user's, else the best of the deduced options ((style, pct), best first).
/// Two close options: a doubt.
pub fn style(user: Option<&str>, options: &[(String, i64)]) -> Field {
    let mut f = Field::default();
    let user = user.filter(|u| !u.trim().is_empty());
    if let Some(u) = user {
        f.shown = Some(g(u, "noté"));
    }
    for (s, _) in options {
        if f.shown.is_none() {
            f.shown = Some(g(s.clone(), "auto"));
        } else {
            push_alt(&mut f, g(s.clone(), "auto"));
        }
    }
    f.doubt = user.is_none() && options.len() >= 2 && options[0].1 - options[1].1 < 15;
    f
}

/// The deduced style options of an open style suggestion (its `reasons` column).
pub fn style_options(reasons: Option<&str>) -> Vec<(String, i64)> {
    let v: Value = reasons.and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| Some((o["style"].as_str()?.to_string(), o["pct"].as_i64().unwrap_or(0))))
                .take(3)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn heard(bpm: Option<f64>, key: Option<&str>) -> AudioMeasure {
        AudioMeasure { bpm, key: key.map(str::to_string) }
    }

    #[test]
    fn the_user_wins_and_the_detections_stay_offered() {
        let f = key(Some("Fm"), &json!({"key_est": "Gm", "key_conf": 0.8}), &heard(None, Some("G minor")));
        assert_eq!(f.shown, Some(g("Fm", "noté")));
        assert_eq!(f.alts, vec![g("Gm", "estimé")], "the audio says the same as the notes: once");
        assert!(!f.doubt, "the user decided");
    }

    #[test]
    fn project_first_then_audio() {
        let f = key(None, &json!({"key_est": "Am", "key_conf": 0.7}), &heard(None, Some("C major")));
        assert_eq!(f.shown, Some(g("Am", "estimé")));
        assert_eq!(f.alts, vec![g("C", "audio")]);
        assert!(!f.doubt, "a relative key is the same notes");
        let f = key(None, &json!({"key_est": "Am", "key_conf": 0.7}), &heard(None, Some("D# major")));
        assert!(f.doubt);
        let f = key(None, &json!({"key_est": "Am", "key_conf": 0.7, "key_bass": true}), &heard(None, Some("D# major")));
        assert!(!f.doubt, "the bass backs the notes");
        // no MIDI in the project: the audio fills in
        let f = key(None, &json!({}), &heard(None, Some("F# minor")));
        assert_eq!(f.shown, Some(g("F#m", "audio")));
        // an unsure estimate is not shown
        let f = key(None, &json!({"key_est": "Am", "key_conf": 0.4}), &heard(None, None));
        assert_eq!(f.shown, None);
    }

    #[test]
    fn live_default_c_major_needs_the_notes() {
        let f = key(None, &json!({"key": "C Major", "key_est": "Em", "key_conf": 0.9}), &AudioMeasure::default());
        assert_eq!(f.shown, Some(g("Em", "estimé")));
        let f = key(None, &json!({"key": "C Major", "key_est": "C", "key_conf": 0.9}), &AudioMeasure::default());
        assert_eq!(f.shown, Some(g("C", "Live")));
        let f = key(None, &json!({"key": "D Dorian"}), &AudioMeasure::default());
        assert_eq!(f.shown, Some(g("D dorien", "Live")));
    }

    #[test]
    fn tempo_from_the_project_unless_left_at_120() {
        let f = tempo(None, &json!({"tempo": 174.0}), &heard(Some(87.0), None));
        assert_eq!(f.shown, Some(g("174", "projet")));
        assert!(f.alts.is_empty() && !f.doubt, "half time is the same groove");
        let f = tempo(None, &json!({"tempo": 120.0}), &heard(Some(92.4), None));
        assert_eq!(f.shown, Some(g("92.4", "audio")));
        assert_eq!(f.alts, vec![g("120", "projet")]);
        let f = tempo(None, &json!({"tempo": 140.0}), &heard(Some(97.0), None));
        assert_eq!(f.shown, Some(g("140", "projet")));
        assert!(f.doubt);
        let f = tempo(Some(141.0), &json!({"tempo": 140.0}), &heard(Some(97.0), None));
        assert_eq!(f.shown, Some(g("141", "noté")));
        assert!(!f.doubt);
    }

    #[test]
    fn style_auto_with_the_others_on_hover() {
        let opts = vec![("dnb".to_string(), 64), ("dubstep".to_string(), 20)];
        let f = style(None, &opts);
        assert_eq!(f.shown, Some(g("dnb", "auto")));
        assert_eq!(f.alts, vec![g("dubstep", "auto")]);
        assert!(!f.doubt);
        assert!(style(None, &[("a".into(), 40), ("b".into(), 35)]).doubt);
        let f = style(Some("neuro"), &opts);
        assert_eq!(f.shown, Some(g("neuro", "noté")));
        assert_eq!(f.alts.len(), 2);
    }
}

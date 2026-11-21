//! Spool's words in each language. The texts are NOT here: they live in the interface's
//! locale files, `web/src/lib/i18n/locales/<lang>/core.json` (the namespace of everything the
//! Rust side says: errors, undo labels, messages), embedded at build time. One set of files
//! for both sides, so a translator edits JSON and never Rust.
//!
//! ```ignore
//! use i18n::tr;
//! Err(tr!("fs.notFound", path = rel))          // "introuvable : {path}" / "not found: {path}"
//! tr!("board.added", n = 3, title = t)          // {"one": ..., "other": ...} picked by n
//! ```
//!
//! A key missing in a language falls back to French, then to the key itself (never a panic:
//! a missing word must not break an action).
use serde_json::Value;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

static FR: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("../../../web/src/lib/i18n/locales/fr/core.json")).expect("fr/core.json"));
static EN: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("../../../web/src/lib/i18n/locales/en/core.json")).expect("en/core.json"));

/// 0 French, 1 English.
static LANG: AtomicU8 = AtomicU8::new(0);

/// The language messages are written in from now on ("fr", "en"; anything else: English).
pub fn set_lang(lang: &str) {
    LANG.store(if lang.starts_with("fr") { 0 } else { 1 }, Ordering::Relaxed);
}

pub fn lang() -> &'static str {
    if LANG.load(Ordering::Relaxed) == 0 { "fr" } else { "en" }
}

fn find<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(root, |v, k| v.get(k))
}

/// The text of `key` in the current language, `{name}` replaced by `vars`. An entry
/// `{"one": .., "other": ..}` is picked by the variable `n` (French: 0 and 1 are singular).
pub fn t(key: &str, vars: &[(&str, String)]) -> String {
    let en = LANG.load(Ordering::Relaxed) == 1;
    let entry = if en { find(&EN, key).or_else(|| find(&FR, key)) } else { find(&FR, key) };
    let Some(entry) = entry else { return key.to_string() };
    let text = match entry {
        Value::String(s) => s.as_str(),
        Value::Object(m) => {
            let n: f64 = vars.iter().find(|(k, _)| *k == "n").and_then(|(_, v)| v.parse().ok()).unwrap_or(0.0);
            let one = if en { n == 1.0 } else { n < 2.0 };
            m.get(if one { "one" } else { "other" }).or_else(|| m.get("other")).and_then(Value::as_str).unwrap_or(key)
        }
        _ => key,
    };
    let mut out = text.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// `tr!("key")`, `tr!("key", name = value, n = 3)`: the text in the current language.
#[macro_export]
macro_rules! tr {
    ($key:expr $(,)?) => { $crate::t($key, &[]) };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::t($key, &[$((stringify!($name), ($val).to_string())),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &Value, prefix: &str, out: &mut Vec<String>) {
        if let Value::Object(m) = v {
            if m.contains_key("other") {
                out.push(prefix.to_string());
                return;
            }
            for (k, x) in m {
                keys(x, &if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, out);
            }
        } else {
            out.push(prefix.to_string());
        }
    }

    #[test]
    fn both_languages_have_the_same_keys() {
        let (mut fr, mut en) = (Vec::new(), Vec::new());
        keys(&FR, "", &mut fr);
        keys(&EN, "", &mut en);
        fr.sort();
        en.sort();
        let only_fr: Vec<_> = fr.iter().filter(|k| !en.contains(k)).collect();
        let only_en: Vec<_> = en.iter().filter(|k| !fr.contains(k)).collect();
        assert!(only_fr.is_empty() && only_en.is_empty(), "French only: {only_fr:?}\nEnglish only: {only_en:?}");
    }

    /// Every namespace of the interface too: same keys in French and English, no em-dash.
    #[test]
    fn every_locale_file_matches() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/lib/i18n/locales");
        let mut problems = Vec::new();
        for e in std::fs::read_dir(dir.join("fr")).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let read = |l: &str| -> Value {
                let txt = std::fs::read_to_string(dir.join(l).join(&name)).unwrap_or_else(|_| panic!("{l}/{name} missing"));
                assert!(!txt.contains('\u{2014}'), "{l}/{name} has an em-dash: use a hyphen");
                serde_json::from_str(&txt).unwrap_or_else(|e| panic!("{l}/{name}: {e}"))
            };
            let (mut fr, mut en) = (Vec::new(), Vec::new());
            keys(&read("fr"), "", &mut fr);
            keys(&read("en"), "", &mut en);
            for k in fr.iter().filter(|k| !en.contains(k)) {
                problems.push(format!("{name}: {k} missing in English"));
            }
            for k in en.iter().filter(|k| !fr.contains(k)) {
                problems.push(format!("{name}: {k} missing in French"));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn no_em_dash_in_any_text() {
        for (lang, v) in [("fr", &*FR), ("en", &*EN)] {
            assert!(!v.to_string().contains('\u{2014}'), "{lang}/core.json has an em-dash: use a hyphen");
        }
    }

    #[test]
    fn words_vars_and_plurals() {
        set_lang("fr");
        assert_eq!(tr!("test.hello", name = "Ana"), "Bonjour Ana");
        assert_eq!(tr!("test.songs", n = 1), "1 morceau");
        assert_eq!(tr!("test.songs", n = 0), "0 morceau");
        assert_eq!(tr!("test.songs", n = 2), "2 morceaux");
        assert_eq!(tr!("test.nowhere"), "test.nowhere");
        set_lang("en");
        assert_eq!(tr!("test.hello", name = "Ana"), "Hello Ana");
        assert_eq!(tr!("test.songs", n = 0), "0 songs");
        assert_eq!(tr!("test.songs", n = 1), "1 song");
        set_lang("fr");
    }
}

//! Plugin names normalised for matching.
use fancy_regex::Regex as FRegex;
use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// Suffixes vendors bolt onto VST2 names for bitness / packaging; stripped for matching.
static NAME_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)[\s_\-]*(\(?\s*(x64|x86|x32|64[\s\-]?bit|32[\s\-]?bit|win64|win32|vst2?|vst3|stereo|mono|portable)\s*\)?)",
    )
    .unwrap()
});
const VENDOR_WORDS: [&str; 14] = [
    "fabfilter",
    "izotope",
    "uad",
    "arturia",
    "native",
    "instruments",
    "waves",
    "softube",
    "plugin",
    "alliance",
    "ni",
    "xln",
    "spectrasonics",
    "audio",
];
static OUTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+\s*out\b").unwrap());
static MK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(v|mk|mkii|mk2)\s*\d*\b").unwrap());
static LONE_VERSION: LazyLock<FRegex> = LazyLock::new(|| FRegex::new(r"(^|\s)\d+(\.\d+)*(?=\s|$)").unwrap());
static DOTTED_VERSION: LazyLock<FRegex> = LazyLock::new(|| FRegex::new(r"[-_]\d+\.\d+(\.\d+)*(?=\s|$)").unwrap());
static NOT_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// NFKD, accents dropped, lower case.
pub fn ascii(s: &str) -> String {
    s.nfkd().filter(char::is_ascii).collect::<String>().to_lowercase()
}

fn denoise(mut s: String) -> String {
    loop {
        let next = NAME_NOISE.replace_all(&s, "").to_string();
        if next == s {
            return s;
        }
        s = next;
    }
}

/// "Serum_x64" and "Serum (x64)" both give "serum".
pub fn norm_name(name: &str) -> String {
    NOT_ALNUM.replace_all(&denoise(ascii(name)), "").to_string()
}

/// Name with the version stripped: "FabFilter Pro-Q 2" and "Pro-Q 4" both give "proq".
pub fn family_name(name: &str) -> String {
    let s = denoise(ascii(name));
    let s = OUTS.replace_all(&s, " "); // Kontakt 5 8out / 16out
    let s = MK.replace_all(&s, " "); // CS-80 V3, Mini V3
    let s = LONE_VERSION.replace_all(&s, " "); // standalone version numbers
    let s = DOTTED_VERSION.replace_all(&s, " "); // Spire-1.5 (dotted only: TAPE-201 is a model)
    let words: String = SPACES.split(&s).filter(|w| !w.is_empty() && !VENDOR_WORDS.contains(w)).collect();
    NOT_ALNUM.replace_all(&words, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(norm_name("Serum_x64"), "serum");
        assert_eq!(norm_name("Kontakt 5 Portable x64 64out"), "kontakt564out");
        assert_eq!(family_name("FabFilter Pro-Q 2"), "proq");
        assert_eq!(family_name("Pro-Q 4"), "proq");
        assert_eq!(family_name("Spire-1.5"), "spire");
        assert_eq!(family_name("Kontakt 5 8out"), "kontakt");
    }
}

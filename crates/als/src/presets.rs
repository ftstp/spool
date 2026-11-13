//! Name of the preset a plugin had loaded, read from the state Live saved in the set.
//!
//! Live stores no preset name for VST plugins: only the plugin's own state blob (hex in
//! <Buffer>, <ProcessorState>...) and a program number. Many plugins write the name of the
//! loaded preset inside that blob, each in its own format, so this is a set of per-format
//! readers tried in order. A name found here is the preset the sound STARTED from.
use crate::model::Preset;
use crate::xml::{N, find_desc, find_desc_path, iter, path, text};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use regex::bytes::Regex as BRegex;
use std::io::Read;
use std::sync::LazyLock;

const DEFAULT_NAMES: [&str; 17] = [
    "default",
    "default setting",
    "init",
    "init 1",
    "initial preset",
    "startup",
    "new (default)",
    "<unknown>",
    "untitled",
    "init preset",
    "basic shape",
    "default.aupreset",
    "initialize",
    "init1",
    "- init -",
    "init patch",
    "default-all around",
];

fn is_default(s: &str) -> bool {
    DEFAULT_NAMES.contains(&s.to_lowercase().as_str())
}

static ASCII: LazyLock<BRegex> = LazyLock::new(|| BRegex::new(r"(?-u)[ -~]{3,}").unwrap());
static UTF16: LazyLock<BRegex> = LazyLock::new(|| BRegex::new(r"(?-u)(?:[ -~]\x00){3,}").unwrap());
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(\.\d+)+$").unwrap());
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)stateversion|^version|^vst|^ccnk|^vc2!|^preset name$|macintosh hd|AAAA").unwrap()
});
/// NI instruments only: NI effects (Passive EQ, Driver...) list parameter names instead.
static NI_INSTRUMENTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)kontakt|massive|fm8|absynth|battery|reaktor|monark|razor|prism|form\b|maschine").unwrap()
});
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static LETTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]").unwrap());
static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9A-Fa-f]+$").unwrap());
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"programname="([^"]+)""#,
        r"#pgm=([^\r\n]+)",
        r#"source_preset_name="([^"]+)""#,
        r#"(?i)preset_?name="([^"]+)""#,
        r"(?i)<key>name</key>\s*<string>([^<]+)</string>",
        r#"(?i)"presetName"\s*:\s*"([^"]+)""#,
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});
static ARTURIA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"serialization::archive(?: \d+){5} (\d+) ").unwrap());
static ARTURIA_NEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ (\d+) ").unwrap());
static AU_DATA: LazyLock<BRegex> =
    LazyLock::new(|| BRegex::new(r"(?-u)<key>data</key>\s*<data>([A-Za-z0-9+/=\s]+)</data>").unwrap());
static AUPRESET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([\w .&'()-]{2,60})\.aupreset").unwrap());
static SPIRE: LazyLock<FRegex> = LazyLock::new(|| FRegex::new(r"(?i)^spire\b(?!-1\.[1-9])").unwrap());
static SERUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)serum").unwrap());
static LEAD_NAME: LazyLock<BRegex> =
    LazyLock::new(|| BRegex::new(r"(?s-u)^(?:[\x00-\x20]{8,24})?([A-Za-z0-9][ -~]{2,62})\x00").unwrap());
static PRESET_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\w .&'-]+(/[\w .&'-]+){1,4}$").unwrap());
static MEDIA_EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.(wav|aif+|png|dll)$").unwrap());
static BASE64ISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+]{16,}").unwrap());
static NI_SHORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]\d+$").unwrap());

/// Bytes as latin-1 text: one char per byte, as the prototype decoded them.
pub fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Python's `str.strip(chars)`.
fn strip_chars<'a>(s: &'a str, chars: &[char]) -> &'a str {
    s.trim_matches(|c| chars.contains(&c))
}

/// A readable name, or "" ("QE", "9_" are bytes that happened to be printable).
fn clean(s: &str) -> String {
    let s = SPACES.replace_all(&s.replace('\0', ""), " ").to_string();
    let s = strip_chars(&s, &[' ', '"', '\'']);
    let n = s.chars().count();
    if (3..=80).contains(&n) && LETTERS.find_iter(s).count() >= 2 { s.to_string() } else { String::new() }
}

/// Bytes of the largest hex blob under `el` (the plugin state).
fn raw(el: N) -> Vec<u8> {
    let mut best: Vec<u8> = Vec::new();
    for e in iter(el) {
        let Some(t) = text(e) else { continue };
        let tlen = t.chars().count();
        if tlen > 16 && tlen > best.len() {
            let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
            let head: String = s.chars().take(4096).collect();
            if s.len().is_multiple_of(2)
                && HEX.is_match(&head)
                && let Some(b) = unhex(&s)
            {
                best = b;
            }
        }
    }
    best
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    let v = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    b.chunks(2).map(|p| Some(v(p[0])? << 4 | v(p[1])?)).collect()
}

fn text_patterns(text: &str) -> String {
    for rx in PATTERNS.iter() {
        if let Some(m) = rx.captures(text) {
            let g = &m[1];
            if !clean(g).is_empty() && !JUNK.is_match(g) {
                return clean(g);
            }
        }
    }
    String::new()
}

/// Arturia V Collection: boost text archive, length-prefixed: <n> name <n> bank <n> author.
fn arturia(text: &str) -> (String, String) {
    let chars: Vec<char> = text.chars().collect();
    // the text is latin-1: byte offsets of the regex match = char offsets
    for m in ARTURIA.captures_iter(text) {
        let n: usize = m[1].parse().unwrap_or(0);
        if n == 0 {
            continue;
        }
        let end = byte_to_char(text, m.get(0).unwrap().end());
        let name: String = chars.iter().skip(end).take(n).collect();
        let after: String = chars.iter().skip(end + n).collect();
        let mut bank = String::new();
        if let Some(r) = ARTURIA_NEXT.captures(&after) {
            let k: usize = r[1].parse().unwrap_or(0);
            let start = end + n + r.get(0).unwrap().as_str().chars().count();
            bank = chars.iter().skip(start).take(k).collect();
        }
        let name = clean(&name);
        if !name.is_empty() {
            return (name, clean(&bank));
        }
    }
    (String::new(), String::new())
}

fn byte_to_char(s: &str, b: usize) -> usize {
    s[..b].chars().count()
}

/// NI 'DSIN' containers (Kontakt, Massive, FM8...): UTF-16 strings, the version first,
/// then the preset / library and instrument names, then '\@color' fields.
fn native_instruments(raw: &[u8], plugin: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let head = &raw[..raw.len().min(200_000)];
    for m in UTF16.find_iter(head) {
        let units: Vec<u16> = m.as_bytes().chunks(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        let s = clean(&String::from_utf16_lossy(&units));
        let low = s.to_lowercase();
        if s.is_empty()
            || VERSION.is_match(&s)
            || (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            || is_default(&s)
            || s.chars().count() < 4
            || NI_SHORT.is_match(&s)
        {
            continue;
        }
        if s.starts_with("\\@") || s.starts_with('<') {
            break;
        }
        if !plugin.is_empty() && plugin.to_lowercase().contains(&low) {
            continue;
        }
        out.push(s);
        if out.len() == 2 {
            break;
        }
    }
    out.join(" / ")
}

fn cstr(b: &[u8], at: usize, size: usize) -> String {
    let end = (at + size).min(b.len());
    let field = &b[at.min(end)..end];
    let field = field.split(|&c| c == 0).next().unwrap_or(&[]);
    clean(&latin1(field))
}

/// Serum 1: zlib stream with fixed fields: preset name 238 bytes before 'MACRO 1',
/// author 192 before. Its newer, smaller state format keeps no name.
fn serum(raw: &[u8]) -> String {
    let mut z = Vec::new();
    let mut dec = flate2::read::ZlibDecoder::new(raw).take(400_000);
    if dec.read_to_end(&mut z).is_err() && z.is_empty() {
        return String::new();
    }
    let Some(i) = find(&z, b"MACRO 1\x00") else { return String::new() };
    if i < 238 {
        return String::new();
    }
    let (name, author) = (cstr(&z, i - 238, 46), cstr(&z, i - 192, 46));
    if !name.is_empty() && !author.is_empty() { format!("{name} (par {author})") } else { name }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Spire 1.0: a fixed-size name field right before the notes field ('no notes'). A shorter
/// name overwrites only the start of the previous one, so the name is the FIRST segment of
/// the trailing run of text segments.
fn spire(raw: &[u8]) -> String {
    let lim = &raw[..raw.len().min(400_000)];
    let Some(i) = find(lim, b"o notes\x00") else { return String::new() };
    if i < 2 {
        return String::new();
    }
    let j = if raw[i - 1] == b'n' { i - 1 } else { i };
    let start = j.saturating_sub(48);
    let mut block: Vec<&[u8]> = Vec::new();
    for seg in raw[start..j].split(|&c| c == 0).rev() {
        if seg.is_empty() {
            continue;
        }
        if seg.len() >= 2 && seg.iter().all(|&c| (b' '..=b'~').contains(&c)) {
            block.push(seg);
        } else {
            break;
        }
    }
    block.last().map(|s| clean(&latin1(s))).unwrap_or_default()
}

/// The preset a plugin had loaded, or None. `info` is the VstPluginInfo / Vst3PluginInfo /
/// AuPluginInfo element.
pub fn preset_of(info: N, fmt: &str, plugin: &str) -> Option<Preset> {
    let pre = path(info, "Preset")?;
    let (mut name, mut bank) = (String::new(), String::new());
    if fmt == "au"
        && let Some(v) = find_desc_path(pre, "AuPreset", "Name").and_then(|n| n.attribute("Value"))
        && !clean(v).is_empty()
        && !is_default(v)
    {
        name = clean(v);
    }
    let rawb = raw(pre);
    if fmt == "au" && !rawb.is_empty() && name.is_empty() {
        // an AU state is a plist whose <data> holds the plugin's own blob (the same one its
        // VST2 writes): read that first, the plist's "Untitled" is a fallback
        if let Some(m) = AU_DATA.captures(&rawb) {
            let b64: Vec<u8> = m[1].iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
            let inner = base64(&b64).unwrap_or_default();
            let (n, b) = if inner.is_empty() { (String::new(), String::new()) } else { from_blob(&inner, plugin) };
            if !n.is_empty() && !is_default(&n) {
                name = n;
                bank = b;
            }
        }
    }
    if !rawb.is_empty() && name.is_empty() {
        (name, bank) = from_blob(&rawb, plugin);
    }
    if name.is_empty() && fmt == "au" {
        let txt = find_desc(pre, "AuPreset").map(|el| latin1(&raw(el))).unwrap_or_default();
        if let Some(m) = AUPRESET.captures(&txt)
            && m[1].to_lowercase() != "default"
        {
            name = clean(&m[1]);
        }
    }
    if name.is_empty() {
        return None;
    }
    let default = is_default(&name);
    Some(Preset { name, bank, default })
}

/// (name, bank) read from a plugin's state blob, empty when none is readable.
fn from_blob(raw: &[u8], plugin: &str) -> (String, String) {
    let (mut name, mut bank) = (String::new(), String::new());
    if raw.is_empty() {
        return (name, bank);
    }
    let head = &raw[..raw.len().min(300_000)];
    let text = latin1(head);
    name = text_patterns(&text);
    if name.is_empty() && text.contains("serialization::archive") {
        (name, bank) = arturia(&text);
    }
    if name.is_empty() && find(&head[..head.len().min(64)], b"DSIN").is_some() && NI_INSTRUMENTS.is_match(plugin) {
        name = native_instruments(head, plugin);
    }
    if name.is_empty() && SERUM.is_match(plugin) {
        name = serum(raw);
    }
    let strs: Vec<String> = ASCII.find_iter(&head[..head.len().min(20_000)]).map(|m| latin1(m.as_bytes())).collect();
    if name.is_empty() && SPIRE.is_match(plugin).unwrap_or(false) {
        name = spire(raw);
    }
    if name.is_empty() {
        // first thing in the blob is a null-terminated name (FabFilter, OTT...; UAD puts a
        // 20-byte header first)
        let cand = LEAD_NAME.captures(raw).map(|m| latin1(&m[1])).unwrap_or_default();
        if !cand.is_empty() && (cand.contains(' ') || cand.contains('-') || is_default(&cand)) && !JUNK.is_match(&cand)
        {
            name = clean(&cand);
        }
    }
    if name.is_empty() {
        // a preset path inside the blob (minimoog V: "All/Basses/K.U_ELPevil")
        for s in strs.iter().take(40) {
            if PRESET_PATH.is_match(s) && !MEDIA_EXT.is_match(s) && !BASE64ISH.is_match(s) && !JUNK.is_match(s) {
                let (b, n) = s.rsplit_once('/').unwrap_or(("", s));
                name = clean(n);
                bank = clean(b);
                break;
            }
        }
    }
    (name, bank)
}

/// Standard base64 (padding optional), None when malformed.
pub fn base64(s: &[u8]) -> Option<Vec<u8>> {
    let v = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let s: Vec<u8> = s.iter().copied().take_while(|&c| c != b'=').collect();
    if s.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= (v(c)? as u32) << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len() - 1]);
    }
    Some(out)
}

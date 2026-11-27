//! Usage statistics and errors, sent ONLY when the user said yes (asked once, in the setup or
//! at the first start of a version that has this; changed in Settings > Spool). No account:
//! a random id made when they say yes, deleted when they say no.
//!
//! What may go out is closed here, not in the interface: three events (`tool_opened`,
//! `tool_feature_used` with a feature from [`FEATURES`], `tool_error` with a place and a
//! message scrubbed of paths and names), plus the version and the system. Never a project,
//! a song, a file name or path, a plugin list, an e-mail or the computer's name.
//!
//! Every event goes through [`payload`], the one gate: nothing is built when the choice is
//! unset or "no". Sending is one HTTPS POST on a thread of its own, no retry, any failure
//! silent: the statistics never slow or break the app.
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

/// PostHog's EU capture endpoint.
pub const ENDPOINT: &str = "https://eu.i.posthog.com/i/v0/e/";
/// The project's public key: it can only send events, never read them (safe to ship).
const API_KEY: &str = "phc_tGKWfzo4YoN5SHyyw8m4yrotdeTmPhxf4QPzWQ3TbDFv";
/// Settings key of the choice: "yes" or "no"; absent while the user has not answered.
pub const CHOICE_KEY: &str = "telemetry";
/// Settings key of the random id (only while the choice is "yes").
pub const ID_KEY: &str = "telemetry_id";

/// The features counted (`tool_feature_used`), once per launch each. A name outside this list
/// is not sent.
pub const FEATURES: &[&str] = &[
    // pages
    "song_sheet_opened",
    "statistics_opened",
    "samples_opened",
    "midi_opened",
    "releases_opened",
    "artists_opened",
    "media_opened",
    "repair_opened",
    "starter_pack_opened",
    "folder_rules_opened",
    // actions
    "scan_finished",
    "repair_run",
    "treatment_run",
    "starter_pack_created",
    "midi_exported",
    "project_played",
    "project_opened_in_live",
    "artists_merged",
    "folders_arranged",
    "notion_imported",
    "catalogue_exported",
    "audio_analysis_enabled",
];

/// A catalogue's size, never its count: the number of projects in one of these ranges.
pub const SIZES: &[&str] = &["<100", "100-1k", "1k-10k", ">10k"];

pub fn size_bucket(n: u64) -> &'static str {
    match n {
        0..100 => SIZES[0],
        100..1_000 => SIZES[1],
        1_000..10_000 => SIZES[2],
        _ => SIZES[3],
    }
}

/// The answer as stored: "yes", "no", or "" (not asked yet; anything else reads as that).
pub fn choice_of(stored: Option<String>) -> String {
    match stored.as_deref() {
        Some("yes") => "yes".into(),
        Some("no") => "no".into(),
        _ => String::new(),
    }
}

/// THE GATE. The id to send under, only when the user said yes and an id exists.
pub fn gate(choice: Option<&str>, id: Option<&str>) -> Option<String> {
    match (choice, id) {
        (Some("yes"), Some(id)) if !id.is_empty() => Some(id.to_string()),
        _ => None,
    }
}

/// A new random id (UUID v4).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// An event's own properties, checked against the closed lists; None: not an event that may
/// be sent.
fn event_props(event: &str, props: &Map<String, Value>, home: Option<&str>) -> Option<Map<String, Value>> {
    let mut out = Map::new();
    match event {
        "tool_opened" => {}
        "tool_feature_used" => {
            let f = props.get("feature")?.as_str()?;
            if !FEATURES.contains(&f) {
                return None;
            }
            out.insert("feature".into(), f.into());
            if let Some(s) = props.get("catalogue_size").and_then(Value::as_str).filter(|s| SIZES.contains(s)) {
                out.insert("catalogue_size".into(), s.into());
            }
        }
        "tool_error" => {
            // a fixed identifier from the code ("cmd:repair_run", "ui_error"), nothing else
            let w = props.get("where")?.as_str()?;
            let ident = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if w.len() > 64 || !w.split(':').all(ident) || w.split(':').count() > 2 {
                return None;
            }
            let m = props.get("message").and_then(Value::as_str).unwrap_or("");
            out.insert("where".into(), w.into());
            out.insert("message".into(), scrub(m, home).into());
        }
        _ => return None,
    }
    Some(out)
}

/// The whole body of one event, or None (the gate closed, or an event not in the list).
pub fn payload(choice: Option<&str>, id: Option<&str>, event: &str, props: &Map<String, Value>) -> Option<Value> {
    let distinct_id = gate(choice, id)?;
    let mut p = event_props(event, props, home().as_deref())?;
    p.insert("surface".into(), "spool".into());
    p.insert("app_version".into(), env!("CARGO_PKG_VERSION").into());
    p.insert("os".into(), os_name().into());
    p.insert("os_version".into(), os_version().into());
    p.insert("$process_person_profile".into(), false.into());
    p.insert("$geoip_disable".into(), true.into());
    Some(json!({ "api_key": API_KEY, "event": event, "distinct_id": distinct_id, "properties": p }))
}

/// What was already sent this launch: the start once, each feature once, each error once
/// (and 20 errors at most, so a loop of failures does not send a flood).
static SENT: LazyLock<Mutex<(HashSet<String>, usize)>> = LazyLock::new(|| Mutex::new((HashSet::new(), 0)));

/// True the first time this event (with these properties) goes out in this launch.
pub fn first_time(body: &Value) -> bool {
    let event = body["event"].as_str().unwrap_or("");
    let p = &body["properties"];
    let key = match event {
        "tool_feature_used" => format!("f:{}", p["feature"].as_str().unwrap_or("")),
        "tool_error" => format!("e:{}:{}", p["where"].as_str().unwrap_or(""), p["message"].as_str().unwrap_or("")),
        e => e.to_string(),
    };
    let mut s = SENT.lock().unwrap();
    if event == "tool_error" {
        if s.1 >= 20 {
            return false;
        }
        if s.0.contains(&key) {
            return false;
        }
        s.1 += 1;
    }
    s.0.insert(key)
}

/// One POST; the HTTP status, or the error. Waits (the caller runs it off the window's way).
pub fn post(body: &Value) -> Result<u16, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .user_agent(concat!("Spool/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let r = agent.post(ENDPOINT).header("Content-Type", "application/json").send(body.to_string()).map_err(|e| e.to_string())?;
    Ok(r.status().as_u16())
}

/// Sent on a thread of its own: never waited for, never retried, a failure says nothing.
pub fn send(body: Value) {
    std::thread::spawn(move || {
        let _ = post(&body);
    });
}

// ------------------------------------------------------------------ scrubbing

fn home() -> Option<String> {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().filter(|h| h.len() > 3)
}

const EXTS: &str = "als|alc|alp|adg|adv|ams|asd|amxd|agr|wav|wave|aif|aiff|mp3|mp4|m4a|flac|ogg|opus|mid|midi|smf|json|csv|txt|md|log|sqlite|db|png|jpe?g|webp|gif|bmp|tiff?|mov|avi|mkv|zip|rar|7z|exe|dll|vst3?|component|fxp|fxb|nmsv|h2p|vstpreset|serumpreset|ttf|otf|pdf|xml|onnx";

static URL: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\b[a-zA-Z][a-zA-Z0-9+.-]*://\S+").unwrap());
static QUOTED: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#""[^"\n]*"|“[^”\n]*”|«[^»\n]*»|‘[^’\n]*’|(^|[\s(\[:=,])'[^'\n]*'"#).unwrap());
/// A path: from a drive, a share, `~` or a separator, through folders (spaces allowed), to a
/// file name with a known extension (spaces allowed) or a last word.
static PATH: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(&format!(
        r#"(?i)(?:[a-z]:|\\\\[^\\/\s]+|~|\.{{1,2}})?[\\/](?:[^\\/\n"'<>|*?]*[\\/])*(?:[^\\/\n"'<>|*?]*?\.(?:{EXTS})\b|[^\s\\/"'<>|*?]*)"#
    ))
    .unwrap()
});
static FILE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(&format!(r#"(?i)[^\s"'<>|\\/*?]+\.(?:{EXTS})\b"#)).unwrap());

/// An error message as it may be sent: one of Spool's own messages becomes its key (what it
/// named is dropped); any other has the home folder turned into `~`, then every path, file
/// name, quoted name and address taken out. At most 300 characters.
pub fn scrub(message: &str, home: Option<&str>) -> String {
    let msg = message.trim();
    let mut s = match i18n::key_of(msg) {
        Some(k) => k,
        None => {
            let mut s = msg.to_string();
            if let Some(h) = home.filter(|h| h.len() > 3) {
                for v in [h.to_string(), h.replace('\\', "/"), h.replace('/', "\\")] {
                    if let Ok(re) = regex::Regex::new(&format!("(?i){}", regex::escape(&v))) {
                        s = re.replace_all(&s, "~").into_owned();
                    }
                }
            }
            s = URL.replace_all(&s, "<url>").into_owned();
            s = QUOTED.replace_all(&s, "${1}<name>").into_owned();
            s = PATH.replace_all(&s, "<path>").into_owned();
            s = FILE.replace_all(&s, "<file>").into_owned();
            s
        }
    };
    if s.chars().count() > 300 {
        s = s.chars().take(300).collect();
    }
    s
}

// ------------------------------------------------------------------ the system

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}

fn os_version() -> String {
    static V: LazyLock<String> = LazyLock::new(read_os_version);
    V.clone()
}

#[cfg(windows)]
fn read_os_version() -> String {
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    // RtlGetVersion: the real version (GetVersionEx answers 6.2 to an app without a manifest)
    let mut v: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    v.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    if unsafe { windows_sys::Wdk::System::SystemServices::RtlGetVersion(&mut v) } != 0 {
        return String::new();
    }
    format!("{}.{}.{}", v.dwMajorVersion, v.dwMinorVersion, v.dwBuildNumber)
}

#[cfg(target_os = "macos")]
fn read_os_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn read_os_version() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("VERSION_ID=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn nothing_is_built_unless_the_user_said_yes() {
        let p = props(json!({ "feature": "repair_run" }));
        for choice in [None, Some(""), Some("no"), Some("maybe")] {
            assert!(payload(choice, Some("an-id"), "tool_opened", &Map::new()).is_none(), "{choice:?}");
            assert!(payload(choice, Some("an-id"), "tool_feature_used", &p).is_none(), "{choice:?}");
        }
        // yes but the id was deleted: nothing either
        assert!(payload(Some("yes"), None, "tool_opened", &Map::new()).is_none());
        assert!(payload(Some("yes"), Some(""), "tool_opened", &Map::new()).is_none());
        let b = payload(Some("yes"), Some("an-id"), "tool_feature_used", &p).unwrap();
        assert_eq!(b["distinct_id"], "an-id");
        assert_eq!(b["properties"]["feature"], "repair_run");
        assert_eq!(b["properties"]["surface"], "spool");
        assert_eq!(b["properties"]["$process_person_profile"], false);
        assert_eq!(b["properties"]["$geoip_disable"], true);
    }

    #[test]
    fn only_the_closed_lists_go_out() {
        let yes = |e: &str, p: Value| payload(Some("yes"), Some("id"), e, &props(p));
        assert!(yes("song_title", json!({})).is_none());
        assert!(yes("tool_feature_used", json!({ "feature": "My Song" })).is_none());
        assert!(yes("tool_feature_used", json!({})).is_none());
        // extra properties are dropped, a size outside the buckets too
        let b = yes("tool_feature_used", json!({ "feature": "scan_finished", "catalogue_size": "1234", "title": "My Song" })).unwrap();
        assert!(b["properties"].get("catalogue_size").is_none() && b["properties"].get("title").is_none());
        let b = yes("tool_feature_used", json!({ "feature": "scan_finished", "catalogue_size": "1k-10k" })).unwrap();
        assert_eq!(b["properties"]["catalogue_size"], "1k-10k");
        assert!(yes("tool_error", json!({ "where": "cmd:repair_run My Song", "message": "x" })).is_none());
        assert!(yes("tool_error", json!({ "where": "C:\\Users", "message": "x" })).is_none());
        let b = yes("tool_error", json!({ "where": "cmd:repair_run", "message": "x" })).unwrap();
        assert_eq!(b["properties"]["where"], "cmd:repair_run");
    }

    #[test]
    fn sizes_are_buckets() {
        assert_eq!(size_bucket(0), "<100");
        assert_eq!(size_bucket(99), "<100");
        assert_eq!(size_bucket(100), "100-1k");
        assert_eq!(size_bucket(9_999), "1k-10k");
        assert_eq!(size_bucket(10_000), ">10k");
    }

    #[test]
    fn paths_and_names_are_scrubbed() {
        let home = Some("C:\\Users\\Ana Lopez");
        // Spool's own message: its key, the folder gone
        assert_eq!(scrub("Folder not found: C:\\Users\\Ana Lopez\\Music\\Summer", home), "settings.folderNotFound");
        // a system message: the path gone, the kind kept
        let s = scrub("failed to read C:\\Users\\Ana Lopez\\Music\\Summer Song\\Summer Song.als: The system cannot find the file specified. (os error 2)", home);
        assert!(!s.contains("Ana") && !s.contains("Summer") && !s.contains("Music"), "{s}");
        assert!(s.contains("cannot find the file specified"), "{s}");
        let s = scrub("cannot open D:/Projects/Club Night/Club Night.als", None);
        assert!(!s.contains("Club") && !s.contains("Projects"), "{s}");
        let s = scrub("error in /home/ana/beats/loop.wav (permission denied)", Some("/home/ana"));
        assert!(!s.contains("ana") && !s.contains("loop"), "{s}");
        assert!(s.contains("permission denied"), "{s}");
        let s = scrub("could not decode Kick 03.wav", None);
        assert!(!s.contains("Kick 03") && !s.contains("03.wav"), "{s}");
        let s = scrub("song \"Midnight Run\" has no render", None);
        assert!(!s.contains("Midnight"), "{s}");
        let s = scrub("TypeError: Cannot read properties of undefined (reading 'title')", None);
        assert!(s.starts_with("TypeError: Cannot read properties of undefined"), "{s}");
        let s = scrub("failed http://media.localhost/C%3A%2FUsers%2Fana%2Fx.wav", None);
        assert!(!s.contains("ana"), "{s}");
        // the home folder alone, outside a path
        let s = scrub("home is C:\\Users\\Ana Lopez", home);
        assert!(!s.contains("Ana"), "{s}");
        assert_eq!(scrub(&"x".repeat(1000), None).chars().count(), 300);
    }

    #[test]
    fn each_feature_once_errors_capped() {
        let b = payload(Some("yes"), Some("id"), "tool_feature_used", &props(json!({ "feature": "midi_opened" }))).unwrap();
        assert!(first_time(&b));
        assert!(!first_time(&b));
    }

    /// Two real events, by hand only (`cargo test -p core telemetry::tests::live -- --ignored`):
    /// distinct id "test-spool-dev", property "test": true.
    #[test]
    #[ignore]
    fn live() {
        for (e, p) in [("tool_opened", json!({})), ("tool_feature_used", json!({ "feature": "scan_finished", "catalogue_size": "100-1k" }))] {
            let mut b = payload(Some("yes"), Some("test-spool-dev"), e, &props(p)).unwrap();
            b["properties"]["test"] = true.into();
            println!("{e}: HTTP {:?}", post(&b));
        }
    }
}

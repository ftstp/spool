//! What the app did, kept on this computer only: a small journal (spool.log in the data
//! folder) of the starts, the analyses (their stages, where they were every half minute,
//! how they ended) and the files the sound analysis could not read. When something seems
//! stuck, it says where; the diagnostic report (written when the user asks, sent by them if
//! they choose) carries its end. Nothing is ever sent by the app.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static FILE: OnceLock<PathBuf> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());
/// Past this size the journal starts again (the last one kept as spool.log.1).
const MAX: u64 = 2 * 1024 * 1024;

pub fn init(data_dir: &Path) {
    let _ = FILE.set(data_dir.join("spool.log"));
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// One line in the journal (never fails: the journal is a help, not a need).
pub fn log(msg: impl AsRef<str>) {
    let Some(p) = FILE.get() else { return };
    let _g = LOCK.lock();
    if std::fs::metadata(p).is_ok_and(|m| m.len() > MAX) {
        let _ = std::fs::rename(p, p.with_extension("log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{} {}", stamp(), msg.as_ref().replace('\n', " "));
    }
}

/// The journal's last `n` lines (the previous file's end first when this one is short).
pub fn tail(n: usize) -> Vec<String> {
    let Some(p) = FILE.get() else { return Vec::new() };
    let read = |q: &Path| std::fs::read_to_string(q).map(|s| s.lines().map(String::from).collect::<Vec<_>>()).unwrap_or_default();
    let mut lines = read(&p.with_extension("log.1"));
    lines.extend(read(p));
    let from = lines.len().saturating_sub(n);
    lines.split_off(from)
}

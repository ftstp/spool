//! The safety copies of the base (backups/): one whole copy kept, compressed. A copy is the
//! base entire (GBs on a large library), and 4 of them were seen taking 6.3 GB beside a 4 GB
//! base. The newest copy is enough to step back over the last update or start over; it is
//! compressed in the background (a few times smaller), which reverting undoes with any
//! archive tool (.gz). Only Spool's own copies in its own folder are ever touched; the small
//! choices-*.json beside a start-over stay (what the user decided, a few KB).
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static BUSY: AtomicBool = AtomicBool::new(false);

/// A whole copy of the base: "before-v<N>-<stamp>" (before an update) or "plugin-fixer-
/// <stamp>" (a start over), as .sqlite or compressed (.sqlite.gz).
fn is_copy(name: &str) -> bool {
    (name.starts_with("before-v") || name.starts_with("plugin-fixer-")) && (name.ends_with(".sqlite") || name.ends_with(".sqlite.gz"))
}

fn modified(p: &Path) -> std::time::SystemTime {
    std::fs::symlink_metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
}

/// Keeps the newest copy, compressed, and removes the others (with their journals), the
/// journals set aside but the newest, and what an interrupted compression left. Runs once
/// at a time; meant for a background thread (compressing a GB takes about a minute).
pub fn tidy(dir: &Path) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = run(dir);
    BUSY.store(false, Ordering::SeqCst);
}

fn run(dir: &Path) -> std::io::Result<()> {
    let names: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for p in names.iter().filter(|p| name(p).ends_with(".part")) {
        let _ = std::fs::remove_file(p);
    }
    let mut copies: Vec<&PathBuf> = names.iter().filter(|p| p.is_file() && is_copy(&name(p))).collect();
    copies.sort_by_key(|p| std::cmp::Reverse(modified(p)));
    for old in copies.iter().skip(1) {
        let n = name(old);
        for f in [n.clone(), format!("{n}-wal"), format!("{n}-shm")] {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
    let mut journals: Vec<&PathBuf> = names.iter().filter(|p| p.is_dir() && name(p).starts_with("journal-")).collect();
    journals.sort_by_key(|p| std::cmp::Reverse(modified(p)));
    for old in journals.iter().skip(1) {
        let _ = std::fs::remove_dir_all(old);
    }
    let Some(keep) = copies.first() else { return Ok(()) };
    let n = name(keep);
    if !n.ends_with(".sqlite") {
        return Ok(());
    }
    // a copy whose journal holds commits is not whole by itself: left as it is
    if std::fs::metadata(dir.join(format!("{n}-wal"))).is_ok_and(|m| m.len() > 0) {
        return Ok(());
    }
    let when = modified(keep);
    let gz = dir.join(format!("{n}.gz"));
    let part = dir.join(format!("{n}.gz.part"));
    {
        let mut src = std::fs::File::open(keep)?;
        let out = std::fs::File::create(&part)?;
        let mut e = flate2::write::GzEncoder::new(std::io::BufWriter::new(out), flate2::Compression::new(3));
        std::io::copy(&mut src, &mut e)?;
        let mut w = e.finish()?;
        w.flush()?;
        let f = w.into_inner().map_err(|e| e.into_error())?;
        f.sync_all()?;
        // the copy keeps its date: the 30 days of an update's copy count from it
        let _ = f.set_modified(when);
    }
    std::fs::rename(&part, &gz)?;
    std::fs::remove_file(keep)?;
    for ext in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(dir.join(format!("{n}{ext}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn keeps_the_newest_copy_compressed_and_nothing_else_of_spools() {
        let dir = std::env::temp_dir().join(format!("spool-backups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let w = |n: &str, body: &[u8]| std::fs::write(dir.join(n), body).unwrap();
        w("before-v9-100.sqlite", b"old update copy");
        std::thread::sleep(std::time::Duration::from_millis(30));
        w("plugin-fixer-2026-01-01_10h00m00.sqlite", b"start over copy");
        w("plugin-fixer-2026-01-01_10h00m00.sqlite-wal", b"");
        w("choices-2026-01-01_10h00m00.json", b"{}");
        std::thread::sleep(std::time::Duration::from_millis(30));
        let newest = vec![7u8; 200_000];
        w("before-v11-200.sqlite", &newest);
        w("stray.gz.part", b"half");
        w("notes.txt", b"not Spool's");
        tidy(&dir);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["before-v11-200.sqlite.gz", "choices-2026-01-01_10h00m00.json", "notes.txt"]);
        let mut back = Vec::new();
        flate2::read::GzDecoder::new(std::fs::File::open(dir.join("before-v11-200.sqlite.gz")).unwrap()).read_to_end(&mut back).unwrap();
        assert_eq!(back, newest);
        // a second pass changes nothing
        tidy(&dir);
        assert!(dir.join("before-v11-200.sqlite.gz").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

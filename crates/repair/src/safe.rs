//! Writing into the user's folders: only NEW files. A user's file (a project, a sample, a
//! render, anything already there) is never overwritten, whatever the caller checked before:
//! the check and the write are one step here, so a file that appeared in between is safe too.
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Copies `from` to `to`, which must not exist (AlreadyExists otherwise). The modification
/// date is kept (as Python's shutil.copy2). A failed copy leaves nothing behind.
pub fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut src = std::fs::File::open(from)?;
    let mut dst = std::fs::File::options().write(true).create_new(true).open(to)?;
    let done = io::copy(&mut src, &mut dst).and_then(|_| dst.flush());
    if let Err(e) = done {
        drop(dst);
        let _ = std::fs::remove_file(to); // our own half-written file
        return Err(e);
    }
    if let Ok(t) = std::fs::metadata(from).and_then(|m| m.modified()) {
        let _ = dst.set_modified(t);
    }
    Ok(())
}

/// A temporary file beside `out` that did not exist (".tmp", ".tmp2"...), created empty.
pub fn new_temp(out: &Path) -> io::Result<(PathBuf, std::fs::File)> {
    let base = out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for i in 1..1000 {
        let ext = if i == 1 { "tmp".to_string() } else { format!("tmp{i}") };
        let p = out.with_file_name(format!("{base}.{ext}"));
        match std::fs::File::options().write(true).create_new(true).open(&p) {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other(i18n::tr!("repair.noTempName")))
}

/// Gives the finished temporary file its name `out`, which must not exist. A hard link
/// fails atomically when the name is taken; a drive without hard links (FAT, exFAT) falls
/// back to "absent? then rename". The temporary file is removed either way.
pub fn publish(tmp: &Path, out: &Path) -> io::Result<()> {
    match std::fs::hard_link(tmp, out) {
        Ok(()) => {
            let _ = std::fs::remove_file(tmp);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists || out.exists() => {
            let _ = std::fs::remove_file(tmp);
            Err(io::Error::new(io::ErrorKind::AlreadyExists, i18n::tr!("repair.pathExists", path = out.display())))
        }
        Err(_) => std::fs::rename(tmp, out).inspect_err(|_| {
            let _ = std::fs::remove_file(tmp);
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pf-safe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_never_overwrites() {
        let d = dir("copy");
        std::fs::write(d.join("a.wav"), b"new").unwrap();
        std::fs::write(d.join("b.wav"), b"user file").unwrap();
        let e = copy_new(&d.join("a.wav"), &d.join("b.wav")).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(d.join("b.wav")).unwrap(), b"user file");
        copy_new(&d.join("a.wav"), &d.join("sub/c.wav")).unwrap();
        assert_eq!(std::fs::read(d.join("sub/c.wav")).unwrap(), b"new");
    }

    #[test]
    fn publish_never_overwrites() {
        let d = dir("pub");
        let out = d.join("Song [R].als");
        std::fs::write(&out, b"user project").unwrap();
        std::fs::write(d.join("Song [R].als.tmp"), b"someone else").unwrap();
        let (tmp, mut f) = new_temp(&out).unwrap();
        assert!(tmp.to_string_lossy().ends_with(".tmp2"));
        f.write_all(b"copy").unwrap();
        drop(f);
        assert!(publish(&tmp, &out).is_err());
        assert_eq!(std::fs::read(&out).unwrap(), b"user project");
        assert_eq!(std::fs::read(d.join("Song [R].als.tmp")).unwrap(), b"someone else");
        assert!(!tmp.exists());
        let out2 = d.join("Song [R] 2.als");
        let (tmp, mut f) = new_temp(&out2).unwrap();
        f.write_all(b"copy").unwrap();
        drop(f);
        publish(&tmp, &out2).unwrap();
        assert_eq!(std::fs::read(&out2).unwrap(), b"copy");
        assert!(!tmp.exists());
    }
}

//! Sample references: where the file may be on this machine.
use crate::xml::{N, val};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// What the reader needs to know about this machine.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub home: PathBuf,
    /// Live's Resources folder (Core Library of ANY Live version maps here).
    pub live_resources: Option<PathBuf>,
    /// The previous system's drive letter (dual boot: its C: is now this drive).
    pub old_drive: Option<String>,
}

impl Env {
    /// This machine, the way the prototype found it.
    pub fn detect(old_drive: Option<String>) -> Env {
        let home =
            std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
        let mut res: Vec<PathBuf> = std::fs::read_dir(r"C:\ProgramData\Ableton")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("Live 1") && n[6..].contains(' ')
            })
            .map(|e| e.path().join("Resources"))
            .filter(|p| p.is_dir())
            .collect();
        res.sort();
        Env { home, live_resources: res.into_iter().next(), old_drive }
    }

    /// Library markers in a stored path -> the same library on this machine, in order.
    fn library_roots(&self) -> [(&'static str, Option<PathBuf>); 3] {
        [
            ("/resources/", self.live_resources.clone()),
            ("/user library/", Some(self.home.join("Documents/Ableton/User Library"))),
            ("/factory packs/", Some(self.home.join("Documents/Ableton/Factory Packs"))),
        ]
    }
}

/// The project folder: the nearest of the set's folder and two parents holding
/// "Ableton Project Info", else the set's folder.
pub fn project_root(als: &Path) -> PathBuf {
    let mut d = als.parent();
    for _ in 0..3 {
        let Some(dir) = d else { break };
        if dir.join("Ableton Project Info").is_dir() {
            return dir.to_path_buf();
        }
        d = dir.parent();
    }
    als.parent().map(Path::to_path_buf).unwrap_or_default()
}

static WIN_USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z]:/users/[^/]+/(.*)").unwrap());
static WIN_C: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^c:[/\\]").unwrap());

/// (path shown, candidates): project-relative, the stored absolute path, the same library
/// file under this machine's Live / User Library, the same path under this Windows user.
/// The old-drive copy is checked separately: Live will NOT look there.
pub fn candidates(fileref: N, proj_root: &Path, env: &Env) -> (String, Vec<PathBuf>) {
    let path = val(Some(fileref), "Path").unwrap_or("");
    let rel = val(Some(fileref), "RelativePath").unwrap_or("");
    path_candidates(path, rel, val(Some(fileref), "RelativePathType"), proj_root, env)
}

/// `candidates` from the reference's values (for a tree that is not roxmltree's).
pub fn path_candidates(path: &str, rel: &str, rel_type: Option<&str>, proj_root: &Path, env: &Env) -> (String, Vec<PathBuf>) {
    let path = path.replace('\\', "/");
    let rel = rel.replace('\\', "/");
    let mut cands = Vec::new();
    if !rel.is_empty() && rel_type == Some("3") {
        cands.push(proj_root.join(&rel));
    }
    if !path.is_empty() {
        cands.push(PathBuf::from(&path));
        let low = path.to_lowercase();
        for (marker, root) in env.library_roots() {
            if let (Some(i), Some(root)) = (low.find(marker), root) {
                // the lowercase copy has the same byte offsets for ASCII markers
                if let Some(rest) = path.get(i + marker.len()..) {
                    cands.push(root.join(rest));
                }
            }
        }
        if let Some(m) = WIN_USER.captures(&path) {
            cands.push(env.home.join(&m[1]));
        }
    }
    (if path.is_empty() { rel } else { path }, cands)
}

/// The same path on the previous system's drive, as Windows writes it.
pub fn old_path(path: &str, old_drive: Option<&str>) -> Option<PathBuf> {
    let d = old_drive.filter(|d| !d.is_empty())?;
    if !WIN_C.is_match(path) {
        return None;
    }
    Some(PathBuf::from(format!("{d}:{}", &path[2..]).replace('/', "\\")))
}

/// Whether a file exists. A file whose permissions deny opening it (old Windows installs
/// keep their ACLs, even on the folder) still counts: it is looked up by name, as Python's
/// `os.stat` does on Windows.
pub fn is_file(p: &Path) -> bool {
    match std::fs::metadata(p) {
        Ok(m) => m.is_file(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => find_file(p),
        Err(_) => false,
    }
}

#[cfg(windows)]
fn find_file(p: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    // GetFileInformationByName (Windows 11 24H2+) reads a file's attributes by name without
    // opening it, which is how Python's os.stat sees files whose ACL denies opening (even
    // the folder listing). Looked up at run time: older Windows simply lacks it.
    type ByName = unsafe extern "system" fn(*const u16, i32, *mut core::ffi::c_void, u32) -> i32;
    static FUNC: OnceLock<Option<ByName>> = OnceLock::new();
    let f = FUNC.get_or_init(|| {
        let dll: Vec<u16> = "kernelbase.dll".encode_utf16().chain(Some(0)).collect();
        // SAFETY: plain Win32 calls with NUL-terminated names; the symbol's signature is the
        // documented one.
        unsafe {
            let h = LoadLibraryW(dll.as_ptr());
            if h.is_null() {
                return None;
            }
            GetProcAddress(h, c"GetFileInformationByName".as_ptr() as *const u8).map(|f| std::mem::transmute(f))
        }
    });
    let Some(f) = f else { return false };
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    // FILE_STAT_BASIC_INFORMATION: 7 x i64 then FileAttributes (u32) at byte 56.
    let mut buf = [0u64; 32];
    const FILE_STAT_BASIC_BY_NAME_INFO: i32 = 3;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    // SAFETY: a NUL-terminated path and an aligned buffer larger than the structure.
    let ok = unsafe { f(wide.as_ptr(), FILE_STAT_BASIC_BY_NAME_INFO, buf.as_mut_ptr().cast(), 256) };
    if ok == 0 {
        return false;
    }
    let attrs = (buf[7] & 0xFFFF_FFFF) as u32;
    attrs & FILE_ATTRIBUTE_DIRECTORY == 0
}

#[cfg(not(windows))]
fn find_file(_: &Path) -> bool {
    false
}

//! Version numbers: what a plugin wrote in a project, what a plugin binary says.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// The version number a VST2 plugin writes into the project ("6010", "197120"), as readable
/// as it gets: vendors encode it differently, so it is a best guess.
pub fn decode_plugin_version(code: Option<&str>) -> String {
    let Some(v) = code.and_then(|c| c.trim().parse::<i64>().ok()) else { return String::new() };
    if v <= 1 {
        return String::new(); // "1" = the plugin does not say
    }
    if v >= 0x10000 {
        // byte-packed: 0x00030200 -> 3.2.0
        return format!("{}.{}.{}", (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF);
    }
    let d = v.to_string();
    let n = |s: &str| s.parse::<i64>().unwrap_or(0);
    match d.len() {
        5 => format!("{}.{}.{}", n(&d[..1]), n(&d[1..3]), n(&d[3..])), // 10103 -> 1.1.3
        4 => format!("{}.{}.{}", &d[..1], &d[1..2], n(&d[2..])),       // 6010 -> 6.0.10
        3 => format!("{}.{}.{}", &d[..1], &d[1..2], &d[2..]),          // 230 -> 2.3.0
        _ => d,
    }
}

static FILE_VERSIONS: LazyLock<Mutex<HashMap<PathBuf, String>>> = LazyLock::new(Default::default);

/// "6.10.0.123" from a plugin binary's Windows version resource, or "". For a .vst3 bundle
/// the Windows binary inside is read. Cached per path.
pub fn file_version(path: &Path) -> String {
    if let Some(v) = FILE_VERSIONS.lock().unwrap().get(path) {
        return v.clone();
    }
    let mut target = path.to_path_buf();
    if target.is_dir() {
        // VST3 bundle: Contents/x86_64-win/<name>.vst3
        let inner = target.join("Contents").join("x86_64-win");
        let mut found: Vec<PathBuf> = std::fs::read_dir(&inner)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vst3")))
            .collect();
        found.sort();
        if let Some(f) = found.into_iter().next() {
            target = f;
        }
    }
    let v = if target.is_file() { read_version(&target) } else { String::new() };
    FILE_VERSIONS.lock().unwrap().insert(path.to_path_buf(), v.clone());
    v
}

#[cfg(windows)]
fn read_version(p: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    };
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: NUL-terminated strings, a buffer of the size Windows asked for, and a pointer
    // into that buffer read only while it lives.
    unsafe {
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr().cast()) == 0 {
            return String::new();
        }
        let root: Vec<u16> = "\\".encode_utf16().chain(Some(0)).collect();
        let (mut ptr, mut n) = (std::ptr::null_mut(), 0u32);
        if VerQueryValueW(buf.as_ptr().cast(), root.as_ptr(), &mut ptr, &mut n) == 0 || n == 0 {
            return String::new();
        }
        let info = &*(ptr as *const VS_FIXEDFILEINFO);
        let (ms, ls) = (info.dwFileVersionMS, info.dwFileVersionLS);
        let v = format!("{}.{}.{}.{}", ms >> 16, ms & 0xFFFF, ls >> 16, ls & 0xFFFF);
        if v == "0.0.0.0" { String::new() } else { v }
    }
}

#[cfg(not(windows))]
fn read_version(_: &Path) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::decode_plugin_version as d;

    #[test]
    fn decodes() {
        assert_eq!(d(Some("6010")), "6.0.10");
        assert_eq!(d(Some("10103")), "1.1.3");
        assert_eq!(d(Some("230")), "2.3.0");
        assert_eq!(d(Some("197120")), "3.2.0");
        assert_eq!(d(Some("1")), "");
        assert_eq!(d(None), "");
    }
}

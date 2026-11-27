//! The drives plugged in now (internal or external), for choosing the previous system's drive
//! when it is searched: its letter, its name, its size, and whether a Windows was installed on
//! it (a "Users" folder at its root).
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Drive {
    /// "D"
    pub letter: String,
    /// The volume's name ("Old SSD"), empty when it has none.
    pub label: String,
    /// Bytes, when known.
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    /// A Windows was installed on it: its Users folder is there.
    pub windows: bool,
}

/// Every drive but the system's own, in letter order.
pub fn list() -> Vec<Drive> {
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).trim_end_matches(':').to_uppercase();
    ('A'..='Z')
        .map(|c| c.to_string())
        .filter(|l| *l != system && l != "A" && l != "B")
        .filter(|l| std::path::Path::new(&format!("{l}:\\")).is_dir())
        .map(|l| {
            let root = format!("{l}:\\");
            let (label, total) = info(&root);
            Drive { windows: std::path::Path::new(&format!("{l}:\\Users")).is_dir(), letter: l, label, total }
        })
        .collect()
}

#[cfg(windows)]
fn info(root: &str) -> (String, Option<u64>) {
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumeInformationW};
    let w: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let mut name = [0u16; 261];
    // SAFETY: a NUL-ended path and a buffer of the size given; the other outputs are optional
    let ok = unsafe {
        GetVolumeInformationW(w.as_ptr(), name.as_mut_ptr(), name.len() as u32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
    };
    let label = if ok != 0 { String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(0)]) } else { String::new() };
    let mut total: u64 = 0;
    // SAFETY: as above; only the total is asked
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), std::ptr::null_mut(), &mut total, std::ptr::null_mut()) };
    (label, (ok != 0).then_some(total))
}

#[cfg(not(windows))]
fn info(_root: &str) -> (String, Option<u64>) {
    (String::new(), None)
}

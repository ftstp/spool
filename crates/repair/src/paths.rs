//! How copies are named: "Name [RCN].als" (the codes of what was done, see the treatments),
//! "Name [R] 2.als" when "Name [R].als" was retouched in Live (never overwritten), and the
//! older "Name - fixed.als".
use std::path::{Path, PathBuf};

pub const CODES: &str = "RCNOPMG";

/// ("Name", Some("RCN")) for "Name [RCN]" (also "Name [RCN] 2"), ("Name", Some("R")) for
/// "Name - fixed", (stem, None) for an original.
pub fn split_processed(stem: &str) -> (String, Option<String>) {
    if let Some(base) = stem.strip_suffix(" - fixed") {
        return (base.to_string(), Some("R".into()));
    }
    // "Base [CODES]" or "Base [CODES] N"
    let (head, _n) = match stem.rsplit_once(' ') {
        Some((h, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && h.ends_with(']') => (h, Some(n)),
        _ => (stem, None),
    };
    if let Some(open) = head.strip_suffix(']').and_then(|h| h.rfind(" [").map(|i| (i, h))) {
        let (i, h) = open;
        let codes = &h[i + 2..];
        if !codes.is_empty() && codes.chars().all(|c| CODES.contains(c)) {
            return (h[..i].to_string(), Some(codes.to_string()));
        }
    }
    (stem.to_string(), None)
}

/// "Name [codes].als" (codes in the canonical order), "Name [codes] n.als" from n = 2.
pub fn processed_path(als: &Path, codes: &str, n: u32) -> PathBuf {
    let stem = als.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = als.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let (base, _) = split_processed(&stem);
    let codes: String = CODES.chars().filter(|c| codes.contains(*c)).collect();
    let num = if n > 1 { format!(" {n}") } else { String::new() };
    als.with_file_name(format!("{base} [{codes}]{num}{ext}"))
}

pub fn is_processed(rel: &str) -> bool {
    let stem = Path::new(rel).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    split_processed(&stem).1.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(split_processed("Song [RCN]"), ("Song".into(), Some("RCN".into())));
        assert_eq!(split_processed("Song [R] 2"), ("Song".into(), Some("R".into())));
        assert_eq!(split_processed("Song - fixed"), ("Song".into(), Some("R".into())));
        assert_eq!(split_processed("Song [demo]"), ("Song [demo]".into(), None));
        assert_eq!(split_processed("Song 2"), ("Song 2".into(), None));
        assert_eq!(processed_path(Path::new("a/Song [R].als"), "NR", 1), PathBuf::from("a/Song [RN].als"));
        assert_eq!(processed_path(Path::new("a/Song - fixed.als"), "R", 3), PathBuf::from("a/Song [R] 3.als"));
    }
}

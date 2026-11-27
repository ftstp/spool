//! Before a folder moves: which samples its projects would lose. Ported from the prototype's
//! fileops.move_risks.
use crate::SAMPLE_EXT;
use crate::samples::{Env, candidates, project_root};
use crate::xml::{iter_tag, val};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The real, full path (case and links resolved), so "inside" compares like Windows does.
fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn inside(p: &Path, folder: &Path) -> bool {
    real(p).starts_with(real(folder))
}

/// Every .als under `folder`, Ableton's own Backup folders left out.
pub fn project_files(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().to_lowercase();
            if ft.is_dir() {
                if name != "backup" {
                    stack.push(e.path());
                }
            } else if name.ends_with(".als") {
                out.push(e.path());
            }
        }
    }
    out
}

/// Samples that would be lost if `folder` moved: [(project file name, sample name)].
///
/// A sample survives when, after the move, Live still finds it: by its path relative to the
/// project (both moving together) or by a stored / library path left untouched. Samples
/// already missing are not this move's problem.
pub fn move_risks(folder: &Path, env: &Env) -> Vec<(String, String)> {
    let mut risks = Vec::new();
    for p in project_files(folder) {
        let mut raw = Vec::new();
        let ok = std::fs::File::open(&p).and_then(|f| flate2::read::MultiGzDecoder::new(f).read_to_end(&mut raw));
        if ok.is_err() {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&raw) else { continue };
        let opts = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: u32::MAX };
        let Ok(doc) = roxmltree::Document::parse_with_options(text, opts) else { continue };
        let proj = project_root(&p);
        let proj_moves = inside(&proj, folder);
        for fr in iter_tag(doc.root_element(), "FileRef") {
            let (shown, cands) = candidates(fr, &proj, env);
            let low = shown.to_lowercase();
            if shown.is_empty() || !SAMPLE_EXT.iter().any(|x| low.ends_with(x)) {
                continue;
            }
            if !cands.iter().any(|c| c.is_file()) {
                continue;
            }
            let rel = val(Some(fr), "RelativePath").unwrap_or("").replace('\\', "/");
            let by_rel = proj_moves
                && !rel.is_empty()
                && val(Some(fr), "RelativePathType") == Some("3")
                && proj.join(&rel).is_file()
                && inside(&proj.join(&rel), folder);
            let elsewhere = cands.iter().any(|c| c.is_file() && !inside(c, folder));
            if !(by_rel || elsewhere) {
                let file = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                risks.push((file, shown.rsplit('/').next().unwrap_or("").to_string()));
            }
        }
    }
    risks
}

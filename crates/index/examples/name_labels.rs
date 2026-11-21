//! The family and category the name rules give each sample of a list (one path a line), as
//! JSON lines: what a model is taught from.
//!     name_labels < paths.txt > labels.jsonl
use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let p = std::path::Path::new(line.trim());
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let folders: Vec<String> = p.parent().map(|d| d.components().rev().take(3).map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().into_iter().rev().collect()).unwrap_or_default();
        let fr: Vec<&str> = folders.iter().map(|s| s.as_str()).collect();
        let n = index::samples::read_name(&stem, &fr);
        println!("{}", serde_json::json!({"path": line.trim(), "family": n.family, "cat": n.cat}));
    }
}

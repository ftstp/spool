//! What the loose-media view lists on a COPY of a data folder (files only read).
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let m = app.fs_media().expect("media");
    let mut kinds = std::collections::BTreeMap::new();
    for f in &m {
        for it in &f.items {
            *kinds.entry(it.kind.clone()).or_insert(0) += 1;
        }
    }
    eprintln!("{} folders; {:?}", m.len(), kinds);
    if let Some(out) = std::env::args().nth(2) {
        let all: Vec<String> = m.iter().flat_map(|f| f.items.iter().map(|i| i.rel.clone())).collect();
        std::fs::write(out, all.join("
")).unwrap();
    }
}

//! automated_check <projects root> [max]: reads the sets (read-only) and counts plugins moved by an envelope.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let max: usize = a.get(1).and_then(|m| m.parse().ok()).unwrap_or(150);
    let env = als::Env::detect(None);
    let (mut sets, mut plugs, mut auto) = (0, 0, 0);
    let mut shown = 0;
    for e in walkdir(std::path::Path::new(&a[0])) {
        if sets >= max { break; }
        let info = als::read_als(&e, &env);
        if info.error.is_some() { continue; }
        sets += 1;
        for p in &info.plugins {
            plugs += 1;
            if p.automated {
                auto += 1;
                if shown < 12 { println!("  {} [{}] in {}", p.name, p.format, e.file_name().unwrap().to_string_lossy()); shown += 1; }
            }
        }
    }
    println!("{sets} projets, {plugs} plugins, {auto} automatises");
}
fn walkdir(d: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut st = vec![d.to_path_buf()];
    while let Some(x) = st.pop() {
        let Ok(rd) = std::fs::read_dir(&x) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { if !p.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("backup") { st.push(p); } }
            else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("als")) { out.push(p); }
        }
    }
    out
}

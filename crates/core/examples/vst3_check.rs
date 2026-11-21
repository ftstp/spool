//! vst3_check <data dir COPY> <projects root>: fresh base, full scan, then every safe VST3
//! switched in one batch (timed), with the projects' counts before and after.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let app = app_core::App::open(&dir).unwrap();
    app.add_root(&a[1]).unwrap();
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    println!("scan {:.0} s", t.elapsed().as_secs_f64());
    let p = |s: &app_core::Stats| format!("{} projets: {} s'ouvrent, {} reparables, {} reglages perdus, {} a installer", s.projects.total, s.projects.ok, s.projects.fix, s.projects.lost, s.projects.bad - s.projects.lost);
    println!("avant  {}", p(&app.stats().unwrap()));
    let rv = app.repair_view(None).unwrap();
    let list: Vec<(String, Option<String>)> = rv.plugins.iter()
        .filter(|p| p.vst3.is_none() && p.automated < p.files)
        .filter_map(|p| p.vst3_targets.as_ref()?.iter().find(|t| t.compatible).map(|t| (p.name.clone(), Some(t.uid.clone()))))
        .collect();
    for p in rv.plugins.iter().filter(|p| p.vst3_targets.as_ref().is_some_and(|t| !t.is_empty())) {
        println!("  {} : {} projets, {} automatises", p.name, p.files, p.automated);
    }
    let t = std::time::Instant::now();
    app.set_vst3_many(&list).unwrap();
    println!("{} plugins passes en VST3 en {} ms", list.len(), t.elapsed().as_millis());
    println!("apres  {}", p(&app.stats().unwrap()));
}

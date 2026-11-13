//! chains_check <data dir COPY> <style> <live> [out.als]: scans, lists the chains, picks one per
//! part for the style and, with an output path, builds the clean start from them.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let (style, live) = (a[1].clone(), a[2].clone());
    let app = app_core::App::open(&dir).unwrap();
    let t = std::time::Instant::now();
    app.scan(false).unwrap();
    println!("scan {:.0} s", t.elapsed().as_secs_f64());
    let con = store::rusqlite::Connection::open(dir.join("plugin-fixer.sqlite")).unwrap();
    let t = std::time::Instant::now();
    let cat = app_core::chains::catalogue(&con, Default::default()).unwrap();
    println!("catalogue {} ms: {} chains, {} playable", t.elapsed().as_millis(), cat.chains.len(), cat.chains.iter().filter(|c| c.playable).count());
    println!("styles {:?}", cat.styles.iter().take(10).map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    println!("parts {:?}", cat.parts.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    println!("versions {:?}", cat.versions.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    for part in app_core::chains::PARTS {
        println!("== {part}");
        for c in cat.chains.iter().filter(|c| c.part == part && c.styles.iter().any(|s| s.label == style)).take(4) {
            let ok = c.uses.iter().filter(|u| u.problems.is_empty()).count();
            println!("  {:>2} songs, {}/{} playable  {}", c.songs, ok, c.uses.len(), c.key.chars().take(110).collect::<String>());
            if ok == 0 {
                println!("       {:?}", c.uses[0].problems);
            }
        }
    }
    let parts: Vec<String> = ["kick", "snare", "hats", "bass", "keys", "lead"].iter().map(|s| s.to_string()).collect();
    let picks = app_core::chains::pick_parts(&cat, &style, &live, &parts);
    println!("== picks for {style} in Live {live}");
    for p in &picks {
        println!("  {:6} {:6} {:28} <- {} / {}", p.part, p.why, p.key.chars().take(28).collect::<String>(), p.title, p.track_name);
    }
    if let Some(out) = a.get(3) {
        let root: String = con.query_row("select v from settings where k='root'", [], |r| r.get::<_, String>(0)).unwrap();
        let root: String = serde_json::from_str(&root).unwrap();
        let abs = |rel: &str| std::path::PathBuf::from(&root).join(rel);
        let colors = [("kick", 0), ("snare", 1), ("hats", 2), ("kit", 0), ("perc", 3), ("bass", 9), ("keys", 5), ("pad", 7), ("lead", 11)];
        let o = repair::assemble::Order {
            base: abs(&picks[0].rel),
            picks: picks
                .iter()
                .map(|p| repair::assemble::Pick {
                    set: abs(&p.rel),
                    track: p.track.clone(),
                    name: Some(p.part[..1].to_uppercase() + &p.part[1..]),
                    color: colors.iter().find(|c| c.0 == p.part).map(|c| c.1),
                })
                .collect(),
            main_devices: vec!["StereoGain".into(), "Limiter".into()],
            donors: vec![],
            tempo: Some(174.0),
            scale: None,
            markers: {
                let known = app_core::chains::known_styles(&con).unwrap();
                let st = |f: Option<&str>, d: &str| app_core::chains::style_of(f, d, &known);
                let s = app_core::structure::learn(&con, &style, Some(174.0), Some(210.0), None, &st).unwrap();
                s.sections.iter().map(|x| (x.bar as f64 * 4.0, x.name.clone())).collect()
            },
            volume_db: -6.0,
            keep_mix: false,
            out: out.into(),
        };
        match repair::assemble::assemble(&o) {
            Ok(r) => println!("{}", serde_json::to_string_pretty(&r).unwrap()),
            Err(e) => println!("ERREUR {e}"),
        }
    }
}

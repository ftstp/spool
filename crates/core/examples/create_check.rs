//! create_check <data dir COPY> <scratch folder> [style]: prints the Create plan, then copies
//! the picked projects into the scratch folder, points the COPIED base at it and creates a song
//! there (the user's folders are only read).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let scratch = std::path::PathBuf::from(&a[1]);
    let app = app_core::App::open(&dir).unwrap();
    let req = app_core::create::CreateReq { style: a.get(2).cloned(), ..Default::default() };
    let t = std::time::Instant::now();
    let plan = app.create_plan(req.clone()).unwrap();
    println!("plan {} ms: style {} live {} folder {}", t.elapsed().as_millis(), plan.style, plan.live, plan.folder);
    println!("styles {:?}", plan.styles.iter().take(8).map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    println!("versions {:?}", plan.versions.iter().map(|c| (&c.label, c.n)).collect::<Vec<_>>());
    println!("structure bpm {:?} from {} : {:?}", plan.structure.bpm, plan.structure.learnt_from, plan.structure.sections.iter().map(|s| (&s.name, s.bars)).collect::<Vec<_>>());
    if std::env::var("DRAW").is_ok() {
        for (seed, secs) in [(1u64, None), (2, Some(150.0)), (3, Some(330.0)), (4, None)] {
            let p = app.create_plan(app_core::create::CreateReq { seed: Some(seed), seconds: secs, ..req.clone() }).unwrap();
            println!("seed {seed} {secs:?}: {:?} = {:.0}s", p.structure.sections.iter().map(|s| format!("{} {}", s.name, s.bars)).collect::<Vec<_>>(), p.structure.seconds);
            for x in p.parts.iter().filter(|x| x.pick.is_some()).take(4) {
                println!("   {} [{}] {} ({})", x.part, x.pool, x.label, x.pick.as_ref().unwrap().why);
            }
        }
        let k = plan.parts.iter().find(|x| x.part == "kit").and_then(|x| x.pick.clone()).or(plan.parts.iter().find_map(|x| x.pick.clone())).unwrap();
        let t = std::time::Instant::now();
        let v = app.track_view(&k.rel, &k.track).unwrap();
        println!("track_view {} ms: {}", t.elapsed().as_millis(), serde_json::to_string(&v).unwrap().chars().take(1500).collect::<String>());
        return;
    }
    if let Ok(f) = std::env::var("DUMP_PLAN") {
        let plan = app.create_plan(app_core::create::CreateReq { seed: Some(7), ..req.clone() }).unwrap();
        std::fs::write(&f, serde_json::to_string(&plan).unwrap()).unwrap();
        let mut tracks = serde_json::Map::new();
        for p in plan.parts.iter().filter_map(|p| p.pick.as_ref()) {
            if let Ok(v) = app.track_view(&p.rel, &p.track) {
                tracks.insert(format!("{}|{}", p.rel, p.track), serde_json::to_value(v).unwrap());
            }
        }
        std::fs::write(f.replace("dev-create", "dev-tracks"), serde_json::Value::Object(tracks).to_string()).unwrap();
        return;
    }
    for p in &plan.parts {
        println!("  {:6} {:?} {} | {} alts", p.part, p.pick.as_ref().map(|x| (&x.why, &x.title, &x.track_name)), p.label, p.alts.len());
    }
    // copy the picked projects into the scratch folder, keeping their names
    let roots = app.roots();
    let parts: Vec<String> = plan.parts.iter().filter(|p| p.pick.is_some()).map(|p| p.part.clone()).take(6).collect();
    for p in plan.parts.iter().filter(|p| parts.contains(&p.part)) {
        let rel = &p.pick.as_ref().unwrap().rel;
        let src = roots.abs(rel).unwrap();
        let dst = scratch.join(rel);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        if !dst.exists() {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
    let con = store::rusqlite::Connection::open(dir.join("plugin-fixer.sqlite")).unwrap();
    con.execute("update settings set v=? where k='root'", [serde_json::to_string(&scratch.to_string_lossy()).unwrap()]).unwrap();
    con.execute("delete from settings where k='more_roots'", []).unwrap();
    let req = app_core::create::CreateReq { parts, name: "Test : création".into(), ..req };
    let t = std::time::Instant::now();
    match app.create_song(req.clone()) {
        Ok(r) => println!("created in {} ms: {}", t.elapsed().as_millis(), serde_json::to_string_pretty(&r).unwrap()),
        Err(e) => println!("ERREUR {e}"),
    }
    // a second time: refused, nothing overwritten
    println!("again: {:?}", app.create_song(req).err());
}

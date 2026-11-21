//! On a COPY of a base: ignoring a plugin, then giving it back; choosing a replacement
//! version, then removing it. Each leaves the projects' states and severities as before.
//!   choices_check <dir with a copy of plugin-fixer.sqlite>
fn files(app: &app_core::App) -> Vec<(String, String, i64)> {
    let st = store::Store::open(&app.data_dir.join("plugin-fixer.sqlite")).unwrap();
    let mut s = st.con.prepare("select rel, plugstate, sev from files order by rel").unwrap();
    s.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let app = app_core::App::open(&dir).unwrap();
    let before = files(&app);
    let v = app.repair_view(None).unwrap();
    let top = v.plugins.iter().find(|p| p.status == "missing" && p.format != "au").expect("a missing plugin");
    println!("ignoring \"{}\" ({} projects)", top.name, top.files);
    let t = std::time::Instant::now();
    app.set_ignored(&top.name, true, false).unwrap();
    println!("  in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let mid = files(&app);
    println!("  severities changed: {}", before.iter().zip(&mid).filter(|(a, b)| a.2 != b.2).count());
    let v2 = app.repair_view(None).unwrap();
    assert!(
        v2.ignored.iter().any(|p| p.name == top.name)
            && !v2.plugins.iter().any(|p| p.name == top.name && p.status == "missing")
    );
    app.set_ignored(&top.name, false, false).unwrap();
    assert!(files(&app) == before, "ignore + give back must restore");
    println!("  given back: IDENTICAL");

    if let Some(p) = v.plugins.iter().find(|p| {
        p.format == "vst2" && p.trial.as_deref() == Some("") && p.trial_targets.as_ref().is_some_and(|t| !t.is_empty())
    }) {
        let to = p.trial_targets.as_ref().unwrap()[0].clone();
        println!("trying \"{}\" with \"{to}\"", p.name);
        app.set_trial(&p.name, "vst2", Some(&to)).unwrap();
        let n = files(&app).iter().zip(&before).filter(|(a, b)| a.1 != b.1).count();
        println!("  projects whose plugin states changed: {n}");
        let plan_has = app.fixable().unwrap().iter().filter(|f| f.trials > 0).count();
        println!("  repairable with a replacement: {plan_has}");
        app.set_trial(&p.name, "vst2", None).unwrap();
        let after = files(&app);
        assert!(after == before, "trial + removal must restore");
        println!("  removed: IDENTICAL");
    } else {
        println!("no plugin with an installed replacement to try");
    }
}

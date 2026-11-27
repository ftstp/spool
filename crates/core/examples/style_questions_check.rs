//! On a COPY of a data folder: the questions the styles page asks (the songs Spool is unsure
//! of), how long they take, and what answering the first one changes.
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    // the suggestions again, with the styles heard by CLAP (the user's words kept)
    let r0 = app.style_review().unwrap();
    app.set_style_vocab(r0.words.clone(), r0.links.clone(), None).unwrap();
    let shown = |app: &app_core::App| -> std::collections::BTreeMap<i64, (String, String)> {
        app.songs().unwrap().into_iter().map(|s| (s.id, (s.style.clone().unwrap_or_default(), s.title.clone().unwrap_or_default()))).collect()
    };
    let before = shown(&app);
    let dump = |name: &str| {
        if let Ok(d) = std::env::var("DUMP") {
            let m: std::collections::BTreeMap<i64, String> = shown(&app).into_iter().map(|(k, v)| (k, v.0)).collect();
            std::fs::write(format!("{d}/{name}.json"), serde_json::to_string(&m).unwrap()).unwrap();
        }
    };
    dump("before");
    let t = std::time::Instant::now();
    let r = app.style_review().unwrap();
    eprintln!("{} questions in {} ms", r.questions.len(), t.elapsed().as_millis());
    for q in &r.questions {
        eprintln!("  #{} {:?}: {:?} (helps {})  [{}]", q.id, q.title, q.options, q.helps, q.temoin);
    }
    let Some(q) = r.questions.first() else { return };
    let style = std::env::var("ANSWER").unwrap_or_else(|_| q.options.first().cloned().unwrap_or_else(|| "dnb".into()));
    let a = app.style_answer(q.id, Some(style.clone())).unwrap();
    dump("after");
    eprintln!("answered #{} {style}: {} other songs now suggested {style}", q.id, a.classed);
    for (id, (st, t)) in shown(&app) {
        if before.get(&id).map(|x| &x.0) != Some(&st) {
            eprintln!("    {id} {t:?}: {:?} -> {st:?}", before.get(&id).map(|x| &x.0));
        }
    }
    if let Ok(ids) = std::env::var("IDS") {
        for id in ids.split(',').filter_map(|x| x.parse::<i64>().ok()) {
            eprintln!("  #{id} {:?}", app.style_guess(id).unwrap().iter().map(|o| (o.style.clone(), o.pct, o.why.clone())).collect::<Vec<_>>());
        }
    }
    eprintln!("next: {:?}", a.review.questions.iter().map(|q| (q.id, q.options.clone(), q.helps)).collect::<Vec<_>>());
    if let Some(q2) = a.review.questions.first() {
        let b = app.style_answer(q2.id, None).unwrap();
        eprintln!("skipped #{}: {} questions left, skipped {:?}", q2.id, b.review.questions.len(), app.style_skipped());
    }
}

//! On a COPY of a data folder: the styles page's questions (words that may be styles,
//! styles heard otherwise) and the styles shown before / after the suggestions are
//! recomputed with the current rules, song by song, then what a "yes" to the first word changes.
fn main() {
    let dir = std::env::args().nth(1).expect("data folder");
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let shown = |app: &app_core::App| -> std::collections::BTreeMap<i64, (String, String, String)> {
        let mut m = std::collections::BTreeMap::new();
        for s in app.songs().unwrap() {
            let d = app.song(s.id).unwrap().unwrap();
            let v = d.detected.style.shown.as_ref().map(|v| v.value.clone()).unwrap_or_else(|| "-".into());
            m.insert(s.id, (v, s.title.clone().unwrap_or_default(), s.dir.clone()));
        }
        m
    };
    let count = |m: &std::collections::BTreeMap<i64, (String, String, String)>| {
        let mut c: std::collections::BTreeMap<String, usize> = Default::default();
        for v in m.values() {
            *c.entry(v.0.clone()).or_default() += 1;
        }
        c
    };
    let before = shown(&app);
    let r = app.set_style_vocab(vec![], vec![], Some(vec![])).unwrap();
    let after = shown(&app);
    eprintln!("before: {:?}", count(&before));
    eprintln!("after:  {:?}", count(&after));
    for (id, (b, t, d)) in &before {
        let a = &after[id].0;
        if a != b {
            eprintln!("  {id} {t:?}: {b} -> {a}   [{d}]");
        }
    }
    if let Ok(ids) = std::env::var("IDS") {
        for id in ids.split(',').filter_map(|x| x.parse::<i64>().ok()) {
            eprintln!("  #{id} {:?}", app.style_guess(id).unwrap().iter().map(|o| (o.style.clone(), o.pct, o.why.clone())).collect::<Vec<_>>());
        }
    }
    eprintln!("heard {}/{}", r.heard, r.renders);
    for c in &r.candidates {
        let titles: Vec<String> = c.songs.iter().take(3).map(|s| s.title.clone().unwrap_or_default()).collect();
        eprintln!("  WORD {:?} n={} heard {:?} ({}) {:?}", c.word, c.n, c.heard, c.genre, titles);
    }
    for g in &r.groups {
        eprintln!("  DISAGREE {:?} heard {:?} n={}", g.mine, g.heard, g.n);
    }
    if std::env::args().nth(2).as_deref() == Some("yes")
        && let Some(c) = r.candidates.first()
    {
        let name = std::env::args().nth(3).unwrap_or_else(|| c.word.clone());
        let r2 = app.style_word_yes(&c.word, &name, None).unwrap();
        eprintln!("after yes to {:?}: words {:?} links {:?}, {} words left", c.word, r2.words, r2.links, r2.candidates.len());
        eprintln!("styles shown: {:?}", count(&shown(&app)));
    }
}

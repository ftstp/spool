//! Changes then undoes them on a COPY of a base, and checks the songs and suggestions are
//! back exactly as they were: undo_check <dir holding plugin-fixer.sqlite (a copy)>
use serde_json::{Map, Value, json};

fn dump(app: &app_core::App) -> String {
    let st = store::Store::open(&app.data_dir.join("plugin-fixer.sqlite")).unwrap();
    let mut out = String::new();
    for sql in [
        "select * from songs order by id",
        "select id, song_id, kind, value, sig, state, decided from suggestions order by id",
        "select * from stage_log order by rowid",
    ] {
        let mut s = st.con.prepare(sql).unwrap();
        let n = s.column_count();
        let rows = s
            .query_map([], |r| Ok((0..n).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join("|")))
            .unwrap();
        for r in rows {
            out += &r.unwrap();
            out.push('\n');
        }
    }
    out
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("dir"));
    let app = app_core::App::open(&dir).unwrap();
    let before = dump(&app);
    // a song with an open stage suggestion, and one with a style suggestion
    let ov = app.overview().unwrap();
    let g = ov.queue.first().expect("a stage suggestion");
    let sg = ov.style_queue.first().expect("a style suggestion");
    let sid = g.song_id;
    let t = std::time::Instant::now();
    let body: Map<String, Value> = serde_json::from_value(json!({"stage": "mix"})).unwrap();
    println!("{:?}", app.song_edit(sid, body).unwrap());
    println!("{:?}", app.check_set(sid, "lyrics", json!(true)).unwrap());
    println!(
        "{:?}",
        app.song_edit(sid, serde_json::from_value(json!({"title": "Essai", "tempo": "128,5"})).unwrap()).unwrap()
    );
    println!("{:?}", app.suggestion_act(sg.id, true, Some(sg.options.last().unwrap().style.clone())).unwrap());
    println!("{:?}", app.song_edit(sg.song_id, serde_json::from_value(json!({"style": ""})).unwrap()).unwrap());
    let ids: Vec<i64> = ov.queue.iter().skip(1).take(3).map(|q| q.id).collect();
    println!("{:?}", app.accept_all(ids).unwrap());
    println!("6 changes in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let d = app.song(sid).unwrap().unwrap();
    println!("sheet: {:?} {:?} {:?}", d.song.title, d.song.stage, d.song.tempo);
    assert!(dir.join("choices.json").is_file());
    let t = std::time::Instant::now();
    for _ in 0..6 {
        let (label, st) = app.undo().unwrap();
        println!("undone: {label:?} ({} left)", st.left);
    }
    println!("6 undos in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let after = dump(&app);
    if before == after {
        println!("IDENTICAL after undo");
    } else {
        let (b, a): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
        println!("DIFFERENT: {} vs {} lines", b.len(), a.len());
        for (x, y) in b.iter().zip(a.iter()).filter(|(x, y)| x != y).take(8) {
            println!("- {x}\n+ {y}");
        }
        std::process::exit(1);
    }
}

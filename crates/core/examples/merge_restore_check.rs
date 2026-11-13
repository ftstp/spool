//! On COPIES of a base: (1) merge a song into another then split it back, songs as before;
//! (2) a fresh base next to the choices.json of the first gets the user's choices back
//! after its first scan.  merge_restore_check <dir with a copy of plugin-fixer.sqlite> <empty dir>
use serde_json::json;

fn songs(app: &app_core::App) -> Vec<String> {
    let st = store::Store::open(&app.data_dir.join("plugin-fixer.sqlite")).unwrap();
    let mut s = st
        .con
        .prepare(
            "select song_dir, vkey, title, artist, style, stage, temoin, tempo, date, notes, user_fields, checklist, archived \
             from songs where gone=0 order by song_dir, vkey",
        )
        .unwrap();
    let n = s.column_count();
    s.query_map([], |r| Ok((0..n).map(|i| format!("{:?}", r.get_ref(i).unwrap())).collect::<Vec<_>>().join("|")))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (dir, fresh) = (std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let app = app_core::App::open(&dir).unwrap();
    let before = songs(&app);
    // two songs sharing a folder are the usual merge
    let list = app.songs().unwrap();
    let (x, y) = list
        .iter()
        .flat_map(|x| list.iter().map(move |y| (x, y)))
        .find(|(x, y)| x.id != y.id && x.dir == y.dir && !x.dir.is_empty())
        .expect("two songs in one folder");
    let t = std::time::Instant::now();
    println!("{}", app.merge_song(x.id, y.id).unwrap());
    let d = app.song(y.id).unwrap().unwrap();
    println!("merged in {:.0} ms: {:?}, {} songs now", t.elapsed().as_secs_f64() * 1000.0, d.merged, songs(&app).len());
    assert!(app.song(x.id).unwrap().is_none_or(|s| s.files.is_empty()) || songs(&app).len() < before.len());
    let key = d.merged[0].key.clone();
    app.split_song(y.id, &key).unwrap();
    let after = songs(&app);
    println!("split: {}", if after == before { "songs IDENTICAL to before" } else { "DIFFERENT" });
    assert_eq!(after, before);

    // a choice to bring back, then a fresh base beside the mirror
    let s0 = &list[0];
    app.song_edit(s0.id, serde_json::from_value(json!({"notes": "restored", "stage": "mix"})).unwrap()).unwrap();
    std::fs::create_dir_all(&fresh).unwrap();
    std::fs::copy(dir.join("choices.json"), fresh.join("choices.json")).unwrap();
    let app2 = app_core::App::open(&fresh).unwrap();
    println!("fresh base, root restored: {:?}", app2.root().is_some());
    let t = std::time::Instant::now();
    app2.scan(false).unwrap();
    println!("first scan in {:.0} s", t.elapsed().as_secs_f64());
    let back = app2.songs().unwrap();
    let s1 = back.iter().find(|s| s.dir == s0.dir && s.title == s0.title).expect("song found again");
    let d1 = app2.song(s1.id).unwrap().unwrap();
    println!("restored: stage {:?}, notes {:?}", d1.song.stage, d1.song.notes);
    assert_eq!((d1.song.stage.as_deref(), d1.song.notes.as_deref()), (Some("mix"), Some("restored")));
    println!("OK");
}

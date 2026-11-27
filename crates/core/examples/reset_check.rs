//! Starting over, on a COPY of a base in a scratch data dir: reset_check <base.sqlite> <scratch dir>
//! The base and the choices mirror go to backups/, the new base is empty and is not refilled
//! from the mirror, the app keeps working, and only the last 3 resets are kept.
fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let data = scratch.join("data");
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();
    std::fs::copy(&src, data.join("plugin-fixer.sqlite")).unwrap();

    let app = app_core::App::open(&data).unwrap();
    let before = app.songs().unwrap().len();
    println!("before: {before} songs, root {:?}", app.root());
    // a change writes the choices mirror, as in real use
    app.set_ignored("ZZ test plugin", true, false).unwrap();
    assert!(data.join("choices.json").exists());

    for i in 0..4 {
        let name = app.reset().unwrap();
        println!("reset {i}: backup {name}");
        if i == 0 {
            let b = store::Store::open(&data.join("backups").join(&name)).unwrap();
            let n: i64 = b.con.query_row("select count(*) from songs", [], |r| r.get(0)).unwrap();
            println!("  the backup holds {n} songs");
            let orig: i64 = store::Store::open(&src).unwrap().con.query_row("select count(*) from songs", [], |r| r.get(0)).unwrap();
            assert_eq!(n, orig, "every song row is in the backup");
        }
        assert_eq!(app.songs().unwrap().len(), 0);
        assert_eq!(app.root(), None);
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }
    assert!(!data.join("choices.json").exists());
    let mut kept: Vec<String> = std::fs::read_dir(data.join("backups"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    kept.sort();
    println!("backups: {kept:?}");
    assert_eq!(kept.iter().filter(|n| n.ends_with(".sqlite")).count(), 3);

    // the app still works after resets, and after a reopen
    app.set_root(scratch.to_str().unwrap()).unwrap();
    drop(app);
    let app = app_core::App::open(&data).unwrap();
    println!("reopened: {} songs, root {:?}", app.songs().unwrap().len(), app.root());
    assert_eq!(app.root().as_deref(), scratch.to_str());
    println!("ok");
}

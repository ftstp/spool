//! arrange_check <empty data dir> <scratch projects folder>: physical switch, release
//! arranging on disk and its undo, virtual mood board links. Only ever on scratch copies.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let app = app_core::App::open(std::path::Path::new(&a[0])).unwrap();
    app.add_root(&a[1]).unwrap();
    app.scan(false).unwrap();
    let songs = app.songs().unwrap();
    println!("{} songs", songs.len());
    // off: nothing on disk moves
    println!("physical {} ; move refused: {:?}", app.physical(), app.fs_move(vec![("x".into(), "y".into())]).err().map(|e| e.chars().take(40).collect::<String>()));
    let hints = app.release_hints().unwrap();
    let h = &hints[0];
    let mk = |title: &str, kind: &str, ids: Vec<i64>, folder: Option<String>| app_core::releases::Release {
        id: String::new(), title: title.into(), kind: kind.into(), artist: String::new(), date: None, cover_song: None, notes: String::new(),
        songs: ids.into_iter().map(Some).collect(), folder, created: 0.0,
    };
    let (sooth, _) = app.release_save(mk(&h.title, &h.kind, h.songs.clone(), Some(h.folder.clone()))).unwrap();
    let p = app.release_arrange_plan(&sooth, None).unwrap();
    println!("Ember plan: target {} moves {} already {}", p.target, p.moves.len(), p.already);
    let three: Vec<i64> = h.songs.iter().take(3).copied().collect();
    let (test, _) = app.release_save(mk("Test", "ep", three, None)).unwrap();
    let p = app.release_arrange_plan(&test, None).unwrap();
    println!("Test plan: target {} exists {} moves {:?} skipped {}", p.target, p.exists, p.moves, p.skipped.len());
    println!("arrange while off: {:?}", app.release_arrange(&test, None).err().map(|e| e.chars().take(40).collect::<String>()));
    app.set_config("physical", serde_json::json!(true)).unwrap();
    let r = app.release_arrange(&test, None).unwrap();
    println!("arranged: done {:?} skipped {:?}", r.done, r.skipped.iter().map(|s| &s.why).collect::<Vec<_>>());
    let target = std::path::Path::new(&a[1]).join(&p.target);
    println!("on disk: {:?}", std::fs::read_dir(&target).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>()));
    for r in app.releases().unwrap() {
        println!("  release {} : {}/{} songs found, folder {:?}", r.title, r.songs.iter().filter(|s| s.is_some()).count(), r.songs.len(), r.folder);
    }
    let (u, _) = app.undo().unwrap();
    println!("undo {:?}; target still there: {}", u, target.exists());
    for r in app.releases().unwrap() {
        println!("  release {} : {}/{} songs found", r.title, r.songs.iter().filter(|s| s.is_some()).count(), r.songs.len());
    }
    // virtual board: a file shown on a song's board, nothing moves
    let sid = songs[0].id;
    let other = songs.iter().find(|s| s.id != sid).unwrap().rel.clone();
    app.board_link(sid, vec![other.clone()]).unwrap();
    let d = app.song(sid).unwrap().unwrap();
    println!("board links: {:?}", d.board.items.iter().filter(|i| i.place == "lien").map(|i| &i.rel).collect::<Vec<_>>());
    println!("file still in place: {}", std::path::Path::new(&a[1]).join(&other).exists());
    app.board_unlink(sid, &other).unwrap();
    println!("after unlink: {}", app.song(sid).unwrap().unwrap().board.items.iter().filter(|i| i.place == "lien").count());
}

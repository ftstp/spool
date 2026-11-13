//! File operations on a COPY of a real project in a scratch root (nothing next to the user's
//! projects): create a folder, move a song folder into it (the song keeps its stage), undo,
//! loose media found, an image into a mood board and back.  fs_check <project folder> <scratch>
use std::path::{Path, PathBuf};

fn copy_tree(src: &Path, dst: &Path) {
    if src.is_dir() {
        std::fs::create_dir_all(dst).unwrap();
        for e in std::fs::read_dir(src).unwrap().flatten() {
            copy_tree(&e.path(), &dst.join(e.file_name()));
        }
    } else {
        std::fs::copy(src, dst).unwrap();
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (PathBuf::from(a.next().unwrap()), PathBuf::from(a.next().unwrap()));
    let _ = std::fs::remove_dir_all(&scratch);
    let root = scratch.join("projets");
    let name = src.file_name().unwrap().to_string_lossy().into_owned();
    copy_tree(&src, &root.join("Groupe").join(&name));
    std::fs::create_dir_all(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox").join("ref.png"), b"\x89PNG\r\n\x1a\nfake").unwrap();

    let app = app_core::App::open(&scratch.join("data")).unwrap();
    app.set_root(root.to_str().unwrap()).unwrap();
    app.scan(false).unwrap();
    let song = app.songs().unwrap().into_iter().find(|s| s.dir.starts_with("Groupe")).expect("the song");
    println!("song {} {:?} in {:?}", song.id, song.title, song.dir);
    let mut body = serde_json::Map::new();
    body.insert("stage".into(), "mix".into());
    app.song_edit(song.id, body).unwrap();

    println!("folders: {:?}", app.fs_folders());
    println!("media: {:?}", app.fs_media().unwrap().iter().map(|m| (&m.dir, m.n)).collect::<Vec<_>>());
    let made = app.fs_mkdir("Rangement").unwrap();
    println!("mkdir -> {made}");
    // the SONG folder ("Groupe"; Ableton's "... Project" folder inside is not the song's)
    let from = "Groupe".to_string();
    let plan = app.fs_plan(&from, "Rangement").unwrap();
    println!("plan: {} -> {} risks {:?}", plan.src, plan.dst, plan.risks);
    let r = app.fs_move(vec![(from.clone(), "Rangement".into())]).unwrap();
    println!("moved: {:?} skipped {:?}", r.done, r.skipped.iter().map(|s| &s.why).collect::<Vec<_>>());
    assert!(root.join("Rangement").join("Groupe").join(&name).is_dir() && !root.join(&from).exists());
    app.scan(false).unwrap();
    let after = app.songs().unwrap().into_iter().find(|s| s.id == song.id).expect("same song id after the move");
    println!("after move: id {} stage {:?} dir {:?}", after.id, after.stage, after.dir);
    assert_eq!(after.stage.as_deref(), Some("mix"));
    assert!(after.dir.starts_with("Rangement"));

    let (label, _) = app.undo().unwrap();
    println!("undo: {label:?}");
    assert!(root.join(&from).is_dir() && !root.join("Rangement").join("Groupe").exists());
    app.scan(false).unwrap();
    let back = app.songs().unwrap().into_iter().find(|s| s.id == song.id).expect("same song id after undo");
    println!("after undo: stage {:?} dir {:?}", back.stage, back.dir);
    assert_eq!(back.stage.as_deref(), Some("mix"));
    let (label, _) = app.undo().unwrap();
    println!("undo: {label:?} (Rangement there: {})", root.join("Rangement").exists());

    let (n, copied) = app.fs_to_board(song.id, vec!["inbox/ref.png".into()]).unwrap();
    let d = app.song(song.id).unwrap().unwrap();
    println!("to board: {n} ({copied} copied), board items: {:?}", d.board.items.iter().map(|i| &i.name).collect::<Vec<_>>());
    let (label, _) = app.undo().unwrap();
    println!("undo: {label:?}, ref.png back: {}", root.join("inbox").join("ref.png").exists());
    // a project folder moved OUT of its song folder: the song follows it, stage kept
    app.fs_mkdir("Ailleurs").unwrap();
    let r = app.fs_move(vec![(format!("Groupe/{name}"), "Ailleurs".into())]).unwrap();
    app.scan(false).unwrap();
    let all = app.songs().unwrap();
    let s2 = all.iter().find(|s| s.rel.starts_with("Ailleurs")).expect("a song for the moved project");
    println!("project folder moved {:?}: song id {} (was {}) stage {:?} dir {:?}", r.done, s2.id, song.id, s2.stage, s2.dir);
    println!("reorg plan: {:?}", app.fs_reorg("artist").unwrap().skipped.iter().map(|s| (&s.src, &s.why)).collect::<Vec<_>>());
    println!("ok");
}

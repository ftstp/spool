//! Reference renders before / after a refresh on a COPY of a base, then a common layout rebuilt in
//! a scratch root.  temoin_check <base.sqlite> <scratch> <a project .als to copy>
use std::collections::HashMap;
fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch, als) = (a.next().unwrap(), std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(scratch.join("data")).unwrap();
    std::fs::copy(&src, scratch.join("data/plugin-fixer.sqlite")).unwrap();
    let st = store::Store::open(&scratch.join("data/plugin-fixer.sqlite")).unwrap();
    let get = |st: &store::Store| -> HashMap<i64, Option<String>> {
        let mut q = st.con.prepare("select id, temoin from songs where gone=0").unwrap();
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().flatten().collect()
    };
    let before = get(&st);
    let tx = st.con.unchecked_transaction().unwrap();
    index::regroup(&tx).unwrap();
    tx.commit().unwrap();
    let after = get(&st);
    let changed: Vec<_> = before.iter().filter(|(k, v)| after.get(k) != Some(v)).collect();
    println!("reference base: {} songs, reference render changed for {}", before.len(), changed.len());
    for (k, v) in changed.iter().take(8) { println!("  song {k}: {:?} -> {:?}", v, after[k]); }
    drop(st);

    // a common layout: Projets/<name> Project/<name>.als + renders, all in one folder
    let root = scratch.join("Projets");
    for (name, renders) in [("312 Bbm_104", vec!["312 Bbm_104.wav", "Master.wav"]), ("327 Am_120", vec!["Master.wav", "bounce final.wav"]), ("447v6 harmo", vec![])] {
        let p = root.join(format!("{name} Project"));
        std::fs::create_dir_all(&p).unwrap();
        std::fs::copy(&als, p.join(format!("{name}.als"))).unwrap();
        for r in renders { std::fs::write(p.join(r), vec![0u8; 300_000]).unwrap(); }
    }
    std::fs::write(root.join("loose.wav"), vec![0u8; 300_000]).unwrap();
    let app = app_core::App::open(&scratch.join("data2")).unwrap();
    app.set_root(root.to_str().unwrap()).unwrap();
    app.scan(false).unwrap();
    for s in app.songs().unwrap() {
        let d = app.song(s.id).unwrap().unwrap();
        println!("{:14} dir {:?} render {:?}\n    candidates {:?}\n    other {:?}", s.title.unwrap_or_default(), s.dir, d.song.temoin,
            d.temoins.iter().map(|c| &c.rel).collect::<Vec<_>>(), d.other_audio.iter().map(|c| &c.rel).collect::<Vec<_>>());
    }
}

//! A Notion CSV import on a COPY of a base: a CSV in the shape of a label's base, built
//! from some of the base's own titles plus two unknown names; preview, import, undo.
//! notion_check <base.sqlite> <scratch dir>
fn main() {
    let mut a = std::env::args().skip(1);
    let (src, scratch) = (std::path::PathBuf::from(a.next().unwrap()), std::path::PathBuf::from(a.next().unwrap()));
    let data = scratch.join("data");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&data).unwrap();
    std::fs::copy(&src, data.join("plugin-fixer.sqlite")).unwrap();
    let app = app_core::App::open(&data).unwrap();
    let songs = app.songs().unwrap();
    let pick: Vec<_> = songs.iter().filter(|s| s.title.as_deref().is_some_and(|t| t.len() > 4)).take(6).collect();
    let states = ["ID", "Starting Point", "Arrangement", "Mix", "Send to a Label", "Terminé"];
    let mut csv = String::from("\u{feff}Nom,Projet,Type,Style,Tonality,Bpm,Interêt,État,Note,Track,Partner\n");
    for (i, s) in pick.iter().enumerate() {
        csv += &format!(
            "\"{}\",NOVA,\"Original, Remix\",Electro,Gm,{},{},{},\"une note, avec virgule\",{}.wav,Kito Vale\n",
            s.title.as_deref().unwrap().to_uppercase(),
            120 + i,
            "⭐".repeat(i % 5 + 1),
            states[i],
            i
        );
    }
    csv += "Inconnu 1,JIVE ME,Collab,Pop,Am,90,⭐⭐,ID,,,\nInconnu 2,,,,,,,,,,\n";
    let file = scratch.join("notion.csv");
    std::fs::write(&file, csv).unwrap();
    let path = file.to_str().unwrap();

    let pv = app.notion_preview(path, None).unwrap();
    for c in &pv.columns {
        println!("  {:12} -> {:8} {:?}", c.name, c.target, c.sample.first());
    }
    println!("stages: {:?}", pv.stages);
    println!("matched {} of {}", pv.matched, pv.rows.len());
    for r in &pv.rows {
        println!("  {:30} -> {:?} ({})", r.name, r.song_title, r.how);
    }
    let stages: std::collections::HashMap<String, String> = pv.stages.iter().map(|(v, s, _)| (v.clone(), s.clone())).collect();
    let before = app.song(pick[0].id).unwrap().unwrap().song;
    let (n, st) = app.notion_apply(path, pv.columns.clone(), stages.clone(), false).unwrap();
    println!("imported {n} songs, undo: {:?}", st.label);
    let after = app.song(pick[0].id).unwrap().unwrap().song;
    println!("first song: stage {:?} -> {:?}, rating {:?}, kinds {:?}, key {:?}, tempo {:?}, artist {:?}, ft {:?}",
        before.stage, after.stage, after.rating, after.kinds, after.key, after.tempo, after.artist, after.ft);
    assert_eq!(after.rating, Some(1));
    assert_eq!(after.kinds, vec!["Original", "Remix"]);
    let list = app.songs().unwrap();
    let it = list.iter().find(|s| s.id == pick[4].id).unwrap();
    println!("5th song in the list: rating {:?} kinds {:?} stage {:?}", it.rating, it.kinds, it.stage);
    let (label, _) = app.undo().unwrap();
    let back = app.song(pick[0].id).unwrap().unwrap().song;
    println!("undo {label:?}: stage {:?} rating {:?} kinds {:?} artist {:?}", back.stage, back.rating, back.kinds, back.artist);
    assert_eq!(back.rating, None);
    assert_eq!(back.stage, before.stage);
    assert_eq!(back.artist, before.artist);
    // only the empty fields: an artist already set stays
    let (n2, _) = app.notion_apply(path, pv.columns.clone(), stages, true).unwrap();
    let kept = app.song(pick[0].id).unwrap().unwrap().song;
    println!("only empty fields: {n2} songs, artist kept {:?} (was {:?}), rating {:?}", kept.artist, before.artist, kept.rating);
    println!("ok");
}

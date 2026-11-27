//! What the interface waits for, on a COPY of a data folder: the reload after a change, a
//! song's sheet, a status saved, and a media check while a heavy view runs.
//! ui_latency <data folder copy>
use std::time::Instant;

fn ms(t: Instant) -> u128 {
    t.elapsed().as_millis()
}

fn main() {
    let dir = std::env::args().nth(1).expect("data folder (a copy)");
    let app = std::sync::Arc::new(app_core::App::open(std::path::Path::new(&dir)).expect("open"));
    let reload = |label: &str| {
        let t = Instant::now();
        let a = Instant::now();
        let n = app.songs().unwrap().len();
        let s = ms(a);
        let a = Instant::now();
        app.overview().unwrap();
        let o = ms(a);
        let a = Instant::now();
        app.stats().unwrap();
        let st = ms(a);
        let a = Instant::now();
        let _ = app.releases();
        let r = ms(a);
        let a = Instant::now();
        let _ = app.home_stats();
        let h = ms(a);
        eprintln!("{label}: reload {} ms (songs[{n}] {s}, overview {o}, stats {st}, releases {r}) + home stats {h}", ms(t));
    };
    reload("cold");
    reload("warm");

    let songs = app.songs().unwrap();
    let mut with_proj: Vec<_> = songs.iter().filter(|s| s.temoin).take(5).collect();
    with_proj.extend(songs.iter().filter(|s| !s.temoin).take(2));
    for s in &with_proj {
        let t = Instant::now();
        let d = app.song(s.id).unwrap();
        let sd = ms(t);
        if d.is_none() { continue }
        let rel = Some(s.rel.clone()).filter(|r| !r.is_empty());
        let (mut ar, mut hm, mut tracks) = (0, 0, 0);
        if let Some(rel) = &rel {
            let t = Instant::now();
            if let Ok(a) = app.arrangement(rel) {
                tracks = a.tracks.len();
            }
            ar = ms(t);
            let t = Instant::now();
            let _ = app.harmony(rel);
            hm = ms(t);
        }
        eprintln!("sheet {:>5}: song {sd} ms, arrangement {ar} ms ({tracks} tracks), harmony {hm} ms  {:?}", s.id, s.title.as_deref().unwrap_or(""));
    }

    // a status saved, then what the interface asks again (the change empties the view cache)
    if let Some(s) = songs.iter().find(|s| s.temoin) {
        let mut body = serde_json::Map::new();
        body.insert("stage".into(), serde_json::json!("mix"));
        let t = Instant::now();
        app.song_edit(s.id, body).unwrap();
        eprintln!("status saved: {} ms", ms(t));
        reload("after the change");
        let t = Instant::now();
        let _ = app.song(s.id);
        eprintln!("its sheet again: {} ms", ms(t));
    }

    // a media request (what <audio> asks, range by range) while a heavy view computes
    if let Some(rel) = songs.iter().find_map(|s| s.temoin_rel.clone()) {
        let t = Instant::now();
        let _ = app.media_path(&rel);
        eprintln!("media check alone: {} ms", ms(t));
        let mut body = serde_json::Map::new();
        body.insert("stage".into(), serde_json::json!("master"));
        app.song_edit(songs[0].id, body).unwrap();
        let a2 = app.clone();
        let h = std::thread::spawn(move || {
            let t = Instant::now();
            let _ = a2.home_stats();
            ms(t)
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        let t = Instant::now();
        let _ = app.media_path(&rel);
        eprintln!("media check during home stats: {} ms (home stats {} ms)", ms(t), h.join().unwrap());
    }
}

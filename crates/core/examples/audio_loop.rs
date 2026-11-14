//! The audio analysis loop on a COPY of a base (never the user's own data folder):
//!
//!     SPOOL_MODEL_DIR=<folder with the model files> cargo run --release -p core --example audio_loop -- <copied data dir> [--dump out.json]
//!
//! Turns the analysis on (the files come from SPOOL_MODEL_DIR, checked like a download),
//! analyses every reference render (read-only), runs a second round to check that nothing
//! unchanged is analysed again, then prints a few songs' "Sounds like". `--dump` writes
//! every song with its style, folder and embedding, for measurements.
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().expect("usage: audio_loop <copied data dir> [--dump out.json]"));
    let mut dump = None;
    let mut no_audio = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dump" => dump = args.next(),
            // only the style leave-one-out, without turning the analysis on (a baseline)
            "--no-audio" => no_audio = true,
            _ => {}
        }
    }
    assert!(
        !dir.to_string_lossy().to_lowercase().contains("app.pluginfixer.desktop"),
        "work on a copy of the base, not the app's own folder"
    );
    let app = app_core::App::open(&dir).expect("open the base");
    if no_audio {
        style_loo(&app);
        return;
    }
    let st = app.audio_enable(true).expect("enable");
    println!("download: {} MB", st.download_mb);
    while !app.audio_state().ready {
        let s = app.audio_state();
        if let Some(e) = s.error {
            panic!("download failed: {e}");
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let t = Instant::now();
    let n = app.audio_round().expect("round");
    let s = app.audio_state();
    println!(
        "analysed {n} renders in {:.0} s ({:.2} s each), {} of {} renders now analysed, error: {:?}",
        t.elapsed().as_secs_f32(),
        t.elapsed().as_secs_f32() / n.max(1) as f32,
        s.analysed,
        s.renders,
        s.error
    );
    let again = app.audio_round().expect("second round");
    println!("second round analysed {again} (0 expected: nothing changed)");
    let ids: Vec<i64> = app.songs().unwrap().iter().map(|s| s.id).collect();
    let mut shown = 0;
    for id in &ids {
        if let Some(d) = app.song(*id).unwrap() {
            if let Some(snd) = &d.sound {
                if shown < 5 {
                    println!(
                        "{:?} ({:?} bpm, project {:?}) sounds like: {}",
                        d.song.title,
                        snd.bpm,
                        d.song.tempo.or(d.files.first().and_then(|f| f.tempo)),
                        snd.similar
                            .iter()
                            .map(|x| format!("{:?} {:.2}", x.title, x.sim))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                shown += 1;
            }
        }
    }
    println!("{shown} songs have a sheet with an analysis");
    style_loo(&app);
    if let Some(out) = dump {
        let con = store::Store::open(&dir.join("plugin-fixer.sqlite")).unwrap().con;
        let mut q = con
            .prepare(
                "select s.id, s.title, s.style, s.song_dir, s.tempo, a.bpm, a.key, a.embedding from songs s \
                 join audio_analysis a on a.rel = s.temoin where s.gone = 0 and a.embedding is not null",
            )
            .unwrap();
        let rows: Vec<serde_json::Value> = q
            .query_map([], |r| {
                let blob: Vec<u8> = r.get(7)?;
                Ok(serde_json::json!({
                    "id": r.get::<_, i64>(0)?, "title": r.get::<_, Option<String>>(1)?, "style": r.get::<_, Option<String>>(2)?,
                    "dir": r.get::<_, Option<String>>(3)?, "tempo": r.get::<_, Option<f64>>(4)?, "bpm": r.get::<_, Option<f64>>(5)?,
                    "key": r.get::<_, Option<String>>(6)?, "embedding": app_core::sound::embedding_of(&blob),
                }))
            })
            .unwrap()
            .filter_map(|x| x.ok())
            .collect();
        std::fs::write(&out, serde_json::to_string(&rows).unwrap()).unwrap();
        println!("{} rows -> {out}", rows.len());
    }
}

/// The style suggestion as the app makes it (every clue, the audio vote included when there
/// are analyses), for every song that has a style, read as if it had none.
fn style_loo(app: &app_core::App) {
    let songs = app.songs().unwrap();
    let (mut n, mut asked, mut right, mut audio, mut audio_right) = (0, 0, 0, 0, 0);
    for s in songs.iter().filter(|s| s.style.as_deref().is_some_and(|x| !x.is_empty())) {
        n += 1;
        let opts = app.style_guess(s.id).unwrap();
        let Some(top) = opts.first() else { continue };
        asked += 1;
        let ok = catalog::canon(&top.style) == catalog::canon(s.style.as_deref().unwrap());
        right += ok as i32;
        let heard = opts.iter().any(|o| {
            o.why.iter().any(|w| {
                w.contains(
                    &i18n::tr!("style.soundsLike", titles = "", n = "")
                        .split(' ')
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
        });
        if heard {
            audio += 1;
            audio_right += ok as i32;
        }
    }
    let marker = i18n::tr!("style.soundsLike", titles = "", n = "").split(' ').take(3).collect::<Vec<_>>().join(" ");
    let (mut bare, mut sugg, mut by_audio, mut only_audio) = (0, 0, 0, 0);
    for s in songs.iter().filter(|s| s.style.as_deref().is_none_or(|x| x.is_empty())) {
        bare += 1;
        let opts = app.style_guess(s.id).unwrap();
        if opts.is_empty() {
            continue;
        }
        sugg += 1;
        let heard: Vec<bool> = opts.iter().map(|o| o.why.iter().any(|w| w.contains(&marker))).collect();
        by_audio += heard[0] as i32;
        only_audio += (heard[0] && opts[0].why.len() == 1) as i32;
    }
    println!(
        "songs without a style: {bare}, with a suggestion: {sugg}, the audio vote behind the first choice: {by_audio}, the audio vote alone: {only_audio}"
    );
    println!(
        "style suggestion, leave-one-out on {n} songs with a style: suggested for {asked}, right {right} ({:.0} %); the audio vote took part for {audio}, right {audio_right}",
        100.0 * right as f64 / asked.max(1) as f64
    );
}

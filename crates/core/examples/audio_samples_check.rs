//! Runs the audio analysis on a COPY of a data folder for a while, then pauses it and prints
//! where it stands: for checking the samples' analysis on a real library (files only read).
//!     SPOOL_MODEL_DIR=<model dir> cargo run --release -p core --example audio_samples_check -- <data folder> <seconds>
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder");
    let secs: u64 = a.next().and_then(|s| s.parse().ok()).unwrap_or(60);
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    // the model in place first, as the app does (from SPOOL_MODEL_DIR, hash-checked)
    app.audio_enable(true).expect("enable");
    while !app.audio_state().ready {
        if let Some(e) = app.audio_state().error {
            panic!("{e}");
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    let worker = std::sync::Arc::clone(&app);
    let t = std::thread::spawn(move || worker.audio_round());
    std::thread::sleep(std::time::Duration::from_secs(secs));
    let s = app.audio_state();
    eprintln!("after {secs} s: {}/{} done, about {} s left, samples to listen {}, sorted by ear {}", s.done, s.total, s.eta_s, s.samples_todo, s.samples_done);
    app.audio_pause(true).unwrap();
    let n = t.join().unwrap();
    app.audio_pause(false).unwrap();
    let s = app.audio_state();
    eprintln!("round stopped after {n:?} files; measured {} ms a file; samples left {}; error {:?}; ready {}", s.ms_per_file, s.samples_todo, s.error, s.ready);
}

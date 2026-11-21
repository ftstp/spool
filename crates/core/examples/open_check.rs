//! open_check <data dir>: opens the base the app's way (run it on a COPY).
fn main() {
    let d = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let app = app_core::App::open(&d).unwrap();
    println!("opened, {} MIDI files", app.midi_summary().unwrap().files);
}

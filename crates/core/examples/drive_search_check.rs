//! On a COPY of a data folder: a search on another drive, then what was read again.
//!     drive_search_check <data folder copy> <drive letter>
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = a.next().expect("data folder (a copy)");
    let drive = a.next().expect("drive");
    let app = std::sync::Arc::new(app_core::App::open(std::path::Path::new(&dir)).expect("open"));
    let _ = app.scan(false);
    let t = std::time::Instant::now();
    app.deep_search(Some(drive)).expect("start");
    std::thread::sleep(std::time::Duration::from_millis(500));
    while app.scan_state().running {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
    while app.scan_state().running {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    println!("search and re-read in {:.0} s", t.elapsed().as_secs_f32());
}

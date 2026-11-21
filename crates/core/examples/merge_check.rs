//! Merges two artists on a COPY of a data folder, scans, undoes: the counts at each step.
fn main() {
    let mut a = std::env::args().skip(1);
    let (dir, from, into) = (a.next().unwrap(), a.next().unwrap(), a.next().unwrap());
    let app = app_core::App::open(std::path::Path::new(&dir)).expect("open");
    let count = |app: &app_core::App| {
        let s = app.songs().unwrap();
        let n = |x: &str| s.iter().filter(|s| s.artist.as_deref() == Some(x)).count();
        (n(&from), n(&into))
    };
    eprintln!("before: {from} {}, {into} {}", count(&app).0, count(&app).1);
    app.merge_artists(&from, &into).unwrap();
    eprintln!("merged: {:?}", count(&app));
    app.scan(true).unwrap();
    eprintln!("after a full scan: {:?}", count(&app));
    app.undo().unwrap();
    eprintln!("after Ctrl+Z: {:?}", count(&app));
    app.scan(true).unwrap();
    eprintln!("after another full scan: {:?}", count(&app));
}

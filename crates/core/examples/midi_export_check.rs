//! midi_export_check <list of "kind<TAB>path"> <out dir>: each file transposed +3 at 1.25x,
//! written to <out dir> (the originals are only read).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let list = std::fs::read(&a[1]).unwrap();
    for line in String::from_utf8_lossy(&list).lines().filter(|l| !l.trim().is_empty()) {
        let (kind, p) = line.split_once('\t').unwrap();
        let p = std::path::Path::new(p.trim());
        let Ok(b) = std::fs::read(p) else { eprintln!("unreadable {}", p.display()); continue };
        match app_core::midi_export::export(p, std::path::Path::new(&a[2]), 3, 1.25, None, kind == "drums") {
            Ok(to) => {
                let o = std::fs::read(&to).unwrap();
                eprintln!("{kind}\t{} -> {} BPM, {} -> {} bytes\t{}", app_core::midi_export::first_bpm(&b), app_core::midi_export::first_bpm(&o), b.len(), o.len(), to.file_name().unwrap().to_string_lossy());
            }
            Err(e) => eprintln!("{kind}\tERR {e}"),
        }
    }
}

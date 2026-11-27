//! arr_check <set.als>: its arrangement as text (read only).
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let a = als::arrangement(std::path::Path::new(&p), &als::Env::detect(None)).expect("readable");
    println!("tempo {:?}, {} beats/bar, {} bars, locators {:?}", a.tempo, a.beats_per_bar, a.len / a.beats_per_bar, a.locators.iter().map(|l| (l.at / a.beats_per_bar, l.name.clone())).collect::<Vec<_>>());
    for t in &a.tracks {
        let midi = t.clips.iter().filter(|c| c.hash.is_some()).count();
        let audio = t.clips.iter().filter(|c| c.sample.is_some()).count();
        println!("  {:<24} {:<6} {:<6} grouped={} {} clips ({} midi, {} audio)", t.name, t.kind, t.role, t.grouped, t.clips.len(), midi, audio);
    }
}

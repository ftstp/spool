//! structure_check <base.sqlite> <style> [bpm] [seconds]: the structure learnt for a style.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let con = store::rusqlite::Connection::open(&a[0]).unwrap();
    let known = app_core::chains::known_styles(&con).unwrap();
    let st = |f: Option<&str>, d: &str| app_core::chains::style_of(f, d, &known);
    let s = app_core::structure::learn(&con, &a[1], a.get(2).and_then(|x| x.parse().ok()), a.get(3).and_then(|x| x.parse().ok()), None, &st).unwrap();
    println!("{} bpm {:?} learnt from {} -> {:.0} s", s.style, s.bpm, s.learnt_from, s.seconds);
    for x in &s.sections {
        println!("  bar {:>3} {:>3} bars  {}", x.bar + 1, x.bars, x.name);
    }
}

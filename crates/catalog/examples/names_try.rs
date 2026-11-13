//! names_try <stem>... | names_try --file <stems.txt>: the title and variant key the catalogue
//! derives from file names (with --file: only the names whose key is a bare number).
fn main() {
    let r = catalog::Rules::new(&[], &[]);
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(String::as_str) == Some("--file") {
        let txt = std::fs::read_to_string(&a[1]).unwrap();
        let hits: Vec<&str> =
            txt.lines().filter(|l| r.variant_key(l).chars().all(|c| c.is_ascii_digit()) && !r.variant_key(l).is_empty()).collect();
        println!("{} of {} names keyed by their number", hits.len(), txt.lines().count());
        for h in hits {
            println!("  {h} -> {}", r.variant_key(h));
        }
        return;
    }
    for a in a {
        println!("{a:30} title {:20} key {:?}", r.pretty_stem(&a), r.variant_key(&a));
    }
}

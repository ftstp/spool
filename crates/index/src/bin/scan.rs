//! One scan pass on a base, from the command line.
//! Usage: scan <base.sqlite> [--import <prototype apf.sqlite>] [--full] [--deep <old_drive_samples.json>]
use index::Scanner;
use std::path::PathBuf;
use std::time::Instant;
use store::Store;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let db = PathBuf::from(
        args.first().expect("usage: scan <base.sqlite> [--import <apf.sqlite>] [--full] [--deep <json>]"),
    );
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let st = Store::open(&db).expect("open base");
    if let Some(p) = opt("--import") {
        for (t, n) in st.import_prototype(&p).expect("import") {
            eprintln!("imported {n:6} rows of {t}");
        }
    }
    let mut sc = Scanner::new();
    sc.deep_file = opt("--deep");
    let t = Instant::now();
    let st = std::sync::Mutex::new(st);
    let rep = sc.sync(&st, args.iter().any(|a| a == "--full")).expect("scan");
    eprintln!("{}", serde_json::to_string_pretty(&rep).unwrap());
    eprintln!("total {:.1} s", t.elapsed().as_secs_f64());
}

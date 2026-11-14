//! The key estimate against a list of files whose key is known.
//!     cargo run --release -p midi --example key_check -- <list.tsv>
//! List: path, tonic (0 = C), minor (0/1), group; the score is printed per group half.
use std::io::BufRead;

fn main() {
    let list = std::env::args().nth(1).expect("list");
    let (mut n, mut exact, mut rel) = (0, 0, 0);
    for line in std::io::BufReader::new(std::fs::File::open(list).unwrap()).lines() {
        let line = line.unwrap();
        let f: Vec<&str> = line.split('\t').collect();
        let (t, m): (usize, bool) = (f[1].parse().unwrap(), f[2] == "1");
        let Ok(bytes) = std::fs::read(f[0]) else { continue };
        let name = f[0].rsplit('/').next().unwrap_or("");
        let info = midi::analyse(&bytes, name);
        n += 1;
        let Some(k) = info.key else { continue };
        let minor = k.ends_with('m');
        let root = k.trim_end_matches('m');
        let names = [["C"], ["C#"], ["D"], ["Eb"], ["E"], ["F"], ["F#"], ["G"], ["Ab"], ["A"], ["Bb"], ["B"]];
        fn alt(r: &str) -> &str {
            match r {
            "Db" => "C#",
            "D#" => "Eb",
            "G#" => "Ab",
            "A#" => "Bb",
            "Gb" => "F#",
            x => x,
            }
        }
        let Some(pc) = names.iter().position(|x| x[0] == alt(root)) else { continue };
        if (pc, minor) == (t, m) {
            exact += 1;
        } else if (minor && !m && (pc + 3) % 12 == t) || (!minor && m && (t + 3) % 12 == pc) {
            rel += 1;
        }
    }
    println!("{n} files: exact {:.1} %, relative {:.1} %", 100.0 * exact as f64 / n as f64, 100.0 * rel as f64 / n as f64);
}

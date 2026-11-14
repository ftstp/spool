//! The audio key on a list of files, with its chroma, for working on the key estimate.
//!     cargo run --release -p audio --example chroma_dump -- <list.tsv> <out.jsonl>
//! First column of the list: a path. Files are only read. No model needed.
use audio::{decode_mono, dsp, key, mel, resample};
use std::io::{BufRead, Write};

fn main() {
    let mut a = std::env::args().skip(1);
    let (list, out) = (a.next().expect("list"), a.next().expect("out"));
    let paths: Vec<String> = std::io::BufReader::new(std::fs::File::open(list).unwrap())
        .lines()
        .map(|l| l.unwrap().split('\t').next().unwrap().to_string())
        .collect();
    let w = std::sync::Mutex::new(std::io::BufWriter::new(std::fs::File::create(out).unwrap()));
    std::thread::scope(|s| {
        let chunks: Vec<&[String]> = paths.chunks(paths.len().div_ceil(8).max(1)).collect();
        for c in chunks {
            let w = &w;
            s.spawn(move || {
                for p in c {
                    let Ok(pcm) = decode_mono(std::path::Path::new(p)) else { continue };
                    let Ok(x48) = resample(&pcm.samples, pcm.sample_rate, mel::SAMPLE_RATE) else { continue };
                    let x12 = dsp::decimate(&x48, (mel::SAMPLE_RATE / 12_000) as usize);
                    let cfg = key::KeyConfig::default();
                    let wide = key::KeyConfig { f_min: 40.0, f_max: 4000.0, ..cfg };
                    let raw = key::KeyConfig { frame_norm: false, ..cfg };
                    let est = key::estimate(&x12, 12_000.0).map(|k| k.name());
                    let line = serde_json::json!({
                        "path": p, "est": est, "dur": pcm.duration_s(),
                        "c": key::chroma(&x12, 12_000.0, &cfg), "wide": key::chroma(&x12, 12_000.0, &wide),
                        "raw": key::chroma(&x12, 12_000.0, &raw),
                    });
                    writeln!(w.lock().unwrap(), "{line}").unwrap();
                }
            });
        }
    });
}

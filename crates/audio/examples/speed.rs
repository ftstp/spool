//! Where an analysis spends its time: preparing (decoding, tempo, key, spectra) vs the model,
//! for a list of files, at a given number of model threads. Files are only read.
//!     cargo run --release -p audio --example speed -- <model dir> <list.tsv> <n files> <model threads>
use std::io::BufRead;
fn main() {
    let mut a = std::env::args().skip(1);
    let (dir, list) = (a.next().unwrap(), a.next().unwrap());
    let n: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(32);
    let threads: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(8);
    let files: Vec<String> = std::io::BufReader::new(std::fs::File::open(list).unwrap()).lines().take(n).map(|l| l.unwrap().split('\t').next().unwrap().to_string()).collect();
    let d = std::path::Path::new(&dir);
    let an = audio::Analyzer::load(&d.join("onnxruntime.dll"), &d.join("audio_model_int8.onnx"), threads).unwrap();
    let t = std::time::Instant::now();
    let prepared: Vec<audio::Prepared> = std::thread::scope(|s| {
        let hs: Vec<_> = files.chunks(files.len().div_ceil(16)).map(|c| s.spawn(move || c.iter().filter_map(|f| audio::prepare(f).ok()).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let prep = t.elapsed().as_secs_f64();
    let t = std::time::Instant::now();
    let k = prepared.len();
    an.finish_many(prepared).unwrap();
    let model = t.elapsed().as_secs_f64();
    println!("{k} files, {threads} model threads: prepare {:.0} ms/file (16 in parallel), model {:.0} ms/file", prep * 1000.0 / k as f64, model * 1000.0 / k as f64);
}

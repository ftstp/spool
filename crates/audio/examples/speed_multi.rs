//! Throughput and memory with K copies of the model, T threads each (files only read).
//!     cargo run --release -p audio --example speed_multi -- <model dir> <list.tsv> <n files> <K> <T>
use std::io::BufRead;
#[cfg(windows)]
fn mem_mb() -> f64 {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct Pmc { cb: u32, PageFaultCount: u32, PeakWorkingSetSize: usize, WorkingSetSize: usize, a: usize, b: usize, c: usize, d: usize, PagefileUsage: usize, PeakPagefileUsage: usize }
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(p: isize, c: *mut Pmc, cb: u32) -> i32;
        fn GetCurrentProcess() -> isize;
    }
    let mut c = Pmc { cb: std::mem::size_of::<Pmc>() as u32, PageFaultCount: 0, PeakWorkingSetSize: 0, WorkingSetSize: 0, a: 0, b: 0, c: 0, d: 0, PagefileUsage: 0, PeakPagefileUsage: 0 };
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    c.PeakPagefileUsage as f64 / 1048576.0
}
fn main() {
    let mut a = std::env::args().skip(1);
    let (dir, list) = (a.next().unwrap(), a.next().unwrap());
    let n: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(48);
    let k: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(2);
    let t: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(4);
    let files: Vec<String> = std::io::BufReader::new(std::fs::File::open(list).unwrap()).lines().take(n).map(|l| l.unwrap().split('\t').next().unwrap().to_string()).collect();
    let d = std::path::Path::new(&dir);
    let ans: Vec<audio::Analyzer> = (0..k).map(|_| audio::Analyzer::load(&d.join("onnxruntime.dll"), &d.join("audio_model_int8.onnx"), t).unwrap()).collect();
    let prepared: Vec<audio::Prepared> = files.iter().filter_map(|f| audio::prepare(f).ok()).collect();
    let m = prepared.len();
    let t0 = std::time::Instant::now();
    let mut parts: Vec<Vec<audio::Prepared>> = (0..k).map(|_| Vec::new()).collect();
    for (i, p) in prepared.into_iter().enumerate() { parts[i % k].push(p); }
    std::thread::scope(|s| { for (an, part) in ans.iter().zip(parts) { s.spawn(move || an.finish_many(part).unwrap()); } });
    println!("K={k} T={t}: model {:.0} ms/file, peak private memory {:.0} MB", t0.elapsed().as_secs_f64() * 1000.0 / m as f64, mem_mb());
}

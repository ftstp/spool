//! Analyse audio files with the CLAP model.
//!
//!     cargo run --release -p audio --example analyze -- <model dir or .onnx> <file>... [--json out.json] [--dump-pcm dir] [--runtime onnxruntime.dll] [--threads n]
//!
//! ONNX Runtime is loaded from `--runtime`, by default `onnxruntime.dll` beside the model.
//!
//! Prints duration, tempo, key and the top styles with per-stage timings.
//! `--json` writes every result (with the embedding and per-window embeddings);
//! `--dump-pcm` writes the decoded 48 kHz mono samples as `<name>.f32` (f32 LE)
//! for comparison with the Python reference (`tools/clap/export.py embed --pcm`).

use std::path::{Path, PathBuf};
use std::time::Instant;

use audio::{Analyzer, decode_mono, dsp, key, mel, resample, styles, tempo};

fn main() {
    let mut args = std::env::args().skip(1);
    let model =
        PathBuf::from(args.next().expect("usage: analyze <model dir|.onnx> <file>... [--json f] [--dump-pcm dir]"));
    let mut files = Vec::new();
    let mut json_out: Option<PathBuf> = None;
    let mut dump: Option<PathBuf> = None;
    let mut runtime: Option<PathBuf> = None;
    let mut threads = 0usize;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json_out = args.next().map(PathBuf::from),
            "--dump-pcm" => dump = args.next().map(PathBuf::from),
            "--runtime" => runtime = args.next().map(PathBuf::from),
            "--threads" => threads = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => files.push(PathBuf::from(a)),
        }
    }
    let t = Instant::now();
    let model_file = if model.is_dir() {
        audio::MODEL_FILES.iter().map(|f| model.join(f)).find(|p| p.is_file()).expect("no model in the directory")
    } else {
        model.clone()
    };
    let runtime = runtime.unwrap_or_else(|| model_file.with_file_name(audio::RUNTIME_FILE));
    let analyzer = Analyzer::load(&runtime, &model_file, threads).unwrap_or_else(|e| panic!("{e}"));
    eprintln!("model {} loaded in {:.2} s", analyzer.model_path().display(), t.elapsed().as_secs_f32());

    let mut results = serde_json::Map::new();
    for f in &files {
        match run(&analyzer, f, dump.as_deref()) {
            Ok(v) => {
                results.insert(f.file_name().unwrap().to_string_lossy().into_owned(), v);
            }
            Err(e) => eprintln!("{}: {e}", f.display()),
        }
    }
    if let Some(out) = json_out {
        std::fs::write(&out, serde_json::to_string(&results).unwrap()).expect("write json");
    }
}

fn run(an: &Analyzer, f: &Path, dump: Option<&Path>) -> Result<serde_json::Value, audio::Error> {
    let t0 = Instant::now();
    let pcm = decode_mono(f)?;
    let t_dec = t0.elapsed().as_secs_f32();
    let duration = pcm.duration_s();
    let t1 = Instant::now();
    let x48 = resample(&pcm.samples, pcm.sample_rate, mel::SAMPLE_RATE)?;
    let t_rs = t1.elapsed().as_secs_f32();
    if let Some(dir) = dump {
        let name = f.file_stem().unwrap().to_string_lossy();
        let bytes: Vec<u8> = x48.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(dir.join(format!("{name}.f32")), bytes).map_err(audio::Error::Io)?;
    }
    let t2 = Instant::now();
    let wins = audio::windows::windows(&x48);
    let mut feats = Vec::new();
    for w in &wins {
        feats.extend(mel::log_mel(w));
    }
    let t_mel = t2.elapsed().as_secs_f32();
    let t3 = Instant::now();
    let per = an.embed_features(feats, wins.len())?;
    let t_inf = t3.elapsed().as_secs_f32();
    let mut emb = vec![0.0f32; audio::EMBEDDING_DIM];
    for e in &per {
        emb.iter_mut().zip(e).for_each(|(m, v)| *m += v);
    }
    audio::normalize(&mut emb);
    let t4 = Instant::now();
    let x12 = dsp::decimate(&x48, 4);
    let bpm = tempo::estimate(&x12, 12_000.0);
    let k = key::estimate(&x12, 12_000.0);
    let t_tk = t4.elapsed().as_secs_f32();
    let st = if an.styles_match() { styles::builtin().score(&emb) } else { Vec::new() };
    let total = t0.elapsed().as_secs_f32();
    let top: Vec<String> = st.iter().take(3).map(|(n, p)| format!("{n} {:.0}%", p * 100.0)).collect();
    println!(
        "{}: {:.1} s, {} bpm, {}, styles: {} | decode {:.2} resample {:.2} mel {:.2} model {:.2} ({} win) tempo+key {:.2} total {:.2} s",
        f.file_name().unwrap().to_string_lossy(),
        duration,
        bpm.map(|b| format!("{b:.1}")).unwrap_or("-".into()),
        k.map(|k| k.name()).unwrap_or("-".into()),
        top.join(", "),
        t_dec,
        t_rs,
        t_mel,
        t_inf,
        wins.len(),
        t_tk,
        total
    );
    Ok(serde_json::json!({
        "embedding": emb,
        "per_window": per,
        "windows": audio::windows::window_starts(x48.len()),
        "samples": x48.len(),
        "duration_s": duration,
        "bpm": bpm,
        "key": k.map(|k| k.name()),
        "styles": st.iter().take(10).collect::<Vec<_>>(),
        "timing": {"decode": t_dec, "resample": t_rs, "mel": t_mel, "model": t_inf, "tempo_key": t_tk, "total": total},
    }))
}

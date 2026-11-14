//! Audio analysis for Spool: from a song's reference render, a CLAP audio
//! embedding (LAION `larger_clap_music`, Apache-2.0, run with ONNX Runtime),
//! zero-shot style scores, tempo and key. See README.md.
//!
//! Audio files are only ever opened for reading.

pub mod decode;
pub mod dsp;
pub mod key;
pub mod mel;
pub mod mp4;
pub mod styles;
pub mod tempo;
pub mod windows;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;

pub use decode::{Pcm, decode_mono, resample};
pub use key::Key;

/// Size of a CLAP embedding.
pub const EMBEDDING_DIM: usize = 512;
/// Model file names looked up by [`Analyzer::new`], in order of preference.
pub const MODEL_FILES: [&str; 2] = ["audio_model_int8.onnx", "audio_model.onnx"];
/// ONNX Runtime's library, looked up beside the model by [`Analyzer::new`].
pub const RUNTIME_FILE: &str = "onnxruntime.dll";
/// Rate at which tempo and key are computed (48 kHz decimated by 4).
const ANALYSIS_RATE: u32 = 12_000;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Decode(String),
    Model(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "cannot read the audio file: {e}"),
            Error::Decode(e) => write!(f, "cannot decode the audio: {e}"),
            Error::Model(e) => write!(f, "audio model: {e}"),
        }
    }
}

impl std::error::Error for Error {}

fn model_err(e: impl fmt::Display) -> Error {
    Error::Model(e.to_string())
}

#[derive(Debug, Clone)]
pub struct Analysis {
    /// L2-normalised CLAP audio embedding ([`EMBEDDING_DIM`] values), the mean
    /// of up to three 10 s windows.
    pub embedding: Vec<f32>,
    /// Tempo folded into 80-180 bpm.
    pub bpm: Option<f32>,
    /// "C major", "F# minor", ...
    pub key: Option<String>,
    pub duration_s: f32,
    /// Every built-in style with its zero-shot probability, highest first
    /// (empty when the model is not the one the styles were computed with).
    pub styles: Vec<(String, f32)>,
}

/// Cosine similarity of two vectors (0 when either is null).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        dot += (*x as f64) * (*y as f64);
        na += (*x as f64) * (*x as f64);
        nb += (*y as f64) * (*y as f64);
    }
    if na == 0.0 || nb == 0.0 { 0.0 } else { (dot / (na.sqrt() * nb.sqrt())) as f32 }
}

/// Scales `v` to unit length in place (left unchanged when null).
pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x = (*x as f64 / n) as f32);
    }
}

/// Holds the ONNX Runtime session for the CLAP audio model. Loading takes a
/// moment and some memory, so create one and reuse it; `analyze` may be called
/// from several threads (inference itself is serialised).
pub struct Analyzer {
    session: Mutex<Session>,
    model_path: PathBuf,
    /// Hugging Face id from the `manifest.json` beside the model, if any.
    model_id: Option<String>,
}

impl Analyzer {
    /// Loads the audio model from `model_dir` (preferring the int8 model), with ONNX
    /// Runtime from `model_dir/onnxruntime.dll`, ONNX Runtime's default thread count.
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Analyzer, Error> {
        let dir = model_dir.as_ref();
        let path = MODEL_FILES
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.is_file())
            .ok_or_else(|| Error::Model(format!("no {} in {}", MODEL_FILES.join(" or "), dir.display())))?;
        Self::load(&dir.join(RUNTIME_FILE), &path, 0)
    }

    /// Loads ONNX Runtime from `runtime` (a library file, loaded once per process: later
    /// calls reuse the first one), then the model `model` with `threads` intra-op threads
    /// (0 = ONNX Runtime's default; 1 = inference runs on the calling thread only).
    ///
    /// ONNX Runtime is never linked into the program: nothing of it is loaded until this
    /// is called, and a missing or broken library is an error here, not a crash.
    pub fn load(runtime: &Path, model: &Path, threads: usize) -> Result<Analyzer, Error> {
        if !runtime.is_file() {
            return Err(Error::Model(format!("ONNX Runtime not found at {}", runtime.display())));
        }
        let env = std::panic::catch_unwind(|| ort::init_from(runtime))
            .map_err(|_| Error::Model("ONNX Runtime failed to load".into()))?
            .map_err(model_err)?
            .with_name("spool-audio")
            .with_telemetry(false)
            .build()
            .map_err(model_err)?;
        let mut builder = Session::builder(&env)
            .map_err(model_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_err)?;
        if threads > 0 {
            builder =
                builder.with_intra_threads(threads).map_err(model_err)?.with_inter_threads(1).map_err(model_err)?;
        }
        let session = builder.commit_from_file(model).map_err(model_err)?;
        let model_id = model
            .parent()
            .and_then(|d| std::fs::read_to_string(d.join("manifest.json")).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("model")?.as_str().map(String::from));
        Ok(Analyzer { session: Mutex::new(session), model_path: model.to_path_buf(), model_id })
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// The model's Hugging Face id, read from `manifest.json` (written by
    /// `tools/clap/export.py`) beside the model file.
    pub fn model_id(&self) -> Option<&str> {
        self.model_id.as_deref()
    }

    /// Whether the built-in style embeddings belong to this model's embedding
    /// space. Text embeddings from another checkpoint would give meaningless
    /// scores, so `analyze` returns no styles when they do not match.
    pub fn styles_match(&self) -> bool {
        self.model_id.as_deref() == Some(styles::builtin().model.as_str())
    }

    /// Full analysis of an audio file (read-only).
    pub fn analyze(&self, path: impl AsRef<Path>) -> Result<Analysis, Error> {
        let pcm = decode_mono(path.as_ref())?;
        let duration_s = pcm.duration_s();
        let x48 = resample(&pcm.samples, pcm.sample_rate, mel::SAMPLE_RATE)?;
        drop(pcm);
        self.analyze_pcm_48k(&x48, duration_s)
    }

    /// Analysis of mono 48 kHz samples.
    pub fn analyze_pcm_48k(&self, x48: &[f32], duration_s: f32) -> Result<Analysis, Error> {
        let embedding = self.embed_48k(x48)?;
        let x12 = dsp::decimate(x48, (mel::SAMPLE_RATE / ANALYSIS_RATE) as usize);
        let sr = ANALYSIS_RATE as f32;
        let bpm = tempo::estimate(&x12, sr);
        let key = key::estimate(&x12, sr).map(|k| k.name());
        let styles = if self.styles_match() { styles::builtin().score(&embedding) } else { Vec::new() };
        Ok(Analysis { embedding, bpm, key, duration_s, styles })
    }

    /// The track embedding of mono 48 kHz samples (mean of the windows, normalised).
    pub fn embed_48k(&self, x48: &[f32]) -> Result<Vec<f32>, Error> {
        let per = self.embed_windows(x48)?;
        let mut mean = vec![0.0f32; EMBEDDING_DIM];
        for e in &per {
            for (m, v) in mean.iter_mut().zip(e) {
                *m += v;
            }
        }
        normalize(&mut mean);
        Ok(mean)
    }

    /// One embedding per 10 s window (see [`windows`]).
    pub fn embed_windows(&self, x48: &[f32]) -> Result<Vec<Vec<f32>>, Error> {
        let wins = windows::windows(x48);
        if wins.is_empty() {
            return Err(Error::Decode("empty audio".into()));
        }
        let mut feats = Vec::with_capacity(wins.len() * mel::FRAMES * mel::N_MELS);
        for w in &wins {
            feats.extend(mel::log_mel(w));
        }
        self.embed_features(feats, wins.len())
    }

    /// Runs the model on `batch` log-mel matrices laid out back to back.
    pub fn embed_features(&self, feats: Vec<f32>, batch: usize) -> Result<Vec<Vec<f32>>, Error> {
        let input = Tensor::from_array(([batch, 1, mel::FRAMES, mel::N_MELS], feats)).map_err(model_err)?;
        let mut session = self.session.lock().map_err(|_| Error::Model("session lock poisoned".into()))?;
        let outputs = session.run(ort::inputs!["input_features" => input]).map_err(model_err)?;
        let (shape, data) = outputs["audio_embeds"].try_extract_tensor::<f32>().map_err(model_err)?;
        if shape.len() != 2 || shape[0] as usize != batch || shape[1] as usize != EMBEDDING_DIM {
            return Err(Error::Model(format!("unexpected output shape {shape:?}")));
        }
        Ok(data
            .chunks_exact(EMBEDDING_DIM)
            .map(|c| {
                let mut v = c.to_vec();
                normalize(&mut v);
                v
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_basics() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert!((cosine(&[1.0, 2.0], &[-1.0, -2.0]) + 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 2.0]), 0.0);
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
    }
}

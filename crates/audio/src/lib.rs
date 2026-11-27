//! Audio analysis for Spool: from a song's reference render, a CLAP audio
//! embedding (LAION `larger_clap_music`, Apache-2.0, run with ONNX Runtime),
//! zero-shot style scores, tempo and key. See README.md.
//!
//! Audio files are only ever opened for reading.

pub mod decode;
pub mod dsp;
pub mod key;
pub mod mel;
pub mod notes;
pub mod sample_family;
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

pub use decode::{Pcm, decode_mono, resample, to_wav};
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

pub(crate) fn model_err(e: impl fmt::Display) -> Error {
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
    /// How well the key fits (the chroma's correlation with its profile, -1..1).
    pub key_score: Option<f32>,
    /// For a short sound: the note it plays (pitch class, 0 = C) and how much it stands out.
    pub note: Option<(usize, f32)>,
    /// For a loop: how much of it each pitch class holds (0 = C), from the notes the note model
    /// hears in it (see `notes`); None for a short sound or without the model.
    pub notes: Option<[f32; 12]>,
    pub duration_s: f32,
    /// Every built-in style with its zero-shot probability, highest first
    /// (empty when the model is not the one the styles were computed with).
    pub styles: Vec<(String, f32)>,
}

/// A file made ready for the model: its log-mel windows, and what is measured without the
/// model (tempo, key, length). Several are prepared in parallel (decoding is most of the
/// time), then embedded ([`Analyzer::finish_many`]).
pub struct Prepared {
    feats: Vec<f32>,
    windows: usize,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub key_score: Option<f32>,
    pub note: Option<(usize, f32)>,
    /// A loop's first 30 s at 22 050 Hz, for the note model (`notes`).
    x22: Option<Vec<f32>>,
    pub duration_s: f32,
}

/// Decodes `path` (read-only) and computes everything but the embedding. No model needed:
/// any thread, any number at once.
pub fn prepare(path: impl AsRef<Path>) -> Result<Prepared, Error> {
    let pcm = decode_mono(path.as_ref())?;
    let duration_s = pcm.duration_s();
    let x48 = resample(&pcm.samples, pcm.sample_rate, mel::SAMPLE_RATE)?;
    drop(pcm);
    let wins = windows::windows(&x48);
    if wins.is_empty() {
        return Err(Error::Decode("empty audio".into()));
    }
    let mut feats = Vec::with_capacity(wins.len() * mel::FRAMES * mel::N_MELS);
    for w in &wins {
        feats.extend(mel::log_mel(w));
    }
    let x12 = dsp::decimate(&x48, (mel::SAMPLE_RATE / ANALYSIS_RATE) as usize);
    let sr = ANALYSIS_RATE as f32;
    let bpm = tempo::estimate(&x12, sr);
    let k = key::estimate(&x12, sr);
    let note = if duration_s < 3.0 { key::dominant_note(&x12, sr) } else { None };
    let x22 = if duration_s >= 3.0 { resample(&x48[..x48.len().min(mel::SAMPLE_RATE as usize * 30)], mel::SAMPLE_RATE, notes::RATE).ok() } else { None };
    Ok(Prepared { feats, windows: wins.len(), bpm, key: k.map(|k| k.name()), key_score: k.map(|k| k.score), note, x22, duration_s })
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

/// ONNX Runtime's environment, from the library `runtime`. ONNX Runtime allows one environment
/// per process: made once, kept, and shared by every model (and by a model loaded again after
/// being removed); the library given the first time is the one loaded.
pub(crate) fn runtime_env(runtime: &Path) -> Result<ort::environment::Environment, Error> {
    if !runtime.is_file() {
        return Err(Error::Model(format!("ONNX Runtime not found at {}", runtime.display())));
    }
    static ENV: std::sync::Mutex<Option<ort::environment::Environment>> = std::sync::Mutex::new(None);
    let mut slot = ENV.lock().map_err(|_| Error::Model("environment lock poisoned".into()))?;
    if slot.is_none() {
        let env = std::panic::catch_unwind(|| ort::init_from(runtime))
            .map_err(|_| Error::Model("ONNX Runtime failed to load".into()))?
            .map_err(model_err)?
            .with_name("spool-audio")
            .with_telemetry(false)
            .build()
            .map_err(model_err)?;
        *slot = Some(env);
    }
    Ok(slot.clone().expect("environment"))
}

/// A model loaded with `threads` intra-op threads (0 = ONNX Runtime's default).
pub(crate) fn session_from_file(env: &ort::environment::Environment, model: &Path, threads: usize) -> Result<Session, Error> {
    let mut builder = Session::builder(env)
        .map_err(model_err)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(model_err)?;
    if threads > 0 {
        builder = builder.with_intra_threads(threads).map_err(model_err)?.with_inter_threads(1).map_err(model_err)?;
    }
    // ONNX Runtime's threads otherwise spin while waiting for work: a core at 100 % for
    // nothing between files
    builder = builder.with_intra_op_spinning(false).map_err(model_err)?.with_inter_op_spinning(false).map_err(model_err)?;
    builder.commit_from_file(model).map_err(model_err)
}

/// Holds the ONNX Runtime session for the CLAP audio model. Loading takes a
/// moment and some memory, so create one and reuse it; `analyze` may be called
/// from several threads (inference itself is serialised).
pub struct Analyzer {
    session: Mutex<Session>,
    /// The note model (notes.rs), loaded with the first loop to hear.
    note_model: Mutex<Option<Option<Session>>>,
    env: ort::environment::Environment,
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
        let env = runtime_env(runtime)?;
        let session = session_from_file(&env, model, threads)?;
        let model_id = model
            .parent()
            .and_then(|d| std::fs::read_to_string(d.join("manifest.json")).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("model")?.as_str().map(String::from));
        Ok(Analyzer { session: Mutex::new(session), note_model: Mutex::new(None), env, model_path: model.to_path_buf(), model_id })
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

    /// The analyses of prepared files. The model runs on each file's windows alone: run on a
    /// batch, the 8-bit model scales its activations over the whole batch and a file's
    /// embedding moves a little (cosine 0.998) with its neighbours; alone, a sample always
    /// gets the same embedding, so one near a threshold never changes family with its batch.
    pub fn finish_many(&self, prepared: Vec<Prepared>) -> Result<Vec<Analysis>, Error> {
        let mut out = Vec::with_capacity(prepared.len());
        for p in prepared {
            let per = self.embed_features(p.feats, p.windows)?;
            let mut mean = vec![0.0f32; EMBEDDING_DIM];
            for e in &per {
                for (m, v) in mean.iter_mut().zip(e) {
                    *m += v;
                }
            }
            normalize(&mut mean);
            let styles = if self.styles_match() { styles::builtin().score(&mean) } else { Vec::new() };
            let notes = p.x22.as_deref().and_then(|x| self.notes(x));
            out.push(Analysis { embedding: mean, bpm: p.bpm, key: p.key, key_score: p.key_score, note: p.note, notes, duration_s: p.duration_s, styles });
        }
        Ok(out)
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
        let k = key::estimate(&x12, sr);
        let note = if duration_s < 3.0 { key::dominant_note(&x12, sr) } else { None };
        let notes = if duration_s >= 3.0 {
            resample(&x48[..x48.len().min(mel::SAMPLE_RATE as usize * 30)], mel::SAMPLE_RATE, notes::RATE).ok().and_then(|x| self.notes(&x))
        } else {
            None
        };
        let styles = if self.styles_match() { styles::builtin().score(&embedding) } else { Vec::new() };
        Ok(Analysis { embedding, bpm, key: k.map(|k| k.name()), key_score: k.map(|k| k.score), note, notes, duration_s, styles })
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

    /// The pitch classes a loop holds (`x22`: mono at 22 050 Hz), from the notes the note model
    /// hears in it. None when the model cannot run or hears nothing.
    pub fn notes(&self, x22: &[f32]) -> Option<[f32; 12]> {
        let mut slot = self.note_model.lock().ok()?;
        if slot.is_none() {
            let s = Session::builder(&self.env)
                .ok()
                .and_then(|b| b.with_intra_threads(1).ok())
                .and_then(|mut b| b.commit_from_memory(notes::MODEL).ok());
            *slot = Some(s);
        }
        let session = slot.as_mut()?.as_mut()?;
        notes::pitch_classes(session, x22)
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
mod batch_tests {
    /// Prepared and embedded together = analysed one by one (needs the model: SPOOL_MODEL_DIR).
    #[test]
    #[ignore]
    fn many_at_once_equals_one_by_one() {
        let Some(dir) = std::env::var_os("SPOOL_MODEL_DIR") else { return };
        let files: Vec<String> = std::env::var("SPOOL_AUDIO_FILES").unwrap_or_default().split(';').filter(|s| !s.is_empty()).map(String::from).collect();
        let an = super::Analyzer::new(dir).unwrap();
        let one: Vec<_> = files.iter().map(|f| an.analyze(f).unwrap()).collect();
        let many = an.finish_many(files.iter().map(|f| super::prepare(f).unwrap()).collect()).unwrap();
        for (a, b) in one.iter().zip(&many) {
            assert!(super::cosine(&a.embedding, &b.embedding) > 0.99999);
            assert_eq!((a.bpm, &a.key), (b.bpm, &b.key));
        }
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

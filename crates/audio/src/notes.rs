//! The notes a loop plays, heard by Basic Pitch's note model (Spotify, Apache 2.0:
//! models/LICENSE-basic-pitch.txt), 230 KB, shipped with Spool. Its note output (88 keys,
//! about 86 frames a second) is summed per pitch class: what the loop holds of each note.
//! Measured on 400 named loops: the key it says is right only half the time (a loop is short
//! and often names its pack's key, not its own), so the app shows the notes, not a key.
use ort::session::Session;
use ort::value::Tensor;

pub const MODEL: &[u8] = include_bytes!("../models/basic-pitch-nmp.onnx");
/// The rate the model reads.
pub const RATE: u32 = 22_050;
/// Its window: 2 s of audio.
const WIN: usize = 43_844;
/// A note counts from this activation.
const ON: f32 = 0.3;

/// What a loop holds of each pitch class (0 = C), shares summing to 1; None if no note.
pub fn pitch_classes(session: &mut Session, x: &[f32]) -> Option<[f32; 12]> {
    if x.len() < RATE as usize {
        return None;
    }
    let n = x.len().div_ceil(WIN).min(15);
    let mut buf = vec![0f32; n * WIN];
    buf[..x.len().min(n * WIN)].copy_from_slice(&x[..x.len().min(n * WIN)]);
    let input = Tensor::from_array(([n, WIN, 1], buf)).ok()?;
    let name = session.inputs().first()?.name().to_string();
    let outs = session.run(ort::inputs![name.as_str() => input]).ok()?;
    // two outputs of 88: the notes held and their starts; the held ones last longer
    let mut best: Option<(f32, Vec<f32>)> = None;
    for (_, v) in outs.iter() {
        let Ok((shape, data)) = v.try_extract_tensor::<f32>() else { continue };
        if shape.last().copied() != Some(88) {
            continue;
        }
        let mean = data.iter().sum::<f32>() / data.len().max(1) as f32;
        if best.as_ref().is_none_or(|b| mean > b.0) {
            best = Some((mean, data.to_vec()));
        }
    }
    let (_, data) = best?;
    let mut pc = [0f32; 12];
    for frame in data.chunks_exact(88) {
        for (k, a) in frame.iter().enumerate() {
            if *a >= ON {
                pc[(k + 21) % 12] += *a;
            }
        }
    }
    let sum: f32 = pc.iter().sum();
    if sum <= 0.0 {
        return None;
    }
    pc.iter_mut().for_each(|v| *v /= sum);
    Some(pc)
}

/// The notes that count (8 % of the loop or more), the strongest first.
pub fn names(pc: &[f32; 12]) -> Vec<usize> {
    let mut v: Vec<usize> = (0..12).filter(|i| pc[*i] >= 0.08).collect();
    v.sort_by(|a, b| pc[*b].total_cmp(&pc[*a]));
    v
}

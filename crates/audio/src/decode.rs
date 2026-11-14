//! Decoding to mono f32 with symphonia, then resampling to 48 kHz with rubato.
//!
//! The file is opened read-only (`File::open`) and never written to.

use std::fs::File;
use std::path::Path;

use rubato::{FftFixedIn, Resampler};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::Error;

/// Decoded audio: mono samples (channels averaged) at the file's own rate.
pub struct Pcm {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Pcm {
    pub fn duration_s(&self) -> f32 {
        if self.sample_rate == 0 { 0.0 } else { self.samples.len() as f32 / self.sample_rate as f32 }
    }
}

/// Decodes the whole default track of `path` and downmixes it to mono.
pub fn decode_mono(path: &Path) -> Result<Pcm, Error> {
    let file = File::open(path).map_err(Error::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| Error::Decode(format!("unsupported or unreadable file: {e}")))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| Error::Decode("no audio track".into()))?;
    let track_id = track.id;
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| Error::Decode(format!("unsupported codec: {e}")))?;

    let mut samples: Vec<f32> = Vec::new();
    if let Some(frames) = track.codec_params.n_frames {
        samples.reserve(frames as usize);
    }
    let mut buf: Option<(SampleBuffer<f32>, usize, usize)> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A corrupt packet is skipped, as players do.
            Err(SymError::DecodeError(_)) => continue,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        };
        let spec = *decoded.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate;
        }
        let channels = spec.channels.count().max(1);
        let frames = decoded.capacity();
        let sb = match &mut buf {
            Some((b, cap, ch)) if *cap >= frames && *ch == channels => b,
            _ => &mut buf.insert((SampleBuffer::<f32>::new(frames as u64, spec), frames, channels)).0,
        };
        sb.copy_interleaved_ref(decoded);
        let data = sb.samples();
        if channels == 1 {
            samples.extend_from_slice(data);
        } else {
            let inv = 1.0 / channels as f32;
            samples.extend(data.chunks_exact(channels).map(|f| f.iter().sum::<f32>() * inv));
        }
    }
    // MP3: symphonia's gapless mode already removed the encoder delay and padding (LAME
    // tag). MP4/M4A: skip the encoder priming the edit list names, as ffmpeg does (ffmpeg
    // keeps the padded tail, and so do we: the windows then fall on the same samples).
    if let Some(t) = crate::mp4::audio_trim(path, sample_rate) {
        let skip = (t.skip as usize).min(samples.len());
        samples.drain(..skip);
    }
    if samples.is_empty() || sample_rate == 0 {
        return Err(Error::Decode("no audio decoded".into()));
    }
    Ok(Pcm { samples, sample_rate })
}

/// Resamples mono audio to `target` Hz (rubato's FFT resampler; output length
/// is `round(len * target / rate)` and the resampler's delay is removed).
pub fn resample(samples: &[f32], rate: u32, target: u32) -> Result<Vec<f32>, Error> {
    if rate == target {
        return Ok(samples.to_vec());
    }
    const CHUNK: usize = 1024;
    let mut rs = FftFixedIn::<f32>::new(rate as usize, target as usize, CHUNK, 2, 1)
        .map_err(|e| Error::Decode(format!("resampler: {e}")))?;
    let delay = rs.output_delay();
    let expected = ((samples.len() as f64) * target as f64 / rate as f64).round() as usize;
    let mut out: Vec<f32> = Vec::with_capacity(expected + delay + CHUNK * 2);
    let mut pos = 0;
    while pos + CHUNK <= samples.len() {
        let o =
            rs.process(&[&samples[pos..pos + CHUNK]], None).map_err(|e| Error::Decode(format!("resampler: {e}")))?;
        out.extend_from_slice(&o[0]);
        pos += CHUNK;
    }
    // Remainder zero-padded to a full chunk, then silence until the delayed tail is out.
    let mut tail = vec![0.0f32; CHUNK];
    tail[..samples.len() - pos].copy_from_slice(&samples[pos..]);
    loop {
        let o = rs.process(&[&tail[..]], None).map_err(|e| Error::Decode(format!("resampler: {e}")))?;
        out.extend_from_slice(&o[0]);
        if out.len() >= expected + delay {
            break;
        }
        tail.iter_mut().for_each(|v| *v = 0.0);
    }
    let end = (delay + expected).min(out.len());
    Ok(out[delay.min(end)..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_keeps_length_and_frequency() {
        let sr = 44100;
        let f = 1000.0f32;
        let x: Vec<f32> = (0..sr * 2).map(|i| (2.0 * std::f32::consts::PI * f * i as f32 / sr as f32).sin()).collect();
        let y = resample(&x, sr as u32, 48000).unwrap();
        assert_eq!(y.len(), 96000);
        // Compare with the ideal 48 kHz sine away from the edges (no phase shift).
        let mut err = 0.0f32;
        for i in 2000..94000 {
            let ideal = (2.0 * std::f32::consts::PI * f * i as f32 / 48000.0).sin();
            err = err.max((y[i] - ideal).abs());
        }
        assert!(err < 5e-3, "max error {err}");
    }
}

/// The duration of `path` from its header alone (nothing decoded): None when the header does
/// not say it (some MP3s) or the file is unreadable. For indexing many samples cheaply.
pub fn duration(path: &Path) -> Option<f32> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()).ok()?;
    let track = probed.format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?;
    let p = &track.codec_params;
    let (frames, rate) = (p.n_frames?, p.sample_rate?);
    (rate > 0).then(|| frames as f32 / rate as f32)
}

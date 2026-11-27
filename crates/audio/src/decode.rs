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

/// A PCM WAV whose "fmt " chunk is 20 bytes long (16 is the standard; some packs write 20,
/// with 4 bytes of nothing after): every player reads it, the decoder refuses it ("malformed
/// fmt_pcm chunk"). Such a file is read into memory and given to the decoder with a standard
/// 16-byte chunk; the file itself is never touched. Measured: 1,753 samples of one library.
fn padded_fmt_wav(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut head = [0u8; 24];
    File::open(path).ok()?.read_exact(&mut head).ok()?;
    let size = u32::from_le_bytes(head[16..20].try_into().ok()?);
    let tag = u16::from_le_bytes(head[20..22].try_into().ok()?);
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" || &head[12..16] != b"fmt " || size != 20 || tag != 1 {
        return None;
    }
    let all = std::fs::read(path).ok()?;
    if all.len() < 40 {
        return None;
    }
    let mut out = Vec::with_capacity(all.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((all.len() - 4 - 8) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&all[20..36]);
    out.extend_from_slice(&all[40..]);
    Some(out)
}

/// The file as the decoder reads it (see padded_fmt_wav).
fn source(path: &Path) -> Result<MediaSourceStream, Error> {
    let src: Box<dyn symphonia::core::io::MediaSource> = match padded_fmt_wav(path) {
        Some(b) => Box::new(std::io::Cursor::new(b)),
        None => Box::new(File::open(path).map_err(Error::Io)?),
    };
    Ok(MediaSourceStream::new(src, Default::default()))
}

/// Decodes the whole default track of `path` and downmixes it to mono.
pub fn decode_mono(path: &Path) -> Result<Pcm, Error> {
    let mss = source(path)?;
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
    let mss = source(path).ok()?;
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

/// A file the app's web view cannot play (AIFF) written as a 16-bit PCM WAV at `dst`, its own
/// rate, mono or stereo (more channels folded to the first two). The source is only read.
pub fn to_wav(src: &Path, dst: &Path) -> Result<(), Error> {
    let mss = source(src)?;
    let mut hint = Hint::new();
    if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| Error::Decode(format!("unsupported or unreadable file: {e}")))?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or_else(|| Error::Decode("no audio track".into()))?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| Error::Decode(format!("unsupported codec: {e}")))?;
    let mut out: Vec<i16> = Vec::new();
    let mut chans_out = 0usize;
    let mut buf: Option<SampleBuffer<f32>> = None;
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
            Err(SymError::DecodeError(_)) => continue,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        };
        let spec = *decoded.spec();
        if rate == 0 {
            rate = spec.rate;
        }
        let ch = spec.channels.count().max(1);
        if chans_out == 0 {
            chans_out = ch.min(2);
        }
        let need = decoded.capacity() as u64;
        if buf.as_ref().is_none_or(|b| (b.capacity() as u64) < need * ch as u64) {
            buf = Some(SampleBuffer::<f32>::new(need, spec));
        }
        let sb = buf.as_mut().unwrap();
        sb.copy_interleaved_ref(decoded);
        for f in sb.samples().chunks_exact(ch) {
            for c in 0..chans_out {
                out.push((f[c.min(ch - 1)].clamp(-1.0, 1.0) * 32767.0).round() as i16);
            }
        }
    }
    if out.is_empty() || rate == 0 || chans_out == 0 {
        return Err(Error::Decode("no audio decoded".into()));
    }
    let data_len = (out.len() * 2) as u32;
    let mut w: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    w.extend(b"RIFF");
    w.extend((36 + data_len).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend((chans_out as u16).to_le_bytes());
    w.extend(rate.to_le_bytes());
    w.extend((rate * chans_out as u32 * 2).to_le_bytes());
    w.extend((chans_out as u16 * 2).to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend(data_len.to_le_bytes());
    for s in out {
        w.extend(s.to_le_bytes());
    }
    let tmp = dst.with_extension("part");
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d).map_err(Error::Io)?;
    }
    std::fs::write(&tmp, w).map_err(Error::Io)?;
    std::fs::rename(&tmp, dst).map_err(Error::Io)
}

#[cfg(test)]
mod padded_fmt_tests {
    #[test]
    fn a_20_byte_fmt_chunk_decodes() {
        // 0.1 s of a 440 Hz sine, mono 16-bit, with a 20-byte fmt chunk as some packs write it
        let rate = 44100u32;
        let n = 4410usize;
        let data: Vec<u8> = (0..n).flat_map(|i| (((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 12000.0) as i16).to_le_bytes()).collect();
        let mut f = Vec::new();
        f.extend_from_slice(b"RIFF");
        f.extend_from_slice(&((4 + 8 + 20 + 8 + data.len()) as u32).to_le_bytes());
        f.extend_from_slice(b"WAVEfmt ");
        f.extend_from_slice(&20u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&rate.to_le_bytes());
        f.extend_from_slice(&(rate * 2).to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&16u16.to_le_bytes());
        f.extend_from_slice(&[0u8; 4]);
        f.extend_from_slice(b"data");
        f.extend_from_slice(&(data.len() as u32).to_le_bytes());
        f.extend_from_slice(&data);
        let p = std::env::temp_dir().join(format!("pf-fmt20-{}.wav", std::process::id()));
        std::fs::write(&p, &f).unwrap();
        let pcm = super::decode_mono(&p).expect("decodes");
        let d = super::duration(&p);
        std::fs::remove_file(&p).unwrap();
        assert_eq!(pcm.samples.len(), n);
        assert!((d.unwrap() - 0.1).abs() < 0.01);
    }
}

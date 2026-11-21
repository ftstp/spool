//! A render's waveform and loudness, read by the app (the interface may not read the file
//! itself): integrated loudness (LUFS, EBU R128), sample peak and RMS in dBFS, and a short
//! list of peaks to draw. Kept on disk next to the base, keyed by the file's path, size and
//! date, so a render is decoded once.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use ts_rs::TS;

/// Points of the drawn waveform.
const POINTS: usize = 600;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AudioInfo {
    /// 0..1, the loudest sample of each slice, POINTS of them.
    pub peaks: Vec<f32>,
    pub duration: f64,
    /// Integrated loudness (EBU R128 / ITU BS.1770), None for a silent file.
    pub lufs: Option<f64>,
    /// Loudest sample, dBFS (0 = full scale).
    pub peak_db: f64,
    /// Average level over the whole file, dBFS.
    pub rms_db: f64,
    /// Loudness range (LU), None when too short.
    pub lra: Option<f64>,
    #[ts(type = "number")]
    pub sample_rate: u32,
    #[ts(type = "number")]
    pub channels: u32,
}

fn db(x: f64) -> f64 {
    if x <= 0.0 { -144.0 } else { 20.0 * x.log10() }
}

/// Decodes the file and measures it.
pub fn analyse(path: &Path) -> Result<AudioInfo, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| i18n::tr!("audio.badFormat", e = e))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| i18n::tr!("audio.noTrack"))?.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| i18n::tr!("audio.badCodec", e = e))?;
    let rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2).max(1) as u32;
    let mut meter = ebur128::EbuR128::new(channels, rate, ebur128::Mode::I | ebur128::Mode::LRA | ebur128::Mode::SAMPLE_PEAK)
        .map_err(|e| e.to_string())?;
    // peaks per block of 1024 frames, reduced to POINTS at the end
    let (mut blocks, mut cur, mut in_block) = (Vec::<f32>::new(), 0f32, 0usize);
    let (mut sum_sq, mut n, mut peak) = (0f64, 0u64, 0f32);
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(i18n::tr!("err.reading", e = e)),
        };
        if packet.track_id() != track.id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(i18n::tr!("audio.decoding", e = e)),
        };
        let b = buf.get_or_insert_with(|| SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec()));
        if (b.capacity() as u64) < decoded.capacity() as u64 * decoded.spec().channels.count() as u64 {
            *b = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        }
        b.copy_interleaved_ref(decoded);
        let s = b.samples();
        meter.add_frames_f32(s).map_err(|e| e.to_string())?;
        for frame in s.chunks(channels as usize) {
            let m = frame.iter().fold(0f32, |a, x| a.max(x.abs()));
            for x in frame {
                sum_sq += (*x as f64) * (*x as f64);
            }
            n += frame.len() as u64;
            peak = peak.max(m);
            cur = cur.max(m);
            in_block += 1;
            if in_block == 1024 {
                blocks.push(cur);
                cur = 0.0;
                in_block = 0;
            }
        }
    }
    if in_block > 0 {
        blocks.push(cur);
    }
    let frames = n / channels as u64;
    let peaks: Vec<f32> = if blocks.is_empty() {
        Vec::new()
    } else {
        (0..POINTS)
            .map(|i| {
                let a = i * blocks.len() / POINTS;
                let b = ((i + 1) * blocks.len() / POINTS).max(a + 1).min(blocks.len());
                blocks[a.min(blocks.len() - 1)..b].iter().fold(0f32, |m, x| m.max(*x)).min(1.0)
            })
            .collect()
    };
    let lufs = meter.loudness_global().ok().filter(|l| l.is_finite());
    let lra = meter.loudness_range().ok().filter(|l| l.is_finite() && *l > 0.0);
    Ok(AudioInfo {
        peaks,
        duration: frames as f64 / rate as f64,
        lufs,
        peak_db: db(peak as f64),
        rms_db: if n > 0 { db((sum_sq / n as f64).sqrt()) } else { -144.0 },
        lra,
        sample_rate: rate,
        channels,
    })
}

/// The measures of `path`, from the cache folder when the file has not changed.
pub fn cached(path: &Path, cache_dir: &Path) -> Result<AudioInfo, String> {
    let md = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let key = format!("{}|{}|{mtime}|v1", path.to_string_lossy(), md.len());
    let hash: String = Sha256::digest(key.as_bytes()).iter().take(12).map(|b| format!("{b:02x}")).collect();
    let file = cache_dir.join(format!("{hash}.json"));
    if let Some(info) = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str::<AudioInfo>(&s).ok()) {
        return Ok(info);
    }
    let info = analyse(path)?;
    let _ = std::fs::create_dir_all(cache_dir);
    let _ = std::fs::write(&file, serde_json::to_string(&info).unwrap_or_default());
    Ok(info)
}

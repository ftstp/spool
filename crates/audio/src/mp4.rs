//! The edit list of an MP4/M4A audio track: how many samples the encoder put before the
//! music (AAC "priming", 1024 or 2112 samples) and how long the music really is (the last
//! frame is padded). symphonia 0.5 decodes every sample of the stream; ffmpeg and players
//! apply the edit list, so without it every window starts ~23 ms late.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Samples to skip at the start and (if known) to keep after that, at the track's own rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trim {
    pub skip: u64,
    pub keep: Option<u64>,
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn be64(b: &[u8]) -> u64 {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// Child boxes of a box body: (type, body).
fn boxes(mut b: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    while b.len() >= 8 {
        let size = be32(b) as u64;
        let kind = [b[4], b[5], b[6], b[7]];
        let (head, size) = match size {
            1 if b.len() >= 16 => (16, be64(&b[8..])),
            0 => (8, b.len() as u64),
            s => (8, s),
        };
        if size < head as u64 || size > b.len() as u64 {
            break;
        }
        out.push((kind, &b[head..size as usize]));
        b = &b[size as usize..];
    }
    out
}

fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(b).into_iter().find(|(k, _)| k == kind).map(|(_, v)| v)
}

/// Timescale of an `mvhd` or `mdhd` body.
fn timescale(b: &[u8]) -> Option<u32> {
    let v = *b.first()?;
    let off = if v == 1 { 4 + 16 } else { 4 + 8 };
    b.get(off..off + 4).map(be32)
}

/// Reads the `moov` box of an MP4 file (None when the file is not MP4 or has none).
fn read_moov(path: &Path) -> Option<Vec<u8>> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    let mut first = true;
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8]).ok()?;
        let mut size = be32(&h) as u64;
        let kind = [h[4], h[5], h[6], h[7]];
        if first && &kind != b"ftyp" {
            return None;
        }
        first = false;
        let mut head = 8;
        if size == 1 {
            f.read_exact(&mut h[8..16]).ok()?;
            size = be64(&h[8..]);
            head = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < head {
            return None;
        }
        if &kind == b"moov" {
            // a moov is a few hundred KB at most; refuse anything absurd
            if size > 64 << 20 {
                return None;
            }
            let mut body = vec![0u8; (size - head) as usize];
            f.read_exact(&mut body).ok()?;
            return Some(body);
        }
        pos += size;
    }
    None
}

/// The trim the first sound track's edit list asks for, or None (not MP4, no edit list,
/// or one this does not understand: several edits, an empty edit after the first...).
pub fn audio_trim(path: &Path, sample_rate: u32) -> Option<Trim> {
    let moov = read_moov(path)?;
    let movie_scale = timescale(child(&moov, b"mvhd")?)?;
    for (k, trak) in boxes(&moov) {
        if &k != b"trak" {
            continue;
        }
        let mdia = child(trak, b"mdia")?;
        let hdlr = child(mdia, b"hdlr")?;
        if hdlr.get(8..12) != Some(b"soun") {
            continue;
        }
        let media_scale = timescale(child(mdia, b"mdhd")?)?;
        let elst = child(child(trak, b"edts")?, b"elst")?;
        let v = *elst.first()?;
        let count = be32(elst.get(4..8)?);
        if count == 0 {
            return None;
        }
        let entry = 8;
        let (dur, media_time) = if v == 1 {
            (be64(elst.get(entry..entry + 8)?), be64(elst.get(entry + 8..entry + 16)?) as i64)
        } else {
            (be32(elst.get(entry..entry + 4)?) as u64, be32(elst.get(entry + 4..entry + 8)?) as i32 as i64)
        };
        // a leading empty edit (media_time -1) would mean silence to insert: not handled
        if media_time < 0 || count > 1 || media_scale == 0 || movie_scale == 0 {
            return None;
        }
        let sr = sample_rate as u128;
        let skip = (media_time as u128 * sr / media_scale as u128) as u64;
        let keep = (dur > 0).then(|| (dur as u128 * sr / movie_scale as u128) as u64);
        return Some(Trim { skip, keep });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn reads_an_aac_edit_list() {
        let mut mvhd = vec![0u8; 4 + 8];
        mvhd.extend_from_slice(&1000u32.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 80]);
        let mut mdhd = vec![0u8; 4 + 8];
        mdhd.extend_from_slice(&44100u32.to_be_bytes());
        mdhd.extend_from_slice(&[0u8; 8]);
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"soun");
        hdlr.extend_from_slice(&[0u8; 13]);
        let mut elst = vec![0u8, 0, 0, 0];
        elst.extend_from_slice(&1u32.to_be_bytes());
        elst.extend_from_slice(&60_000u32.to_be_bytes()); // 60 s in movie units
        elst.extend_from_slice(&1024u32.to_be_bytes()); // priming
        elst.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        let mdia = [bx(b"mdhd", &mdhd), bx(b"hdlr", &hdlr)].concat();
        let trak = [bx(b"edts", &bx(b"elst", &elst)), bx(b"mdia", &mdia)].concat();
        let moov = [bx(b"mvhd", &mvhd), bx(b"trak", &trak)].concat();
        let file = [bx(b"ftyp", b"M4A \0\0\0\0"), bx(b"mdat", &[0u8; 32]), bx(b"moov", &moov)].concat();
        let dir = std::env::temp_dir().join(format!("spool-mp4-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.m4a");
        std::fs::write(&p, file).unwrap();
        let t = audio_trim(&p, 44100);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(t, Some(Trim { skip: 1024, keep: Some(44100 * 60) }));
    }
}

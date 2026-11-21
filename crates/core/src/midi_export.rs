//! A MIDI file as heard on the MIDI page, written as a new file to drag into Live: its notes
//! transposed (drums left as they are), its tempo changed (each tempo event scaled, one added
//! when the file has none). The original is only read; the copy goes into the app's own
//! `exports` folder, emptied of files older than a day.
use std::path::{Path, PathBuf};

/// The speed that brings `bpm` to `target`, the closest to 1 among x1/2, x1, x2 (as the
/// MIDI page's "Lock to"), kept between half and double speed.
pub fn lock_rate(bpm: f64, target: f64) -> f64 {
    let r = target / bpm;
    let best = [r, r / 2.0, r * 2.0].into_iter().min_by(|a, b| a.ln().abs().total_cmp(&b.ln().abs())).unwrap_or(1.0);
    best.clamp(0.5, 2.0)
}

/// The tempo at the start of the file (120 when it says none).
pub fn first_bpm(b: &[u8]) -> f64 {
    let mut out = None;
    walk(b, |_, _, ev| {
        if let Ev::Tempo(at, tick) = ev
            && tick == 0
            && out.is_none()
        {
            let us = ((b[at] as u32) << 16) | ((b[at + 1] as u32) << 8) | b[at + 2] as u32;
            if us > 0 {
                out = Some(60e6 / us as f64);
            }
        }
    });
    out.unwrap_or(120.0)
}

enum Ev {
    /// The note byte's position, the channel.
    Note(usize, u8),
    /// The tempo's 3 bytes position, its tick.
    Tempo(usize, u64),
}

/// Every note and tempo event of every track: (track index, the track's data start, event).
fn walk(b: &[u8], mut f: impl FnMut(usize, usize, Ev)) -> Option<usize> {
    let tag = |i: usize| b.get(i..i + 4);
    let u32_at = |i: usize| b.get(i..i + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]) as usize);
    let h = (0..b.len().saturating_sub(14)).find(|&i| tag(i) == Some(b"MThd"))?;
    let ntr = b.get(h + 10..h + 12).map(|x| u16::from_be_bytes([x[0], x[1]]) as usize)?;
    let mut p = h + 8 + u32_at(h + 4)?;
    let mut first = None;
    for trk in 0..ntr {
        while p + 8 <= b.len() && tag(p) != Some(b"MTrk") {
            p += 8 + u32_at(p + 4)?;
        }
        if p + 8 > b.len() {
            break;
        }
        first.get_or_insert(p);
        let end = (p + 8 + u32_at(p + 4)?).min(b.len());
        let mut q = p + 8;
        let mut tick = 0u64;
        let mut status = 0u8;
        let vlq = |q: &mut usize| {
            let mut v = 0u64;
            for _ in 0..4 {
                let Some(&c) = b.get(*q) else { break };
                *q += 1;
                v = (v << 7) | (c & 0x7f) as u64;
                if c & 0x80 == 0 {
                    break;
                }
            }
            v
        };
        while q < end {
            tick += vlq(&mut q);
            let Some(&c) = b.get(q) else { break };
            let st = if c & 0x80 != 0 {
                q += 1;
                if c < 0xf0 {
                    status = c;
                }
                c
            } else {
                status
            };
            if st == 0xff {
                let Some(&ty) = b.get(q) else { break };
                q += 1;
                let len = vlq(&mut q) as usize;
                if ty == 0x51 && len >= 3 && q + 3 <= end {
                    f(trk, p, Ev::Tempo(q, tick));
                }
                q += len;
                if ty == 0x2f {
                    break;
                }
            } else if st == 0xf0 || st == 0xf7 {
                q += vlq(&mut q) as usize;
            } else if st >= 0x80 {
                let hi = st & 0xf0;
                if hi == 0xc0 || hi == 0xd0 {
                    q += 1;
                } else {
                    if (hi == 0x80 || hi == 0x90 || hi == 0xa0) && q < end {
                        f(trk, p, Ev::Note(q, st & 15));
                    }
                    q += 2;
                }
            } else {
                break; // a broken track: what was read is kept
            }
        }
        p = end;
    }
    first
}

/// The file rewritten: notes moved by `semis` (not channel 10, not at all for a drum file),
/// every tempo `rate` times faster (one added at the start of the first track when it has
/// none and the speed changes).
pub fn rewrite(b: &[u8], semis: i32, rate: f64, drums: bool) -> Option<Vec<u8>> {
    let mut out = b.to_vec();
    let mut tempos = 0;
    let rate = if rate.is_finite() && rate > 0.0 { rate } else { 1.0 };
    let first = walk(b, |_, _, ev| match ev {
        Ev::Note(at, ch) => {
            if !drums && ch != 9 && semis != 0 {
                out[at] = (b[at] as i32 + semis).clamp(0, 127) as u8;
            }
        }
        Ev::Tempo(at, _) => {
            tempos += 1;
            let us = ((b[at] as u32) << 16) | ((b[at + 1] as u32) << 8) | b[at + 2] as u32;
            let n = ((us as f64 / rate).round() as u32).clamp(1, 0xff_ffff);
            out[at..at + 3].copy_from_slice(&n.to_be_bytes()[1..]);
        }
    })?;
    if tempos == 0 && (rate - 1.0).abs() > 1e-6 {
        let us = ((500_000.0 / rate).round() as u32).clamp(1, 0xff_ffff);
        let ev = [0x00, 0xff, 0x51, 0x03, (us >> 16) as u8, (us >> 8) as u8, us as u8];
        let len = u32::from_be_bytes([out[first + 4], out[first + 5], out[first + 6], out[first + 7]]) + ev.len() as u32;
        out[first + 4..first + 8].copy_from_slice(&len.to_be_bytes());
        out.splice(first + 8..first + 8, ev);
    }
    Some(out)
}

/// The copy written into `dir` (files older than a day there removed first: it only ever
/// holds these copies). Returns its path.
pub fn export(src: &Path, dir: &Path, semis: i32, rate: f64, target: Option<f64>, drums: bool) -> Result<PathBuf, String> {
    let b = std::fs::read(src).map_err(|e| e.to_string())?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "midi".into());
    export_bytes(&b, &stem, dir, semis, rate, target, drums)
}

/// The same from a MIDI file's bytes (a clip of a project), named `stem` (made safe for a
/// file name).
pub fn export_bytes(b: &[u8], stem: &str, dir: &Path, semis: i32, rate: f64, target: Option<f64>, drums: bool) -> Result<PathBuf, String> {
    let stem: String = stem.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 32 { '-' } else { c }).collect();
    let stem = stem.trim().chars().take(120).collect::<String>();
    let b = b.to_vec();
    let rate = match target.filter(|t| *t > 0.0) {
        Some(t) => lock_rate(first_bpm(&b), t),
        None => rate.clamp(0.5, 2.0),
    };
    let out = rewrite(&b, semis, rate, drums).ok_or_else(|| i18n::tr!("midi.notMidi"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(86_400);
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.metadata().and_then(|m| m.modified()).is_ok_and(|m| m < old) && e.path().extension().is_some_and(|x| x == "mid") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let mut tags = Vec::new();
    if semis != 0 && !drums {
        tags.push(format!("{semis:+}"));
    }
    if (rate - 1.0).abs() > 1e-3 {
        tags.push(format!("{} BPM", (first_bpm(&b) * rate * 10.0).round() / 10.0));
    }
    let name = if tags.is_empty() { format!("{stem}.mid") } else { format!("{stem} ({}).mid", tags.join(", ")) };
    let to = dir.join(name);
    std::fs::write(&to, out).map_err(|e| e.to_string())?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One track: tempo 120 (or none), C4 on channel 1, D2 on channel 10, running status.
    fn file(tempo: bool) -> Vec<u8> {
        let mut t: Vec<u8> = Vec::new();
        if tempo {
            t.extend([0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20]);
        }
        t.extend([0x00, 0x90, 60, 100, 0x60, 60, 0]); // on, then off by running status
        t.extend([0x00, 0x99, 38, 90, 0x60, 0x89, 38, 0]);
        t.extend([0x00, 0xff, 0x2f, 0x00]);
        let mut f = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
        f.extend((t.len() as u32).to_be_bytes());
        f.extend(t);
        f
    }

    #[test]
    fn transposes_notes_but_not_drums_and_scales_the_tempo() {
        let f = file(true);
        assert_eq!(first_bpm(&f), 120.0);
        let o = rewrite(&f, 2, 1.5, false).unwrap();
        assert_eq!(o.len(), f.len());
        assert_eq!(first_bpm(&o).round(), 180.0);
        let notes: Vec<u8> = o.windows(2).filter(|w| w[0] == 0x90 || w[0] == 0x99 || w[0] == 0x89).map(|w| w[1]).collect();
        assert_eq!(notes, [62, 38, 38], "the note moved, the drum did not");
        // the note off by running status moved too
        assert!(o.windows(3).any(|w| w == [0x60, 62, 0]));
        assert_eq!(rewrite(&f, 2, 1.0, true).unwrap(), f, "a drum file keeps its notes");
    }

    #[test]
    fn a_tempo_is_added_when_the_file_has_none() {
        let f = file(false);
        let o = rewrite(&f, 0, 2.0, false).unwrap();
        assert_eq!(o.len(), f.len() + 7);
        assert_eq!(first_bpm(&o).round(), 240.0);
        assert_eq!(rewrite(&f, 0, 1.0, false).unwrap(), f, "no change, no tempo added");
        assert_eq!(lock_rate(87.0, 174.0), 1.0, "half time keeps its speed");
        assert!((lock_rate(170.0, 174.0) - 174.0 / 170.0).abs() < 1e-9);
    }
}

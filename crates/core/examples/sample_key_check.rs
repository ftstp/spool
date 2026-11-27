//! How right the key (loops) and the note (one-shots) measured on samples are, against the
//! samples whose name says them, on a COPY of a base (the files are only read).
//!     sample_key_check <base copy.sqlite> [how many of each]
use store::rusqlite::Connection;

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

fn pc(name: &str) -> Option<usize> {
    let n = name.trim_end_matches('m');
    NOTES.iter().position(|x| *x == n)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let con = Connection::open(a.next().expect("base copy")).unwrap();
    let n: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(300);
    let pick = |sql: &str| -> Vec<(String, String)> {
        let mut q = con.prepare(sql).unwrap();
        q.query_map([n as i64], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().flatten().collect()
    };
    // loops whose name gives a key (melodic families: a drum loop says nothing)
    let loops = pick("select path, key from samples where key is not null and is_loop = 1 and family in ('bass', 'melodic', 'vocal') and dur between 3 and 60 order by random() limit ?");
    // (score bucket) -> (n, exact, relative major/minor or same tonic)
    let mut by: std::collections::BTreeMap<i32, (usize, usize, usize)> = Default::default();
    for (path, said) in &loops {
        let Ok(p) = audio::decode::decode_mono(std::path::Path::new(path)) else { continue };
        let x = audio::decode::resample(&p.samples, p.sample_rate, 12_000).unwrap_or_default();
        let Some(k) = audio::key::estimate(&x, 12_000.0) else { continue };
        let (Some(t), minor) = (pc(said), said.ends_with('m')) else { continue };
        let exact = k.tonic == t && k.minor == minor;
        // the relative (C and Am share their notes) counts as near
        let rel = if minor { (t + 3) % 12 } else { (t + 9) % 12 };
        let near = exact || (k.tonic == rel && k.minor != minor);
        let b = (k.score * 10.0).floor() as i32;
        let e = by.entry(b).or_default();
        e.0 += 1;
        e.1 += exact as usize;
        e.2 += near as usize;
    }
    println!("loops ({}): score >= ... -> n, exact %, exact or relative %", loops.len());
    let mut cum = (0, 0, 0);
    for (b, (n, e, r)) in by.iter().rev() {
        cum = (cum.0 + n, cum.1 + e, cum.2 + r);
        println!("  >= {:.1}: {:>4}  exact {:>3}%  near {:>3}%", *b as f32 / 10.0, cum.0, cum.1 * 100 / cum.0.max(1), cum.2 * 100 / cum.0.max(1));
    }
    // one-shots whose name gives a note
    let shots = pick("select path, note from samples where note is not null and is_loop = 0 and family in ('bass', 'melodic', 'vocal') order by random() limit ?");
    let mut by: std::collections::BTreeMap<i32, (usize, usize)> = Default::default();
    for (path, said) in &shots {
        let Ok(p) = audio::decode::decode_mono(std::path::Path::new(path)) else { continue };
        let x = audio::decode::resample(&p.samples, p.sample_rate, 12_000).unwrap_or_default();
        let Some((got, share)) = audio::key::dominant_note(&x, 12_000.0) else { continue };
        let Some(t) = pc(said) else { continue };
        let b = (share * 20.0).floor() as i32;
        let e = by.entry(b).or_default();
        e.0 += 1;
        e.1 += (got == t) as usize;
    }
    println!("one-shots ({}): share >= ... -> n, right %", shots.len());
    let mut cum = (0, 0);
    for (b, (n, r)) in by.iter().rev() {
        cum = (cum.0 + n, cum.1 + r);
        println!("  >= {:.2}: {:>4}  right {:>3}%", *b as f32 / 20.0, cum.0, cum.1 * 100 / cum.0.max(1));
    }
}

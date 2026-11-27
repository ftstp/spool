//! The key of melodic loops measured with several chroma settings, against their names, on a
//! COPY of a base (the files are only read): which setting is right most often.
//!     sample_key_configs <base copy.sqlite> [how many]
use audio::key::{KeyConfig, Profile};
use store::rusqlite::Connection;

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

fn main() {
    let mut a = std::env::args().skip(1);
    let con = Connection::open(a.next().expect("base copy")).unwrap();
    let n: i64 = a.next().and_then(|x| x.parse().ok()).unwrap_or(300);
    let mut q = con
        .prepare("select path, key from samples where key is not null and is_loop = 1 and family in ('bass', 'melodic', 'vocal') and dur between 3 and 60 order by random() limit ?")
        .unwrap();
    let rows: Vec<(String, String)> = q.query_map([n], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().flatten().collect();
    let audio: Vec<(Vec<f32>, usize, bool)> = rows
        .iter()
        .filter_map(|(p, k)| {
            let d = audio::decode::decode_mono(std::path::Path::new(p)).ok()?;
            let x = audio::decode::resample(&d.samples, d.sample_rate, 12_000).ok()?;
            let t = NOTES.iter().position(|x| *x == k.trim_end_matches('m'))?;
            Some((x, t, k.ends_with('m')))
        })
        .collect();
    let base = KeyConfig::default();
    let configs: Vec<(&str, KeyConfig)> = vec![
        ("default (bgate 80-2000)", base),
        ("krumhansl", KeyConfig { profile: Profile::Krumhansl, ..base }),
        ("temperley", KeyConfig { profile: Profile::Temperley, ..base }),
        ("bgate 40-1000", KeyConfig { f_min: 40.0, f_max: 1000.0, ..base }),
        ("bgate 40-2000 log", KeyConfig { f_min: 40.0, log_gain: 100.0, ..base }),
        ("krumhansl 40-1000", KeyConfig { profile: Profile::Krumhansl, f_min: 40.0, f_max: 1000.0, ..base }),
        ("temperley 40-1000 log", KeyConfig { profile: Profile::Temperley, f_min: 40.0, f_max: 1000.0, log_gain: 100.0, ..base }),
        ("bgate no frame norm", KeyConfig { frame_norm: false, ..base }),
    ];
    println!("{} loops", audio.len());
    // the scale that holds the most of the chroma (relative keys share it), its tonic the
    // stronger of the two
    for (label, cfg) in [("scale fit 80-2000", base), ("scale fit 40-1000", KeyConfig { f_min: 40.0, f_max: 1000.0, ..base })] {
        let (mut ex, mut near, mut n) = (0, 0, 0);
        for (x, t, minor) in &audio {
            let Some(c) = audio::key::chroma(x, 12_000.0, &cfg) else { continue };
            let sum: f32 = c.iter().sum();
            if sum <= 0.0 {
                continue;
            }
            let major = [0usize, 2, 4, 5, 7, 9, 11];
            let (mut best, mut bt) = (f32::MIN, 0usize);
            for tonic in 0..12 {
                let inside: f32 = major.iter().map(|i| c[(tonic + i) % 12]).sum();
                if inside > best {
                    best = inside;
                    bt = tonic;
                }
            }
            // major tonic bt, or its relative minor (bt + 9): the one whose tonic and fifth weigh more
            let rel = (bt + 9) % 12;
            let wmaj = c[bt] + 0.5 * c[(bt + 7) % 12];
            let wmin = c[rel] + 0.5 * c[(rel + 7) % 12];
            let (kt, km) = if wmin > wmaj { (rel, true) } else { (bt, false) };
            n += 1;
            let e = kt == *t && km == *minor;
            let r = if *minor { (t + 3) % 12 } else { (t + 9) % 12 };
            ex += e as usize;
            near += (e || (kt == r && km != *minor)) as usize;
        }
        println!("  {label:<26} exact {:>3}%  exact or relative {:>3}%  ({n})", ex * 100 / n.max(1), near * 100 / n.max(1));
    }
    for (name, cfg) in configs {
        let (mut ex, mut near, mut n) = (0, 0, 0);
        for (x, t, minor) in &audio {
            let Some(k) = audio::key::estimate_with(x, 12_000.0, &cfg) else { continue };
            n += 1;
            let e = k.tonic == *t && k.minor == *minor;
            let rel = if *minor { (t + 3) % 12 } else { (t + 9) % 12 };
            ex += e as usize;
            near += (e || (k.tonic == rel && k.minor != *minor)) as usize;
        }
        println!("  {name:<26} exact {:>3}%  exact or relative {:>3}%  ({n})", ex * 100 / n.max(1), near * 100 / n.max(1));
    }
}

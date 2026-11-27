//! style_eval <sqlite COPY> [clap|old] [scale] [power]: how often the style suggestions find
//! the style a song's folders name when its names say nothing, with the styles heard by CLAP
//! (scale and power of `audio::styles::heard`, to tune them) or, while a base still has them,
//! by the style model of 0.1.69 to 0.1.77 (`audio_styles`, for comparison).
//! The songs whose folders name a style are hidden by groups (their folder, their names), in
//! two cases: the other groups keep their names ("folders"), or every one is hidden ("none");
//! and with 0, 6 or 12 answers, the songs asked chosen as the styles page chooses them.
use std::collections::{HashMap, HashSet};

fn f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn unit(mut v: Vec<f32>) -> Vec<f32> {
    audio::normalize(&mut v);
    v
}

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let st = store::Store::open(std::path::Path::new(&a[1])).unwrap();
    let con = &st.con;
    let mode = a.get(2).map(String::as_str).unwrap_or("clap");
    let scale: f32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(audio::styles::HEARD_SCALE);
    let power: f32 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(audio::styles::HEARD_POWER);
    let (base, rules) = index::style_inputs(con).unwrap();
    let read = |sql: &str| -> HashMap<i64, Vec<f32>> {
        let mut s = con.prepare(sql).unwrap();
        s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))).unwrap().map(Result::unwrap).map(|(i, b)| (i, f32s(&b))).collect()
    };
    let clap = read(&format!(
        "select s.id, a.embedding from songs s join audio_analysis a on a.rel = s.temoin where s.gone = 0 and a.model = '{}' and a.embedding is not null",
        index::AUDIO_MODEL
    ));
    let old = read("select s.id, a.styles from songs s join audio_styles a on a.rel = s.temoin where s.gone = 0 and a.styles is not null");
    let classes: Vec<String> = serde_json::from_str(include_str!("../../audio/styles/discogs400.json")).unwrap();
    // what each song is heard as, and the space its sound is compared in
    let mut space: HashMap<i64, Vec<f32>> = HashMap::new();
    let mut heard: HashMap<i64, Vec<(String, f32)>> = HashMap::new();
    for s in &base {
        let (Some(c), Some(o)) = (clap.get(&s.id), old.get(&s.id)) else { continue };
        if mode == "old" {
            let mut top: Vec<(usize, f32)> = o.iter().copied().enumerate().collect();
            top.sort_by(|x, y| y.1.total_cmp(&x.1));
            heard.insert(s.id, top.into_iter().take(5).map(|(k, v)| (classes[k].clone(), v)).collect());
            space.insert(s.id, unit(o.clone()));
        } else {
            let (hs, hp) = (audio::styles::HEARD_SCALE, audio::styles::HEARD_POWER);
            let h = audio::styles::heard(&unit(c.clone()), &[])
                .into_iter()
                .take(5)
                .map(|(k, v)| (k, scale * (v / hs).powf(1.0 / hp).powf(power)))
                .collect();
            heard.insert(s.id, h);
            space.insert(s.id, unit(c.clone()));
        }
    }
    let parent = |d: &str| d.trim_end_matches('/').rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
    let vocab = catalog::StyleVocab::default();
    let labelled: Vec<(usize, String)> = base
        .iter()
        .enumerate()
        .filter(|(_, s)| space.contains_key(&s.id))
        .filter_map(|(i, s)| catalog::named_style(&s.dir, &vocab).map(|t| (i, t)))
        .collect();
    let mut groups: Vec<String> = labelled.iter().map(|(i, _)| parent(&base[*i].dir)).collect::<HashSet<_>>().into_iter().collect();
    groups.sort();
    let fold: HashMap<String, usize> = groups.iter().enumerate().map(|(k, g)| (g.clone(), k % 5)).collect();
    eprintln!("{mode} scale {scale} power {power}: {} songs scored", labelled.len());
    for scenario in ["folders", "none"] {
        let mut line = format!("  {scenario:8}");
        for nq in [0usize, 6, 12] {
            let (mut n, mut h1, mut h3) = (0, 0, 0);
            let rounds: Vec<Vec<(usize, String)>> = if scenario == "none" {
                vec![labelled.clone()]
            } else {
                (0..5).map(|f| labelled.iter().filter(|(i, _)| fold[&parent(&base[*i].dir)] == f).cloned().collect()).collect()
            };
            for hidden in rounds {
                let hid: HashSet<usize> = hidden.iter().map(|x| x.0).collect();
                // the songs asked: the one most like the hidden songs least like any song whose
                // style is known (named by its folders), each in turn
                let known: Vec<&Vec<f32>> =
                    labelled.iter().filter(|(i, _)| !hid.contains(i)).map(|(i, _)| &space[&base[*i].id]).collect();
                let hv: Vec<&Vec<f32>> = hidden.iter().map(|(i, _)| &space[&base[*i].id]).collect();
                let mut cover: Vec<f64> = hv.iter().map(|v| known.iter().map(|k| dot(v, k)).fold(-1.0, f64::max)).collect();
                let mut asked: HashSet<usize> = HashSet::new();
                for _ in 0..nq.min(hidden.len()) {
                    let best = (0..hv.len())
                        .filter(|b| !asked.contains(&hidden[*b].0))
                        .max_by(|x, y| {
                            let g = |b: usize| (0..hv.len()).map(|j| dot(hv[b], hv[j]).max(cover[j]) - cover[j]).sum::<f64>();
                            g(*x).total_cmp(&g(*y))
                        })
                        .unwrap();
                    asked.insert(hidden[best].0);
                    for j in 0..hv.len() {
                        cover[j] = cover[j].max(dot(hv[best], hv[j]));
                    }
                }
                let truth: HashMap<usize, &String> = hidden.iter().map(|(i, t)| (*i, t)).collect();
                let songs: Vec<catalog::StyleSong> = base
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let mut s = s.clone();
                        if hid.contains(&i) {
                            s.dir = format!("eval/{}", s.id);
                            s.stems.clear();
                            s.audio.clear();
                            s.shared = false;
                            if asked.contains(&i) {
                                s.style = Some(truth[&i].clone());
                            }
                        }
                        s.heard = heard.get(&s.id).cloned().unwrap_or_default();
                        s
                    })
                    .collect();
                // the sound neighbours in that space, the same folder and the same render left out
                let near: HashMap<i64, Vec<(i64, f64)>> = songs
                    .iter()
                    .filter_map(|s| space.get(&s.id).map(|v| (s, v)))
                    .map(|(s, v)| {
                        let mut nb: Vec<(i64, f64)> = songs
                            .iter()
                            .filter(|o| o.id != s.id && o.dir != s.dir)
                            .filter_map(|o| space.get(&o.id).map(|w| (o.id, dot(v, w))))
                            .filter(|x| x.1 < 0.9999)
                            .collect();
                        nb.sort_by(|x, y| y.1.total_cmp(&x.1));
                        nb.truncate(20);
                        (s.id, nb)
                    })
                    .collect();
                let songs: Vec<catalog::StyleSong> = songs
                    .into_iter()
                    .map(|mut s| {
                        s.sound = near.get(&s.id).cloned().unwrap_or_default();
                        s
                    })
                    .collect();
                let out: HashMap<i64, Vec<catalog::StyleOption>> = catalog::deduce_with_examples(&songs, &rules).into_iter().collect();
                for (i, t) in &hidden {
                    if asked.contains(i) {
                        continue;
                    }
                    n += 1;
                    let o: Vec<String> = out.get(&base[*i].id).map(|o| o.iter().map(|x| catalog::canon(&x.style)).collect()).unwrap_or_default();
                    h1 += usize::from(o.first() == Some(t));
                    h3 += usize::from(o.iter().take(3).any(|x| x == t));
                }
            }
            line += &format!("   {nq:2} q: {:3.0} % / {:3.0} %", 100.0 * h1 as f64 / n as f64, 100.0 * h3 as f64 / n as f64);
        }
        eprintln!("{line}");
    }
}

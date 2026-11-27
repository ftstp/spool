//! A sample's family (drums, drum loop, bass, melodic, vocal, effect) from its CLAP embedding:
//! a small softmax regression shipped as weights (`styles/sample_families.json`). It sorts what
//! a library cannot sort by itself (too few named samples to learn from, or a sample the
//! library's own model is not sure of): from 0.95, right 97 % of the time on a library it was
//! not learnt on.
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct Weights {
    families: Vec<String>,
    /// one row of 512 per family
    w: Vec<Vec<f32>>,
    b: Vec<f32>,
}

static WEIGHTS: LazyLock<Weights> =
    LazyLock::new(|| serde_json::from_str(include_str!("../styles/sample_families.json")).expect("sample_families.json"));

/// The most likely family and its probability. The embedding is normalised here.
pub fn family(embedding: &[f32]) -> (String, f32) {
    let w = &*WEIGHTS;
    let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    let logits: Vec<f32> = w.w.iter().zip(&w.b).map(|(row, b)| row.iter().zip(embedding).map(|(a, x)| a * x / norm).sum::<f32>() + b).collect();
    let max = logits.iter().cloned().fold(f32::MIN, f32::max);
    let e: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
    let sum: f32 = e.iter().sum();
    let (i, p) = e.iter().enumerate().fold((0, 0.0f32), |a, (i, v)| if *v > a.1 { (i, *v) } else { a });
    (w.families[i].clone(), p / sum)
}

#[cfg(test)]
mod tests {
    #[test]
    fn weights_load_and_say_a_family() {
        let (f, p) = super::family(&[0.04; 512]);
        assert!(super::WEIGHTS.families.contains(&f));
        assert!((0.0..=1.0).contains(&p));
        assert_eq!(super::WEIGHTS.w.len(), super::WEIGHTS.families.len());
        assert!(super::WEIGHTS.w.iter().all(|r| r.len() == 512));
    }
}

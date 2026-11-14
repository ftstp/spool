//! Zero-shot style scores: the audio embedding against text embeddings of style
//! prompts, precomputed at build time with the CLAP text branch
//! (`tools/clap/export.py styles`) and embedded in the crate, so tagging
//! needs only the audio model.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StyleSet {
    pub model: String,
    pub revision: String,
    pub templates: Vec<String>,
    /// CLAP's audio-to-text logit scale (softmax temperature).
    pub logit_scale: f32,
    pub styles: Vec<Style>,
}

#[derive(Debug, Deserialize)]
pub struct Style {
    pub name: String,
    pub prompts: Vec<String>,
    pub embedding: Vec<f32>,
}

/// The style set shipped with the crate.
pub fn builtin() -> &'static StyleSet {
    static S: OnceLock<StyleSet> = OnceLock::new();
    S.get_or_init(|| {
        let mut set: StyleSet =
            serde_json::from_str(include_str!("../styles/styles.json")).expect("styles.json is valid");
        for s in &mut set.styles {
            crate::normalize(&mut s.embedding);
        }
        set
    })
}

impl StyleSet {
    /// Softmax probabilities of every style for a normalised audio embedding,
    /// highest first.
    pub fn score(&self, embedding: &[f32]) -> Vec<(String, f32)> {
        let logits: Vec<f32> =
            self.styles.iter().map(|s| crate::cosine(embedding, &s.embedding) * self.logit_scale).collect();
        let max = logits.iter().cloned().fold(f32::MIN, f32::max);
        let exps: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let mut out: Vec<(String, f32)> =
            self.styles.iter().zip(exps).map(|(s, e)| (s.name.clone(), e / sum)).collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// Like [`score`](Self::score), after subtracting `library_mean` (the mean
    /// embedding of the user's whole catalogue) from the embedding. Removing what
    /// every track shares stops a few prompts from winning everywhere; on the
    /// evaluation set it doubled top-1 accuracy (10 % -> 21 %).
    pub fn score_centered(&self, embedding: &[f32], library_mean: &[f32]) -> Vec<(String, f32)> {
        let mut v: Vec<f32> = embedding.iter().zip(library_mean).map(|(e, m)| e - m).collect();
        crate::normalize(&mut v);
        self.score(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_set_is_consistent() {
        let set = builtin();
        assert!(set.styles.len() >= 40);
        for s in &set.styles {
            assert_eq!(s.embedding.len(), crate::EMBEDDING_DIM, "{}", s.name);
        }
        // A style's own text embedding scores itself highest.
        let e = &set.styles[3].embedding;
        let scores = set.score(e);
        assert_eq!(scores[0].0, set.styles[3].name);
        let total: f32 = scores.iter().map(|s| s.1).sum();
        assert!((total - 1.0).abs() < 1e-4);
    }
}

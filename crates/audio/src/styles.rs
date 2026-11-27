//! Zero-shot style scores: the audio embedding against text embeddings of style
//! prompts, precomputed at build time with the CLAP text branch
//! (`tools/clap/export.py styles`) and embedded in the crate, so tagging
//! needs only the audio model.
//!
//! The styles a render is heard as ([`heard`]) come from the same scores, each style under
//! its class in Discogs' "Genre---Style" form, the scale brought to the one the style
//! suggestions read (see `HEARD_SCALE`).

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
    /// The class it is heard as ("Electronic---Drum n Bass").
    #[serde(default)]
    pub class: String,
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

/// Discogs' 400 classes ("Genre---Style"), in `styles/discogs400.json`'s order, each with the
/// text embedding of its style's name (`tools/clap/export.py discogs`, int8): a class the
/// user linked to a style of theirs ("Brass Band" is their jazz fanfare) is listened for too.
pub fn discogs() -> &'static [(String, Vec<f32>)] {
    static D: OnceLock<Vec<(String, Vec<f32>)>> = OnceLock::new();
    D.get_or_init(|| {
        let classes: Vec<String> =
            serde_json::from_str(include_str!("../styles/discogs400.json")).expect("discogs400.json is valid");
        let bytes = include_bytes!("../styles/discogs_text.bin");
        assert_eq!(bytes.len(), classes.len() * crate::EMBEDDING_DIM, "discogs_text.bin matches discogs400.json");
        classes
            .into_iter()
            .zip(bytes.chunks_exact(crate::EMBEDDING_DIM))
            .map(|(c, b)| {
                let mut v: Vec<f32> = b.iter().map(|x| *x as i8 as f32 / 127.0).collect();
                crate::normalize(&mut v);
                (c, v)
            })
            .collect()
    })
}

/// A style's share of the scores becomes how strongly it is heard: `HEARD_SCALE * p^HEARD_POWER`.
/// CLAP's softmax puts most of the weight on its first style, where the suggestions were tuned
/// on independent activations (half the renders heard at 0.22 or more, the second style at
/// 0.15). Tuned with `crates/index/examples/style_eval.rs` on 113 renders whose folders name
/// their style (the suggestions right 51 % of the time with no answer, 58 % with 12; the
/// results move by 3 points at most between 0.2 and 0.6, powers 0.3 to 1).
pub const HEARD_SCALE: f32 = 0.45;
pub const HEARD_POWER: f32 = 0.7;

/// The styles a normalised audio embedding is heard as: (class, strength 0..1), strongest
/// first. `linked`: other Discogs classes to listen for (the user's links), by class.
pub fn heard(embedding: &[f32], linked: &[&str]) -> Vec<(String, f32)> {
    let set = builtin();
    let mut cands: Vec<(&str, &[f32])> = set.styles.iter().map(|s| (s.class.as_str(), s.embedding.as_slice())).collect();
    for (c, e) in discogs() {
        if linked.contains(&c.as_str()) && !cands.iter().any(|x| x.0 == c) {
            cands.push((c.as_str(), e.as_slice()));
        }
    }
    let logits: Vec<f32> = cands.iter().map(|(_, e)| crate::cosine(embedding, e) * set.logit_scale).collect();
    let max = logits.iter().cloned().fold(f32::MIN, f32::max);
    let exps: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    let mut out: Vec<(String, f32)> =
        cands.iter().zip(exps).map(|((c, _), e)| (c.to_string(), HEARD_SCALE * (e / sum).powf(HEARD_POWER))).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
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
        // every style is heard under a class of the "Genre---Style" form
        for s in &set.styles {
            assert!(s.class.split_once("---").is_some_and(|(g, st)| !g.is_empty() && !st.is_empty()), "{}", s.name);
        }
    }

    #[test]
    fn discogs_names_match_the_styles() {
        let d = discogs();
        assert_eq!(d.len(), 400);
        // "techno" in the set and Discogs' "Techno" are the same prompts
        let techno = &builtin().styles.iter().find(|s| s.name == "techno").unwrap().embedding;
        let dt = &d.iter().find(|x| x.0 == "Electronic---Techno").unwrap().1;
        assert!(crate::cosine(techno, dt) > 0.99);
    }

    #[test]
    fn heard_names_classes_and_listens_for_links() {
        let set = builtin();
        let dnb = &set.styles.iter().find(|s| s.name == "drum and bass").unwrap().embedding;
        let h = heard(dnb, &[]);
        assert_eq!(h[0].0, "Electronic---Drum n Bass");
        assert!(h[0].1 > 0.0 && h[0].1 <= HEARD_SCALE);
        assert!(!h.iter().any(|x| x.0 == "Brass & Military---Brass Band"));
        let bb = &discogs().iter().find(|x| x.0 == "Brass & Military---Brass Band").unwrap().1;
        assert_eq!(heard(bb, &["Brass & Military---Brass Band"])[0].0, "Brass & Military---Brass Band");
    }
}

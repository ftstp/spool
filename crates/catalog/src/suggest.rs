//! The reference render of a song and the stage the evidence suggests.
//! The app proposes, the user validates: a validated stage never changes by itself.
use crate::names::{Rules, similarity, status_hint};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub const STAGES: [&str; 6] = ["draft", "arrangement", "mix", "master", "done", "pause"];
pub const MP3_DIR: &str = "- MP3/";
const DAY: f64 = 86400.0;

static LIMITER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)limit|maximi|ozone|pro-l|\bl[123]\b|elevate|smart:?limit|clipper|standardclip|gold ?clip|kclip|invisible|ash|master")
        .unwrap()
});
static MASTER_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)master").unwrap());
static MIX_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bmix|bounce|final|export|rendu|render").unwrap());
static STEM_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)stem|\btrack\s*\d|^\d{4}\s|\d+-audio|freeze|consolidat|sample|loop").unwrap());

#[derive(Debug, Clone, Deserialize)]
pub struct Audio {
    pub rel: String,
    pub size: i64,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct Candidate {
    pub rel: String,
    /// "dossier" (the song folder) or "MP3" (the shared "- MP3" renders folder).
    #[serde(rename = "where")]
    pub place: String,
    pub mtime: f64,
    #[ts(type = "number")]
    pub size: i64,
    /// "master", "mix" or "" from the file name.
    pub kind: String,
    /// Tiny files and stems / freezes are listed but never picked as the reference render.
    pub pickable: bool,
    /// Saved at most two days before the newest project.
    pub fresh: bool,
}

/// Every audio file of the song's folder (old / version subfolders included), newest first.
/// The reference render is the newest pickable one. In a folder shared by several songs only files
/// whose name is close to the song count. "- MP3" renders are a fallback for songs whose
/// folder holds no audio at all. `audio`: (file, the song folder it sits in).
/// The sub-folders of a song folder that hold this song's projects ("312 Project" under a
/// "Projects" folder every song shares): the first folder below `song_dir` on each file's
/// path, when it is an Ableton project folder. A file lying loose in `song_dir` gives none.
pub fn own_dirs(song_dir: &str, rels: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for rel in rels {
        let inner = if song_dir.is_empty() { Some(*rel) } else { rel.strip_prefix(&format!("{song_dir}/")) };
        let Some(inner) = inner else { continue };
        let Some((first, _)) = inner.split_once('/') else { continue };
        // only Ableton's project folders: an "old" / "v2" folder holds versions, not a song
        if !crate::is_project_dir(first) {
            continue;
        }
        let d = if song_dir.is_empty() { first.to_string() } else { format!("{song_dir}/{first}") };
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// `own` / `foreign`: in a folder shared by several songs, the sub-folders holding this
/// song's projects and those holding the others' (see `own_dirs`). A render inside the
/// song's own project folder is its, whatever its name; one inside another song's is never
/// its; elsewhere in the shared folder, the name decides.
#[allow(clippy::too_many_arguments)]
pub fn temoin_candidates_owned(
    rules: &Rules,
    song_dir: &str,
    vkey: &str,
    title: &str,
    audio: &[(Audio, String)],
    newest_mtime: f64,
    shared_folder: bool,
    own: &[String],
    foreign: &[String],
) -> Vec<Candidate> {
    let inside = |rel: &str, dirs: &[String]| dirs.iter().any(|d| rel.starts_with(&format!("{d}/")));
    let kept: Vec<(Audio, String)> = audio
        .iter()
        .filter(|(a, _)| !(shared_folder || song_dir.is_empty()) || !inside(&a.rel, foreign) || inside(&a.rel, own))
        .cloned()
        .collect();
    // the renders named after the song first, as always
    let mut out = temoin_candidates(rules, song_dir, vkey, title, &kept, newest_mtime, shared_folder);
    if shared_folder || song_dir.is_empty() {
        // then, only behind those, the renders of its own project folder whatever their name
        let mut extra: Vec<Candidate> = kept
            .iter()
            .filter(|(a, d)| d == song_dir && inside(&a.rel, own) && !out.iter().any(|c| c.rel == a.rel))
            .map(|(a, _)| {
                let file = a.rel.rsplit('/').next().unwrap_or("");
                let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
                candidate(a, stem, "dossier", newest_mtime)
            })
            .collect();
        extra.sort_by(|x, y| y.mtime.partial_cmp(&x.mtime).unwrap_or(std::cmp::Ordering::Equal));
        out.extend(extra);
    }
    out
}

fn candidate(a: &Audio, stem: &str, place: &str, newest_mtime: f64) -> Candidate {
    Candidate {
        rel: a.rel.clone(),
        place: place.into(),
        mtime: a.mtime,
        size: a.size,
        kind: if MASTER_WORD.is_match(stem) {
            "master".into()
        } else if MIX_WORD.is_match(stem) {
            "mix".into()
        } else {
            String::new()
        },
        pickable: a.size >= 200_000 && !STEM_WORD.is_match(stem),
        fresh: newest_mtime != 0.0 && a.mtime >= newest_mtime - 2.0 * DAY,
    }
}

pub fn temoin_candidates(
    rules: &Rules,
    song_dir: &str,
    vkey: &str,
    title: &str,
    audio: &[(Audio, String)],
    newest_mtime: f64,
    shared_folder: bool,
) -> Vec<Candidate> {
    let title_key = rules.variant_key(title);
    let (mut folder, mut mp3) = (Vec::new(), Vec::new());
    for (a, a_dir) in audio {
        let file = a.rel.rsplit('/').next().unwrap_or("");
        let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
        let akey = rules.variant_key(stem);
        let sim = similarity(&akey, vkey, 0.6).max(similarity(&akey, &title_key, 0.6));
        if a.rel.starts_with(MP3_DIR) {
            if sim >= 0.75 {
                mp3.push((a, stem, "MP3"));
            }
            continue;
        }
        // projects right in the projects folder (no folder of their own: one parent folder
        // holding every project) take a render of the same folder only by its name, as in a
        // folder shared by several songs
        if a_dir != song_dir || ((shared_folder || song_dir.is_empty()) && sim < 0.6) {
            continue;
        }
        folder.push((a, stem, "dossier"));
    }
    let chosen = if folder.is_empty() { mp3 } else { folder };
    let mut out: Vec<Candidate> = chosen
        .into_iter()
        .map(|(a, stem, place)| Candidate {
            rel: a.rel.clone(),
            place: place.into(),
            mtime: a.mtime,
            size: a.size,
            kind: if MASTER_WORD.is_match(stem) {
                "master".into()
            } else if MIX_WORD.is_match(stem) {
                "mix".into()
            } else {
                String::new()
            },
            pickable: a.size >= 200_000 && !STEM_WORD.is_match(stem),
            fresh: newest_mtime != 0.0 && a.mtime >= newest_mtime - 2.0 * DAY,
        })
        .collect();
    out.sort_by(|x, y| y.mtime.partial_cmp(&x.mtime).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// What the newest project of a song says (from the .als reader).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Newest {
    #[serde(default)]
    pub main_chain: Vec<String>,
    #[serde(default)]
    pub length_s: Option<i64>,
    #[serde(default)]
    pub tracks: std::collections::BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reason {
    pub code: String,
    pub stage: String,
    pub points: i64,
    pub text: String,
}

/// The reasons for each stage. `newest`: the song's newest active project, if any.
pub fn stage_evidence(song_dir: &str, newest: Option<&Newest>, cands: &[Candidate]) -> Vec<Reason> {
    let mut reasons = Vec::new();
    let mut push = |code: String, stage: &str, points: i64, text: String| {
        reasons.push(Reason { code, stage: stage.into(), points, text });
    };
    let parts: Vec<&str> = if song_dir.is_empty() { Vec::new() } else { song_dir.split('/').collect() };
    let parents: &[&str] = if parts.len() > 1 { &parts[..parts.len() - 1] } else { &[] };
    if let Some((hint, folder)) = status_hint(parents) {
        push(
            format!("folder:{hint}"),
            hint,
            3,
            i18n::tr!("suggest.filedIn", folder = folder.trim_matches(|c| c == ' ' || c == '-')),
        );
    }
    if let Some(d) = newest {
        if let Some(lim) = d.main_chain.iter().find(|n| LIMITER.is_match(n)) {
            push("limiter".into(), "master", 1, i18n::tr!("suggest.limiter", name = lim));
        }
        let (l, tr) = (d.length_s.unwrap_or(0), d.tracks.values().sum::<i64>());
        if l >= 150 && tr >= 8 {
            push("long".into(), "arrangement", 2, i18n::tr!("suggest.long", min = l / 60, sec = format!("{:02}", l % 60), tracks = tr));
        } else if (l != 0 && l < 60 && tr <= 8) || tr <= 3 {
            // a near-empty arrangement with 99 tracks is session work
            push("short".into(), "draft", 2, i18n::tr!("suggest.short", s = l, n = tr));
        } else {
            push("base".into(), "draft", 1, i18n::tr!("suggest.base"));
        }
    }
    for c in cands.iter().filter(|c| !c.kind.is_empty()).take(3) {
        let name = c.rel.rsplit('/').next().unwrap_or("");
        if c.kind == "master" {
            let pts = if c.fresh { 4 } else { 2 };
            let code = format!("export-master{}", if c.fresh { "-fresh" } else { "" });
            let text = if c.fresh { i18n::tr!("suggest.masterFresh", name = name) } else { i18n::tr!("suggest.masterOld", name = name) };
            push(code, "master", pts, text);
            break;
        }
        if c.kind == "mix" && c.fresh {
            push("export-mix".into(), "mix", 3, i18n::tr!("suggest.mixFresh", name = name));
            break;
        }
    }
    reasons
}

/// The best supported stage; ties go to the EARLIER stage (claiming "master" needs evidence
/// that outweighs the rest).
pub fn best_stage(reasons: &[Reason]) -> Option<&'static str> {
    let mut score: Vec<(&'static str, i64)> = Vec::new();
    for r in reasons {
        let st = STAGES.iter().find(|s| **s == r.stage).copied()?;
        match score.iter_mut().find(|(s, _)| *s == st) {
            Some(e) => e.1 += r.points,
            None => score.push((st, r.points)),
        }
    }
    let idx = |s: &str| STAGES.iter().position(|x| *x == s).unwrap_or(0) as i64;
    score.into_iter().max_by_key(|(s, p)| (*p, -idx(s))).map(|(s, _)| s)
}

/// The suggestion's signature: same stage and same kinds of evidence = the same suggestion
/// (a refused one does not come back until its evidence changes).
pub fn signature(stage: &str, reasons: &[Reason]) -> String {
    let mut codes: Vec<&str> = reasons.iter().filter(|r| r.stage == stage).map(|r| r.code.as_str()).collect();
    codes.sort();
    format!("{stage}|{}", codes.join(","))
}

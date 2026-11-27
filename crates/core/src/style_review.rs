//! "Refine your styles": what the user's folders call a style, checked against what the
//! renders sound like (CLAP against the styles' names, crates/audio's `styles::heard`).
//!
//! No question about styles at the start. Spool knows the styles of the world (Discogs' 400
//! and a few recent ones), not the user's own. Two kinds of questions, asked once each:
//! - a word of their folder names that comes back over several songs which sound alike
//!   ("Phonk", six songs heard as Trap): is it a style of theirs? Yes makes it one.
//! - a style their folders name where the renders sound like another one ("lofi" heard as
//!   Downtempo): is that their name for it?
//!
//! And, where Spool is unsure of a song's style, a confirmation in one click ("Spool thinks
//! this is dnb. Right?"), on the song the answer helps most (`StyleQuestion`).
//!
//! Every answer feeds the style suggestions of every song.
use crate::{App, Result, err};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

/// Spool must hear its style this strongly for a disagreement to count (half the renders
/// are heard at 0.22 or more; lofi, soft and mixed, at 0.11 to 0.17).
const SURE: f32 = 0.12;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReviewSong {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
    /// How strongly Spool hears its style (0..1).
    pub act: f32,
}

/// Songs whose style says one thing and Spool hears another.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReviewGroup {
    /// The user's style (as their folders or they wrote it).
    pub mine: String,
    /// The style heard ("Brass Band") and its genre ("Brass & Military").
    pub heard: String,
    pub genre: String,
    #[ts(type = "number")]
    pub n: i64,
    pub songs: Vec<ReviewSong>,
}

/// A word of the user's folder names that may be a style Spool does not know: it names
/// several songs, and they sound alike.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StyleCandidate {
    /// As their folders write it most.
    pub word: String,
    /// What the renders of its songs sound like most ("Trap"), and its genre; empty when they
    /// do not sound alike.
    pub heard: String,
    pub genre: String,
    #[ts(type = "number")]
    pub n: i64,
    pub songs: Vec<ReviewSong>,
    /// Names to offer for it: the style heard, its genre (lower case, Spool's names).
    pub near: Vec<String>,
    /// Other songs where the style heard is among the three strongest: what a link would move.
    #[ts(type = "number")]
    pub others: i64,
}

/// A word must name this many songs, this many of them heard, and this share of those must
/// sound like one style. A word that names a known style and more ("jazz fanfare", "electro
/// 80s") is a style of the user's on two songs, however they sound.
const CANDIDATE_SONGS: usize = 3;
const CANDIDATE_SHARE: f64 = 0.5;
const CANDIDATE_NARROWER: usize = 2;

/// A song whose style Spool is unsure of, to confirm in one click: the one most like the
/// songs whose style is unsure too and that no song of a known style sounds like.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StyleQuestion {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
    /// Its reference render, to hear.
    pub temoin: String,
    /// What Spool thinks it is, best first (one to three styles).
    pub options: Vec<String>,
    /// The other unsure songs that sound like it: what an answer helps.
    #[ts(type = "number")]
    pub helps: i64,
}

/// What an answer did: the other songs now suggested the style given, and the page again.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct StyleAnswer {
    #[ts(type = "number")]
    pub classed: i64,
    pub review: StyleReview,
}

/// A series of questions: a few at a time, never a questionnaire.
const SERIES: usize = 5;
/// An answer must help this many other songs for the question to be asked.
const MIN_HELP: i64 = 3;
/// A song suggested a style this clearly (percent) is not asked about (catalog's CLEAR).
const CLEAR: i64 = 60;
/// A song "sounds like" another when it is among that song's closest renders.
const CLOSEST: usize = 5;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct StyleReview {
    /// Songs to confirm, the most useful first (at most a series).
    pub questions: Vec<StyleQuestion>,
    /// Words of their folders that may be styles.
    pub candidates: Vec<StyleCandidate>,
    pub groups: Vec<ReviewGroup>,
    /// The user's own style words.
    pub words: Vec<String>,
    /// Their names for the styles Spool hears: (Spool's, lower case; theirs).
    pub links: Vec<(String, String)>,
    /// Their names for words of their folders: (the word, the style it counts for).
    pub names: Vec<(String, String)>,
    /// Every style Spool knows (lower case), to name one.
    pub known: Vec<String>,
    /// Renders heard / with a reference render.
    #[ts(type = "number")]
    pub heard: i64,
    #[ts(type = "number")]
    pub renders: i64,
}

impl App {
    /// The disagreements, the biggest first (two songs or more), and what the user taught.
    pub fn style_review(&self) -> Result<StyleReview> {
        // read before the reader is taken (never the state under the reader)
        let version = self.state.lock().unwrap().version;
        let r = self.reader.lock().unwrap();
        let con = &r.con;
        let vocab = index::style_vocab(con);
        let off: Vec<(String, String)> = r.get("style_review_off").unwrap_or_default();
        let heard = index::heard_styles(con).map_err(err)?;
        let renders: i64 = con
            .query_row("select count(distinct temoin) from songs where gone=0 and temoin is not null and temoin != ''", [], |x| x.get(0))
            .unwrap_or(0);
        let mut s = con
            .prepare("select id, title, style, song_dir from songs where gone=0 and archived=0")
            .map_err(err)?;
        let rows: Vec<(i64, Option<String>, Option<String>, Option<String>)> = s
            .query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?)))
            .map_err(err)?
            .flatten()
            .collect();
        let candidates = candidates(&r, &vocab, &heard, &rows);
        let mut groups: Vec<ReviewGroup> = Vec::new();
        let mut index: HashMap<(String, String), usize> = HashMap::new();
        for (id, title, style, dir) in rows {
            let Some(h) = heard.get(&id) else { continue };
            let Some((class, act)) = h.first().filter(|x| x.1 >= SURE) else { continue };
            // the user's style: the one written, else the one the folders name
            let mine = style
                .as_deref()
                .and_then(|x| x.split(',').next())
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .or_else(|| catalog::named_style(dir.as_deref().unwrap_or(""), &vocab));
            let Some(mine) = mine else { continue };
            let canon = catalog::canon(&mine);
            // agreement: one of the three strongest heard counts for the user's style, or
            // names its genre ("Boom Bap" for hip hop)
            if h.iter().take(3).filter(|x| x.1 >= catalog::HEARD_MIN).any(|(c, _)| {
                catalog::heard_style(c, &vocab).0 == canon || catalog::heard_genres(c).contains(&canon.as_str())
            }) {
                continue;
            }
            let (genre, name) = class.split_once("---").unwrap_or(("", class.as_str()));
            if off.iter().any(|(m, hd)| catalog::canon(m) == canon && hd.eq_ignore_ascii_case(name)) {
                continue;
            }
            let key = (canon.clone(), name.to_lowercase());
            let i = *index.entry(key).or_insert_with(|| {
                groups.push(ReviewGroup { mine: mine.clone(), heard: name.to_string(), genre: genre.to_string(), n: 0, songs: Vec::new() });
                groups.len() - 1
            });
            let g = &mut groups[i];
            g.n += 1;
            g.songs.push(ReviewSong { id, title, act: *act });
        }
        groups.retain(|g| g.n >= 2);
        for g in &mut groups {
            g.songs.sort_by(|a, b| b.act.total_cmp(&a.act));
            g.songs.truncate(6);
        }
        groups.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.mine.cmp(&b.mine)));
        let questions = {
            let mut memo = QUESTIONS.lock().unwrap();
            match memo.as_ref() {
                Some((v, q)) if *v == version => q.clone(),
                _ => {
                    let skipped: Vec<i64> = r.get("style_q_skipped").unwrap_or_default();
                    let q = questions(con, &vocab, &skipped).map_err(err)?;
                    *memo = Some((version, q.clone()));
                    q
                }
            }
        };
        Ok(StyleReview { questions, candidates, groups, words: vocab.words, links: vocab.links, names: vocab.names, known: catalog::known_styles(), heard: heard.len() as i64, renders })
    }

    /// "What Spool calls `heard` is my `mine`": every render it hears so counts for `mine`.
    pub fn style_keep(&self, heard: &str, mine: &str) -> Result<StyleReview> {
        let (heard, mine) = (heard.trim().to_lowercase(), mine.trim().to_string());
        if heard.is_empty() || mine.is_empty() {
            return Err("style".into());
        }
        {
            let st = self.store.lock().unwrap();
            let mut links: Vec<(String, String)> = st.get("style_links").unwrap_or_default();
            links.retain(|(h, _)| *h != heard);
            links.push((heard, mine.clone()));
            st.set("style_links", &links).map_err(err)?;
            // the user's name is then a style word of theirs (if it is not one already)
            if !catalog::is_style_name(&catalog::canon(&mine)) {
                let mut words: Vec<String> = st.get("style_words").unwrap_or_default();
                if !words.iter().any(|w| w.eq_ignore_ascii_case(&mine)) {
                    words.push(mine);
                    st.set("style_words", &words).map_err(err)?;
                }
            }
        }
        self.styles_changed()
    }

    /// "Yes, `word` is a style of mine, called `name`" (the word itself, a style Spool knows,
    /// or one of theirs): Spool reads the word in names from now on as that style. With
    /// `heard`, what Spool hears as that style anywhere counts for it too.
    pub fn style_word_yes(&self, word: &str, name: &str, heard: Option<&str>) -> Result<StyleReview> {
        let word = word.trim().to_string();
        let name = Some(name.trim()).filter(|n| !n.is_empty()).unwrap_or(&word).to_string();
        if word.is_empty() {
            return Err("style".into());
        }
        {
            let st = self.store.lock().unwrap();
            let mut words: Vec<String> = st.get("style_words").unwrap_or_default();
            if !words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                words.push(word.clone());
                st.set("style_words", &words).map_err(err)?;
            }
            let mut names: Vec<(String, String)> = st.get("style_names").unwrap_or_default();
            names.retain(|(w, _)| !w.eq_ignore_ascii_case(&word));
            if catalog::canon(&name) != catalog::canon(&word) {
                names.push((word.clone(), name.clone()));
            }
            st.set("style_names", &names).map_err(err)?;
            if let Some(heard) = heard.map(|h| h.trim().to_lowercase()).filter(|h| !h.is_empty() && catalog::canon(h) != catalog::canon(&name)) {
                let mut links: Vec<(String, String)> = st.get("style_links").unwrap_or_default();
                links.retain(|(h, _)| *h != heard);
                links.push((heard, name));
                st.set("style_links", &links).map_err(err)?;
            }
        }
        self.styles_changed()
    }

    /// "No, `word` is not a style": never asked again.
    pub fn style_word_no(&self, word: &str) -> Result<StyleReview> {
        {
            let st = self.store.lock().unwrap();
            let mut off: Vec<String> = st.get("style_words_off").unwrap_or_default();
            off.push(crate::norm(word));
            st.set("style_words_off", &off).map_err(err)?;
        }
        self.style_review()
    }

    /// Not a disagreement worth asking about again.
    pub fn style_dismiss(&self, mine: &str, heard: &str) -> Result<StyleReview> {
        {
            let st = self.store.lock().unwrap();
            let mut off: Vec<(String, String)> = st.get("style_review_off").unwrap_or_default();
            off.push((mine.to_string(), heard.to_string()));
            st.set("style_review_off", &off).map_err(err)?;
        }
        self.style_review()
    }

    /// The user's own style words (and their names for the styles Spool hears), as given.
    pub fn set_style_vocab(&self, words: Vec<String>, links: Vec<(String, String)>, names: Option<Vec<(String, String)>>) -> Result<StyleReview> {
        {
            let st = self.store.lock().unwrap();
            let mut seen = std::collections::HashSet::new();
            let words: Vec<String> = words
                .into_iter()
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty() && seen.insert(w.to_lowercase()))
                .collect();
            st.set("style_words", &words).map_err(err)?;
            let links: Vec<(String, String)> =
                links.into_iter().map(|(h, m)| (h.trim().to_lowercase(), m.trim().to_string())).filter(|(h, m)| !h.is_empty() && !m.is_empty()).collect();
            st.set("style_links", &links).map_err(err)?;
            if let Some(names) = names {
                // a word no longer theirs has no name of theirs either
                let names: Vec<(String, String)> =
                    names.into_iter().filter(|(w, n)| words.iter().any(|x| x.eq_ignore_ascii_case(w)) && !n.trim().is_empty()).collect();
                st.set("style_names", &names).map_err(err)?;
            }
        }
        self.styles_changed()
    }

    /// The answer to a question: the song's style (as if chosen by hand: undoable), or none
    /// ("Mixed", "I do not know": the song is not asked about again).
    pub fn style_answer(&self, sid: i64, style: Option<String>) -> Result<StyleAnswer> {
        let style = style.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
        let Some(style) = style else {
            {
                let st = self.store.lock().unwrap();
                let mut skipped: Vec<i64> = st.get("style_q_skipped").unwrap_or_default();
                if !skipped.contains(&sid) {
                    skipped.push(sid);
                }
                let from = skipped.len().saturating_sub(5000);
                st.set("style_q_skipped", &skipped[from..].to_vec()).map_err(err)?;
            }
            *QUESTIONS.lock().unwrap() = None;
            return Ok(StyleAnswer { classed: 0, review: self.style_review()? });
        };
        let before = suggested(&self.reader.lock().unwrap().con);
        let mut body = serde_json::Map::new();
        body.insert("style".into(), serde_json::Value::String(style.clone()));
        self.song_edit(sid, body)?;
        // the others learn from it at once
        {
            let st = self.store.lock().unwrap();
            let tx = st.con.unchecked_transaction().map_err(err)?;
            index::refresh_styles(&tx).map_err(err)?;
            tx.commit().map_err(err)?;
        }
        self.state.lock().unwrap().version += 1;
        self.notify();
        let after = suggested(&self.reader.lock().unwrap().con);
        let c = catalog::canon(&style);
        let classed = after.iter().filter(|(id, st)| **id != sid && **st == c && before.get(*id) != Some(&c)).count() as i64;
        Ok(StyleAnswer { classed, review: self.style_review()? })
    }

    /// The songs not asked about again.
    pub fn style_skipped(&self) -> Vec<i64> {
        self.reader.lock().unwrap().get("style_q_skipped").unwrap_or_default()
    }

    /// The style suggestions again, with what the user just taught; the pages reload.
    fn styles_changed(&self) -> Result<StyleReview> {
        {
            let st = self.store.lock().unwrap();
            let tx = st.con.unchecked_transaction().map_err(err)?;
            index::refresh_styles(&tx).map_err(err)?;
            tx.commit().map_err(err)?;
        }
        self.state.lock().unwrap().version += 1;
        self.notify();
        self.style_review()
    }
}

/// The questions of the last version of the base (they take a second on a large library).
static QUESTIONS: std::sync::Mutex<Option<(u64, Vec<StyleQuestion>)>> = std::sync::Mutex::new(None);

/// The first style suggested for each song without one (canonical).
fn suggested(con: &store::rusqlite::Connection) -> HashMap<i64, String> {
    let Ok(mut s) = con.prepare("select song_id, value from suggestions where kind='style' and state='open'") else { return HashMap::new() };
    s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map(|rows| rows.flatten().map(|(id, v)| (id, catalog::canon(&v))).collect())
        .unwrap_or_default()
}

/// The next series of questions (see `StyleQuestion`): each time the unsure song whose
/// answer would help the most other unsure songs (those it is among the closest renders of,
/// closer than any song of a known style), as long as it helps `MIN_HELP` of them.
pub(crate) fn questions(con: &store::rusqlite::Connection, vocab: &catalog::StyleVocab, skipped: &[i64]) -> store::rusqlite::Result<Vec<StyleQuestion>> {
    struct S {
        id: i64,
        title: Option<String>,
        dir: String,
        temoin: String,
        emb: Vec<f32>,
        known: bool,
        options: Vec<String>,
    }
    // what is suggested for each song without a style: (style, pct), best first
    let mut opts: HashMap<i64, Vec<(String, i64)>> = HashMap::new();
    {
        let mut s = con.prepare("select song_id, reasons from suggestions where kind='style' and state='open'")?;
        for (id, reasons) in s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.flatten() {
            let v: Vec<serde_json::Value> = serde_json::from_str(&reasons).unwrap_or_default();
            opts.insert(id, v.iter().filter_map(|o| Some((o.get("style")?.as_str()?.to_string(), o.get("pct")?.as_i64()?))).collect());
        }
    }
    let mut songs: Vec<S> = Vec::new();
    {
        let mut s = con.prepare(
            "select s.id, s.title, s.style, coalesce(s.song_dir, ''), s.temoin, a.embedding from songs s \
             join audio_analysis a on a.rel = s.temoin \
             where s.gone = 0 and s.archived = 0 and a.model = ? and a.embedding is not null",
        )?;
        let rows = s.query_map([index::AUDIO_MODEL], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
            ))
        })?;
        for (id, title, style, dir, temoin, blob) in rows.flatten() {
            let mut emb: Vec<f32> = blob.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            if emb.len() != audio::EMBEDDING_DIM {
                continue;
            }
            audio::normalize(&mut emb);
            let o = opts.get(&id).cloned().unwrap_or_default();
            let written = style.as_deref().is_some_and(|x| !x.trim().is_empty());
            // a style written, one its folders name, or one clearly suggested
            let known = written || o.first().is_some_and(|x| x.1 >= CLEAR) || catalog::named_style(&dir, vocab).is_some();
            songs.push(S { id, title, dir, temoin, emb, known, options: o.into_iter().map(|x| x.0).collect() });
        }
    }
    let unsure: Vec<usize> = (0..songs.len()).filter(|&i| !songs[i].known && !skipped.contains(&songs[i].id)).collect();
    if unsure.len() <= MIN_HELP as usize {
        return Ok(Vec::new());
    }
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let same = |i: usize, j: usize| !songs[i].dir.is_empty() && songs[i].dir == songs[j].dir;
    // each unsure song: its similarity to the other unsure ones, how close its CLOSEST-th
    // closest render is, and how close the closest song of a known style is (versions of the
    // same song, in the same folder, left out)
    let n = unsure.len();
    let mut sim = vec![f32::MIN; n * n];
    let mut near = vec![f32::MIN; n];
    let mut cover = vec![f32::MIN; n];
    for (a, &i) in unsure.iter().enumerate() {
        let mut all: Vec<f32> = Vec::with_capacity(songs.len());
        for (j, o) in songs.iter().enumerate() {
            if j == i || same(i, j) {
                continue;
            }
            let x = dot(&songs[i].emb, &o.emb);
            all.push(x);
            if o.known {
                cover[a] = cover[a].max(x);
            }
        }
        for (b, &j) in unsure.iter().enumerate() {
            if a != b && !same(i, j) {
                sim[a * n + b] = dot(&songs[i].emb, &songs[j].emb);
            }
        }
        all.sort_by(|x, y| y.total_cmp(x));
        near[a] = all.get(CLOSEST - 1).copied().unwrap_or(f32::MIN);
    }
    let mut out = Vec::new();
    let mut asked = vec![false; n];
    while out.len() < SERIES {
        let helps = |b: usize, cover: &[f32]| (0..n).filter(|&j| j != b && sim[b * n + j] >= near[j] && sim[b * n + j] > cover[j]).count() as i64;
        let Some((b, h)) = (0..n).filter(|&b| !asked[b]).map(|b| (b, helps(b, &cover))).max_by_key(|x| x.1) else { break };
        if h < MIN_HELP {
            break;
        }
        asked[b] = true;
        for j in 0..n {
            cover[j] = cover[j].max(sim[b * n + j]);
        }
        let s = &songs[unsure[b]];
        // what Spool thinks: the suggestions, then what it hears
        let mut options: Vec<String> = Vec::new();
        let heard = index::heard_for(con, &s.emb);
        for o in s.options.iter().cloned().chain(heard.iter().take(3).map(|(c, _)| catalog::heard_style(c, vocab).1)) {
            if options.len() < 3 && !options.iter().any(|x| catalog::canon(x) == catalog::canon(&o)) {
                options.push(o);
            }
        }
        out.push(StyleQuestion { id: s.id, title: s.title.clone(), temoin: s.temoin.clone(), options, helps: h });
    }
    Ok(out)
}

/// The words of the folder names that may be styles of the user's (see `StyleCandidate`).
fn candidates(
    r: &store::Store,
    vocab: &catalog::StyleVocab,
    heard: &HashMap<i64, Vec<(String, f32)>>,
    rows: &[(i64, Option<String>, Option<String>, Option<String>)],
) -> Vec<StyleCandidate> {
    let con = &r.con;
    let roots = store::Roots::load(con);
    // names that are not styles: artists, song titles, what the user said no to
    let mut not: HashSet<String> = r
        .get::<Vec<String>>("artists")
        .unwrap_or_default()
        .iter()
        .chain(roots.list.iter().map(|x| &x.artist))
        .chain(r.get::<std::collections::BTreeMap<String, String>>("artist_aliases").unwrap_or_default().keys())
        .chain(r.get::<std::collections::BTreeMap<String, String>>("artist_feats").unwrap_or_default().keys())
        .chain(r.get::<Vec<String>>("artist_ignored").unwrap_or_default().iter())
        .chain(r.get::<Vec<String>>("style_words_off").unwrap_or_default().iter())
        .map(|a| crate::norm(a))
        .collect();
    not.extend(rows.iter().filter_map(|x| x.1.as_deref().map(crate::norm)));
    let ok = |low: &str| {
        let words: Vec<&str> = low.split(' ').collect();
        words.len() <= 3
            && low.chars().filter(|c| c.is_alphabetic()).count() >= 3
            && !words.iter().all(|w| w.len() <= 2 || w.chars().all(|c| c.is_ascii_digit()))
            && !crate::GENERIC_FOLDERS.contains(&low)
            && !catalog::is_style_name(low)
            && !vocab.has_word(low)
            && !not.contains(low)
    };
    let skip = |f: &str| catalog::status_hint(&[f]).is_some() || catalog::is_project_dir(f) || catalog::is_version_dir(f);
    let split = crate::regex_lite_split();
    // each word: how it is written (and how often), the songs it names
    let mut found: HashMap<String, (HashMap<String, usize>, HashSet<i64>)> = HashMap::new();
    for (id, _, _, dir) in rows {
        let dir = dir.as_deref().unwrap_or("");
        let inner = roots.split(dir).map(|x| x.1).unwrap_or(dir);
        let parts: Vec<&str> = inner.split('/').filter(|p| !p.is_empty()).collect();
        for (i, folder) in parts.iter().enumerate() {
            if skip(folder) {
                continue;
            }
            let pieces = split(folder);
            // a song's own folder is its title, unless cut in parts ("Phonk - Night Drive")
            if i + 1 == parts.len() && pieces.len() < 2 {
                continue;
            }
            for piece in pieces {
                let low = crate::norm(&piece);
                if ok(&low) {
                    let e = found.entry(low).or_default();
                    *e.0.entry(piece).or_default() += 1;
                    e.1.insert(*id);
                }
            }
        }
    }
    let titles: HashMap<i64, Option<String>> = rows.iter().map(|x| (x.0, x.1.clone())).collect();
    let dirs: HashMap<i64, &str> = rows.iter().map(|x| (x.0, x.3.as_deref().unwrap_or(""))).collect();
    let mut out: Vec<StyleCandidate> = Vec::new();
    for (low, (spell, ids)) in found {
        // a known style and more: "jazz fanfare" (each other word a real word, not "d" or "80")
        let words: Vec<&str> = low.split(' ').collect();
        let styled = (0..words.len()).any(|i| {
            catalog::is_style_name(words[i]) || (i + 1 < words.len() && catalog::is_style_name(&format!("{} {}", words[i], words[i + 1])))
        });
        let narrower = styled && words.iter().all(|w| catalog::is_style_name(w) || (w.len() >= 3 && w.chars().all(char::is_alphabetic)))
            && !words.iter().all(|w| catalog::is_style_name(w));
        if ids.len() < if narrower { CANDIDATE_NARROWER } else { CANDIDATE_SONGS } {
            continue;
        }
        // what its heard songs sound like: the style most of them have among their three strongest
        let hs: Vec<(i64, &Vec<(String, f32)>)> = ids.iter().filter_map(|id| heard.get(id).map(|h| (*id, h))).collect();
        let mut count: HashMap<String, (usize, String, Vec<(i64, f32)>)> = HashMap::new();
        for (id, h) in &hs {
            for (class, act) in h.iter().take(3).filter(|x| x.1 >= catalog::HEARD_MIN) {
                let e = count.entry(catalog::canon(class.rsplit("---").next().unwrap_or(class))).or_insert((0, class.clone(), Vec::new()));
                e.0 += 1;
                e.2.push((*id, *act));
            }
        }
        let best = count
            .into_iter()
            .max_by(|a, b| a.1.0.cmp(&b.1.0).then_with(|| b.0.cmp(&a.0)))
            .filter(|(_, (k, _, _))| hs.len() >= CANDIDATE_SONGS && *k as f64 >= CANDIDATE_SHARE * hs.len() as f64);
        let songs: Vec<(i64, f32)> = match &best {
            Some((canon, (_, _, songs))) => {
                // the songs that sound so already have that style in their names ("DNB -
                // Croozader - Variable"): the word is something else, an artist, an album
                let named = songs.iter().filter(|(id, _)| catalog::named_style(dirs[id], vocab).as_deref() == Some(canon.as_str())).count();
                if !narrower && named * 2 >= songs.len() {
                    continue;
                }
                songs.clone()
            }
            None if narrower => ids.iter().map(|id| (*id, heard.get(id).and_then(|h| h.first()).map(|x| x.1).unwrap_or(0.0))).collect(),
            None => continue,
        };
        let mut songs = songs;
        songs.sort_by(|a, b| b.1.total_cmp(&a.1));
        let word = spell.into_iter().max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0))).map(|x| x.0).unwrap_or_default();
        let (genre, name) = best
            .as_ref()
            .map(|(_, (_, class, _))| class.split_once("---").map(|(g, n)| (g.to_string(), n.to_string())).unwrap_or_default())
            .unwrap_or_default();
        // names to offer: the style heard and its genre
        let mut near: Vec<String> = Vec::new();
        if let Some((canon, (_, class, _))) = &best {
            near.push(canon.clone());
            near.extend(catalog::heard_genres(class).into_iter().take(1).map(String::from));
        }
        let others = best.as_ref().map_or(0, |(canon, _)| {
            heard
                .iter()
                .filter(|(id, h)| {
                    !ids.contains(id)
                        && h.iter().take(3).filter(|x| x.1 >= catalog::HEARD_MIN).any(|(c, _)| catalog::canon(c.rsplit("---").next().unwrap_or(c)) == *canon)
                })
                .count() as i64
        });
        out.push(StyleCandidate {
            word,
            heard: name,
            genre,
            n: ids.len() as i64,
            near,
            others,
            songs: songs.into_iter().take(6).map(|(id, act)| ReviewSong { id, title: titles.get(&id).cloned().flatten(), act }).collect(),
        });
    }
    out.sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.word.cmp(&b.word)));
    out.truncate(12);
    out
}

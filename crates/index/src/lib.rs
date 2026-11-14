//! The scanner: one pass over the projects folder.
//!
//! 1. walk the folder; re-read only the .als whose size or date changed (all of them once
//!    when the reader learns something new), in parallel;
//! 2. find moved samples (same name and size elsewhere, the old system's user folder);
//! 3. classify every plugin reference against what Live has installed;
//! 4. group files into songs, keeping song ids and everything the user edited;
//! 5. refresh the reference renders, the finishing checklist, the stage and style suggestions.
//!
//! Ported from the prototype (`apf/indexer.py`, `apf/suggest.py`, `apf/style.py`).
mod songs;
mod suggestions;

/// The audio model whose analyses (`audio_analysis.model`) the suggestions and the song
/// sheet use (core's `sound`): LAION CLAP larger_clap_general at revision ada0c23.
pub const AUDIO_MODEL: &str = "laion/larger_clap_general@ada0c23";
pub mod walk;

use plugins::{Matcher, Ref, trial_key};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime};
use store::Store;
use store::rusqlite::{self, params};

/// Bump when the reader extracts something new: every set is then read once again.
pub const READER_VERSION: i64 = 3;

pub struct Scanner {
    matcher: Option<Matcher>,
    sample_idx: Option<walk::SampleIndex>,
    /// The search of the previous system's user folder (name -> [[size, path]]).
    pub deep_file: Option<PathBuf>,
    deep: Option<HashMap<String, Vec<(i64, String)>>>,
    /// When the MIDI library and sample folders were last walked, and which: big libraries
    /// are walked again every 10 minutes (or when the list changes), not at every pass.
    lib_walk: Option<(Vec<String>, Instant)>,
    /// Set to stop the pass in progress (what was read is kept).
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Report {
    pub read: usize,
    pub gone: usize,
    pub classified: usize,
    pub songs: usize,
    pub ms_walk: u128,
    pub ms_read: u128,
    pub ms_classify: u128,
    pub ms_group: u128,
    pub ms_suggest: u128,
    pub changed: bool,
    /// Stopped by the user: what was read is kept, the rest waits for the next pass.
    pub cancelled: bool,
    /// Files moved by the app while the disk was read: nothing was written, run it again.
    pub stale: bool,
}

/// Where a scan is, for the interface: the step, how far into it, what is being read.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Progress {
    pub stage: String,
    pub done: usize,
    /// 0: not known (the walk counts files as it finds them).
    pub total: usize,
    pub detail: String,
}

pub type Tick<'a> = &'a (dyn Fn(Progress) + Sync);

fn say(tick: Tick, stage: &str, done: usize, total: usize, detail: &str) {
    tick(Progress { stage: stage.into(), done, total, detail: detail.into() });
}

fn now() -> f64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    pub fn new() -> Scanner {
        Scanner { matcher: None, sample_idx: None, deep_file: None, deep: None, lib_walk: None, cancel: Default::default() }
    }

    /// The old drive's search is read again at the next need (a new search was written).
    pub fn forget_deep(&mut self) {
        self.deep = None;
    }

    /// The sample index is built again at the next need (sample folders changed).
    pub fn forget_samples(&mut self) {
        self.sample_idx = None;
    }

    /// One pass. `full`: re-read and re-classify everything.
    pub fn sync(&mut self, st: &Mutex<Store>, full: bool) -> rusqlite::Result<Report> {
        self.sync_with(st, full, &|_| {}, &AtomicU64::new(0))
    }

    /// One pass, saying where it is through `tick`. The base is locked only to read what is
    /// known and to write what was found, never while the disk is walked or the files read: the
    /// app stays usable during a long pass. `fs_gen` goes up whenever the app moves files; if it
    /// moved during the pass, nothing is written and the report says `stale` (run it again).
    pub fn sync_with(&mut self, st: &Mutex<Store>, full: bool, tick: Tick, fs_gen: &AtomicU64) -> rusqlite::Result<Report> {
        let mut rep = Report::default();
        let g0 = fs_gen.load(Ordering::SeqCst);
        let (roots, old_drive) = {
            let st = st.lock().unwrap();
            let d = st.get::<String>("old_drive").map(|d| d.trim_end_matches(':').to_uppercase()).filter(|d| !d.is_empty());
            (store::Roots::load(&st.con), d)
        };
        if !roots.list.iter().any(|r| Path::new(&r.path).is_dir()) {
            return Ok(rep);
        }
        let env = als::Env::detect(old_drive.clone());

        let t = Instant::now();
        let stage = "Recherche des projets";
        say(tick, stage, 0, 0, "");
        let mut w = walk::walk_roots(&roots, &mut |n, dir| say(tick, stage, n, 0, &dir.to_string_lossy()));
        // the MIDI files the walk met in the projects folders: read last
        let project_midi = std::mem::take(&mut w.midi);
        rep.ms_walk = t.elapsed().as_millis();
        // what the base knows, read at once (and the lock let go)
        let (known, reader_v, live_stored, mut audio_known, sample_dirs) = {
            let st = st.lock().unwrap();
            let known: HashMap<String, (i64, f64)> = {
                let mut s = st.con.prepare("select rel, size, mtime from files")?;
                s.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?.collect::<Result<_, _>>()?
            };
            let audio: HashMap<String, (i64, f64)> = {
                let mut s = st.con.prepare("select rel, size, mtime from audio")?;
                s.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?.collect::<Result<_, _>>()?
            };
            (known, st.get::<i64>("reader_version_rs"), st.get::<f64>("live_db_mtime_rs"), audio, st.get::<Vec<String>>("sample_dirs").unwrap_or_default())
        };
        // a folder that is not there (a drive unplugged) keeps what was known of it: its
        // files are not "gone", nothing of it is read again
        let absent: Vec<&str> =
            roots.list.iter().filter(|r| !Path::new(&r.path).is_dir()).map(|r| r.label.as_str()).collect();
        let in_absent = |rel: &str| roots.split(rel).is_none_or(|(r, _)| absent.contains(&r.label.as_str()));
        if !absent.is_empty() {
            for (rel, (size, mtime)) in &known {
                if in_absent(rel) {
                    w.als.insert(rel.clone(), (*size, *mtime, 0.0));
                }
            }
        }
        // a new reader reads the projects again (the MIDI files keep theirs)
        let midi_full = full;
        let full = full || reader_v != Some(READER_VERSION);
        let todo: Vec<&String> = w
            .als
            .iter()
            .filter(|(rel, (size, mtime, _))| full || known.get(*rel) != Some(&(*size, *mtime)))
            .map(|(r, _)| r)
            .collect();
        let gone: Vec<&String> = known.keys().filter(|r| !w.als.contains_key(*r)).collect();
        let live_db = plugins::live_db();
        let live_mtime = [live_db.clone(), live_db.with_extension("db-wal")]
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
            .max()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let reclassify_all = full || live_stored != Some(live_mtime) || self.matcher.is_none();
        let audio_known = std::mem::take(&mut audio_known);
        if !absent.is_empty() {
            for (rel, (size, mtime)) in &audio_known {
                if in_absent(rel) {
                    w.audio.insert(rel.clone(), (*size, *mtime, 0.0));
                }
            }
        }
        let audio_changed = w.audio.len() != audio_known.len()
            || w.audio.iter().any(|(k, (s, m, _))| audio_known.get(k) != Some(&(*s, *m)));
        let absent_labels: Vec<String> = absent.iter().map(|a| a.to_string()).collect();
        if todo.is_empty() && gone.is_empty() && !reclassify_all && !audio_changed {
            rep.changed |= self.sync_midi(st, &roots, project_midi, midi_full, &absent_labels, tick)?;
            rep.cancelled = self.stopped();
            return Ok(rep);
        }
        rep.changed = true;

        // 1. read what changed, in parallel (past ~16 threads the 100 MB sets fight over memory)
        let t = Instant::now();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16))
            .build()
            .expect("thread pool");
        let (stage, total) = ("Lecture des projets", todo.len());
        let count = std::sync::atomic::AtomicUsize::new(0);
        say(tick, stage, 0, total, "");
        let mut results: Vec<(String, Value)> = pool.install(|| {
            todo.par_iter()
                .filter_map(|rel| {
                    if self.stopped() {
                        return None;
                    }
                    let p = roots.abs(rel)?;
                    let v = serde_json::to_value(als::read_als(&p, &env)).unwrap_or(Value::Null);
                    let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    say(tick, stage, n, total, rel);
                    Some(((*rel).clone(), v))
                })
                .collect()
        });
        self.resolve_samples(&sample_dirs, &roots, &env, &mut results, tick);
        rep.read = results.len();
        rep.ms_read = t.elapsed().as_millis();

        // the base from here to the commit: what was found is written in one go
        let guard = st.lock().unwrap();
        if fs_gen.load(Ordering::SeqCst) != g0 {
            rep.changed = false;
            rep.stale = true;
            return Ok(rep);
        }
        let tx = guard.con.unchecked_transaction()?;
        {
            let mut up = tx.prepare(
                "insert into files(rel, size, mtime, ctime, data, read_at) values(?,?,?,?,?,?) \
                 on conflict(rel) do update set size=excluded.size, mtime=excluded.mtime, ctime=excluded.ctime, \
                 data=excluded.data, read_at=excluded.read_at",
            )?;
            let t_now = now();
            for (rel, data) in &results {
                let (size, mtime, ctime) = w.als[rel];
                up.execute(params![rel, size, mtime, ctime, data.to_string(), t_now])?;
            }
            for g in &gone {
                tx.execute("delete from files where rel=?", [g])?;
            }
            tx.execute("delete from backups", [])?;
            for (d, n) in &w.backups {
                tx.execute("insert into backups(dir, n) values(?, ?)", params![d, n])?;
            }
            if audio_changed {
                tx.execute("delete from audio", [])?;
                for (rel, (s, m, _)) in &w.audio {
                    tx.execute("insert into audio(rel, size, mtime) values(?,?,?)", params![rel, s, m])?;
                }
            }
        }
        rep.gone = gone.len();

        // 2. plugins: the files read now, or every file when Live's list changed
        let t = Instant::now();
        say(tick, "Comparaison avec les plugins installés", 0, 0, "");
        if reclassify_all {
            self.matcher = Some(Matcher::load(old_drive.as_deref()));
            store_set(&tx, "live_db_mtime_rs", &live_mtime)?;
        }
        let only: Option<HashSet<&String>> = (!reclassify_all).then(|| todo.iter().copied().collect());
        rep.classified = classify(&tx, self.matcher.as_ref().unwrap(), only.as_ref())?;
        store_set(&tx, "reader_version_rs", &READER_VERSION)?;
        rep.ms_classify = t.elapsed().as_millis();

        // 3. songs, 4. suggestions
        let t = Instant::now();
        say(tick, "Regroupement des morceaux", 0, 0, "");
        rep.songs = songs::group(&tx)?;
        apply_pending(&tx)?;
        rep.ms_group = t.elapsed().as_millis();
        let t = Instant::now();
        say(tick, "Témoins, moodboards et suggestions", 0, 0, "");
        suggestions::refresh_all(&tx, &roots)?;
        rep.ms_suggest = t.elapsed().as_millis();
        tx.commit()?;
        drop(guard);
        // 5. the MIDI files, last: a big library must not hold the projects back
        if !self.stopped() {
            rep.changed |= self.sync_midi(st, &roots, project_midi, midi_full, &absent_labels, tick)?;
        }
        rep.cancelled = self.stopped();
        Ok(rep)
    }

    fn stopped(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The MIDI files: those met in the projects folders, those of the MIDI library folders and
    /// of the sample folders. Read in batches, each saved at once: a stopped scan keeps what it
    /// read and the next one goes on from there. Returns whether anything changed.
    fn sync_midi(
        &mut self,
        st: &Mutex<Store>,
        roots: &store::Roots,
        project_midi: std::collections::BTreeMap<String, walk::Stat>,
        midi_full: bool,
        absent: &[String],
        tick: Tick,
    ) -> rusqlite::Result<bool> {
        let full = midi_full;
        let in_absent = |rel: &str| roots.split(rel).is_none_or(|(r, _)| absent.contains(&r.label));
        let mut w = walk::Walk { midi: project_midi, ..Default::default() };
        // MIDI files: the MIDI library folders, and the sample folders (packs hold MIDI too)
        let (mut midi_dirs, sample_dirs, midi_known) = {
            let st = st.lock().unwrap();
            let known: HashMap<String, (i64, f64)> = {
                let mut s = st.con.prepare("select rel, size, mtime from midi")?;
                s.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?.collect::<Result<_, _>>()?
            };
            (st.get::<Vec<String>>("midi_dirs").unwrap_or_default(), st.get::<Vec<String>>("sample_dirs").unwrap_or_default(), known)
        };
        for d in sample_dirs {
            if !midi_dirs.iter().any(|m| m.eq_ignore_ascii_case(&d)) {
                midi_dirs.push(d);
            }
        }
        let walk_libs = full
            || self.lib_walk.as_ref().is_none_or(|(dirs, at)| *dirs != midi_dirs || at.elapsed() >= std::time::Duration::from_secs(600));
        if walk_libs && !midi_dirs.is_empty() {
            let stage = "Recherche des fichiers MIDI";
            let cancel = std::sync::Arc::clone(&self.cancel);
            walk::walk_midi_dirs(&midi_dirs, &mut w, &mut |n, dir| say(tick, stage, n, 0, &dir.to_string_lossy()), &|| {
                cancel.load(std::sync::atomic::Ordering::Relaxed)
            });
            say(tick, stage, w.midi.len(), 0, "");
        }
        if walk_libs && !self.stopped() {
            self.lib_walk = Some((midi_dirs.clone(), Instant::now()));
        }
        // a MIDI library folder not there (a drive unplugged) keeps what was read of it
        let midi_lib = |rel: &str| rel.as_bytes().get(1) == Some(&b':') || rel.starts_with('/');
        let lib_absent = |rel: &str| {
            midi_lib(rel)
                && !midi_dirs.iter().any(|d| {
                    let d = d.replace('\\', "/");
                    rel.to_lowercase().starts_with(&format!("{}/", d.trim_end_matches('/').to_lowercase())) && Path::new(&d).is_dir()
                })
        };
        // MIDI files: read again when new or changed (a folder not there keeps its own)
        if !walk_libs {
            // libraries not walked this time: what was read of them stands
            for (rel, (s, m)) in &midi_known {
                if midi_lib(rel) {
                    w.midi.entry(rel.clone()).or_insert((*s, *m, 0.0));
                }
            }
        }
        let midi_todo: Vec<(&String, &walk::Stat)> =
            w.midi.iter().filter(|(rel, (s, m, _))| midi_full || midi_known.get(*rel) != Some(&(*s, *m))).collect();
        let midi_gone: Vec<&String> = midi_known
            .keys()
            .filter(|r| !w.midi.contains_key(*r) && if midi_lib(r) { !lib_absent(r) || midi_dirs.is_empty() } else { !in_absent(r) })
            .collect();
        if self.stopped() {
            return Ok(false);
        }
        if midi_todo.is_empty() && midi_gone.is_empty() {
            return Ok(false);
        }
        let (stage, total) = ("Analyse des fichiers MIDI", midi_todo.len());
        say(tick, stage, 0, total, "");
        let count = std::sync::atomic::AtomicUsize::new(0);
        {
            let st = st.lock().unwrap();
            let tx = st.con.unchecked_transaction()?;
            for g in &midi_gone {
                tx.execute("delete from midi where rel=?", [g])?;
            }
            tx.commit()?;
        }
        for batch in midi_todo.chunks(2000) {
            if self.stopped() {
                break;
            }
            let read: Vec<(String, (i64, f64), String)> = batch
                .par_iter()
                .filter_map(|(rel, (s, m, _))| {
                    let p = if midi_lib(rel) { PathBuf::from(rel.as_str()) } else { roots.abs(rel)? };
                    let bytes = std::fs::read(&p).ok()?;
                    // the file name and the two folders above it
                    let name: String = rel.rsplit('/').take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/");
                    let name = name.as_str();
                    let info = midi::analyse(&bytes, name);
                    let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    say(tick, stage, n, total, rel);
                    Some(((*rel).clone(), (*s, *m), serde_json::to_string(&info).unwrap_or_default()))
                })
                .collect();
            // each batch saved at once, the base held only for its writing
            let st = st.lock().unwrap();
            let tx = st.con.unchecked_transaction()?;
            for (rel, (s, m), data) in &read {
                tx.execute(
                    "insert into midi(rel, size, mtime, data) values(?,?,?,?) on conflict(rel) do update set \
                     size=excluded.size, mtime=excluded.mtime, data=excluded.data",
                    params![rel, s, m, data],
                )?;
            }
            tx.commit()?;
        }
        Ok(true)
    }


    fn deep(&mut self) -> &HashMap<String, Vec<(i64, String)>> {
        if self.deep.is_none() {
            let hits = self
                .deep_file
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .and_then(|v| serde_json::from_value(v["hits"].clone()).ok())
                .unwrap_or_default();
            self.deep = Some(hits);
        }
        self.deep.as_ref().unwrap()
    }

    /// A missing sample with the same name (and size) elsewhere is "moved", not lost.
    fn resolve_samples(
        &mut self,
        sample_dirs: &[String],
        roots: &store::Roots,
        env: &als::Env,
        results: &mut [(String, Value)],
        tick: Tick,
    ) {
        let any = results
            .iter()
            .any(|(_, d)| d["samples_missing"].as_array().is_some_and(|a| a.iter().any(|s| s["found_at"].is_null())));
        if !any {
            return;
        }
        if self.sample_idx.is_none() {
            say(tick, "Indexation des samples", 0, 0, "");
            let mut roots: Vec<PathBuf> = roots.list.iter().map(|r| PathBuf::from(&r.path)).collect();
            roots.extend(env.live_resources.clone());
            roots.push(env.home.join("Documents/Ableton/User Library"));
            roots.push(env.home.join("Documents/Ableton/Factory Packs"));
            roots.extend(sample_dirs.iter().map(PathBuf::from).filter(|p| p.is_dir()));
            self.sample_idx = Some(walk::sample_index(&roots));
        }
        let deep = self.deep().clone();
        let idx = self.sample_idx.as_ref().unwrap();
        for (_, d) in results.iter_mut() {
            let Some(missing) = d["samples_missing"].as_array_mut() else { continue };
            for s in missing.iter_mut() {
                if !s["found_at"].is_null() {
                    continue;
                }
                let path = s["path"].as_str().unwrap_or("").to_string();
                let name = path.replace('\\', "/").rsplit('/').next().unwrap_or("").to_lowercase();
                // a size of 0 (or none) says nothing: any file of that name will do
                let size = s["size"].as_i64().filter(|z| *z != 0);
                let fits = |h: &&(i64, String)| size.is_none_or(|z| h.0 == z);
                let mut hits: Vec<(i64, String)> =
                    idx.get(&name).map(|v| v.iter().filter(fits).cloned().collect()).unwrap_or_default();
                let mut by_name = false;
                if hits.is_empty() {
                    hits = deep
                        .get(&name)
                        .map(|v| {
                            v.iter()
                                .filter(fits)
                                .filter(|h| als::is_file(Path::new(&h.1)) || Path::new(&h.1).exists())
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    by_name = !hits.is_empty();
                }
                s["found_at"] = walk::best_hit(&path, &hits).map(|h| Value::String(h.1.clone())).unwrap_or(Value::Null);
                s["by_name"] = Value::Bool(by_name);
            }
        }
    }
}

/// The previous system's user folder: the old drive + the user name most lost sample paths
/// carry ("C:/Users/<name>/..."). None without an old drive, or when no such folder exists.
pub fn old_user_dir(st: &Store) -> Option<PathBuf> {
    let drive = st.get::<String>("old_drive")?.trim_end_matches(':').to_uppercase();
    if drive.is_empty() {
        return None;
    }
    let mut users: HashMap<String, usize> = HashMap::new();
    let mut s = st.con.prepare("select data from files where data like '%samples_missing%'").ok()?;
    let rows: Vec<String> = s.query_map([], |r| r.get::<_, Option<String>>(0)).ok()?.flatten().flatten().collect();
    for d in rows {
        let v: Value = serde_json::from_str(&d).unwrap_or(Value::Null);
        for smp in v["samples_missing"].as_array().into_iter().flatten() {
            let p = smp["path"].as_str().unwrap_or("").replace('\\', "/");
            if p.get(..9).is_some_and(|x| x.eq_ignore_ascii_case("c:/users/"))
                && let Some(u) = p.get(9..).and_then(|r| r.split('/').next()).filter(|u| !u.is_empty())
            {
                *users.entry(u.to_string()).or_default() += 1;
            }
        }
    }
    let mut us: Vec<(String, usize)> = users.into_iter().collect();
    us.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    us.into_iter().map(|(u, _)| PathBuf::from(format!("{drive}:/Users/{u}"))).find(|p| p.is_dir())
}

/// Walks the old user folder once for the samples still lost (by name) and writes what it
/// found to `deep_file` ({root, at, files, hits: name -> [[size, path]]}), the prototype's
/// format. Returns (folder, files read, names found). `tick(files read)` reports progress.
pub fn deep_search(st: &Store, deep_file: &Path, tick: &mut dyn FnMut(usize)) -> Result<(String, usize, usize), String> {
    let root = old_user_dir(st).ok_or_else(|| i18n::tr!("scan.noOldDisk"))?;
    let mut wanted: HashSet<String> = HashSet::new();
    {
        let mut s = st.con.prepare("select data from files").map_err(|e| e.to_string())?;
        let rows: Vec<String> =
            s.query_map([], |r| r.get::<_, Option<String>>(0)).map_err(|e| e.to_string())?.flatten().flatten().collect();
        for d in rows {
            let v: Value = serde_json::from_str(&d).unwrap_or(Value::Null);
            for smp in v["samples_missing"].as_array().into_iter().flatten() {
                if smp["found_at"].is_null() || smp["by_name"] == true {
                    let p = smp["path"].as_str().unwrap_or("").replace('\\', "/");
                    wanted.insert(p.rsplit('/').next().unwrap_or("").to_lowercase());
                }
            }
        }
    }
    const SKIP: [&str; 3] = ["appdata", "$recycle.bin", "system volume information"];
    let mut hits: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    let (mut n, mut stack) = (0usize, vec![root.clone()]);
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().to_lowercase();
            if ft.is_dir() {
                if !SKIP.contains(&name.as_str()) {
                    stack.push(e.path());
                }
                continue;
            }
            n += 1;
            if n % 2000 == 0 {
                tick(n);
            }
            if wanted.contains(&name)
                && let Ok(md) = e.metadata()
            {
                hits.entry(name).or_default().push((md.len() as i64, e.path().to_string_lossy().into_owned()));
            }
        }
    }
    let found = hits.len();
    let body = json!({"root": root.to_string_lossy(), "at": now(), "files": n, "hits": hits});
    std::fs::write(deep_file, body.to_string()).map_err(|e| e.to_string())?;
    Ok((root.to_string_lossy().into_owned(), n, found))
}

fn store_set<T: serde::Serialize>(tx: &rusqlite::Transaction, k: &str, v: &T) -> rusqlite::Result<()> {
    tx.execute(
        "insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v",
        params![k, serde_json::to_string(v).unwrap()],
    )?;
    Ok(())
}

/// After a merge or a split decided in the app: regroup and refresh, nothing re-read.
pub fn regroup(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let roots = store::Roots::load(tx);
    songs::group(tx)?;
    apply_pending(tx)?;
    suggestions::refresh_all(tx, &roots)
}

/// The user's choices restored from choices.json onto a rebuilt base (see
/// `app_core::App::open`), put back on their songs once a scan has found them again, matched by
/// place (folder + variant). Kept until then.
fn apply_pending(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let Some(pend) = setting::<Value>(tx, "pending_choices").filter(|v| !v.is_null()) else { return Ok(()) };
    const COLS: [&str; 17] = [
        "title",
        "artist",
        "style",
        "key",
        "tempo",
        "date",
        "stage",
        "stage_source",
        "temoin",
        "temoin_source",
        "notes",
        "rating",
        "kinds",
        "ft",
        "user_fields",
        "checklist",
        "archived",
    ];
    let sql = format!("update songs set {} where id=?", COLS.map(|c| format!("{c}=?")).join(", "));
    for s in pend["songs"].as_array().into_iter().flatten() {
        let id: Option<i64> = tx
            .query_row(
                "select id from songs where song_dir=? and vkey=? and gone=0",
                rusqlite::params![s["song_dir"].as_str(), s["vkey"].as_str()],
                |r| r.get(0),
            )
            .ok();
        let Some(id) = id else { continue };
        let mut vals: Vec<rusqlite::types::Value> = COLS
            .iter()
            .map(|c| match &s[*c] {
                Value::Null => rusqlite::types::Value::Null,
                Value::Bool(b) => rusqlite::types::Value::Integer(*b as i64),
                Value::Number(n) => n
                    .as_i64()
                    .map(rusqlite::types::Value::Integer)
                    .unwrap_or_else(|| rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0))),
                Value::String(t) => rusqlite::types::Value::Text(t.clone()),
                other => rusqlite::types::Value::Text(other.to_string()),
            })
            .collect();
        vals.push(rusqlite::types::Value::Integer(id));
        tx.execute(&sql, rusqlite::params_from_iter(vals))?;
    }
    store_set(tx, "pending_choices", &Value::Null)
}

/// After an edit on one song: its reference render, checks, card picture and stage suggestion.
pub fn refresh_song(tx: &rusqlite::Transaction, sid: i64) -> rusqlite::Result<()> {
    let roots = store::Roots::load(tx);
    suggestions::refresh_song(tx, &roots, sid)
}

/// What the app would say of one song's style, a noted one included (the song page's hover).
pub fn style_guess(con: &rusqlite::Connection, sid: i64) -> rusqlite::Result<Vec<catalog::StyleOption>> {
    let get = |k: &str| -> Vec<String> {
        con.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0))
            .ok()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    };
    suggestions::style_guess(con, &catalog::Rules::new(&get("artists"), &get("work_words")), sid)
}

/// After a style was emptied: the deduced styles come back at once.
pub fn refresh_styles(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let artists: Vec<String> = setting(tx, "artists").unwrap_or_default();
    let words: Vec<String> = setting(tx, "work_words").unwrap_or_default();
    suggestions::refresh_styles(tx, &catalog::Rules::new(&artists, &words))
}

fn setting<T: serde::de::DeserializeOwned>(tx: &rusqlite::Transaction, k: &str) -> Option<T> {
    let v: String = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok()?;
    serde_json::from_str(&v).ok()
}

/// A project file's severity: its worst plugin (the ignored ones left out), a sample found
/// elsewhere makes it repairable, a sample lost or a file that could not be read blocks it.
/// The same for a full classify and for a recompute after a choice changed.
fn file_severity<'a>(plugins: impl Iterator<Item = (&'a str, &'a str)>, ignored: &HashSet<String>, data: &Value) -> i64 {
    let mut sev = 0;
    for (name, status) in plugins {
        if !ignored.contains(&name.to_lowercase()) {
            sev = sev.max(plugins::status_severity(status));
        }
    }
    let missing = data["samples_missing"].as_array().map(Vec::as_slice).unwrap_or_default();
    let lost = missing.iter().filter(|s| s["found_at"].is_null()).count();
    if missing.len() > lost {
        sev = sev.max(1);
    }
    if lost > 0 || !data["error"].is_null() {
        sev = 3;
    }
    sev
}

/// A version found nowhere that the user chose to replace by an installed one: "trial",
/// repairable. The original status is kept in "was" so stopping the trial restores it.
/// After the ignore list or the trials changed: the tried plugins' statuses and the
/// severities only, nothing re-read or re-classified. Then the suggestions follow (checks).
pub fn recompute_severity(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    let ignored: HashSet<String> =
        setting::<Vec<String>>(tx, "ignored").unwrap_or_default().iter().map(|s| s.to_lowercase()).collect();
    let trials: HashMap<String, Value> = setting(tx, "trials").unwrap_or_default();
    let vst3: HashMap<String, Value> = setting(tx, "to_vst3").unwrap_or_default();
    let rows: Vec<(String, String, String)> = {
        let mut s = tx.prepare("select rel, data, plugstate from files")?;
        s.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    let out: Vec<(String, i64, String)> = rows
        .par_iter()
        .map(|(rel, data, states)| {
            let data: Value = serde_json::from_str(data).unwrap_or(Value::Null);
            let mut states: Vec<Value> = serde_json::from_str(states).unwrap_or_default();
            let mut names = Vec::new();
            for (p, d) in data["plugins"].as_array().cloned().unwrap_or_default().into_iter().zip(states.iter_mut()) {
                let r: Ref = serde_json::from_value(p).unwrap_or_default();
                apply_trials(&r, d, &trials, &vst3);
                names.push(r.name);
            }
            let sev = file_severity(names.iter().zip(&states).map(|(n, d)| (n.as_str(), d["status"].as_str().unwrap_or(""))), &ignored, &data);
            (Value::Array(states).to_string(), sev, rel.clone())
        })
        .collect();
    let mut up = tx.prepare("update files set plugstate=?, sev=? where rel=?")?;
    for (states, sev, rel) in &out {
        up.execute(params![states, sev, rel])?;
    }
    drop(up);
    let roots = store::Roots::load(tx);
    suggestions::refresh_all(tx, &roots)
}

/// A VST2 the user chose to replace by its installed VST3 (the to_vst3 choices): "vst3",
/// repairable too, the VST3's name in "vst3".
fn apply_trials(r: &Ref, d: &mut Value, trials: &HashMap<String, Value>, vst3: &HashMap<String, Value>) {
    if d["status"] == "trial" || d["status"] == "vst3" {
        let was = d.get("was").cloned().unwrap_or(json!("missing"));
        d["status"] = was;
        d.as_object_mut().map(|o| (o.remove("was"), o.remove("trial"), o.remove("vst3")));
    }
    let ok = r.format == "vst2" || (r.format == "au" && r.au_state.as_deref().is_some_and(|s| !s.is_empty()));
    let t = if ok { trials.get(&trial_key(&r.format, &r.name)) } else { None };
    let status = d["status"].as_str().unwrap_or("").to_string();
    if let Some(t) = t.filter(|t| !t.is_null())
        && !matches!(status.as_str(), "ok" | "renamed" | "other_vst2")
    {
        d["was"] = json!(status);
        d["status"] = json!("trial");
        d["trial"] = t["to"].clone();
        return;
    }
    // the statuses a repair switches to the chosen VST3 (repair::fix::plan)
    // an automated plugin keeps waiting for its VST2: the switch would lose the automation
    if r.format == "vst2"
        && !r.automated
        && matches!(status.as_str(), "to_vst3" | "missing" | "other_vst3")
        && let Some(t) = vst3.get(&r.name.to_lowercase()).filter(|t| !t.is_null())
    {
        d["was"] = json!(status);
        d["status"] = json!("vst3");
        d["vst3"] = t["name"].clone();
    }
}

fn classify(tx: &rusqlite::Transaction, m: &Matcher, only: Option<&HashSet<&String>>) -> rusqlite::Result<usize> {
    let ignored: HashSet<String> =
        setting::<Vec<String>>(tx, "ignored").unwrap_or_default().iter().map(|s| s.to_lowercase()).collect();
    let trials: HashMap<String, Value> = setting(tx, "trials").unwrap_or_default();
    let vst3: HashMap<String, Value> = setting(tx, "to_vst3").unwrap_or_default();
    let rows: Vec<(String, String)> = {
        let mut s = tx.prepare("select rel, data from files")?;
        s.query_map([], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?
            .collect::<Result<_, _>>()?
    };
    let todo: Vec<&(String, String)> = rows.iter().filter(|(rel, _)| only.is_none_or(|o| o.contains(rel))).collect();
    let out: Vec<(String, i64, String)> = todo
        .par_iter()
        .map(|(rel, data)| {
            let data: Value = serde_json::from_str(data).unwrap_or(Value::Null);
            let mut states = Vec::new();
            let mut names = Vec::new();
            for p in data["plugins"].as_array().cloned().unwrap_or_default() {
                let r: Ref = serde_json::from_value(p).unwrap_or_default();
                let mut d = serde_json::to_value(m.describe(&r)).unwrap();
                d["kind"] = json!(m.kind(&r));
                apply_trials(&r, &mut d, &trials, &vst3);
                states.push(d);
                names.push(r.name);
            }
            let sev = file_severity(names.iter().zip(&states).map(|(n, d)| (n.as_str(), d["status"].as_str().unwrap_or(""))), &ignored, &data);
            (Value::Array(states).to_string(), sev, rel.clone())
        })
        .collect();
    let mut up = tx.prepare("update files set plugstate=?, sev=? where rel=?")?;
    for (states, sev, rel) in &out {
        up.execute(params![states, sev, rel])?;
    }
    Ok(out.len())
}

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn a_file_counts_its_worst_plugin_and_its_samples() {
        let none = json!({});
        let ign: HashSet<String> = ["oldsynth".to_string()].into();
        let p = |v: &[(&'static str, &'static str)]| v.to_vec().into_iter();
        assert_eq!(file_severity(p(&[("A", "ok"), ("B", "renamed")]), &HashSet::new(), &none), 1);
        assert_eq!(file_severity(p(&[("A", "newer"), ("B", "renamed")]), &HashSet::new(), &none), 2);
        // an ignored plugin counts for nothing
        assert_eq!(file_severity(p(&[("OldSynth", "missing")]), &ign, &none), 0);
        // a sample found elsewhere: repairable; one lost, or an unreadable file: blocked
        assert_eq!(file_severity(p(&[]), &ign, &json!({"samples_missing": [{"path": "a.wav", "found_at": "b.wav"}]})), 1);
        assert_eq!(file_severity(p(&[]), &ign, &json!({"samples_missing": [{"path": "a.wav", "found_at": null}]})), 3);
        assert_eq!(file_severity(p(&[("A", "ok")]), &ign, &json!({"error": "illisible"})), 3);
    }
}

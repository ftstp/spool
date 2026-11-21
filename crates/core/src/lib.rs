//! The application, independent of any window or server: its base, the scan running in the
//! background, and the queries and commands the interface uses (the API). The desktop shell
//! (Tauri) and a future web server both talk to this, and to nothing below it.
pub use midi;
pub mod actions;
mod artists;
pub use artists::{ArtistCard, ArtistPage};
mod references;
pub use references::RefItem;
pub mod als_diff;
pub mod atelier;
pub mod audio;
pub mod board_edit;
pub mod chains;
pub mod create;
pub mod detail;
pub mod detect;
pub mod samples_view;
mod sample_learn;
pub mod fsops;
pub mod insights;
pub mod keys;
pub mod media;
pub mod backups;
pub mod diag;
pub mod media_view;
pub mod reading;
pub mod resources;
pub mod midi_export;
pub mod midi_view;
pub mod notion;
pub mod repair_view;
pub mod releases;
pub mod repairs;
pub mod sound;
pub mod structure;
pub mod views;

use i18n::tr;
use index::Scanner;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use store::Store;
use ts_rs::TS;

pub use catalog::StyleOption;
pub use views::{Overview, SongItem, Stats};

/// What Spool keeps on this computer, in bytes, for Settings: nothing outside its own folder
/// is counted (the projects and the copies written beside them are the user's).
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Storage {
    /// Spool's data folder.
    pub dir: String,
    /// The catalogue (the base and its journal).
    #[ts(type = "number")]
    pub base: u64,
    /// Of which the journal (emptied at each start; large only after a long session).
    #[ts(type = "number")]
    pub journal: u64,
    /// Of which the index of the MIDI files found (rebuilt by a scan if it went missing).
    #[ts(type = "number")]
    pub midi: u64,
    /// Of which the index of the samples (the same: rebuilt by a scan).
    #[ts(type = "number")]
    pub samples: u64,
    /// The application itself (its installed folder) and the interface's cache (WebView2,
    /// beside the data folder): measured by the desktop shell, 0 when unknown.
    #[ts(type = "number")]
    pub app: u64,
    #[ts(type = "number")]
    pub webview: u64,
    /// The safety copies: before an update, before a start over.
    #[ts(type = "number")]
    pub backups: u64,
    pub backup_count: u32,
    /// What removing the copies but the newest would free.
    #[ts(type = "number")]
    pub trimmable: u64,
    /// Picture thumbnails, the templates, the audio previews, the sound model (opt-in).
    #[ts(type = "number")]
    pub cache: u64,
    #[ts(type = "number")]
    pub model: u64,
    #[ts(type = "number")]
    pub total: u64,
}

/// Bytes under `p` (a file or a folder, links not followed).
pub fn size_of(p: &Path) -> u64 {
    let Ok(md) = std::fs::symlink_metadata(p) else { return 0 };
    if md.is_dir() {
        std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| size_of(&e.path())).sum()
    } else {
        md.len()
    }
}

/// The safety copies in `backups/`, newest first: each one a base (`.sqlite`) with the files
/// that share its name (journal, choices), or a journal set aside (`journal-*`).
fn backup_sets(dir: &Path) -> Vec<(std::time::SystemTime, Vec<PathBuf>)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    let mut sets: Vec<(std::time::SystemTime, Vec<PathBuf>)> = vec![];
    for p in &names {
        let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let modified = |p: &Path| std::fs::symlink_metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        if let Some(stem) = n.strip_suffix(".sqlite").or_else(|| n.strip_suffix(".sqlite.gz")) {
            // "plugin-fixer-<stamp>" goes with "choices-<stamp>.json"
            let stamp = stem.strip_prefix("plugin-fixer-").map(|s| format!("choices-{s}.json"));
            let mut set = vec![p.clone()];
            for q in &names {
                let m = q.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if m != n && (m.starts_with(&format!("{n}-")) || stamp.as_deref() == Some(m.as_str())) {
                    set.push(q.clone());
                }
            }
            sets.push((modified(p), set));
        } else if n.starts_with("journal-") {
            sets.push((modified(p), vec![p.clone()]));
        }
    }
    sets.sort_by(|a, b| b.0.cmp(&a.0));
    sets
}

/// The copies `trim_backups` removes: all but the newest whole base (a journal set aside is
/// not a base by itself).
fn trimmable_sets(dir: &Path) -> Vec<Vec<PathBuf>> {
    let sets = backup_sets(dir);
    let keep = sets.iter().position(|(_, set)| set[0].extension().is_some_and(|e| e == "sqlite" || e == "gz"));
    sets.into_iter().enumerate().filter(|(i, _)| Some(*i) != keep).map(|(_, (_, set))| set).collect()
}

/// What the reference renders' analysis measured, per song (audio analysis, opt-in; only the
/// analyses of the current model and method).
pub(crate) fn detect_audio(con: &store::rusqlite::Connection) -> std::collections::HashMap<i64, detect::AudioMeasure> {
    let mut out = std::collections::HashMap::new();
    let Ok(mut s) = con.prepare(
        "select s.id, a.bpm, a.key from songs s join audio_analysis a on a.rel = s.temoin          where s.gone = 0 and a.model = ? and a.error is null",
    ) else {
        return out;
    };
    if let Ok(rows) = s.query_map([index::AUDIO_MODEL], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<f64>>(1)?, r.get::<_, Option<String>>(2)?))) {
        for (id, bpm, key) in rows.flatten() {
            out.insert(id, detect::AudioMeasure { bpm, key });
        }
    }
    out
}

/// The pages' views as last computed, with the version of the catalogue they show (the count
/// the interface follows to reload: it moves with a scan that changed something, an edit, the
/// end of an audio round). Not SQLite's own write count: the samples' analysis writes every
/// second for hours without changing these views. Until it moves, a page asks again for free.
#[derive(Default)]
struct ViewCache {
    at: Option<i64>,
    songs: Option<Vec<SongItem>>,
    overview: Option<Overview>,
    stats: Option<Stats>,
    home: Option<insights::HomeStats>,
}

/// What became of the files dropped on a song.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct BoardDrop {
    /// In the projects folders: shown where they are.
    pub linked: Vec<String>,
    /// From elsewhere: copied onto the board.
    pub copied: Vec<String>,
    pub refused: Vec<String>,
}

/// After a change: what the undo button can offer.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct UndoState {
    /// The newest change that can be undone.
    pub label: Option<String>,
    #[ts(type = "number")]
    pub left: i64,
    /// Undoing it moves files on disk: the interface asks first.
    pub files: bool,
    /// What Ctrl+Y would put back (the newest change undone), if anything.
    pub redo: Option<String>,
}

pub type Error = String;
pub type Result<T> = std::result::Result<T, Error>;

fn err<E: std::fmt::Display>(e: E) -> Error {
    e.to_string()
}

/// What the background scan is doing, for the status line.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct ScanState {
    pub running: bool,
    /// The pass running is a long one: the first reading of a library, a full analysis asked
    /// for, the old drive searched (not the quick check of what changed).
    pub full: bool,
    /// The automatic analysis is off and projects changed since the last one: the sidebar's
    /// "Analyse" button says so.
    pub pending: bool,
    pub stage: String,
    /// How far into the stage (total 0: not known, `done` counts what was found so far).
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    /// What is being read right now.
    pub detail: String,
    pub last: Option<f64>,
    pub error: Option<String>,
    /// The last pass was stopped by the user (what it read is kept).
    pub cancelled: bool,
    /// Bumped after every change: the interface reloads what it shows.
    #[ts(type = "number")]
    pub version: u64,
    /// The audio analysis of reference renders (opt-in), its download and its progress.
    pub audio: sound::AudioState,
}

/// The settings the setup and Settings pages edit.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct FolderPeek {
    #[ts(type = "number")]
    pub files: u64,
    #[ts(type = "number")]
    pub midi: u64,
    #[ts(type = "number")]
    pub midi_bytes: u64,
    #[ts(type = "number")]
    pub audio: u64,
    /// The whole folder was looked at (false: it is at least this big).
    pub complete: bool,
    /// About how long reading its MIDI files takes, in seconds.
    #[ts(type = "number")]
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Config {
    #[ts(type = "{ label: string; path: string; artist: string }[]")]
    pub roots: Vec<store::Root>,
    pub sample_dirs: Vec<String>,
    /// Folders of MIDI files (construction kits, packs): only their MIDI files are read.
    pub midi_dirs: Vec<String>,
    pub old_drive: String,
    pub artists: Vec<String>,
    /// Artists merged: old name -> the name it was merged into.
    #[ts(type = "Record<string, string>")]
    pub artist_aliases: std::collections::BTreeMap<String, String>,
    /// Names read as featured artists, not as the user's own: name -> the artist the songs go
    /// to (empty: the projects folder's own artist).
    #[ts(type = "Record<string, string>")]
    pub artist_feats: std::collections::BTreeMap<String, String>,
    /// Names found in folders that are not artists (said so on the Artists page): never
    /// offered to sort again.
    pub artist_ignored: Vec<String>,
    pub work_words: Vec<String>,
    /// The first-run setup was finished: the rest of the app opens.
    pub setup_done: bool,
    /// Live's plugin list was found (Live opened at least once on this computer).
    pub live_db: bool,
    /// Files may be moved and folders made on disk (off: everything is arranged in the app
    /// only, nothing on disk moves).
    pub physical: bool,
    /// The interface's language ("fr", "en"; empty: not chosen yet). Messages from the core
    /// follow it (crates/i18n).
    pub lang: String,
    /// Projects read again and sounds analysed on their own (Settings; on by default).
    pub auto_analysis: bool,
    /// The finishing checks the user wrote, kept as presets: `{ presets: { name: [labels] },
    /// default: name | null }` (the default one shows on every song).
    #[ts(type = "{ presets?: Record<string, string[]>; default?: string | null }")]
    pub check_presets: serde_json::Value,
}

/// Something changed in the base: the interface reloads.
pub type OnChange = Arc<dyn Fn(&ScanState) + Send + Sync>;

pub struct App {
    /// Written by the scan and the commands.
    store: Mutex<Store>,
    /// Read by the views: SQLite (WAL) lets them read while a scan writes, so the interface
    /// never waits for a scan. Several connections: one slow view (the statistics take
    /// seconds on a large catalogue) used to hold the only one, and every page, sheet and
    /// audio request of the player waited behind it.
    reader: Readers,
    /// The folders, as the player's file requests check them: read again after 2 s at most
    /// (a playing file asks range after range; each read the settings from the base).
    media_roots: Mutex<Option<(std::time::Instant, store::Roots)>>,
    scanner: Mutex<Scanner>,
    state: Mutex<ScanState>,
    on_change: Mutex<Option<OnChange>>,
    repair: Mutex<repairs::RepairState>,
    /// The Repair view of the whole catalogue (the costly part of the statistics), kept
    /// until the next change: (version, view).
    rv_cache: Mutex<Option<(u64, Arc<repair_view::RepairView>)>>,
    midi_cache: Mutex<Option<(u64, Arc<midi_view::MidiSummary>)>>,
    /// The pages' heavier views, kept until the base changes (see `cached`).
    views: Mutex<ViewCache>,
    /// The sample folders changed: the next scan reads them again.
    samples_stale: std::sync::atomic::AtomicBool,
    /// Goes up whenever the app moves files: a scan that read the disk before is not written.
    fs_gen: std::sync::atomic::AtomicU64,
    /// The scanner's stop switch, reachable while a pass holds the scanner.
    cancel: Arc<std::sync::atomic::AtomicBool>,
    /// A stop by the user also quiets the background scans for a while (they would start
    /// again on what was just stopped).
    stopped_at: Mutex<Option<std::time::Instant>>,
    /// The audio analysis (sound.rs).
    sound: sound::Sound,
    pub data_dir: PathBuf,
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// A fresh base (deleted or rebuilt) gets the user's choices back from the mirror: settings
/// at once, songs once a scan has found them again (index's `apply_pending`).
fn restore_choices(st: &Store, file: &Path) {
    let empty = st
        .con
        .query_row("select count(*) from settings where k != 'archive_by_user'", [], |r| r.get::<_, i64>(0))
        .map(|n| n == 0)
        .unwrap_or(false);
    if !empty {
        return;
    }
    let Some(ch) = std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    else {
        return;
    };
    if let Some(m) = ch["settings"].as_object() {
        for (k, v) in m.iter().filter(|(_, v)| !v.is_null()) {
            let _ = st.set(k, v);
        }
    }
    let _ = st.set("pending_choices", &serde_json::json!({"songs": ch["songs"], "suggestions": ch["suggestions"]}));
}

/// Opens the base. A journal (-wal) left by a session that did not close cleanly is checked
/// first, READ-ONLY: one that does not match the base (seen once: a 22 MB journal older than
/// the base) makes every read fail, and the first normal open-and-close folds it into the
/// base for good. Such a journal is set aside in `backups/` and the base opened alone: by
/// itself it is always a whole committed state (at worst without the last commits).
fn open_or_drop_stale_wal(data_dir: &Path, path: &Path) -> Result<Store> {
    let wal = data_dir.join("plugin-fixer.sqlite-wal");
    if path.exists() && wal.metadata().is_ok_and(|m| m.len() > 0) && !journal_fits(path) {
        let dir = data_dir.join("backups").join(format!("journal-{}", chrono::Local::now().format("%Y-%m-%d_%Hh%Mm%S")));
        std::fs::create_dir_all(&dir).map_err(err)?;
        for ext in ["-wal", "-shm"] {
            let f = data_dir.join(format!("plugin-fixer.sqlite{ext}"));
            if f.exists() {
                std::fs::rename(&f, dir.join(format!("plugin-fixer.sqlite{ext}"))).map_err(err)?;
            }
        }
    }
    Store::open(path).map_err(err)
}

/// The base read through its journal, read-only (nothing written, nothing folded): sound?
fn journal_fits(path: &Path) -> bool {
    use store::rusqlite::{Connection, OpenFlags};
    let Ok(con) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return true };
    match con.query_row("pragma quick_check", [], |r| r.get::<_, String>(0)) {
        Ok(v) => v == "ok",
        Err(store::rusqlite::Error::SqliteFailure(f, _)) => {
            !matches!(f.code, store::rusqlite::ErrorCode::DatabaseCorrupt | store::rusqlite::ErrorCode::NotADatabase)
        }
        Err(_) => true,
    }
}

/// The views' connections to the base. `lock` takes a free one, else waits for one.
struct Readers {
    all: Vec<Mutex<Store>>,
    next: std::sync::atomic::AtomicUsize,
}

impl Readers {
    const N: usize = 4;
    fn open(path: &Path) -> Result<Readers> {
        let all = (0..Self::N).map(|_| Store::open(path).map(Mutex::new).map_err(err)).collect::<Result<Vec<_>>>()?;
        Ok(Readers { all, next: Default::default() })
    }
    /// A connection free now, else the next one in turn (waited for).
    fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Store>> {
        for m in &self.all {
            match m.try_lock() {
                Ok(g) => return Ok(g),
                Err(std::sync::TryLockError::Poisoned(p)) => return Err(p),
                Err(std::sync::TryLockError::WouldBlock) => {}
            }
        }
        let i = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % self.all.len();
        self.all[i].lock()
    }
    /// Every connection (the base replaced: they are all closed, then opened again).
    fn lock_all(&self) -> Vec<std::sync::MutexGuard<'_, Store>> {
        self.all.iter().map(|m| m.lock().unwrap()).collect()
    }
}

impl App {
    /// Opens (or creates) the base in `data_dir`.
    pub fn open(data_dir: &Path) -> Result<Arc<App>> {
        std::fs::create_dir_all(data_dir).map_err(err)?;
        diag::init(data_dir);
        let path = data_dir.join("plugin-fixer.sqlite");
        let store = open_or_drop_stale_wal(data_dir, &path)?;
        App::audio_cleanup_on_start(data_dir);
        restore_choices(&store, &data_dir.join("choices.json"));
        let reader = Readers::open(&path)?;
        // the safety copies: the newest kept, compressed, in the background
        // (only what is there when Spool opens: a copy made later is tidied by what made it)
        let dir = data_dir.join("backups");
        if dir.is_dir() {
            std::thread::spawn(move || backups::tidy(&dir));
        }
        // messages in the language the user chose (French until the interface says)
        i18n::set_lang(&reader.lock().unwrap().get::<String>("lang").unwrap_or_else(|| "fr".into()));
        let mut scanner = Scanner::new();
        let deep = data_dir.join("old_drive_samples.json");
        scanner.deep_file = Some(deep);
        let cancel = Arc::clone(&scanner.cancel);
        Ok(Arc::new(App {
            store: Mutex::new(store),
            reader,
            media_roots: Mutex::new(None),
            scanner: Mutex::new(scanner),
            state: Mutex::new(ScanState::default()),
            on_change: Mutex::new(None),
            repair: Mutex::new(repairs::RepairState::default()),
            rv_cache: Mutex::new(None),
            midi_cache: Mutex::new(None),
            views: Mutex::new(ViewCache::default()),
            samples_stale: std::sync::atomic::AtomicBool::new(false),
            fs_gen: std::sync::atomic::AtomicU64::new(0),
            cancel,
            stopped_at: Mutex::new(None),
            sound: sound::Sound::default(),
            data_dir: data_dir.to_path_buf(),
        }))
    }

    pub fn on_change(&self, f: OnChange) {
        *self.on_change.lock().unwrap() = Some(f);
    }

    fn notify(&self) {
        let st = self.state.lock().unwrap().clone();
        if let Some(f) = self.on_change.lock().unwrap().clone() {
            f(&st);
        }
    }

    pub fn scan_state(&self) -> ScanState {
        self.state.lock().unwrap().clone()
    }

    /// One scan pass now (the background loop calls this too).
    pub fn scan(&self, full: bool) -> Result<bool> {
        {
            let mut s = self.state.lock().unwrap();
            if s.running {
                return Ok(false);
            }
            s.running = true;
            s.full = full || s.last.is_none();
            s.cancelled = false;
            s.stage = "Mise à jour".into();
        }
        self.cancel.store(false, std::sync::atomic::Ordering::SeqCst);
        self.notify();
        let last_emit = Mutex::new(std::time::Instant::now());
        let projects_shown = std::sync::atomic::AtomicBool::new(false);
        // the journal: each stage, and where it is every half minute (a pass that hangs or
        // starts again says where)
        let began = std::time::Instant::now();
        diag::log(format!("scan: start ({})", if full { "full" } else { "quick check" }));
        let journal = Mutex::new((std::time::Instant::now(), std::time::Instant::now(), String::new()));
        let tick = |p: index::Progress| {
            {
                let mut j = journal.lock().unwrap();
                if j.2 != p.stage {
                    if !j.2.is_empty() {
                        diag::log(format!("scan: \"{}\" done in {} s", j.2, j.0.elapsed().as_secs()));
                    }
                    diag::log(format!("scan: \"{}\" ({}/{})", p.stage, p.done, p.total));
                    *j = (std::time::Instant::now(), std::time::Instant::now(), p.stage.clone());
                } else if j.1.elapsed() >= Duration::from_secs(30) {
                    diag::log(format!("scan: \"{}\" {}/{} at {}", p.stage, p.done, p.total, p.detail));
                    j.1 = std::time::Instant::now();
                }
            }
            {
                let mut s = self.state.lock().unwrap();
                let new_stage = s.stage != p.stage;
                // the projects are saved before the MIDI files are looked at: the pages can
                // show them now, while the MIDI files are read in the background
                if new_stage && p.stage.contains("fichiers MIDI") && !projects_shown.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    s.version += 1;
                }
                s.stage = p.stage;
                s.done = p.done as u64;
                s.total = p.total as u64;
                s.detail = p.detail;
                let mut l = last_emit.lock().unwrap();
                // a new stage at once, progress inside a stage ~6 times a second
                if !new_stage && l.elapsed() < Duration::from_millis(150) {
                    return;
                }
                *l = std::time::Instant::now();
            }
            self.notify();
        };
        let res = {
            // the base is not held for the pass, only by the scanner while it reads or writes
            // it: edits, moves and pages go on during a long scan
            let mut scanner = self.scanner.lock().unwrap();
            if self.samples_stale.swap(false, std::sync::atomic::Ordering::SeqCst) {
                scanner.forget_samples();
            }
            // the whole pass on the scan's own pool, in the background: the window and the
            // commands keep the processor and the disk first (index::background)
            let sc: &mut index::Scanner = &mut scanner;
            let r = index::background::run(|| {
                let mut r = sc.sync_with(&self.store, full, &tick, &self.fs_gen);
                // the app moved files meanwhile: what was read is out of date, read again
                for _ in 0..3 {
                    if !r.as_ref().is_ok_and(|r| r.stale) || self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    r = sc.sync_with(&self.store, full, &tick, &self.fs_gen);
                }
                r
            });
            drop(scanner);
            // one history point per change, for the Progression page
            if r.as_ref().is_ok_and(|r| r.changed) {
                let _ = insights::snapshot(&self.store.lock().unwrap().con);
            }
            r
        };
        let changed = {
            let mut s = self.state.lock().unwrap();
            s.running = false;
            s.stage.clear();
            s.detail.clear();
            s.done = 0;
            s.total = 0;
            s.last = Some(now());
            match &res {
                Ok(rep) => diag::log(format!("scan: end in {} s{}{}", began.elapsed().as_secs(), if rep.cancelled { ", stopped" } else { "" }, if rep.changed { ", changes" } else { "" })),
                Err(e) => diag::log(format!("scan: failed after {} s: {e}", began.elapsed().as_secs())),
            }
            match &res {
                Ok(rep) => {
                    s.error = None;
                    s.cancelled = rep.cancelled;
                    if rep.changed {
                        s.version += 1;
                    }
                    rep.changed
                }
                Err(e) => {
                    s.error = Some(e.to_string());
                    false
                }
            }
        };
        self.notify();
        res.map(|_| changed).map_err(err)
    }

    /// Scans now, then whenever a project or a render changes under the projects root (a few
    /// seconds after the last change, so a save in progress is read once, whole), and at the
    /// latest every `every`.
    pub fn start_background(self: &Arc<Self>, every: Duration) {
        self.start_audio();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let dirty = Arc::new(Mutex::new(None::<std::time::Instant>));
            let mut watching: Option<(Vec<String>, Vec<notify::RecommendedWatcher>)> = None;
            let mut last = std::time::Instant::now().checked_sub(every).unwrap_or_else(std::time::Instant::now);
            loop {
                // the setup runs its own first analysis, with its progress on screen; a scan
                // started behind it would hold the base while the user is still choosing
                let quieted = app.stopped_at.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_secs(30 * 60));
                if !app.setup_done() || quieted {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                // (re)watch the projects folders when they are set or changed
                let paths: Vec<String> = app.roots().list.into_iter().map(|r| r.path).collect();
                if Some(&paths) != watching.as_ref().map(|w| &w.0) {
                    let ws = paths.iter().filter_map(|r| watch(r, Arc::clone(&dirty))).collect();
                    watching = Some((paths, ws));
                }
                let quiet = dirty.lock().unwrap().is_some_and(|t| t.elapsed() >= Duration::from_secs(3));
                // the automatic analysis turned off (Settings): a change is only noted, the
                // "Analyse" button of the sidebar runs it
                if !app.auto_analysis() {
                    if quiet {
                        *dirty.lock().unwrap() = None;
                        app.state.lock().unwrap().pending = true;
                        app.notify();
                    }
                    last = std::time::Instant::now();
                } else if quiet || last.elapsed() >= every {
                    *dirty.lock().unwrap() = None;
                    let _ = app.scan(false);
                    last = std::time::Instant::now();
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    /// On unless the user turned it off: projects read again on their own when they change,
    /// and the sounds analysed in the background.
    pub fn auto_analysis(&self) -> bool {
        self.reader.lock().unwrap().get::<bool>("auto_analysis").unwrap_or(true)
    }

    /// The sidebar's "Analyse" (the automatic analysis off): the projects read now, and the
    /// sounds analysed until done (or the app closes).
    pub fn analyse_now(self: &Arc<Self>) {
        sound::allow_manual();
        self.state.lock().unwrap().pending = false;
        self.notify();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = app.scan(false);
        });
    }

    /// Copies the Python prototype's base into this (empty) one, choices included.
    pub fn import_prototype(&self, proto: &Path) -> Result<Vec<(String, usize)>> {
        let out = self.store.lock().unwrap().import_prototype(proto).map_err(err)?;
        // the prototype's search of the old drive comes along
        let deep = proto.parent().map(|p| p.join("data").join("old_drive_samples.json"));
        if let Some(d) = deep.filter(|d| d.is_file()) {
            let _ = std::fs::copy(d, self.data_dir.join("old_drive_samples.json"));
        }
        // and the VST2 templates its Mac plugin conversions are built on
        if let Some(Ok(rd)) = proto.parent().map(|p| std::fs::read_dir(p.join("data").join("templates"))) {
            let to = self.data_dir.join("templates");
            let _ = std::fs::create_dir_all(&to);
            for e in rd.flatten() {
                let _ = std::fs::copy(e.path(), to.join(e.file_name()));
            }
        }
        self.state.lock().unwrap().version += 1;
        self.notify();
        Ok(out)
    }

    /// Starts over with an empty base. Nothing is deleted: the base and the choices mirror
    /// move to `backups/` under one date stamp (the last 2 resets are kept), so the mirror
    /// cannot refill the new base. The projects, the copies written and the templates stay.
    /// Returns the backup's file name.
    pub fn reset(&self) -> Result<String> {
        if self.state.lock().unwrap().running || self.repair.lock().unwrap().running {
            return Err(tr!("err.busy"));
        }
        // the writer is held the whole time, so no scan or command can slip in
        let mut store = self.store.lock().unwrap();
        self.fs_gen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut readers = self.reader.lock_all();
        let base = self.data_dir.join("plugin-fixer.sqlite");
        let dir = self.data_dir.join("backups");
        std::fs::create_dir_all(&dir).map_err(err)?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%Hh%Mm%S").to_string();
        let name = format!("plugin-fixer-{stamp}.sqlite");
        // Windows cannot move an open file: both connections close first (the last one to
        // close folds the WAL into the base)
        for reader in readers.iter_mut() {
            drop(std::mem::replace(&mut **reader, Store::memory().map_err(err)?));
        }
        drop(std::mem::replace(&mut *store, Store::memory().map_err(err)?));
        let moved = std::fs::rename(&base, dir.join(&name));
        if let Err(e) = moved {
            // put the base back in service as it was
            *store = Store::open(&base).map_err(err)?;
            for reader in readers.iter_mut() {
                **reader = Store::open(&base).map_err(err)?;
            }
            return Err(tr!("settings.resetFailed", e = e));
        }
        for ext in ["-wal", "-shm"] {
            let f = self.data_dir.join(format!("plugin-fixer.sqlite{ext}"));
            if f.exists() {
                let _ = std::fs::rename(&f, dir.join(format!("{name}{ext}")));
            }
        }
        let ch = self.data_dir.join("choices.json");
        if ch.exists() {
            let _ = std::fs::rename(&ch, dir.join(format!("choices-{stamp}.json")));
        }
        prune_backups(&dir, 2);
        {
            let dir = dir.clone();
            std::thread::spawn(move || backups::tidy(&dir));
        }
        *store = Store::open(&base).map_err(err)?;
        for reader in readers.iter_mut() {
            **reader = Store::open(&base).map_err(err)?;
        }
        drop((store, readers));
        let mut scanner = Scanner::new();
        scanner.deep_file = Some(self.data_dir.join("old_drive_samples.json"));
        *self.scanner.lock().unwrap() = scanner;
        *self.repair.lock().unwrap() = repairs::RepairState::default();
        {
            let mut s = self.state.lock().unwrap();
            let v = s.version;
            *s = ScanState::default();
            s.version = v + 1;
        }
        self.notify();
        Ok(name)
    }

    /// One change on songs, in one transaction: photographed first (for the undo), mirrored
    /// to choices.json after.
    fn change<F>(
        &self,
        sids: &[i64],
        label: F,
        apply: impl FnOnce(&store::rusqlite::Transaction) -> std::result::Result<(), actions::Refused>,
    ) -> Result<UndoState>
    where
        F: FnOnce(&store::rusqlite::Transaction) -> std::result::Result<String, actions::Refused>,
    {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let label = label(&tx).map_err(err)?;
        let snap = actions::snapshot(&tx, sids).map_err(err)?;
        apply(&tx).map_err(err)?;
        actions::push(&tx, &label, snap).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    fn after_change(&self, st: &Store) -> Result<UndoState> {
        // the mirror is a safety copy: failing to write it never undoes the change
        let _ = actions::save_choices(&st.con, &self.data_dir.join("choices.json"));
        let (label, left) = actions::last(&st.con).map_err(err)?;
        let files = actions::last_moves_files(&st.con);
        self.state.lock().unwrap().version += 1;
        self.notify();
        Ok(UndoState { label, left, files, redo: actions::redo_stack(&st.con, |r| r.last().map(|x| x.0.clone())) })
    }

    /// Fields (title, artist, style, key, notes, tempo, date), stage, archived, reference render.
    pub fn song_edit(&self, sid: i64, body: serde_json::Map<String, serde_json::Value>) -> Result<UndoState> {
        self.change(&[sid], |tx| Ok(actions::edit_label(tx, sid, &body)?), |tx| actions::song_edit(tx, sid, &body))
    }

    /// A finishing check set by hand (true / false / "na"), or given back to the tool (null).
    pub fn check_set(&self, sid: i64, key: &str, value: serde_json::Value) -> Result<UndoState> {
        self.change(
            &[sid],
            |tx| Ok(tr!("undo.check", title = actions::title_of(tx, sid)?)),
            |tx| actions::check_set(tx, sid, key, &value),
        )
    }

    /// Accept (optionally another choice than the one proposed) or refuse a suggestion.
    pub fn suggestion_act(&self, gid: i64, accept: bool, choice: Option<String>) -> Result<UndoState> {
        let sid = {
            let st = self.reader.lock().unwrap();
            st.con
                .query_row("select song_id from suggestions where id=?", [gid], |r| r.get::<_, i64>(0))
                .map_err(|_| tr!("err.suggestionNotFound"))?
        };
        self.change(
            &[sid],
            |tx| Ok(actions::suggestion_label(tx, gid, accept)?.1),
            |tx| actions::suggestion_act(tx, gid, accept, choice.as_deref()),
        )
    }

    /// Several suggestions accepted at once: one undo.
    pub fn accept_all(&self, ids: Vec<i64>) -> Result<UndoState> {
        if ids.is_empty() {
            return self.undo_state();
        }
        let sids: Vec<i64> = {
            let st = self.reader.lock().unwrap();
            let marks = vec!["?"; ids.len()].join(",");
            let mut s =
                st.con.prepare(&format!("select song_id from suggestions where id in ({marks})")).map_err(err)?;
            s.query_map(store::rusqlite::params_from_iter(&ids), |r| r.get(0))
                .map_err(err)?
                .collect::<std::result::Result<_, _>>()
                .map_err(err)?
        };
        let n = ids.len();
        self.change(
            &sids,
            |_| Ok(tr!("undo.acceptAll", n = n)),
            |tx| {
                for g in &ids {
                    actions::suggestion_act(tx, *g, true, None)?;
                }
                Ok(())
            },
        )
    }

    pub fn undo_state(&self) -> Result<UndoState> {
        let r = self.reader.lock().unwrap();
        let (label, left) = actions::last(&r.con).map_err(err)?;
        Ok(UndoState { label, left, files: actions::last_moves_files(&r.con), redo: actions::redo_stack(&r.con, |x| x.last().map(|y| y.0.clone())) })
    }

    /// Puts the last change back. Returns what was undone (None: nothing left).
    pub fn undo(&self) -> Result<(Option<String>, UndoState)> {
        // read before the writer is taken (the reader is never awaited under it)
        let physical = self.physical();
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        // what is there now, for Ctrl+Y (taken before the undo puts the past back)
        let counter = actions::newest(&tx).and_then(|(l, snap)| actions::counter_snapshot(&tx, &snap).ok().flatten().map(|c| (l, c)));
        let covers_before = chosen_covers(&tx);
        let Some(u) = actions::undo(&tx).map_err(err)? else {
            return Ok((None, UndoState::default()));
        };
        if let Some(ops) = &u.ops {
            // the switch holds for Ctrl+Z too: with it off, nothing moves on disk, ever
            if !physical {
                return Err(tr!("undo.physicalOff", label = u.label));
            }
            // a file operation: played backwards on disk (all or nothing), the entry dropped once that worked
            let ops: Vec<fsops::Op> = serde_json::from_value(ops.clone()).unwrap_or_default();
            fsops::undo_ops(&tx, &store::Roots::load(&tx), &ops)?;
            actions::drop_entry(&tx, u.id).map_err(err)?;
        }
        if let Some(n) = &u.note {
            // a note's text back; the text it had is kept aside first, so this undo loses neither
            let path = PathBuf::from(n["path"].as_str().unwrap_or(""));
            if path.is_file() {
                board_edit::keep_version(&path)?;
            }
            match n["old"].as_str() {
                Some(old) => board_edit::write_whole(&path, old)?,
                // a note the change created: set aside in the board's .corbeille, not deleted
                None if path.is_file() => {
                    let dir = path.parent().ok_or_else(|| tr!("board.noteNotFound"))?;
                    let name = path.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
                    board_edit::remove(dir, &name)?;
                }
                None => {}
            }
            actions::drop_entry(&tx, u.id).map_err(err)?;
        }
        if u.regroup {
            // the grouping rules came back: the files go back to their songs
            index::regroup(&tx).map_err(err)?;
        }
        for sid in &u.sids {
            // checks and suggestions follow the restored song
            index::refresh_song(&tx, *sid).map_err(err)?;
        }
        refresh_covers(&tx, &covers_before)?;
        tx.commit().map_err(err)?;
        actions::redo_stack(&st.con, |redo| match counter {
            Some(c) => redo.push(c),
            // a file move undone: not redone, and what was before it no longer follows
            None => redo.clear(),
        });
        let state = self.after_change(&st)?;
        Ok((Some(u.label), state))
    }

    /// Puts back the newest change Ctrl+Z took back (Ctrl+Y); Ctrl+Z undoes it again.
    pub fn redo(&self) -> Result<(Option<String>, UndoState)> {
        let st = self.store.lock().unwrap();
        let Some((label, snap)) = actions::redo_stack(&st.con, |r| r.pop()) else {
            drop(st);
            return Ok((None, self.undo_state()?));
        };
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let back = actions::counter_snapshot(&tx, &snap).map_err(err)?;
        let covers_before = chosen_covers(&tx);
        let mut sids = Vec::new();
        let mut regroup = false;
        if snap["kind"] == "note" {
            let path = PathBuf::from(snap["path"].as_str().unwrap_or(""));
            if path.is_file() {
                board_edit::keep_version(&path)?;
            }
            match snap["old"].as_str() {
                Some(text) => board_edit::write_whole(&path, text)?,
                None if path.is_file() => {
                    let dir = path.parent().ok_or_else(|| tr!("board.noteNotFound"))?;
                    let name = path.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
                    board_edit::remove(dir, &name)?;
                }
                None => {}
            }
        } else {
            (sids, regroup) = actions::restore(&tx, &snap, actions::now()).map_err(err)?;
        }
        if regroup {
            index::regroup(&tx).map_err(err)?;
        }
        for sid in &sids {
            index::refresh_song(&tx, *sid).map_err(err)?;
        }
        refresh_covers(&tx, &covers_before)?;
        actions::push_keep_redo(&tx, &label, back).map_err(err)?;
        tx.commit().map_err(err)?;
        let state = self.after_change(&st)?;
        Ok((Some(label), state))
    }

    /// Song `sid` (its whole variant, present and future files) becomes part of song `into`.
    pub fn merge_song(&self, sid: i64, into: i64) -> Result<String> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let row = |id: i64| -> Option<(String, String, String)> {
            tx.query_row("select song_dir, vkey, coalesce(title, '') from songs where id=? and gone=0", [id], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get(2)?,
                ))
            })
            .ok()
        };
        let (Some(src), Some(dst)) = (row(sid), row(into)) else { return Err(tr!("merge.impossible")) };
        if sid == into {
            return Err(tr!("merge.impossible"));
        }
        let mut merges: serde_json::Map<String, serde_json::Value> = setting_tx(&tx, "merges").unwrap_or_default();
        for v in merges.values_mut() {
            // what was merged into the source now goes to the target
            if v.as_i64() == Some(sid) {
                *v = into.into();
            }
        }
        merges.insert(format!("{}|{}", src.0, src.1), into.into());
        set_tx(&tx, "merges", &merges)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)?;
        Ok(tr!("merge.done", src = src.2, dst = dst.2))
    }

    /// Undoes a merge: the variant `key` ("folder|variant") is a song of its own again.
    pub fn split_song(&self, sid: i64, key: &str) -> Result<()> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let mut merges: serde_json::Map<String, serde_json::Value> = setting_tx(&tx, "merges").unwrap_or_default();
        if merges.get(key).and_then(|v| v.as_i64()) != Some(sid) {
            return Err(tr!("merge.notFound"));
        }
        merges.remove(key);
        set_tx(&tx, "merges", &merges)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st).map(|_| ())
    }

    /// The Repair page; `years`: only the projects saved in the last N years.
    // ------------------------------------------------------------ Notion import

    fn read_text(path: &str) -> Result<String> {
        let b = std::fs::read(path).map_err(|e| tr!("err.reading", e = e))?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    }

    /// A Notion CSV: its columns (guessed, or as the user set them), status values, matches.
    pub fn notion_preview(&self, path: &str, columns: Option<Vec<notion::Column>>) -> Result<notion::ImportPreview> {
        let text = Self::read_text(path)?;
        notion::preview(&self.reader.lock().unwrap().con, &text, columns)
    }

    /// Writes it: one undo entry for every song changed. Returns how many songs changed.
    pub fn notion_apply(
        &self,
        path: &str,
        columns: Vec<notion::Column>,
        stages: std::collections::HashMap<String, String>,
        only_empty: bool,
    ) -> Result<(usize, UndoState)> {
        let text = Self::read_text(path)?;
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let pv = notion::preview(&tx, &text, Some(columns.clone()))?;
        let sids: Vec<i64> = pv.rows.iter().filter_map(|m| m.song).collect();
        let snap = actions::snapshot(&tx, &sids).map_err(err)?;
        let changed = notion::apply(&tx, &text, &columns, &stages, only_empty)?;
        if changed.is_empty() {
            return Err(tr!("notion.nothing"));
        }
        actions::push(&tx, &tr!("undo.notion", n = changed.len()), snap).map_err(err)?;
        for sid in &changed {
            index::refresh_song(&tx, *sid).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        let state = self.after_change(&st)?;
        Ok((changed.len(), state))
    }

    // ------------------------------------------------------------ files and folders

    /// A file operation: under the writer, journaled for undo, then the base is saved, the
    /// interface told, and a scan picks up the rest. A scan running meanwhile sees `fs_gen`
    /// moved and reads the disk again before writing anything.
    fn file_op<T>(self: &Arc<Self>, f: impl FnOnce(&store::rusqlite::Transaction, &store::Roots) -> Result<(T, Option<(String, Vec<fsops::Op>)>)>) -> Result<T> {
        let out = {
            let st = self.store.lock().unwrap();
            // a scan that read the disk before this is not written (it reads again)
            self.fs_gen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let tx = st.con.unchecked_transaction().map_err(err)?;
            let roots = store::Roots::load(&tx);
            let (out, journal) = f(&tx, &roots)?;
            let ops: Vec<fsops::Op> = journal.as_ref().map(|j| j.1.clone()).unwrap_or_default();
            // songs follow their files (a project folder moved out of its song's folder)
            let saved = index::regroup(&tx)
                .map_err(err)
                .and_then(|_| match journal.filter(|(_, o)| !o.is_empty()) {
                    Some((label, ops)) => actions::push(&tx, &label, Some(fsops::journal(&ops))).map_err(err),
                    None => Ok(()),
                })
                .and_then(|_| tx.commit().map_err(err));
            if let Err(e) = saved {
                // the base did not take the change (rolled back): the disk goes back as well
                return Err(match fsops::undo_ops(&st.con, &roots, &ops) {
                    Ok(()) => tr!("fs.nothingMoved", e = e),
                    Err(b) => tr!("fs.andThen", e = e, b = b),
                });
            }
            let _ = actions::save_choices(&st.con, &self.data_dir.join("choices.json"));
            out
        };
        self.state.lock().unwrap().version += 1;
        self.notify();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = app.scan(false);
        });
        Ok(out)
    }

    fn env(&self) -> als::Env {
        let drive = self.reader.lock().unwrap().get::<String>("old_drive").filter(|d| !d.is_empty());
        als::Env::detect(drive)
    }

    /// Every folder of the projects folders, for pickers.
    pub fn fs_folders(&self) -> Vec<String> {
        fsops::all_folders(&self.roots(), 5000)
    }

    /// Loose media folders (no project, outside every song folder).
    pub fn fs_media(&self) -> Result<Vec<fsops::MediaFolder>> {
        // the base read first, the lock let go before walking the disk: a long walk must not
        // keep every other reader (the player's file requests among them) waiting
        let (roots, dirs) = {
            let st = self.reader.lock().unwrap();
            let mut q = st.con.prepare("select coalesce(song_dir, ''), count(*) from songs where gone=0 group by song_dir").map_err(err)?;
            let dirs: Vec<(String, i64)> = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.flatten().collect();
            (store::Roots::load(&st.con), dirs)
        };
        Ok(fsops::media_folders(&roots, &dirs))
    }

    /// A picture's thumbnail (JPEG), kept in the app's data folder.
    pub fn thumb(&self, rel_or_abs: &str, size: u32) -> Option<Vec<u8>> {
        let p = self.media_path(rel_or_abs)?;
        media::thumb(&p, &self.data_dir.join("thumbs"), size.clamp(64, 1024))
    }

    /// What reorganising by artist would move (nothing is touched).
    pub fn fs_reorg(&self, template: &str) -> Result<fsops::ReorgPlan> {
        let st = self.reader.lock().unwrap();
        let mut q = st.con.prepare("select song_dir, artist, style, title from songs where gone=0 and archived=0").map_err(err)?;
        let rows: Vec<fsops::SongRow> = q
            .query_map([], |r| Ok(fsops::SongRow { song_dir: r.get::<_, Option<String>>(0)?.unwrap_or_default(), artist: r.get(1)?, style: r.get(2)?, title: r.get(3)? }))
            .map_err(err)?
            .flatten()
            .collect();
        Ok(fsops::reorg_plan(&store::Roots::load(&st.con), &rows, template))
    }

    /// Where a move would land and what it would lose (nothing is touched).
    pub fn fs_plan(&self, src: &str, dest: &str) -> Result<fsops::MovePlan> {
        fsops::plan_move(&self.roots(), &self.env(), src, dest, true)
    }

    pub fn fs_mkdir(self: &Arc<Self>, rel: &str) -> Result<String> {
        self.physical_on()?;
        self.file_op(|_, roots| {
            let n = fsops::mkdir(roots, rel)?;
            let label = tr!("undo.mkdir", name = n);
            Ok((n.clone(), Some((label, vec![("mkdir".into(), String::new(), n)]))))
        })
    }

    /// Moves folders or files ([src, destination folder]); one undo for them all. A folder
    /// whose projects would lose a sample is skipped, with why.
    pub fn fs_move(self: &Arc<Self>, moves: Vec<(String, String)>) -> Result<fsops::MoveResult> {
        self.physical_on()?;
        let env = self.env();
        self.file_op(|tx, roots| {
            let r = fsops::move_many(tx, roots, &env, &moves);
            let label = match r.done.as_slice() {
                [one] => {
                    let leaf = one.rsplit('/').next().unwrap_or(one);
                    let into = one.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_else(|| tr!("undo.theRoot"));
                    tr!("undo.movedInto", leaf = leaf, into = into)
                }
                many => tr!("undo.movedMany", n = many.len()),
            };
            let ops = r.ops.clone();
            Ok((r, Some((label, ops))))
        })
    }

    /// Files or folders into a song's mood board (a sample a project uses is copied).
    pub fn fs_to_board(self: &Arc<Self>, sid: i64, items: Vec<String>) -> Result<(usize, usize)> {
        self.physical_on()?;
        let board = self.board_dir_of(sid)?;
        let title: String = self.reader.lock().unwrap().con
            .query_row("select coalesce(title, '') from songs where id=?", [sid], |r| r.get(0))
            .map_err(err)?;
        let out = self.file_op(|tx, roots| {
            let ops = fsops::to_board(tx, roots, &board, &items)?;
            let n = ops.iter().filter(|o| o.0 != "mkdir").count();
            let copied = ops.iter().filter(|o| o.0 == "copy").count();
            let label = tr!("undo.toBoard", n = n, title = title);
            Ok(((n, copied), Some((label, ops))))
        })?;
        let _ = self.board_done(sid);
        Ok(out)
    }

    /// The MIDI files of the projects folders: kinds, keys, progressions, the list.
    pub fn midi_summary(&self) -> Result<midi_view::MidiSummary> {
        let mut c = self.midi_cache.lock().unwrap();
        let v = self.state.lock().unwrap().version;
        if let Some((cv, m)) = c.as_ref()
            && *cv == v
        {
            return Ok((**m).clone());
        }
        // a big library takes seconds to sum up: on its own connection, so the shared reader
        // keeps answering the other pages meanwhile
        let path = self.reader.lock().unwrap().con.path().filter(|p| !p.is_empty()).map(String::from);
        let own = path
            .and_then(|p| store::rusqlite::Connection::open_with_flags(p, store::rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok())
            .filter(|c| store::add_functions(c).is_ok());
        let m = match own {
            Some(con) => midi_view::summary(&con),
            None => midi_view::summary(&self.reader.lock().unwrap().con),
        };
        let m = Arc::new(m.map_err(err)?);
        *c = Some((v, Arc::clone(&m)));
        Ok((*m).clone())
    }

    /// A view from the cache when the base has not changed since, else computed and kept.
    fn cached<T: Clone>(&self, slot: fn(&mut ViewCache) -> &mut Option<T>, make: impl FnOnce() -> Result<T>) -> Result<T> {
        let now: Option<i64> = Some(self.state.lock().unwrap().version as i64);
        {
            let mut c = self.views.lock().unwrap();
            if now.is_none() || c.at != now {
                *c = ViewCache { at: now, ..Default::default() };
            } else if let Some(v) = slot(&mut c).clone() {
                return Ok(v);
            }
        }
        let v = make()?;
        let mut c = self.views.lock().unwrap();
        // kept only if the base did not change while it was computed
        if c.at == now && now.is_some() {
            *slot(&mut c) = Some(v.clone());
        }
        Ok(v)
    }

    /// The home page's statistics block (heavier than the overview: every project's devices).
    pub fn home_stats(&self) -> Result<insights::HomeStats> {
        self.cached(|c| &mut c.home, || {
            let rv = self.whole_repair_view()?;
            let songs = self.songs()?;
            // the base held only for the read: the JSON is gone through without it
            let rows = insights::device_rows(&self.reader.lock().unwrap().con).map_err(err)?;
            Ok(insights::home_stats_from(rows, &songs, &rv))
        })
    }

    pub fn progress(&self) -> Result<insights::Progress> {
        let rv = self.whole_repair_view()?;
        insights::progress(&self.reader.lock().unwrap().con, &rv).map_err(err)
    }

    /// The Repair view of every period, computed once per change of the base.
    fn whole_repair_view(&self) -> Result<Arc<repair_view::RepairView>> {
        let mut c = self.rv_cache.lock().unwrap();
        let v = self.state.lock().unwrap().version;
        if let Some((cv, rv)) = c.as_ref()
            && *cv == v
        {
            return Ok(Arc::clone(rv));
        }
        let rv = Arc::new(
            repair_view::repair_view(&self.reader.lock().unwrap(), None, &self.data_dir.join("old_drive_samples.json"))
                .map_err(err)?,
        );
        *c = Some((v, Arc::clone(&rv)));
        Ok(rv)
    }

    pub fn repair_view(&self, years: Option<i64>) -> Result<repair_view::RepairView> {
        repair_view::repair_view(&self.reader.lock().unwrap(), years, &self.data_dir.join("old_drive_samples.json"))
            .map_err(err)
    }

    /// A plugin no longer used: left out of what to install, removed from repaired copies
    /// (or `keep`: left in them, to be replaced by hand). `ignored` false gives it back.
    pub fn set_ignored(&self, name: &str, ignored: bool, keep: bool) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("err.noName"));
        }
        let st = self.store.lock().unwrap();
        let low = name.to_lowercase();
        let sorted = |mut v: Vec<String>| {
            v.sort_by_key(|x| x.to_lowercase());
            v
        };
        let mut rest: Vec<String> = st
            .get::<Vec<String>>("ignored")
            .unwrap_or_default()
            .into_iter()
            .filter(|x| x.to_lowercase() != low)
            .collect();
        if ignored {
            rest.push(name.into());
        }
        let mut kept: Vec<String> = st
            .get::<Vec<String>>("ignored_keep")
            .unwrap_or_default()
            .into_iter()
            .filter(|x| x.to_lowercase() != low)
            .collect();
        if ignored && keep {
            kept.push(name.into());
        }
        let tx = st.con.unchecked_transaction().map_err(err)?;
        set_tx(&tx, "ignored", &sorted(rest))?;
        set_tx(&tx, "ignored_keep", &sorted(kept))?;
        index::recompute_severity(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st).map(|_| ())
    }

    /// A version found nowhere, replaced by an installed VST2 in repaired copies (`to`
    /// None: no longer). A Mac plugin needs a real device of that VST2 in some project.
    /// A VST2 replaced in repaired copies by an installed VST3 (`uid`: its class id, as Live's
    /// database writes it), or no longer (`None`).
    pub fn set_vst3(&self, name: &str, uid: Option<&str>) -> Result<()> {
        self.set_vst3_many(&[(name.to_string(), uid.map(String::from))])
    }

    /// Several VST3 choices at once ("replace them all"): Live's plugin list read once, the
    /// projects' statuses recomputed once (a chosen VST3 makes the project repairable).
    pub fn set_vst3_many(&self, list: &[(String, Option<String>)]) -> Result<()> {
        let inst = if list.iter().any(|(_, u)| u.as_deref().is_some_and(|u| !u.is_empty())) {
            let db = plugins::live_db();
            if db.exists() { plugins::load_installed(&db, true).unwrap_or_default() } else { Vec::new() }
        } else {
            Vec::new()
        };
        let st = self.store.lock().unwrap();
        let mut map: serde_json::Map<String, serde_json::Value> =
            st.get::<serde_json::Value>("to_vst3").and_then(|v| v.as_object().cloned()).unwrap_or_default();
        for (name, uid) in list {
            let name = name.trim();
            if name.is_empty() {
                return Err(tr!("err.noName"));
            }
            map.remove(&name.to_lowercase());
            if let Some(uid) = uid.as_deref().filter(|u| !u.is_empty()) {
                let x = inst
                    .iter()
                    .find(|x| x.format == "vst3" && x.id.eq_ignore_ascii_case(uid))
                    .ok_or_else(|| tr!("plugins.vst3NotInstalled", name = name))?;
                map.insert(name.to_lowercase(), serde_json::json!({"name": x.name, "uid": x.id, "kind": x.kind}));
            }
        }
        let tx = st.con.unchecked_transaction().map_err(err)?;
        set_tx(&tx, "to_vst3", &map)?;
        index::recompute_severity(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        *self.rv_cache.lock().unwrap() = None;
        self.after_change(&st).map(|_| ())
    }

    pub fn set_trial(&self, name: &str, format: &str, to: Option<&str>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(tr!("err.noName"));
        }
        let key = plugins::trial_key(format, name);
        let mut trials: serde_json::Map<String, serde_json::Value> = self
            .reader
            .lock()
            .unwrap()
            .get::<serde_json::Value>("trials")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        trials.remove(&key);
        if let Some(to) = to.filter(|t| !t.is_empty()) {
            let db = plugins::live_db();
            let inst = if db.exists() { plugins::load_installed(&db, true).unwrap_or_default() } else { Vec::new() };
            let x = inst
                .into_iter()
                .find(|x| x.format == "vst2" && x.name == to)
                .ok_or_else(|| tr!("plugins.notInstalled", name = to))?;
            if format == "au" {
                // the converted device is built on a real VST2 device from one of the projects
                let sets: Vec<PathBuf> = {
                    let st = self.reader.lock().unwrap();
                    let roots = store::Roots::load(&st.con);
                    let mut q = st.con.prepare("select rel, unz(data) from files").map_err(err)?;
                    let rows = q
                        .query_map([], |r| {
                            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()))
                        })
                        .map_err(err)?;
                    rows.flatten()
                        .filter(|(_, d)| {
                            serde_json::from_str::<serde_json::Value>(d).ok().is_some_and(|d| {
                                d["plugins"].as_array().is_some_and(|a| {
                                    a.iter().any(|p| p["format"] == "vst2" && p["id"].as_str() == Some(&x.id))
                                })
                            })
                        })
                        .filter_map(|(rel, _)| roots.abs(&rel))
                        .collect()
                };
                if !repair::fix::ensure_template(&self.data_dir.join("templates"), &x.id, &sets) {
                    return Err(tr!("plugins.noTemplate", name = x.name));
                }
            }
            let file = Path::new(&x.path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            trials.insert(key, serde_json::json!({"to": x.name, "file": file, "id": x.id}));
        }
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        set_tx(&tx, "trials", &trials)?;
        index::recompute_severity(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st).map(|_| ())
    }

    /// The treatments' presets (the default one when none was saved).
    pub fn atelier_presets(&self) -> Vec<serde_json::Value> {
        atelier::presets(&self.reader.lock().unwrap())
    }

    pub fn atelier_save_presets(&self, presets: Vec<serde_json::Value>) -> Result<()> {
        let pr: Vec<serde_json::Value> = presets
            .into_iter()
            .filter(|p| p.get("name").and_then(|n| n.as_str()).is_some_and(|n| !n.is_empty()))
            .collect();
        if pr.is_empty() {
            return Err(tr!("atelier.atLeastOne"));
        }
        let st = self.store.lock().unwrap();
        st.set("studio_presets", &pr).map_err(err)?;
        let _ = actions::save_choices(&st.con, &self.data_dir.join("choices.json"));
        Ok(())
    }

    /// The newest active project of a song.
    pub fn latest_project(&self, sid: i64) -> Option<String> {
        self.reader
            .lock()
            .unwrap()
            .con
            .query_row(
                "select rel from files where song_id=? and in_version=0 and archived=0 order by mtime desc limit 1",
                [sid],
                |r| r.get(0),
            )
            .ok()
    }

    /// What a preset would do to a song's newest project: nothing is written.
    pub fn atelier_preview(&self, sid: i64, preset: serde_json::Value) -> Result<serde_json::Value> {
        let rel = self.latest_project(sid).ok_or_else(|| tr!("err.noActiveProject"))?;
        Ok(atelier::run(&self.store, &rel, &preset, false, false, &self.data_dir.join("templates")))
    }

    /// Copies treated with a preset, for these songs (their newest project), in the
    /// background like the repairs.
    pub fn atelier_run(self: &Arc<Self>, songs: Vec<i64>, preset: serde_json::Value) -> Result<()> {
        let rels: Vec<String> = songs.iter().filter_map(|s| self.latest_project(*s)).collect();
        self.atelier_run_rels(rels, preset)
    }

    /// Repairs these projects AND applies a treatments preset, in one copy "Name [R...].als".
    pub fn repair_run_preset(self: &Arc<Self>, rels: Vec<String>, mut preset: serde_json::Value) -> Result<()> {
        preset["R"] = true.into();
        self.atelier_run_rels(rels, preset)
    }

    /// Searches the old drive's user folder for the samples still lost, then reads every
    /// project again so they are found. In the background; the scan state tells where it is.
    pub fn deep_search(self: &Arc<Self>) -> Result<()> {
        {
            let mut s = self.state.lock().unwrap();
            if s.running {
                return Err(tr!("err.scanBusy"));
            }
            s.running = true;
            s.full = true;
            s.stage = "Recherche sur l'ancien disque".into();
            s.done = 0;
            s.total = 0;
            s.detail.clear();
        }
        self.notify();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let file = app.data_dir.join("old_drive_samples.json");
            let res = {
                let st = app.reader.lock().unwrap();
                let last = Mutex::new(std::time::Instant::now());
                index::deep_search(&st, &file, &mut |n| {
                    app.state.lock().unwrap().done = n as u64;
                    let mut l = last.lock().unwrap();
                    if l.elapsed() >= Duration::from_millis(300) {
                        *l = std::time::Instant::now();
                        drop(l);
                        app.notify();
                    }
                })
            };
            {
                let mut s = app.state.lock().unwrap();
                s.running = false;
                s.stage.clear();
                s.done = 0;
                s.error = res.as_ref().err().cloned();
            }
            app.scanner.lock().unwrap().forget_deep();
            *app.rv_cache.lock().unwrap() = None;
            app.notify();
            if res.is_ok() {
                // every lost sample is looked for again
                let _ = app.scan(true);
            }
        });
        Ok(())
    }

    fn atelier_run_rels(self: &Arc<Self>, rels: Vec<String>, preset: serde_json::Value) -> Result<()> {
        if rels.is_empty() {
            return Err(tr!("atelier.noProject"));
        }
        {
            let mut r = self.repair.lock().unwrap();
            if r.running {
                return Err(tr!("atelier.busy"));
            }
            *r = repairs::RepairState {
                running: true,
                done: 0,
                total: rels.len() as i64,
                results: Vec::new(),
                at: Some(now()),
            };
        }
        self.notify();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let templates = app.data_dir.join("templates");
            for rel in rels {
                let r = atelier::run(&app.store, &rel, &preset, true, false, &templates);
                {
                    let mut s = app.repair.lock().unwrap();
                    s.results.push(atelier::as_result(&r));
                    s.done += 1;
                }
                app.notify();
            }
            app.repair.lock().unwrap().running = false;
            let _ = app.scan(false);
            app.notify();
        });
        Ok(())
    }

    /// The projects a repair would improve (the Repair page).
    pub fn fixable(&self) -> Result<Vec<repairs::Fixable>> {
        repairs::fixable(&self.reader.lock().unwrap()).map_err(err)
    }

    /// What a repair of this project would do.
    pub fn repair_plan(&self, rel: &str) -> Result<Option<repair::fix::Plan>> {
        let st = self.reader.lock().unwrap();
        let row: Option<(String, String)> = st
            .con
            .query_row("select unz(data), unz(plugstate) from files where rel=?", [rel], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            })
            .ok();
        Ok(row.map(|(d, p)| {
            repair::fix::plan(
                rel,
                &serde_json::from_str(&d).unwrap_or_default(),
                &serde_json::from_str(&p).unwrap_or_default(),
                &repairs::choices(&st),
            )
        }))
    }

    /// Every copy the app wrote, so the user can find them (and tidy up themselves).
    /// The format of the base in `data_dir` before it is opened (`None`: no base yet), and the
    /// format this version writes: the start screen says when an older catalogue is updated.
    pub fn base_formats(data_dir: &Path) -> (Option<usize>, usize) {
        (Store::peek(&data_dir.join("plugin-fixer.sqlite")), Store::format())
    }

    /// The base's size on disk (with its journal), before it is opened.
    pub fn base_bytes(data_dir: &Path) -> u64 {
        ["", "-wal"].iter().map(|x| size_of(&data_dir.join(format!("plugin-fixer.sqlite{x}")))).sum()
    }

    /// What Spool keeps on disk, for Settings > Storage.
    pub fn storage(&self) -> Storage {
        let d = &self.data_dir;
        let base: u64 = ["", "-wal", "-shm"].iter().map(|x| size_of(&d.join(format!("plugin-fixer.sqlite{x}")))).sum();
        let journal = size_of(&d.join("plugin-fixer.sqlite-wal"));
        let (midi, samples) = {
            let st = self.reader.lock().unwrap();
            let one = |sql: &str| st.con.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0).max(0) as u64;
            (
                one("select coalesce(sum(length(rel) + length(data) + 16), 0) from midi"),
                // a row's text and its fixed columns, the embedding when there is one
                one("select coalesce(sum(length(path) + length(name) + length(folder) + 70 + coalesce(length(emb), 0)), 0) from samples"),
            )
        };
        let backups = size_of(&d.join("backups"));
        let backup_count = backup_sets(&d.join("backups")).len() as u32;
        let trimmable = trimmable_sets(&d.join("backups")).iter().flatten().map(|p| size_of(p)).sum();
        let model = size_of(&d.join("models"));
        let total = size_of(d);
        // everything else in the folder (thumbnails, templates, previews, the WAV copies of the
        // AIFF files listened to, the reports, the journal): the lines add up to the total
        let cache = total.saturating_sub(base + backups + model);
        Storage { dir: d.to_string_lossy().into_owned(), base, journal, midi: midi.min(base), samples: samples.min(base), app: 0, webview: 0, backups, backup_count, trimmable, cache, model, total }
    }

    /// Frees the room taken by the safety copies, keeping the newest one (a step back stays
    /// possible). Only inside `backups/`; the base in use and the projects are not touched.
    pub fn trim_backups(&self) -> Result<Storage> {
        let dir = self.data_dir.join("backups");
        for set in trimmable_sets(&dir) {
            for p in set {
                // never anything else than Spool's own folder of copies
                if p.parent() != Some(dir.as_path()) {
                    continue;
                }
                let r = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
                r.map_err(err)?;
            }
        }
        Ok(self.storage())
    }

    pub fn written_copies(&self) -> Result<Vec<repairs::WrittenCopy>> {
        let st = self.reader.lock().unwrap();
        repairs::written(&st.con, &store::Roots::load(&st.con)).map_err(err)
    }

    pub fn repair_state(&self) -> repairs::RepairState {
        self.repair.lock().unwrap().clone()
    }

    /// Repairs these projects one after the other, in the background: a copy next to each,
    /// the originals untouched. The new copies are read by the scan that follows.
    pub fn repair_run(self: &Arc<Self>, rels: Vec<String>) -> Result<()> {
        {
            let mut r = self.repair.lock().unwrap();
            if r.running {
                return Err(tr!("repair.busy"));
            }
            *r = repairs::RepairState {
                running: true,
                done: 0,
                total: rels.len() as i64,
                results: Vec::new(),
                at: Some(now()),
            };
        }
        self.notify();
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let templates = app.data_dir.join("templates");
            for rel in rels {
                let res = repairs::repair_one(&app.store, &rel, &templates);
                {
                    let mut r = app.repair.lock().unwrap();
                    r.results.push(res);
                    r.done += 1;
                }
                app.notify();
            }
            app.repair.lock().unwrap().running = false;
            let _ = app.scan(false);
            app.notify();
        });
        Ok(())
    }

    /// The mood board folder of a song (created on first use).
    fn board_dir_of(&self, sid: i64) -> Result<PathBuf> {
        let st = self.reader.lock().unwrap();
        let roots = store::Roots::load(&st.con);
        let (dir, vkey): (Option<String>, Option<String>) = st
            .con
            .query_row("select song_dir, vkey from songs where id=?", [sid], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| tr!("err.songNotFound"))?;
        let dir = dir.unwrap_or_default();
        let n: i64 = st
            .con
            .query_row("select count(*) from songs where song_dir=? and gone=0", [&dir], |r| r.get(0))
            .map_err(err)?;
        let (r, inner) = roots.split(&dir).ok_or_else(|| tr!("err.rootNotFound"))?;
        Ok(catalog::board_dir(Path::new(&r.path), inner, &vkey.unwrap_or_default(), n > 1))
    }

    /// After a board change: this song's checks and card picture follow, the interface reloads.
    fn board_done(&self, sid: i64) -> Result<()> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        index::refresh_song(&tx, sid).map_err(err)?;
        tx.commit().map_err(err)?;
        self.state.lock().unwrap().version += 1;
        drop(st);
        self.notify();
        Ok(())
    }

    /// A note on the board: a new idea, the lyrics, or an existing note (`name`) rewritten.
    /// The version it replaces is kept in the board's .corbeille, and Ctrl+Z puts it back.
    pub fn board_note(&self, sid: i64, text: &str, lyrics: bool, name: Option<&str>) -> Result<String> {
        let n = board_edit::save_note(&self.board_dir_of(sid)?, text, lyrics, name)?;
        if n.old.as_deref() == Some(text) {
            return Ok(n.name);
        }
        let label = match (&n.old, lyrics) {
            (None, true) => tr!("undo.lyricsAdded"),
            (None, false) => tr!("undo.ideaAdded"),
            (Some(_), _) => tr!("undo.noteEdited", name = n.name.trim_end_matches(".md").trim_end_matches(".txt")),
        };
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let entry = serde_json::json!({"kind": "note", "path": n.path.to_string_lossy(), "old": n.old, "at": actions::now()});
        actions::push(&tx, &label, Some(entry)).map_err(err)?;
        index::refresh_song(&tx, sid).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)?;
        Ok(n.name)
    }

    pub fn board_add(&self, sid: i64, name: &str, data: &[u8]) -> Result<String> {
        let n = board_edit::add_file(&self.board_dir_of(sid)?, name, data)?;
        self.board_done(sid)?;
        Ok(n)
    }

    /// Set aside. With the physical arranging on, in the board's .corbeille folder (never
    /// deleted); off, only hidden from the board (the file stays where it is).
    pub fn board_remove(&self, sid: i64, name: &str) -> Result<()> {
        let dir = self.board_dir_of(sid)?;
        if self.physical() {
            board_edit::remove(&dir, name)?;
            return self.board_done(sid);
        }
        let rel = self.roots().rel(&dir.join(name)).ok_or_else(|| tr!("board.itemNotFound"))?;
        self.board_list_edit(sid, "board_hidden", &tr!("undo.boardHidden"), |v| {
            if !v.contains(&rel) {
                v.push(rel.clone());
            }
        })
        .map(|_| ())
    }

    /// Files dropped on a song's sheet, by their paths: one already in the projects folders is
    /// linked where it is (nothing copied, nothing moved); one from elsewhere (Downloads, the
    /// desktop) is copied onto the board, so the board does not break when that folder is
    /// tidied. Only what a board shows (pictures, videos, sounds, texts, fonts); folders and
    /// other files are refused and named.
    pub fn board_drop(&self, sid: i64, paths: Vec<String>) -> Result<BoardDrop> {
        let roots = self.roots();
        let dir = self.board_dir_of(sid)?;
        let mut out = BoardDrop::default();
        let mut links = Vec::new();
        for p in paths {
            let path = PathBuf::from(&p);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
            if !path.is_file() || catalog::kind_of(&name) == "file" {
                out.refused.push(name);
                continue;
            }
            if roots.contains(&path) {
                if path.parent() == Some(dir.as_path()) {
                    continue; // on the board already
                }
                match roots.rel(&path) {
                    Some(rel) => {
                        links.push(rel);
                        out.linked.push(name);
                    }
                    None => out.refused.push(name),
                }
            } else {
                match board_edit::add_path(&dir, &path) {
                    Ok(n) => out.copied.push(n),
                    Err(e) => out.refused.push(format!("{name} ({e})")),
                }
            }
        }
        if !links.is_empty() {
            self.board_link(sid, links)?;
        }
        if !out.copied.is_empty() {
            self.board_done(sid)?;
        }
        Ok(out)
    }

    /// Files shown on a song's board where they are (nothing moves): the virtual mood board.
    pub fn board_link(&self, sid: i64, items: Vec<String>) -> Result<UndoState> {
        let title = self.song_title(sid);
        let n = items.len();
        self.board_list_edit(sid, "board_links", &tr!("undo.boardLinked", n = n, title = title), |v| {
            for i in &items {
                let i = fsops::norm_rel(i);
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        })
    }

    /// A linked file off the board (the file is not touched).
    pub fn board_unlink(&self, sid: i64, rel: &str) -> Result<UndoState> {
        let rel = fsops::norm_rel(rel);
        self.board_list_edit(sid, "board_links", &tr!("undo.boardUnlinked"), |v| v.retain(|x| *x != rel))
    }

    fn song_title(&self, sid: i64) -> String {
        self.reader.lock().unwrap().con.query_row("select coalesce(title, '') from songs where id=?", [sid], |r| r.get(0)).unwrap_or_default()
    }

    /// How the app reads the library's folders, with its own examples.
    pub fn reading_report(&self) -> Result<reading::ReadingReport> {
        reading::report(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// What the grouping would make with these folder rules: nothing written.
    pub fn group_preview(&self, all: bool, dirs: Vec<String>, title_from: String) -> Result<reading::GroupPreview> {
        let t = match title_from.as_str() {
            "folder" => Some(true),
            "file" => Some(false),
            _ => None,
        };
        reading::preview(&self.reader.lock().unwrap().con, &catalog::OneSong { all, dirs: dirs.into_iter().collect(), title_from_folder: t }).map_err(err)
    }

    /// The song folders beside `dir` that hold several songs too.
    pub fn folder_neighbours(&self, dir: &str) -> Result<Vec<String>> {
        reading::neighbours(&self.reader.lock().unwrap().con, dir).map_err(err)
    }

    /// The folder rules changed (every song folder one song, or these ones), the songs
    /// grouped again: one Ctrl+Z puts the rules and the songs back.
    pub fn set_grouping(&self, all: bool, dirs: Vec<String>, title_from: String) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let get = |k: &str| -> Option<String> { tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok() };
        let old_mode = get("group_mode");
        let old_dirs = get("one_song_dirs");
        let old_title = get("title_from");
        let mut snap = serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0});
        snap["settings"] = serde_json::json!({
            "group_mode": old_mode.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            "one_song_dirs": old_dirs.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            "title_from": old_title.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
        });
        let mut dirs: Vec<String> = dirs.into_iter().filter(|d| !d.is_empty()).collect();
        dirs.sort();
        dirs.dedup();
        set_tx(&tx, "group_mode", &if all { "folder" } else { "auto" })?;
        set_tx(&tx, "one_song_dirs", &dirs)?;
        let tf = if matches!(title_from.as_str(), "file" | "folder") { title_from.as_str() } else { "auto" };
        set_tx(&tx, "title_from", &tf)?;
        let label = if all { tr!("undo.groupFolders") } else { tr!("undo.groupDirs", n = dirs.len()) };
        actions::push(&tx, &label, Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// A diagnostic report, written when the user asks (Settings), for them to send if they
    /// choose: the version, the computer, the base's size, what is being analysed and where,
    /// and the journal's end. Read here only; nothing is sent. Returns the file's path.
    pub fn diagnostic_report(&self, version: &str) -> Result<String> {
        use std::fmt::Write as _;
        let mut r = String::new();
        let _ = writeln!(r, "Spool diagnostic report");
        let _ = writeln!(r, "version: {version}");
        let _ = writeln!(r, "system: {} {} ({} threads)", std::env::consts::OS, std::env::consts::ARCH, std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0));
        let base = self.data_dir.join("plugin-fixer.sqlite");
        let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) / 1_048_576;
        let _ = writeln!(r, "data folder: {} (base {} MB, journal {} KB)", self.data_dir.display(), size(&base), std::fs::metadata(self.data_dir.join("spool.log")).map(|m| m.len() / 1024).unwrap_or(0));
        {
            let st = self.reader.lock().unwrap();
            let n = |q: &str| st.con.query_row(q, [], |x| x.get::<_, i64>(0)).map(|v| v.to_string()).unwrap_or_else(|_| "?".into());
            let _ = writeln!(r, "projects: {} files, {} songs", n("select count(*) from files"), n("select count(*) from songs where gone=0"));
            let _ = writeln!(r, "renders: {} audio files, {} analysed", n("select count(*) from audio"), n("select count(*) from audio_analysis"));
            let _ = writeln!(r, "samples: {} known, {} listened to", n("select count(*) from samples"), n("select count(*) from samples where emb is not null"));
            let _ = writeln!(r, "midi: {} files, {} clips", n("select count(*) from midi"), n("select count(*) from clips"));
            let roots = store::Roots::load(&st.con);
            for root in &roots.list {
                let _ = writeln!(r, "projects folder: {}{}", root.path, if Path::new(&root.path).is_dir() { "" } else { " (NOT FOUND)" });
            }
        }
        let c = self.config();
        let _ = writeln!(r, "automatic analysis: {}", c.auto_analysis);
        let s = self.scan_state();
        let _ = writeln!(r, "\n[analysis] running: {} (full: {}), stage: {}, {}/{}, at: {}", s.running, s.full, s.stage, s.done, s.total, s.detail);
        let _ = writeln!(r, "last ended: {:?}, error: {:?}, stopped by the user: {}", s.last, s.error, s.cancelled);
        let a = &s.audio;
        let _ = writeln!(
            r,
            "[sound] on: {}, ready: {}, running: {}, paused: {}, waiting for the computer: {}, Live open: {}, renders {}/{}, unsorted samples left {}, other samples left {}, heard {}, error: {:?}, at: {}",
            a.enabled, a.ready, a.running, a.paused, a.busy_wait, a.live, a.analysed, a.renders, a.samples_todo, a.samples_rest, a.samples_heard, a.error, a.detail
        );
        let _ = writeln!(r, "\n[journal, last lines]");
        for l in diag::tail(800) {
            let _ = writeln!(r, "{l}");
        }
        let dir = self.data_dir.join("exports");
        std::fs::create_dir_all(&dir).map_err(err)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let to = dir.join(format!("spool-diagnostic-{stamp}.txt"));
        std::fs::write(&to, r).map_err(err)?;
        diag::log("report: written");
        Ok(to.to_string_lossy().replace('\\', "/"))
    }

    /// Every song's media (the Media page).
    pub fn media(&self) -> Result<Vec<media_view::MediaSong>> {
        media_view::media(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// The audio files no song uses (the Media page's Audio list).
    pub fn loose_audio(&self) -> Result<Vec<media_view::LooseAudio>> {
        media_view::loose_audio(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// A song's picture chosen by hand (None: back to the app's choice), one Ctrl+Z.
    pub fn set_cover(&self, sid: i64, rel: Option<String>) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let title: String = tx.query_row("select coalesce(title,'') from songs where id=?", [sid], |r| r.get(0)).map_err(|_| tr!("err.songNotFound"))?;
        let k = "cover_chosen";
        let old: Option<String> = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok();
        let mut m: serde_json::Map<String, serde_json::Value> = old.as_deref().and_then(|v| serde_json::from_str(v).ok()).unwrap_or_default();
        match &rel {
            Some(r) => m.insert(sid.to_string(), serde_json::json!(fsops::norm_rel(r))),
            None => m.remove(&sid.to_string()),
        };
        set_tx(&tx, k, &m)?;
        actions::push(&tx, &tr!("undo.coverChosen", title = title), Some(serde_json::json!({"kind": "setting", "k": k, "v": old, "at": actions::now()}))).map_err(err)?;
        index::refresh_song(&tx, sid).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// One song's list in a {"song_dir|vkey": [rel]} setting, changed as one Ctrl+Z.
    fn board_list_edit(&self, sid: i64, k: &str, label: &str, f: impl FnOnce(&mut Vec<String>)) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let key: String = tx
            .query_row("select coalesce(song_dir,'') || '|' || coalesce(vkey,'') from songs where id=?", [sid], |r| r.get(0))
            .map_err(|_| tr!("err.songNotFound"))?;
        let old: Option<String> = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok();
        let mut m: serde_json::Map<String, serde_json::Value> = old.as_deref().and_then(|v| serde_json::from_str(v).ok()).unwrap_or_default();
        let mut v: Vec<String> = m.get(&key).and_then(|x| serde_json::from_value(x.clone()).ok()).unwrap_or_default();
        f(&mut v);
        if v.is_empty() {
            m.remove(&key);
        } else {
            m.insert(key, serde_json::json!(v));
        }
        set_tx(&tx, k, &m)?;
        actions::push(&tx, label, Some(serde_json::json!({"kind": "setting", "k": k, "v": old, "at": actions::now()}))).map_err(err)?;
        index::refresh_song(&tx, sid).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// What changed between two saves of a project (older `a`, newer `b`).
    pub fn als_diff(&self, a: &str, b: &str) -> Result<als_diff::AlsDiff> {
        als_diff::diff(&self.reader.lock().unwrap().con, &fsops::norm_rel(a), &fsops::norm_rel(b))
    }

    /// A file the app's web view can play. An AIFF (which it cannot decode, though Windows
    /// plays it) is served as a WAV made once into the app's own `playable` folder, keyed by
    /// the file's path, size and date; the AIFF itself is only read. Copies unused for two
    /// weeks go.
    pub fn playable(&self, p: PathBuf) -> PathBuf {
        let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
        if ext != "aif" && ext != "aiff" {
            return p;
        }
        let Ok(m) = std::fs::metadata(&p) else { return p };
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (p.to_string_lossy(), m.len(), m.modified().ok()).hash(&mut h);
        let dir = self.data_dir.join("playable");
        let dst = dir.join(format!("{:016x}.wav", h.finish()));
        if dst.is_file() {
            return dst;
        }
        // one conversion at a time: the player asks for several byte ranges at once
        static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _one = ONE.lock().unwrap();
        if dst.is_file() {
            return dst;
        }
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 86_400);
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if e.metadata().and_then(|m| m.accessed().or(m.modified())).is_ok_and(|t| t < old) {
                let _ = std::fs::remove_file(e.path());
            }
        }
        match ::audio::to_wav(&p, &dst) {
            Ok(()) => dst,
            Err(_) => p,
        }
    }

    /// A song's timed comments ("the drop is empty at 1:32"), on any of its renders.
    pub fn comments(&self, sid: i64) -> Result<Vec<Comment>> {
        let r = self.reader.lock().unwrap();
        let key = song_key(&r.con, sid)?;
        let m: serde_json::Map<String, serde_json::Value> = r.get("comments").unwrap_or_default();
        let mut v: Vec<Comment> = m.get(&key).and_then(|x| serde_json::from_value(x.clone()).ok()).unwrap_or_default();
        v.sort_by(|a, b| a.at.total_cmp(&b.at));
        Ok(v)
    }

    /// A comment added, changed (`text` / `done`), or removed (`remove`); one Ctrl+Z each.
    /// `id` 0 adds one, at `at` seconds into the render `rel`.
    pub fn comment_edit(&self, sid: i64, c: Comment, remove: bool) -> Result<UndoState> {
        let label = if remove {
            tr!("undo.commentRemoved")
        } else if c.id == 0 {
            tr!("undo.commentAdded")
        } else {
            tr!("undo.commentEdited")
        };
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let key = song_key(&tx, sid)?;
        let old: Option<String> = tx.query_row("select v from settings where k='comments'", [], |r| r.get(0)).ok();
        let mut m: serde_json::Map<String, serde_json::Value> = old.as_deref().and_then(|v| serde_json::from_str(v).ok()).unwrap_or_default();
        let mut v: Vec<Comment> = m.get(&key).and_then(|x| serde_json::from_value(x.clone()).ok()).unwrap_or_default();
        if remove {
            v.retain(|x| x.id != c.id);
        } else if c.id == 0 {
            let text = c.text.trim().to_string();
            if text.is_empty() {
                return Err(tr!("comments.empty"));
            }
            let id = (actions::now() * 1000.0) as i64;
            v.push(Comment { id, created: actions::now(), text, rel: fsops::norm_rel(&c.rel), at: c.at.max(0.0), done: false });
        } else if let Some(x) = v.iter_mut().find(|x| x.id == c.id) {
            x.text = c.text.trim().to_string();
            x.done = c.done;
        }
        if v.is_empty() {
            m.remove(&key);
        } else {
            m.insert(key, serde_json::json!(v));
        }
        set_tx(&tx, "comments", &m)?;
        actions::push(&tx, &label, Some(serde_json::json!({"kind": "setting", "k": "comments", "v": old, "at": actions::now()}))).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// The reference tracks (released songs to compare a render with), absolute paths.
    pub fn ref_tracks(&self) -> Vec<String> {
        self.reader.lock().unwrap().get("ref_tracks").unwrap_or_default()
    }

    /// A reference track added (an audio file anywhere: it is only read) or removed.
    pub fn ref_track(&self, path: &str, add: bool) -> Result<Vec<String>> {
        let p = PathBuf::from(path);
        let mut v = self.ref_tracks();
        let s = p.to_string_lossy().replace('\\', "/");
        if add {
            if !p.is_file() || catalog::kind_of(&s) != "audio" {
                return Err(tr!("refs.notAudio"));
            }
            if !v.contains(&s) {
                v.push(s);
            }
        } else {
            v.retain(|x| *x != s);
        }
        let st = self.store.lock().unwrap();
        st.set("ref_tracks", &v).map_err(err)?;
        Ok(v)
    }

    /// The physical arranging is on: files may move on disk.
    pub fn physical(&self) -> bool {
        self.reader.lock().unwrap().get("physical").unwrap_or(false)
    }

    fn physical_on(&self) -> Result<()> {
        if self.physical() {
            Ok(())
        } else {
            Err(tr!("fs.physicalOff"))
        }
    }

    /// Where "Ranger sur le disque" would put a release's songs, and what would move (nothing
    /// is touched). The songs already in the release's folder stay; a song folder shared with
    /// songs outside the release stays too.
    pub fn release_arrange_plan(&self, id: &str, parent: Option<String>) -> Result<releases::ArrangePlan> {
        let roots = self.roots();
        let st = self.reader.lock().unwrap();
        let r = releases::load(&st.con).into_iter().find(|r| r.id == id).ok_or_else(|| tr!("releases.notFound"))?;
        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let mut q = st.con.prepare("select coalesce(song_dir,'') from songs where gone=0").map_err(err)?;
        for d in q.query_map([], |r| r.get::<_, String>(0)).map_err(err)?.flatten() {
            *counts.entry(d).or_default() += 1;
        }
        let dirs: Vec<String> = r.tracks.iter().map(|t| t.dir.clone()).collect();
        if dirs.is_empty() {
            return Err(tr!("releases.noSong"));
        }
        let inside = |d: &str, f: &str| d == f || d.starts_with(&format!("{f}/"));
        let parent = parent.filter(|p| !p.trim().is_empty());
        // already in its own folder (made from it, or arranged before): that folder
        let target = match (&parent, &r.folder) {
            (None, Some(f)) if !f.is_empty() && dirs.iter().all(|d| inside(d, f)) => f.clone(),
            _ => {
                let parent_rel = match &parent {
                    Some(p) => {
                        let pb = PathBuf::from(p);
                        if !roots.contains(&pb) {
                            return Err(tr!("releases.pickInsideRoots"));
                        }
                        roots.rel(&pb).ok_or_else(|| tr!("releases.outsideRoots"))?
                    }
                    None => {
                        // the folder above the one the songs share
                        let parts: Vec<Vec<&str>> = dirs.iter().map(|d| d.split('/').collect()).collect();
                        let mut common: Vec<&str> = Vec::new();
                        for (i, w) in parts[0].iter().enumerate() {
                            if parts.iter().all(|p| p.len() > i + 1 && p[i] == *w) {
                                common.push(w);
                            } else {
                                break;
                            }
                        }
                        common.join("/")
                    }
                };
                let kind = match r.kind.as_str() {
                    "single" => "Single",
                    "ep" => "EP",
                    "lp" => "LP",
                    _ => "Compil",
                };
                let name = fsops::safe_part(&format!("{kind} - {}", r.title)).map_err(err)?;
                if parent_rel.is_empty() { name } else { format!("{parent_rel}/{name}") }
            }
        };
        let mut in_release: std::collections::HashMap<&String, i64> = Default::default();
        for d in &dirs {
            *in_release.entry(d).or_default() += 1;
        }
        let mut skipped = Vec::new();
        let mut moves: Vec<String> = Vec::new();
        let mut already = 0;
        for d in &dirs {
            if inside(d, &target) {
                already += 1;
                continue;
            }
            if counts.get(d).copied().unwrap_or(0) > in_release.get(d).copied().unwrap_or(0) {
                skipped.push(fsops::Skipped { src: d.clone(), why: tr!("releases.sharedFolder"), risks: Vec::new() });
                continue;
            }
            if !moves.contains(d) {
                moves.push(d.clone());
            }
        }
        let exists = roots.abs(&target).is_some_and(|p| p.exists());
        Ok(releases::ArrangePlan { target, exists, moves, skipped, already })
    }

    /// Puts a release's songs in its folder on disk (release_arrange_plan): the folder made if
    /// needed, each song's own folder moved into it (a move that would lose a sample is
    /// skipped, with why). One Ctrl+Z puts everything back. Only with the physical arranging on.
    pub fn release_arrange(self: &Arc<Self>, id: &str, parent: Option<String>) -> Result<fsops::MoveResult> {
        self.physical_on()?;
        let plan = self.release_arrange_plan(id, parent)?;
        let title = releases::load(&self.reader.lock().unwrap().con).into_iter().find(|r| r.id == id).map(|r| r.title).unwrap_or_default();
        if plan.moves.is_empty() {
            return Ok(fsops::MoveResult { skipped: plan.skipped, ..Default::default() });
        }
        let env = self.env();
        let target = plan.target.clone();
        let moves: Vec<(String, String)> = plan.moves.iter().map(|m| (m.clone(), target.clone())).collect();
        let id = id.to_string();
        let mut out = self.file_op(|tx, roots| {
            let mut ops: Vec<fsops::Op> = Vec::new();
            if roots.abs(&target).is_none_or(|p| !p.exists()) {
                let n = fsops::mkdir(roots, &target)?;
                ops.push(("mkdir".into(), String::new(), n));
            }
            let res = fsops::move_many(tx, roots, &env, &moves);
            ops.extend(res.ops.clone());
            // the release remembers its folder (repath already moved its tracks)
            let mut list = releases::load(tx);
            if let Some(x) = list.iter_mut().find(|x| x.id == id) {
                x.folder = Some(target.clone());
            }
            set_tx(tx, "releases", &list)?;
            let label = tr!("undo.releaseArranged", title = title, target = target);
            Ok((res, Some((label, ops))))
        })?;
        out.skipped.extend(plan.skipped);
        Ok(out)
    }

    pub fn board_meta(&self, sid: i64, patch: serde_json::Map<String, serde_json::Value>) -> Result<()> {
        board_edit::set_meta(&self.board_dir_of(sid)?, &patch)?;
        self.board_done(sid)
    }

    /// A file the interface may show or play: only under the projects root (relative to it,
    /// or absolute), and only an existing file. Anything else is refused.
    pub fn media_path(&self, rel_or_abs: &str) -> Option<PathBuf> {
        let roots = {
            let mut c = self.media_roots.lock().unwrap();
            match c.as_ref() {
                Some((at, r)) if at.elapsed() < std::time::Duration::from_secs(2) => r.clone(),
                _ => {
                    let r = self.roots();
                    *c = Some((std::time::Instant::now(), r.clone()));
                    r
                }
            }
        };
        let p = Path::new(rel_or_abs);
        let p = if p.is_absolute() { p.to_path_buf() } else { roots.abs(rel_or_abs)? };
        ((roots.contains(&p) || self.in_libraries(&p) || self.used_by_a_project(&p) || self.is_ref_track(&p) || in_kept_arrangement(&p) || self.in_artist_dirs(&p) || self.in_pictures(&p)) && p.is_file()).then(|| p.canonicalize().ok()).flatten()
    }

    /// A reference track the user added (Compare, on a song's sheet).
    fn is_ref_track(&self, p: &Path) -> bool {
        let Ok(real) = p.canonicalize() else { return false };
        self.ref_tracks().iter().filter_map(|r| Path::new(r).canonicalize().ok()).any(|r| r == real)
    }

    /// Inside a sample or MIDI library folder of the settings (the Samples page plays and
    /// shows those files, nothing else outside the projects).
    fn in_libraries(&self, p: &Path) -> bool {
        let st = self.reader.lock().unwrap();
        let dirs: Vec<String> = [st.get::<Vec<String>>("sample_dirs"), st.get::<Vec<String>>("midi_dirs")].into_iter().flatten().flatten().collect();
        drop(st);
        // compared on the real paths: "..\" cannot climb out
        let Ok(real) = p.canonicalize() else { return false };
        dirs.iter().filter_map(|d| Path::new(d).canonicalize().ok()).any(|d| real.starts_with(d))
    }

    /// A page of MIDI files (MIDI page).
    pub fn midi_page(&self, q: midi_view::MidiQuery) -> Result<midi_view::MidiPage> {
        midi_view::midi_page(&self.reader.lock().unwrap().con, &q).map_err(err)
    }

    /// A MIDI file transposed and at another tempo, written as a new file in the app's
    /// `exports` folder (the original is only read), to drag into Live. Returns its path.
    pub fn midi_export(&self, rel: &str, semis: i32, rate: f64, target: Option<f64>, drums: bool) -> Result<String> {
        let dir = self.data_dir.join("exports");
        // a clip of a project: its notes, named after its song and track
        if let Some(h) = rel.strip_prefix("clip:") {
            let (smf, clip_drums) = self.clip_midi(h)?;
            let uses = self.clip_uses(h)?;
            let stem = uses.first().map(|u| {
                let (track, name) = (midi_view::clean(&u.track), midi_view::clean(&u.name));
                let what = if name.is_empty() || name == track { track } else if track.is_empty() { name } else { format!("{track} - {name}") };
                if u.title.is_empty() { what } else { format!("{} - {}", u.title, what) }
            });
            let to = midi_export::export_bytes(&smf, &stem.unwrap_or_else(|| "clip".into()), &dir, semis, rate, target, drums || clip_drums)?;
            return Ok(to.to_string_lossy().into_owned());
        }
        let src = self.media_path(rel).ok_or_else(|| tr!("err.fileNotFound"))?;
        let to = midi_export::export(&src, &dir, semis, rate, target, drums)?;
        Ok(to.to_string_lossy().into_owned())
    }

    /// A project's arrangement as a picture (song sheet): tracks, placed clips, locators. Read
    /// from the set (a few seconds for a large one), the last ones kept while unchanged.
    pub fn arrangement(&self, rel: &str) -> Result<als::Arrangement> {
        let p = self.project_path(&fsops::norm_rel(rel)).filter(|p| p.is_file()).ok_or_else(|| tr!("err.projectNotFound"))?;
        let mtime = std::fs::metadata(&p).and_then(|m| m.modified()).map_err(err)?;
        if let Some((_, _, a)) = KEPT.lock().unwrap().iter().find(|(q, m, _)| *q == p && *m == mtime) {
            return Ok((**a).clone());
        }
        let a = als::arrangement(&p, &self.env()).ok_or_else(|| tr!("diff.unreadable", error = "XML"))?;
        let mut kept = KEPT.lock().unwrap();
        kept.retain(|(q, _, _)| *q != p);
        kept.push((p, mtime, std::sync::Arc::new(a.clone())));
        if kept.len() > 6 {
            kept.remove(0);
        }
        Ok(a)
    }

    /// A project's harmony bar by bar (its key, the chord of each bar), from the notes of its
    /// arrangement's harmonic tracks (keys, pads, plucks, basses; not drums, percussion,
    /// effects or voices).
    pub fn harmony(&self, rel: &str) -> Result<midi::Harmony> {
        let a = self.arrangement(rel)?;
        let mut notes: Vec<(u8, f64, f64, bool)> = Vec::new();
        // what is heard in Live: a track or group switched off there says nothing of the harmony
        for t in a.tracks.iter().filter(|t| t.kind == "midi" && !t.muted && !matches!(t.role.as_str(), "drums" | "perc" | "fx" | "vox" | "return")) {
            let bass = t.role == "bass";
            for c in t.clips.iter().filter(|c| !c.muted) {
                for &(p, s, l) in &c.notes {
                    notes.push((p, c.start + s, l, bass));
                }
            }
        }
        let bars = (a.len / a.beats_per_bar).ceil().max(1.0) as usize;
        // the song's own key (its folder's name, or set by the user) settles a key the notes
        // leave open (see midi::harmony_with)
        let said: Option<String> = self
            .reader
            .lock()
            .unwrap()
            .con
            .query_row("select s.key from files f join songs s on s.id = f.song_id where f.rel = ?", [fsops::norm_rel(rel)], |r| r.get(0))
            .ok()
            .flatten();
        Ok(midi::harmony_with(&notes, a.beats_per_bar, bars, said.as_deref().and_then(midi::parse_key)))
    }

    /// Notes (pitch, start, length in beats) written as a MIDI file in the app's `exports`
    /// folder, to drag into Live (a chord proposed on a song's sheet). Returns its path.
    pub fn write_midi(&self, name: &str, notes: &[(u8, f64, f64)], bpm: f64) -> Result<String> {
        let len = notes.iter().map(|n| n.1 + n.2).fold(0.0, f64::max).max(1.0);
        let n: Vec<(u8, f64, f64, u8)> = notes.iter().map(|&(p, s, l)| (p, s, l, 100)).collect();
        let smf = midi::write_smf(name, &n, (len / 4.0).ceil() * 4.0, bpm, false);
        let to = midi_export::export_bytes(&smf, name, &self.data_dir.join("exports"), 0, 1.0, None, false)?;
        Ok(to.to_string_lossy().into_owned())
    }

    /// A clip of a project as a MIDI file (its notes), and whether they are drums.
    pub fn clip_midi(&self, hash: &str) -> Result<(Vec<u8>, bool)> {
        midi_view::clip_smf(&self.reader.lock().unwrap().con, hash).map_err(|_| tr!("err.fileNotFound"))
    }

    /// Every project holding a clip.
    pub fn clip_uses(&self, hash: &str) -> Result<Vec<midi_view::ClipUse>> {
        midi_view::clip_uses(&self.reader.lock().unwrap().con, hash).map_err(err)
    }

    /// The samples kept twice or more (only read: nothing is deleted).
    pub fn sample_dupes(&self) -> Result<Vec<samples_view::Dupes>> {
        samples_view::dupes(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// The songs whose projects use a sample.
    pub fn sample_songs(&self, path: &str) -> Result<Vec<i64>> {
        samples_view::sample_songs(&self.reader.lock().unwrap().con, path).map_err(err)
    }

    /// Samples sorted by hand (Samples page; `family` None: back to the automatic sort). Kept
    /// across scans, and taught to the library's own model at its next learning.
    pub fn sort_samples(&self, paths: &[String], family: Option<&str>, cat: Option<&str>) -> Result<usize> {
        samples_view::sort_by_hand(&self.store.lock().unwrap().con, paths, family, cat).map_err(err)
    }

    /// The unsorted samples of one folder, sorted by hand at once.
    pub fn sort_sample_folder(&self, dir: &str, family: &str, cat: Option<&str>) -> Result<usize> {
        let paths = samples_view::unsorted_in(&self.reader.lock().unwrap().con, dir).map_err(err)?;
        self.sort_samples(&paths, Some(family), cat)
    }

    /// A page of samples (Samples page).
    pub fn samples(&self, q: samples_view::SampleQuery) -> Result<samples_view::SamplePage> {
        samples_view::samples(&self.reader.lock().unwrap().con, &q).map_err(err)
    }

    /// A library file's real path, only inside a sample or MIDI folder of the settings, or a
    /// sample a project plays (a song's Resources).
    pub fn library_path(&self, abs: &str) -> Option<PathBuf> {
        let p = Path::new(abs);
        (p.is_absolute() && (self.in_libraries(p) || self.used_by_a_project(p))).then(|| p.to_path_buf())
    }

    /// A sample one of the projects plays (read from the sets: `sample_use`), wherever it is:
    /// its song's Resources play it and show it in Explorer.
    fn used_by_a_project(&self, p: &Path) -> bool {
        let s = p.to_string_lossy().replace('\\', "/").to_lowercase();
        if s.contains("/../") {
            return false;
        }
        let st = self.reader.lock().unwrap();
        st.con.query_row("select 1 from sample_use where path = ? limit 1", [&s], |_| Ok(())).is_ok()
    }

    /// A song's resources: the samples its project plays and its Samples folders' files.
    pub fn resources(&self, sid: i64, rel: Option<String>) -> Result<resources::Resources> {
        resources::resources(&self.reader.lock().unwrap().con, sid, rel.as_deref()).map_err(err)
    }

    /// A file the app wrote in its own exports folder (a diagnostic report), to show it.
    pub fn export_path(&self, abs: &str) -> Option<PathBuf> {
        let p = Path::new(abs);
        let dir = self.data_dir.join("exports");
        (p.is_absolute() && !abs.contains("..") && p.starts_with(&dir)).then(|| p.to_path_buf())
    }

    /// The real path of a project file, only inside the projects folders.
    pub fn project_path(&self, rel: &str) -> Option<PathBuf> {
        let roots = self.roots();
        roots.abs(rel).filter(|p| roots.contains(p))
    }

    /// A render's waveform and loudness (LUFS, peak, RMS), decoded once then kept.
    pub fn audio_info(&self, rel_or_abs: &str) -> Result<audio::AudioInfo> {
        let p = self.media_path(rel_or_abs).ok_or_else(|| tr!("err.fileNotFound"))?;
        audio::cached(&p, &self.data_dir.join("audio"))
    }

    /// One song's sheet (None: no such song).
    pub fn song(&self, sid: i64) -> Result<Option<detail::SongDetail>> {
        detail::song_detail(&self.reader.lock().unwrap().con, sid).map_err(err)
    }

    /// What a new song of a style would be made of (Create page).
    pub fn create_plan(&self, req: create::CreateReq) -> Result<create::CreatePlan> {
        let st = self.reader.lock().unwrap();
        let roots = store::Roots::load(&st.con);
        let folder = roots
            .main()
            .map(|r| std::path::Path::new(&r.path).join(create::FOLDER).to_string_lossy().into_owned())
            .unwrap_or_default();
        let cat = create::catalogue(&st.con, st.get("chain_prefs").unwrap_or_default())?;
        create::plan(&st.con, cat, &req, folder)
    }

    /// The releases (EP, LP...), the next to come out first.
    pub fn releases(&self) -> Result<Vec<releases::Release>> {
        releases::list(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// Folders whose name says EP / LP / album, not made a release yet.
    pub fn release_hints(&self) -> Result<Vec<releases::ReleaseHint>> {
        releases::hints(&self.reader.lock().unwrap().con).map_err(err)
    }

    /// Creates (empty id) or replaces a release. Returns its id and what Ctrl+Z undoes.
    pub fn release_save(&self, mut r: releases::Release) -> Result<(String, UndoState)> {
        if r.title.trim().is_empty() {
            return Err(tr!("releases.noTitle"));
        }
        let new = r.id.is_empty();
        if new {
            r.id = format!("r{}", (actions::now() * 1000.0) as i64);
            r.created = actions::now();
        }
        let label = if new { tr!("undo.releaseCreated", title = r.title.trim()) } else { tr!("undo.releaseEdited", title = r.title.trim()) };
        let id = r.id.clone();
        let st = self.change_releases(&label, |tx, list| {
            let old = list.iter().position(|x| x.id == r.id);
            let s = releases::to_stored(tx, &r, old.map(|i| &list[i])).map_err(err)?;
            match old {
                Some(i) => list[i] = s,
                None => list.push(s),
            }
            Ok(())
        })?;
        Ok((id, st))
    }

    pub fn release_delete(&self, id: &str) -> Result<UndoState> {
        let title = releases::load(&self.reader.lock().unwrap().con).into_iter().find(|r| r.id == id).map(|r| r.title).unwrap_or_default();
        self.change_releases(&tr!("undo.releaseDeleted", title = title), |_, list| {
            list.retain(|r| r.id != id);
            Ok(())
        })
    }

    /// One change to the releases, kept whole for Ctrl+Z.
    fn change_releases(
        &self,
        label: &str,
        f: impl FnOnce(&store::rusqlite::Transaction, &mut Vec<releases::Stored>) -> Result<()>,
    ) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let snap = releases::snapshot(&tx);
        let mut list = releases::load(&tx);
        f(&tx, &mut list)?;
        set_tx(&tx, "releases", &list)?;
        actions::push(&tx, label, Some(snap)).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// What a track of a project holds (its devices, racks opened), for the Create page.
    pub fn track_view(&self, rel: &str, track: &str) -> Result<Vec<repair::track_view::DevView>> {
        let p = self.project_path(rel).ok_or_else(|| tr!("err.projectNotFound"))?;
        repair::track_view::track_view(&p, track)
    }

    /// Writes the new song in a NEW folder "<main folder>/Nouveaux morceaux/<name> Project":
    /// refused when that folder exists (nothing is ever written over), the source projects
    /// are only read. Returns the new set's name in the base, and what was done.
    pub fn create_song(&self, req: create::CreateReq) -> Result<serde_json::Value> {
        let name = create::clean_name(&req.name);
        if name.is_empty() {
            return Err(tr!("create.noName"));
        }
        let plan = self.create_plan(req.clone())?;
        let roots = self.roots();
        let main = roots.main().ok_or_else(|| tr!("create.noRoot"))?;
        let parent = match req.folder.as_deref().filter(|f| !f.trim().is_empty()) {
            Some(f) => {
                let p = std::path::PathBuf::from(f);
                if !roots.contains(&p) {
                    return Err(tr!("create.pickInsideRoots"));
                }
                p
            }
            None => std::path::Path::new(&main.path).join(create::FOLDER),
        };
        let dir = parent.join(format!("{name} Project"));
        if dir.exists() {
            return Err(tr!("create.exists", name = name, parent = parent.display()));
        }
        let out = dir.join(format!("{name}.als"));
        let picks: Vec<&chains::SlotPick> =
            req.parts.iter().filter_map(|p| plan.parts.iter().find(|x| &x.part == p).and_then(|x| x.pick.as_ref())).collect();
        if picks.is_empty() {
            return Err(tr!("create.noPart"));
        }
        let abs = |rel: &str| roots.abs(rel).ok_or_else(|| tr!("create.projectNotFound", rel = rel));
        let o = repair::assemble::Order {
            base: abs(&picks[0].rel)?,
            picks: picks
                .iter()
                .map(|p| {
                    Ok(repair::assemble::Pick {
                        set: abs(&p.rel)?,
                        track: p.track.clone(),
                        name: Some(create::part_name(&p.part).to_string()),
                        color: create::color_of(&p.part),
                    })
                })
                .collect::<Result<_>>()?,
            main_devices: vec!["StereoGain".into(), "Limiter".into()],
            donors: vec![],
            tempo: req.tempo.or(plan.structure.bpm),
            scale: None,
            markers: plan.structure.sections.iter().map(|x| (x.bar as f64 * 4.0, x.name.clone())).collect(),
            volume_db: -6.0,
            keep_mix: false,
            out: out.clone(),
        };
        match repair::assemble::assemble(&o) {
            Ok(mut rep) => {
                rep["rel"] = roots.rel(&out).map(serde_json::Value::from).unwrap_or_default();
                Ok(rep)
            }
            Err(e) => {
                // the folder was not there before: leave nothing behind if it is still empty
                let _ = std::fs::remove_dir(&dir);
                Err(e)
            }
        }
    }

    /// The styles the app would suggest for a song, with why, even when one is noted.
    pub fn style_guess(&self, sid: i64) -> Result<Vec<catalog::StyleOption>> {
        index::style_guess(&self.reader.lock().unwrap().con, sid).map_err(err)
    }

    pub fn songs(&self) -> Result<Vec<SongItem>> {
        self.cached(|c| &mut c.songs, || views::songs_list(&self.reader.lock().unwrap().con).map_err(err))
    }

    pub fn overview(&self) -> Result<Overview> {
        self.cached(|c| &mut c.overview, || views::overview(&self.reader.lock().unwrap().con).map_err(err))
    }

    pub fn stats(&self) -> Result<Stats> {
        self.cached(|c| &mut c.stats, || {
            let st = self.reader.lock().unwrap();
            let ignored: Vec<String> = st.get("ignored").unwrap_or_default();
            views::stats(&st.con, &ignored).map_err(err)
        })
    }

    /// A project file made a song of its own (it was grouped with another file of a similar
    /// name): a new song, its title from the file's name, the old song keeping the rest. One
    /// Ctrl+Z puts it back. Returns the new song's id.
    pub fn file_to_song(&self, rel: &str) -> Result<i64> {
        let rel = fsops::norm_rel(rel);
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let from: Option<i64> = tx.query_row("select song_id from files where rel=?", [&rel], |r| r.get(0)).map_err(|_| tr!("err.projectNotFound"))?;
        let old: Option<String> = tx.query_row("select v from settings where k='own_files'", [], |r| r.get(0)).ok();
        let mut snap = actions::snapshot(&tx, &from.into_iter().collect::<Vec<_>>()).map_err(err)?
            .unwrap_or_else(|| serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0}));
        snap["settings"] = serde_json::json!({"own_files": old.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)});
        let mut own: Vec<String> = old.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        if !own.contains(&rel) {
            own.push(rel.clone());
        }
        set_tx(&tx, "own_files", &own)?;
        let name = rel.rsplit('/').next().unwrap_or(&rel).trim_end_matches(".als").to_string();
        actions::push(&tx, &tr!("undo.fileToSong", name = name), Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        let id: i64 = tx.query_row("select song_id from files where rel=?", [&rel], |r| r.get::<_, Option<i64>>(0)).map_err(err)?.ok_or_else(|| tr!("err.projectNotFound"))?;
        tx.commit().map_err(err)?;
        self.after_change(&st)?;
        Ok(id)
    }

    /// One artist merged into another: every song, release and projects folder saying `from`
    /// says `into`; the artists list keeps `into`; and a rule (artist_aliases) makes the scan
    /// read `from` in folder names as `into` from now on. One Ctrl+Z puts it all back. The
    /// folders are not touched: Folders > Reorganise proposes the moves, one by one.
    pub fn merge_artists(&self, from: &str, into: &str) -> Result<UndoState> {
        let (from, into) = (from.trim(), into.trim());
        if from.is_empty() || into.is_empty() || from.eq_ignore_ascii_case(into) {
            return Err(tr!("artists.mergeSame"));
        }
        let same = |a: &str| a.trim().eq_ignore_ascii_case(from);
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let sids: Vec<i64> = {
            let mut s = tx.prepare("select id, artist, ft from songs where gone = 0").map_err(err)?;
            let rows = s
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)))
                .map_err(err)?;
            rows.flatten()
                .filter(|(_, a, ft)| a.as_deref().is_some_and(same) || ft.as_deref().is_some_and(|f| f.split(',').any(same)))
                .map(|(id, _, _)| id)
                .collect()
        };
        let mut snap = actions::snapshot(&tx, &sids).map_err(err)?.unwrap_or_else(|| serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0}));
        let keys = ["artist_aliases", "artists", "artist_feats", "artist_ignored", "root_artist", "more_roots", "releases", "artist_profiles"];
        let mut old = serde_json::Map::new();
        for k in keys {
            let v: Option<String> = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok();
            old.insert(k.into(), v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
        }
        snap["settings"] = serde_json::Value::Object(old);
        let get = |k: &str| -> Option<serde_json::Value> {
            tx.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0)).ok().and_then(|v| serde_json::from_str(&v).ok())
        };
        let put = |k: &str, v: &serde_json::Value| tx.execute("insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v", store::rusqlite::params![k, v.to_string()]);
        // the rule: `from` (and whatever was merged into it before) now means `into`
        let mut aliases: std::collections::BTreeMap<String, String> = get("artist_aliases").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        for v in aliases.values_mut() {
            if same(v) {
                *v = into.to_string();
            }
        }
        aliases.retain(|k, _| !k.eq_ignore_ascii_case(into));
        aliases.insert(from.to_string(), into.to_string());
        put("artist_aliases", &serde_json::json!(aliases)).map_err(err)?;
        // `from` was a guest, or said not to be an artist: it is one of the user's names now
        let mut feats: std::collections::BTreeMap<String, String> = get("artist_feats").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        feats.retain(|k, _| !same(k));
        put("artist_feats", &serde_json::json!(feats)).map_err(err)?;
        let mut ignored: Vec<String> = get("artist_ignored").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        ignored.retain(|k| !same(k));
        put("artist_ignored", &serde_json::json!(ignored)).map_err(err)?;
        // the profile written for `from` goes to `into`, unless `into` has one (then both are
        // kept: nothing written about an artist is lost in a merge)
        let mut profiles: serde_json::Map<String, serde_json::Value> = get("artist_profiles").and_then(|v| v.as_object().cloned()).unwrap_or_default();
        if let Some(mut p) = profiles.remove(&from.to_lowercase()) {
            if profiles.contains_key(&into.to_lowercase()) {
                profiles.insert(from.to_lowercase(), p);
            } else {
                p["name"] = serde_json::json!(into);
                profiles.insert(into.to_lowercase(), p);
            }
            put("artist_profiles", &serde_json::Value::Object(profiles)).map_err(err)?;
        }
        // the artists list: `into` kept, `from` gone
        let mut list: Vec<String> = get("artists").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        list.retain(|a| !same(a));
        if !list.iter().any(|a| a.eq_ignore_ascii_case(into)) {
            list.push(into.to_string());
        }
        put("artists", &serde_json::json!(list)).map_err(err)?;
        // the projects folders' own artist
        if get("root_artist").and_then(|v| v.as_str().map(same)).unwrap_or(false) {
            put("root_artist", &serde_json::json!(into)).map_err(err)?;
        }
        if let Some(serde_json::Value::Array(mut more)) = get("more_roots") {
            for m in &mut more {
                if m["artist"].as_str().is_some_and(same) {
                    m["artist"] = serde_json::json!(into);
                }
            }
            put("more_roots", &serde_json::Value::Array(more)).map_err(err)?;
        }
        // the releases
        if let Some(serde_json::Value::Array(mut rel)) = get("releases") {
            for r in &mut rel {
                if r["artist"].as_str().is_some_and(same) {
                    r["artist"] = serde_json::json!(into);
                }
            }
            put("releases", &serde_json::Value::Array(rel)).map_err(err)?;
        }
        // the songs: the artist, and the name in the featurings
        for sid in &sids {
            let (a, ft): (Option<String>, Option<String>) =
                tx.query_row("select artist, ft from songs where id=?", [sid], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
            let a = a.map(|a| if same(&a) { into.to_string() } else { a });
            let ft = ft.map(|f| f.split(',').map(|x| if same(x) { into.to_string() } else { x.trim().to_string() }).collect::<Vec<_>>().join(", "));
            tx.execute("update songs set artist=?, ft=? where id=?", store::rusqlite::params![a, ft, sid]).map_err(err)?;
        }
        actions::push(&tx, &tr!("undo.artistsMerged", from = from, into = into), Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// A merge undone later on: the old name is read as itself again at the next regrouping
    /// (the songs whose artist was typed by hand keep the merged name).
    pub fn unmerge_artist(&self, from: &str) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let old: Option<String> = tx.query_row("select v from settings where k='artist_aliases'", [], |r| r.get(0)).ok();
        let snap = serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0,
            "settings": {"artist_aliases": old.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)}});
        let mut aliases: std::collections::BTreeMap<String, String> = old.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        aliases.retain(|k, _| !k.eq_ignore_ascii_case(from.trim()));
        tx.execute("insert into settings(k, v) values('artist_aliases', ?) on conflict(k) do update set v=excluded.v", [serde_json::json!(aliases).to_string()])
            .map_err(err)?;
        actions::push(&tx, &tr!("undo.artistUnmerged", from = from.trim()), Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// A name the first analysis took for one of the user's artists that is a featured artist:
    /// its songs go to `main` (empty: the projects folder's own artist) with the name in their
    /// featurings, and it leaves the artists list. Undone with Ctrl+Z, or `feat_back`.
    pub fn artist_to_feat(&self, name: &str, main: &str) -> Result<UndoState> {
        let (name, main) = (name.trim(), main.trim());
        if name.is_empty() || name.eq_ignore_ascii_case(main) {
            return Err(tr!("err.noName"));
        }
        self.feat_rule(name, Some(main), tr!("undo.artistFeat", name = name))
    }

    /// A featured name read as one of the user's artists again (back in the artists list).
    pub fn feat_back(&self, name: &str) -> Result<UndoState> {
        let name = name.trim();
        self.feat_rule(name, None, tr!("undo.artistFeatBack", name = name))
    }

    /// `main` Some: `name` becomes a featured artist going to `main`; None: one of the user's
    /// artists again. One Ctrl+Z puts both settings back.
    fn feat_rule(&self, name: &str, main: Option<&str>, label: String) -> Result<UndoState> {
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let mut old = serde_json::Map::new();
        for k in ["artists", "artist_feats"] {
            let v: Option<String> = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok();
            old.insert(k.into(), v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
        }
        let snap = serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0, "settings": old});
        let get = |k: &str| -> Option<serde_json::Value> {
            tx.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0)).ok().and_then(|v| serde_json::from_str(&v).ok())
        };
        let put = |k: &str, v: &serde_json::Value| tx.execute("insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v", store::rusqlite::params![k, v.to_string()]);
        let mut feats: std::collections::BTreeMap<String, String> = get("artist_feats").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        feats.retain(|k, _| !k.eq_ignore_ascii_case(name));
        let mut list: Vec<String> = get("artists").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        match main {
            Some(main) => {
                feats.insert(name.to_string(), main.to_string());
                list.retain(|a| !a.trim().eq_ignore_ascii_case(name));
            }
            None => {
                if !list.iter().any(|a| a.trim().eq_ignore_ascii_case(name)) {
                    list.push(name.to_string());
                }
            }
        }
        put("artist_feats", &serde_json::json!(feats)).map_err(err)?;
        put("artists", &serde_json::json!(list)).map_err(err)?;
        actions::push(&tx, &label, Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// The Artists page's one question about a name, "who is it?":
    /// - "me": one of the user's own names (its songs are its own);
    /// - "alias": the user under another name, read as `target` (a merge: see merge_artists);
    /// - "guest": someone featured: the songs go to `target` (empty: the projects folder's
    ///   own artist) with the name in their featurings;
    /// - "none": not an artist (a word of the folder names): left in the titles, never offered
    ///   to sort again.
    /// Whatever the name was before stops (a guest made one of the user's names is no longer a
    /// guest; an alias made "me" is read as itself again). One Ctrl+Z puts it back.
    pub fn artist_role(&self, name: &str, role: &str, target: &str) -> Result<UndoState> {
        let (name, target) = (name.trim(), target.trim());
        if name.is_empty() {
            return Err(tr!("err.noName"));
        }
        if role == "alias" {
            return self.merge_artists(name, target);
        }
        if !["me", "guest", "none"].contains(&role) {
            return Err(tr!("err.noName"));
        }
        if role == "guest" && name.eq_ignore_ascii_case(target) {
            return Err(tr!("err.noName"));
        }
        let same = |a: &str| a.trim().eq_ignore_ascii_case(name);
        let st = self.store.lock().unwrap();
        let tx = st.con.unchecked_transaction().map_err(err)?;
        let keys = ["artists", "artist_aliases", "artist_feats", "artist_ignored"];
        let mut old = serde_json::Map::new();
        for k in keys {
            let v: Option<String> = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok();
            old.insert(k.into(), v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
        }
        let snap = serde_json::json!({"at": actions::now(), "songs": [], "sugg": [], "max_sugg": 0, "settings": old});
        let get = |k: &str| -> Option<serde_json::Value> {
            tx.query_row("select v from settings where k=?", [k], |r| r.get::<_, String>(0)).ok().and_then(|v| serde_json::from_str(&v).ok())
        };
        let put = |k: &str, v: &serde_json::Value| tx.execute("insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v", store::rusqlite::params![k, v.to_string()]);
        let mut list: Vec<String> = get("artists").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let mut aliases: std::collections::BTreeMap<String, String> = get("artist_aliases").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let mut feats: std::collections::BTreeMap<String, String> = get("artist_feats").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let mut ignored: Vec<String> = get("artist_ignored").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        // whatever it was
        list.retain(|a| !same(a));
        aliases.retain(|k, _| !same(k));
        feats.retain(|k, _| !same(k));
        ignored.retain(|k| !same(k));
        let label = match role {
            "me" => {
                list.push(name.to_string());
                tr!("undo.artistMine", name = name)
            }
            "guest" => {
                feats.insert(name.to_string(), target.to_string());
                // other names read as this one stay with it (they are guests too)
                tr!("undo.artistFeat", name = name)
            }
            _ => {
                ignored.push(name.to_string());
                tr!("undo.artistNone", name = name)
            }
        };
        put("artists", &serde_json::json!(list)).map_err(err)?;
        put("artist_aliases", &serde_json::json!(aliases)).map_err(err)?;
        put("artist_feats", &serde_json::json!(feats)).map_err(err)?;
        put("artist_ignored", &serde_json::json!(ignored)).map_err(err)?;
        actions::push(&tx, &label, Some(snap)).map_err(err)?;
        index::regroup(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.after_change(&st)
    }

    /// The merges made (old name -> name), for Settings.
    pub fn artist_aliases(&self) -> std::collections::BTreeMap<String, String> {
        self.reader.lock().unwrap().get("artist_aliases").unwrap_or_default()
    }

    /// The main projects folder (or, without one, the first other folder): "is it set up".
    pub fn root(&self) -> Option<String> {
        self.roots().list.first().map(|r| r.path.clone())
    }

    pub fn roots(&self) -> store::Roots {
        store::Roots::load(&self.reader.lock().unwrap().con)
    }

    /// Replaces the main folder (kept for the old one-folder settings).
    pub fn set_root(&self, root: &str) -> Result<()> {
        if !Path::new(root).is_dir() {
            return Err(tr!("settings.folderNotFound", path = root));
        }
        self.store.lock().unwrap().set("root", &root).map_err(err)
    }

    /// Adds a projects folder: the main one if there is none, else one more, with its label.
    /// A folder inside another one (or holding one) is refused: its projects would be read
    /// twice.
    pub fn add_root(&self, path: &str) -> Result<store::Roots> {
        let p = Path::new(path.trim());
        if !p.is_dir() {
            return Err(tr!("settings.folderNotFound", path = path));
        }
        let real = p.canonicalize().map_err(err)?;
        let st = self.settings_store()?;
        let roots = store::Roots::load(&st.con);
        for r in &roots.list {
            if let Ok(o) = Path::new(&r.path).canonicalize()
                && (real.starts_with(&o) || o.starts_with(&real))
            {
                return Err(if real == o {
                    tr!("settings.alreadyListed")
                } else {
                    tr!("settings.alreadyCovered", path = r.path)
                });
            }
        }
        let clean = p.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
        if roots.main().is_none() {
            st.set("root", &clean).map_err(err)?;
        } else {
            let mut more: Vec<store::Root> = st.get("more_roots").unwrap_or_default();
            more.push(store::Root { label: roots.new_label(p), path: clean, artist: String::new() });
            st.set("more_roots", &more).map_err(err)?;
        }
        Ok(store::Roots::load(&st.con))
    }

    /// Takes a projects folder out of the list. Its projects leave the app at the next scan
    /// (nothing is touched on disk). The main folder can only go when it is the last one, so
    /// the other folders' names never change.
    pub fn remove_root(&self, path: &str) -> Result<store::Roots> {
        let st = self.settings_store()?;
        let roots = store::Roots::load(&st.con);
        let Some(r) = roots.list.iter().find(|r| r.path == path) else { return Err(tr!("settings.unknownFolder")) };
        if r.label.is_empty() {
            if roots.list.len() > 1 {
                return Err(tr!("settings.mainFolder"));
            }
            st.con.execute("delete from settings where k='root'", []).map_err(err)?;
        } else {
            let more: Vec<store::Root> =
                st.get::<Vec<store::Root>>("more_roots").unwrap_or_default().into_iter().filter(|m| m.path != path).collect();
            st.set("more_roots", &more).map_err(err)?;
        }
        Ok(store::Roots::load(&st.con))
    }

    /// The artist of every song of a projects folder that names none (empty: none). The songs
    /// are named again at once.
    pub fn set_root_artist(&self, path: &str, artist: &str) -> Result<Config> {
        {
            let st = self.settings_store()?;
            let roots = store::Roots::load(&st.con);
            let r = roots.list.iter().find(|r| r.path == path).ok_or_else(|| tr!("settings.unknownFolder"))?;
            if r.label.is_empty() {
                st.set("root_artist", &artist.trim()).map_err(err)?;
            } else {
                let more: Vec<store::Root> = st
                    .get::<Vec<store::Root>>("more_roots")
                    .unwrap_or_default()
                    .into_iter()
                    .map(|mut m| {
                        if m.path == path {
                            m.artist = artist.trim().to_string();
                        }
                        m
                    })
                    .collect();
                st.set("more_roots", &more).map_err(err)?;
            }
            let tx = st.con.unchecked_transaction().map_err(err)?;
            index::regroup(&tx).map_err(err)?;
            tx.commit().map_err(err)?;
            let _ = actions::save_choices(&st.con, &self.data_dir.join("choices.json"));
        }
        self.state.lock().unwrap().version += 1;
        self.notify();
        Ok(self.config())
    }

    /// The settings the setup and Settings pages edit.
    /// Stops the pass in progress: what it read is kept, the rest waits for the next pass
    /// (the background ones pause 30 minutes; "Analyser maintenant" goes on at once).
    pub fn cancel_scan(&self) {
        if self.state.lock().unwrap().running {
            self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            *self.stopped_at.lock().unwrap() = Some(std::time::Instant::now());
        }
    }

    /// A quick look at a folder before it is added: how many files, MIDI and audio among
    /// them, how big, and how long reading its MIDI files would take. Stops after a few
    /// seconds on a huge folder (`complete` false: at least that much).
    pub fn folder_peek(&self, path: &str) -> Result<FolderPeek> {
        let root = Path::new(path.trim());
        if !root.is_dir() {
            return Err(tr!("settings.folderNotFound", path = path));
        }
        let t = std::time::Instant::now();
        let mut p = FolderPeek::default();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            if t.elapsed() > Duration::from_secs(6) {
                p.complete = false;
                break;
            }
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    stack.push(e.path());
                    continue;
                }
                p.files += 1;
                let low = e.file_name().to_string_lossy().to_lowercase();
                if low.ends_with(".mid") || low.ends_with(".midi") {
                    p.midi += 1;
                    p.midi_bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                } else if [".wav", ".aif", ".aiff", ".mp3", ".flac", ".ogg"].iter().any(|x| low.ends_with(x)) {
                    p.audio += 1;
                }
            }
            p.complete = true;
        }
        // measured: ~7,000 MIDI files a second on an 8-core desktop; a slower machine, a
        // network or USB drive: counted at 1,500
        p.seconds = (p.midi as f64 / 1500.0).ceil() as u64;
        Ok(p)
    }

    fn setup_done(&self) -> bool {
        self.reader.lock().unwrap().get::<bool>("setup_done").unwrap_or(false)
    }

    /// A connection of its own for a setting: a scan holds the main one for its whole pass
    /// (minutes on a first analysis), and a setting must not wait for it. SQLite lets this
    /// one write between the scan's writes.
    fn settings_store(&self) -> Result<Store> {
        let st = Store::open(&self.data_dir.join("plugin-fixer.sqlite")).map_err(err)?;
        st.con.busy_timeout(Duration::from_secs(30)).map_err(err)?;
        Ok(st)
    }

    pub fn config(&self) -> Config {
        let st = self.reader.lock().unwrap();
        let roots = store::Roots::load(&st.con).list;
        Config {
            // a base set up before the setup existed (a folder already chosen) counts as done
            setup_done: st.get("setup_done").unwrap_or(!roots.is_empty()),
            roots,
            sample_dirs: st.get("sample_dirs").unwrap_or_default(),
            midi_dirs: st.get("midi_dirs").unwrap_or_default(),
            old_drive: st.get("old_drive").unwrap_or_default(),
            artists: st.get("artists").unwrap_or_default(),
            artist_aliases: st.get("artist_aliases").unwrap_or_default(),
            artist_feats: st.get("artist_feats").unwrap_or_default(),
            artist_ignored: st.get("artist_ignored").unwrap_or_default(),
            work_words: st.get("work_words").unwrap_or_default(),
            live_db: plugins::live_db().is_file(),
            physical: st.get("physical").unwrap_or(false),
            lang: st.get("lang").unwrap_or_default(),
            auto_analysis: st.get("auto_analysis").unwrap_or(true),
            check_presets: st.get("check_presets").unwrap_or(serde_json::Value::Null),
        }
    }

    /// One setting of the setup / Settings pages. Sample folders: the sample index is built
    /// again at the next scan.
    pub fn set_config(&self, key: &str, value: serde_json::Value) -> Result<Config> {
        if !["sample_dirs", "midi_dirs", "old_drive", "artists", "work_words", "setup_done", "physical", "lang", "auto_analysis", "check_presets"].contains(&key) {
            return Err(tr!("settings.unknownKey", key = key));
        }
        if (key == "sample_dirs" || key == "midi_dirs")
            && let Some(bad) = value.as_array().into_iter().flatten().filter_map(|v| v.as_str()).find(|d| !Path::new(d).is_dir())
        {
            return Err(tr!("settings.folderNotFound", path = bad));
        }
        {
            let st = self.settings_store()?;
            st.set(key, &value).map_err(err)?;
            let _ = actions::save_choices(&st.con, &self.data_dir.join("choices.json"));
        }
        if key == "lang" {
            i18n::set_lang(value.as_str().unwrap_or("fr"));
        }
        if key == "sample_dirs" {
            self.samples_stale.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        // artists and working words change how songs are named and grouped
        if key == "artists" || key == "work_words" {
            let st = self.settings_store()?;
            let tx = st.con.unchecked_transaction().map_err(err)?;
            index::regroup(&tx).map_err(err)?;
            tx.commit().map_err(err)?;
            drop(st);
            self.state.lock().unwrap().version += 1;
            self.notify();
        }
        Ok(self.config())
    }

    /// Names that look like artists or aliases, found in the projects themselves. A name is
    /// a candidate when a folder holding several songs carries it ("- STUDIO NORTH - ODA KEEL",
    /// "- VLT & RNO"), or when several file names do ("Velveteer - Asset v3"). It is
    /// `sure` when it shows in BOTH: in at least 3 file names and in at least 2 different
    /// folder names. A song title has one folder; a category ("Serious", "Pop tunes") is in
    /// no file name. Measured on the reference corpus: the sure ones are the real aliases.
    pub fn artist_suggestions(&self) -> Result<Vec<ArtistGuess>> {
        let st = self.reader.lock().unwrap();
        let roots = store::Roots::load(&st.con);
        // the names already given: the artists list, and each projects folder's artist
        let known: Vec<String> = st
            .get::<Vec<String>>("artists")
            .unwrap_or_default()
            .iter()
            .chain(roots.list.iter().map(|r| &r.artist))
            .chain(st.get::<std::collections::BTreeMap<String, String>>("artist_aliases").unwrap_or_default().keys())
            .chain(st.get::<std::collections::BTreeMap<String, String>>("artist_feats").unwrap_or_default().keys())
            .chain(st.get::<Vec<String>>("artist_ignored").unwrap_or_default().iter())
            .filter(|a| !a.trim().is_empty())
            .map(|a| norm(a))
            .collect();
        let (dirs, titles): (Vec<String>, std::collections::HashSet<String>) = {
            let mut q = st.con.prepare("select song_dir, title from songs where gone=0").map_err(err)?;
            let rows: Vec<(Option<String>, Option<String>)> =
                q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?.flatten().collect();
            (rows.iter().filter_map(|r| r.0.clone()).collect(), rows.iter().filter_map(|r| r.1.as_deref().map(norm)).collect())
        };
        let (rels, song_of): (Vec<String>, Vec<Option<i64>>) = {
            let mut q = st.con.prepare("select rel, song_id from files").map_err(err)?;
            q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))).map_err(err)?.flatten().unzip()
        };
        let inner = |d: &str| roots.split(d).map(|(_, i)| i.to_string()).unwrap_or_else(|| d.to_string());
        // song folders under each folder: a song's own folder holding its variants ("388
        // 80's Fight" with three takes) names a song, not an artist
        let mut below: std::collections::HashMap<String, std::collections::HashSet<String>> = Default::default();
        for d in &dirs {
            let i = inner(d);
            let parts: Vec<&str> = i.split('/').filter(|p| !p.is_empty()).collect();
            for k in 1..parts.len() {
                below.entry(parts[..k].join("/")).or_default().insert(i.clone());
            }
        }
        let under: std::collections::HashMap<String, i64> = below.into_iter().map(|(k, v)| (k, v.len() as i64)).collect();
        let split = regex_lite_split();
        let skip = |f: &str| catalog::status_hint(&[f]).is_some() || catalog::is_project_dir(f) || catalog::is_version_dir(f);
        let ok = |low: &str| {
            low.chars().filter(|c| c.is_alphabetic()).count() >= 2
                && !GENERIC_FOLDERS.contains(&low)
                && !catalog::is_style_name(low)
                && !known.iter().any(|k| k == low)
                && !low.split(' ').all(|w| w.len() <= 2 || w.chars().all(|c| c.is_ascii_digit()))
        };
        let mut cand: std::collections::HashMap<String, String> = Default::default();
        for (path, n) in &under {
            let folder = path.rsplit('/').next().unwrap_or("");
            if *n >= 2 && !skip(folder) {
                for part in split(folder) {
                    let low = norm(&part);
                    if ok(&low) {
                        cand.entry(low).or_insert(part);
                    }
                }
            }
        }
        // file names: "<name> - ..." parts that come back in several files
        let stems: Vec<String> = rels
            .iter()
            .map(|r| norm(r.rsplit('/').next().unwrap_or("").trim_end_matches(".als")))
            .collect();
        let mut in_stems: std::collections::HashMap<String, (String, usize)> = Default::default();
        for r in &rels {
            let stem = r.rsplit('/').next().unwrap_or("").trim_end_matches(".als");
            let parts = split(stem);
            if parts.len() < 2 {
                continue;
            }
            for part in parts {
                let low = norm(&part);
                if ok(&low) {
                    in_stems.entry(low).or_insert((part, 0)).1 += 1;
                }
            }
        }
        for (low, (part, n)) in in_stems {
            if n >= 3 {
                cand.entry(low).or_insert(part);
            }
        }
        // every folder name, once
        let folder_names: std::collections::HashSet<String> = rels
            .iter()
            .flat_map(|r| {
                let i = inner(r);
                let mut v: Vec<String> = i.split('/').map(norm).collect();
                v.pop();
                v
            })
            .collect();
        let has = |hay: &str, low: &str| format!(" {hay} ").contains(&format!(" {low} "));
        let mut out: Vec<ArtistGuess> = cand
            .into_iter()
            .map(|(low, name)| {
                let hits: Vec<usize> = (0..stems.len()).filter(|&i| has(&stems[i], &low)).collect();
                let files = hits.len() as i64;
                // a song title with many versions is in many file names, but of ONE song
                let in_songs = hits.iter().filter_map(|&i| song_of[i]).collect::<std::collections::HashSet<_>>().len();
                let folders = folder_names.iter().filter(|f| has(f, &low)).count() as i64;
                let songs = under
                    .iter()
                    .filter(|(p, _)| has(&norm(p.rsplit('/').next().unwrap_or("")), &low))
                    .map(|(_, n)| *n)
                    .max()
                    .unwrap_or(0);
                let g = ArtistGuess {
                    name: display_name(&name),
                    songs,
                    files,
                    folders,
                    // a song's own title ("Lantern", "Driftwood") is not an artist
                    sure: files >= 3 && in_songs >= 3 && folders >= 2 && songs >= 1 && !titles.contains(&low),
                };
                (low, g)
            })
            .filter(|(_, g)| g.sure || g.songs >= 2)
            .map(|(_, g)| g)
            .collect();
        // "Sam" inside "Sam Rivera": only the longer name is ticked in advance
        let sure: Vec<String> = out.iter().filter(|g| g.sure).map(|g| norm(&g.name)).collect();
        for g in out.iter_mut() {
            let n = norm(&g.name);
            if g.sure && sure.iter().any(|o| o != &n && has(o, &n)) {
                g.sure = false;
            }
        }
        out.sort_by(|a, b| b.sure.cmp(&a.sure).then((b.files + b.songs).cmp(&(a.files + a.songs))).then(a.name.cmp(&b.name)));
        out.truncate(40);
        Ok(out)
    }
}

fn setting_tx<T: serde::de::DeserializeOwned>(tx: &store::rusqlite::Transaction, k: &str) -> Option<T> {
    let v: String = tx.query_row("select v from settings where k=?", [k], |r| r.get(0)).ok()?;
    serde_json::from_str(&v).ok()
}

/// The songs' pictures chosen by hand (song id -> file).
fn chosen_covers(tx: &store::rusqlite::Transaction) -> std::collections::HashMap<String, String> {
    tx.query_row("select v from settings where k='cover_chosen'", [], |r| r.get::<_, String>(0)).ok().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
}

/// After an undo or a redo: the songs whose chosen picture came or went get theirs again.
fn refresh_covers(tx: &store::rusqlite::Transaction, before: &std::collections::HashMap<String, String>) -> Result<()> {
    let after = chosen_covers(tx);
    let changed: std::collections::BTreeSet<&String> = before.keys().chain(after.keys()).filter(|k| before.get(*k) != after.get(*k)).collect();
    for k in changed {
        if let Ok(sid) = k.parse::<i64>() {
            index::refresh_song(tx, sid).map_err(err)?;
        }
    }
    Ok(())
}

fn set_tx<T: Serialize>(tx: &store::rusqlite::Transaction, k: &str, v: &T) -> Result<()> {
    tx.execute(
        "insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v",
        [k, &serde_json::to_string(v).map_err(err)?],
    )
    .map(|_| ())
    .map_err(err)
}

/// A name that may be an artist or an alias (see `App::artist_suggestions`).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ArtistGuess {
    pub name: String,
    /// Songs in the biggest folder carrying it.
    #[ts(type = "number")]
    pub songs: i64,
    /// Project files whose name carries it.
    #[ts(type = "number")]
    pub files: i64,
    /// Different folder names carrying it.
    #[ts(type = "number")]
    pub folders: i64,
    /// In file names AND in several folders: ticked in advance.
    pub sure: bool,
}

const GENERIC_FOLDERS: [&str; 43] = [
    "archive", "archives", "old", "vrac", "trash", "remix", "remixes", "collab", "collabs", "templates", "template",
    "stems", "released", "release", "lp", "ep", "album", "projets", "projects", "ableton", "musique", "music",
    "sketches", "idees", "ideas", "divers", "misc", "wip", "en cours", "demos", "demo", "cover", "covers",
    "artist templates", "master", "mix", "version", "final", "test", "new", "intro", "outro", "edit",
];

/// Lower case, accents and punctuation out, single spaces: how names are compared.
fn norm(s: &str) -> String {
    catalog::ascii_low(s).split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A folder name cut into the names it may hold: " - ", "&", ",", " x ", "feat".
fn regex_lite_split() -> impl Fn(&str) -> Vec<String> {
    |name: &str| {
        let mut s = format!(" {} ", name.replace(['_', '&', ',', '+'], " - "));
        for w in [" feat. ", " feat ", " ft. ", " ft ", " x ", " vs "] {
            s = s.replace(w, " - ").replace(&w.to_uppercase(), " - ");
        }
        s.split(" - ")
            .map(|p| p.trim_matches(|c: char| c == '-' || c == ' ' || c == '.').to_string())
            .filter(|p| !p.is_empty())
            .collect()
    }
}

/// "ODA KEEL" -> "Oda Keel" (a name written all in capitals); any other case is kept.
fn display_name(n: &str) -> String {
    if n.chars().any(|c| c.is_lowercase()) {
        return n.to_string();
    }
    n.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Keeps the `keep` most recent reset backups (a base and its companions share one stamp).
fn prune_backups(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut stamps: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_prefix("plugin-fixer-").and_then(|s| s.strip_suffix(".sqlite").or_else(|| s.strip_suffix(".sqlite.gz"))).map(str::to_string)
        })
        .collect();
    stamps.sort();
    let old = stamps.len().saturating_sub(keep);
    for s in &stamps[..old] {
        for f in [
            format!("plugin-fixer-{s}.sqlite"),
            format!("plugin-fixer-{s}.sqlite.gz"),
            format!("plugin-fixer-{s}.sqlite-wal"),
            format!("plugin-fixer-{s}.sqlite-shm"),
            format!("choices-{s}.json"),
        ] {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
}

/// What a scan reads: a change to anything else (mood board pictures, Live's temp files) does
/// not start one.
fn watched(p: &Path) -> bool {
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    let in_backup = p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("Backup"));
    !in_backup && matches!(ext.as_str(), "als" | "wav" | "mp3" | "aif" | "aiff" | "flac" | "m4a" | "ogg")
}

fn watch(root: &str, dirty: Arc<Mutex<Option<std::time::Instant>>>) -> Option<notify::RecommendedWatcher> {
    use notify::{RecursiveMode, Watcher};
    let mut w = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
        if let Ok(ev) = ev
            && !ev.kind.is_access()
            && ev.paths.iter().any(|p| watched(p))
        {
            *dirty.lock().unwrap() = Some(std::time::Instant::now());
        }
    })
    .ok()?;
    w.watch(Path::new(root), RecursiveMode::Recursive).ok()?;
    Some(w)
}

/// A timed comment on a song's render: "the drop is empty at 1:32".
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub struct Comment {
    /// 0 for a new one.
    #[ts(type = "number")]
    #[serde(default)]
    pub id: i64,
    /// The render it was made on (relative to the projects).
    #[serde(default)]
    pub rel: String,
    /// Seconds into it.
    #[serde(default)]
    pub at: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub created: f64,
}

/// "song_dir|vkey": how a song is named in the settings that follow it across rebuilds.
fn song_key(con: &store::rusqlite::Connection, sid: i64) -> Result<String> {
    con.query_row("select coalesce(song_dir,'') || '|' || coalesce(vkey,'') from songs where id=?", [sid], |r| r.get(0))
        .map_err(|_| tr!("err.songNotFound"))
}

#[cfg(test)]
mod stale_wal_tests {
    /// A sound journal left by a crash is kept (its commits are not lost).
    #[test]
    fn a_drop_links_what_is_in_the_projects_and_copies_the_rest() {
        let base = std::env::temp_dir().join(format!("pf-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let proj = base.join("proj");
        std::fs::create_dir_all(proj.join("Song A").join("pics")).unwrap();
        std::fs::create_dir_all(base.join("outside")).unwrap();
        std::fs::write(proj.join("Song A").join("pics").join("inside.png"), "img").unwrap();
        std::fs::write(base.join("outside").join("ext.jpg"), "img2").unwrap();
        std::fs::write(base.join("outside").join("tool.exe"), "exe").unwrap();
        let app = crate::App::open(&base.join("data")).unwrap();
        {
            let st = app.store.lock().unwrap();
            st.set("root", &proj.to_string_lossy().to_string()).unwrap();
            st.con.execute("insert into songs(id, song_dir, vkey, title, gone, archived) values(1, 'Song A', 'song a', 'Song A', 0, 0)", []).unwrap();
        }
        let s = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
        let r = app
            .board_drop(1, vec![s(proj.join("Song A").join("pics").join("inside.png")), s(base.join("outside").join("ext.jpg")), s(base.join("outside").join("tool.exe"))])
            .unwrap();
        assert_eq!((r.linked.as_slice(), r.copied.as_slice(), r.refused.as_slice()), (&["inside.png".to_string()][..], &["ext.jpg".to_string()][..], &["tool.exe".to_string()][..]));
        let board = app.board_dir_of(1).unwrap();
        assert!(board.join("ext.jpg").exists(), "the outside file copied onto the board");
        assert!(!board.join("inside.png").exists(), "the projects' file is not copied");
        assert!(base.join("outside").join("ext.jpg").exists() && proj.join("Song A").join("pics").join("inside.png").exists(), "the originals stay");
        drop(app);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn comments_are_added_ticked_removed_and_undone() {
        let d = std::env::temp_dir().join(format!("pf-comments-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = crate::App::open(&d).unwrap();
        app.store.lock().unwrap().con.execute_batch("insert into songs(id, song_dir, vkey, title, gone, archived, user_fields) values (1, 'A', 'a', 'A', 0, 0, '[]');").unwrap();
        let c = |at: f64, text: &str| crate::Comment { rel: "A/a.wav".into(), at, text: text.into(), ..Default::default() };
        app.comment_edit(1, c(92.5, "  drop trop vide "), false).unwrap();
        app.comment_edit(1, c(10.0, "intro"), false).unwrap();
        assert!(app.comment_edit(1, c(1.0, "   "), false).is_err(), "an empty comment is refused");
        let v = app.comments(1).unwrap();
        assert_eq!(v.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["intro", "drop trop vide"], "in time order, trimmed");
        let mut first = v[0].clone();
        first.done = true;
        app.comment_edit(1, first.clone(), false).unwrap();
        assert!(app.comments(1).unwrap()[0].done);
        app.comment_edit(1, first, true).unwrap();
        assert_eq!(app.comments(1).unwrap().len(), 1);
        app.undo().unwrap();
        assert_eq!(app.comments(1).unwrap().len(), 2, "Ctrl+Z puts it back");
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn merging_artists_renames_everywhere_and_undoes_in_one_go() {
        let d = std::env::temp_dir().join(format!("pf-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = crate::App::open(&d).unwrap();
        {
            let st = app.store.lock().unwrap();
            st.set("artists", &vec!["MosMarni".to_string(), "Madeby".to_string()]).unwrap();
            st.con.execute_batch(
                "insert into songs(id, song_dir, vkey, title, artist, ft, gone, archived, user_fields) values
                   (1, 'A', 'a', 'A', 'MosMarni', null, 0, 0, '[\"artist\"]'),
                   (2, 'B', 'b', 'B', 'Madeby', 'mosmarni, Teo', 0, 0, '[]'),
                   (3, 'C', 'c', 'C', 'Teo', null, 0, 0, '[]');",
            )
            .unwrap();
        }
        app.merge_artists("MosMarni", "Mos Marni").unwrap();
        let q = |sql: &str| app.reader.lock().unwrap().con.query_row(sql, [], |r| r.get::<_, Option<String>>(0)).unwrap();
        assert_eq!(q("select artist from songs where id=1").as_deref(), Some("Mos Marni"));
        assert_eq!(q("select ft from songs where id=2").as_deref(), Some("Mos Marni, Teo"));
        assert_eq!(app.artist_aliases().get("MosMarni").map(String::as_str), Some("Mos Marni"));
        let list: Vec<String> = app.reader.lock().unwrap().get("artists").unwrap();
        assert_eq!(list, ["Madeby", "Mos Marni"]);
        // one Ctrl+Z: the songs and the settings as they were
        app.undo().unwrap();
        assert_eq!(q("select artist from songs where id=1").as_deref(), Some("MosMarni"));
        assert_eq!(q("select ft from songs where id=2").as_deref(), Some("mosmarni, Teo"));
        assert!(app.artist_aliases().is_empty());
        // Ctrl+Y: the merge again, songs and settings; Ctrl+Z takes it back once more
        assert_eq!(app.undo_state().unwrap().redo.is_some(), true);
        app.redo().unwrap();
        assert_eq!(q("select artist from songs where id=1").as_deref(), Some("Mos Marni"));
        assert_eq!(q("select ft from songs where id=2").as_deref(), Some("Mos Marni, Teo"));
        assert_eq!(app.artist_aliases().get("MosMarni").map(String::as_str), Some("Mos Marni"));
        assert!(app.undo_state().unwrap().redo.is_none());
        app.undo().unwrap();
        assert_eq!(q("select artist from songs where id=1").as_deref(), Some("MosMarni"));
        // a new change: nothing left to redo
        app.song_edit(3, serde_json::from_value(serde_json::json!({"title": "C2"})).unwrap()).unwrap();
        assert!(app.undo_state().unwrap().redo.is_none());
        assert!(app.merge_artists("Teo", "teo").is_err(), "the same name is not a merge");
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_pages_cache_follows_every_write() {
        let d = std::env::temp_dir().join(format!("pf-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = crate::App::open(&d).unwrap();
        let a = app.overview().unwrap();
        // nothing written: the same view, from the cache
        assert_eq!(format!("{:?}", app.overview().unwrap()), format!("{a:?}"));
        let v0 = app.views.lock().unwrap().at;
        app.set_config("artists", serde_json::json!(["Oda Keel"])).unwrap();
        let _ = app.overview().unwrap();
        assert_ne!(app.views.lock().unwrap().at, v0, "a change moves the catalogue's version: computed anew");
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn audio_analysis_is_on_by_default_and_a_no_stays_a_no() {
        let d = std::env::temp_dir().join(format!("pf-audio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = crate::App::open(&d).unwrap();
        assert!(app.audio_enabled(), "on for a new catalogue");
        app.audio_enable(false).unwrap();
        drop(app);
        let app = crate::App::open(&d).unwrap();
        assert!(!app.audio_enabled(), "the user's off survives a restart");
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_sample_plays_only_from_a_library_folder() {
        let d = std::env::temp_dir().join(format!("pf-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let lib = d.join("Samples");
        std::fs::create_dir_all(lib.join("Pack")).unwrap();
        std::fs::write(lib.join("Pack").join("Kick 1.wav"), "x").unwrap();
        std::fs::write(d.join("secret.wav"), "x").unwrap();
        let app = crate::App::open(&d.join("data")).unwrap();
        app.store.lock().unwrap().set("sample_dirs", &vec![lib.to_string_lossy().to_string()]).unwrap();
        let s = |p: std::path::PathBuf| p.to_string_lossy().replace('\\', "/");
        assert!(app.media_path(&s(lib.join("Pack").join("Kick 1.wav"))).is_some());
        assert!(app.media_path(&s(d.join("secret.wav"))).is_none(), "outside the library");
        assert!(app.media_path(&s(lib.join("..").join("secret.wav"))).is_none(), "no climbing out");
        assert!(app.library_path(&s(lib.join("Pack").join("Kick 1.wav"))).is_some());
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn trimming_keeps_the_newest_copy_and_only_touches_backups() {
        let d = std::env::temp_dir().join(format!("pf-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let app = crate::App::open(&d).unwrap();
        let b = d.join("backups");
        std::fs::create_dir_all(b.join("journal-2026-01-01_10h00m00")).unwrap();
        std::fs::write(b.join("journal-2026-01-01_10h00m00").join("plugin-fixer.sqlite-wal"), "j").unwrap();
        let put = |n: &str, age: u64| {
            std::fs::write(b.join(n), vec![0u8; 1000]).unwrap();
            let t = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
            std::fs::File::options().write(true).open(b.join(n)).unwrap().set_modified(t).unwrap();
        };
        put("before-v3-1.sqlite", 9000);
        put("plugin-fixer-2026-02-01_10h00m00.sqlite", 5000);
        put("plugin-fixer-2026-02-01_10h00m00.sqlite-wal", 5000);
        put("choices-2026-02-01_10h00m00.json", 5000);
        put("before-v4-2.sqlite", 10);
        // a file of the user's that strayed in: not a copy, never removed
        put("notes.txt", 20000);
        let before = app.storage();
        assert_eq!(before.backup_count, 4);
        assert!(before.backups >= 5000);
        assert_eq!(before.trimmable, before.backups - 1000 - 1000, "all but the newest copy and the stray file");
        let after = app.trim_backups().unwrap();
        assert_eq!(after.backup_count, 1);
        let mut left: Vec<String> = std::fs::read_dir(&b).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["before-v4-2.sqlite", "notes.txt"]);
        assert!(d.join("plugin-fixer.sqlite").exists(), "the base in use is not touched");
        assert!(after.total < before.total);
        drop(app);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_sound_journal_is_kept() {
        let d = std::env::temp_dir().join(format!("pf-wal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("plugin-fixer.sqlite");
        let st = store::Store::open(&p).unwrap();
        st.con.execute_batch("pragma wal_autocheckpoint=0; create table t(x); insert into t values (42);").unwrap();
        // a crash: the journal stays (copied aside before the clean close removes it)
        std::fs::copy(d.join("plugin-fixer.sqlite-wal"), d.join("keep-wal")).unwrap();
        let d2 = d.join("b");
        std::fs::create_dir_all(&d2).unwrap();
        std::fs::copy(&p, d2.join("plugin-fixer.sqlite")).unwrap();
        std::fs::copy(d.join("keep-wal"), d2.join("plugin-fixer.sqlite-wal")).unwrap();
        // closed only once the crashed state is copied (it was forgotten instead, which kept
        // the file open until the end and left the folder behind in Temp at every run)
        drop(st);
        assert!(super::journal_fits(&d2.join("plugin-fixer.sqlite")));
        let st = super::open_or_drop_stale_wal(&d2, &d2.join("plugin-fixer.sqlite")).unwrap();
        let x: i64 = st.con.query_row("select x from t", [], |r| r.get(0)).unwrap();
        assert_eq!(x, 42);
        assert!(!d2.join("backups").exists());
        drop(st);
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The window stays quick during a full scan. Not run by default: it needs a copy of a real
/// data folder (SPOOL_BENCH_DATA, never the user's own) and reads the projects it names.
///   SPOOL_BENCH_DATA=<copy> cargo test --release -p core --lib responsive -- --ignored --nocapture
#[cfg(test)]
mod responsive_bench {
    #[test]
    #[ignore]
    fn the_pages_answer_during_a_full_scan() {
        let Some(d) = std::env::var_os("SPOOL_BENCH_DATA") else { return };
        let app = crate::App::open(std::path::Path::new(&d)).unwrap();
        let a = std::sync::Arc::clone(&app);
        let t0 = std::time::Instant::now();
        let scan = std::thread::spawn(move || a.scan(true));
        let mut worst = [0f64; 3];
        let mut n = 0;
        while !scan.is_finished() {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let t = std::time::Instant::now();
            let _ = app.songs().unwrap();
            let _ = app.overview().unwrap();
            let read = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let _ = app.releases().unwrap();
            let rel = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let r = crate::releases::Release { id: String::new(), title: format!("bench {n}"), kind: "ep".into(), artist: String::new(), date: None, cover_song: None, cover: None, notes: String::new(), songs: vec![], folder: None, created: 0.0, board: vec![] };
            let _ = app.release_save(r).unwrap();
            let save = t.elapsed().as_secs_f64();
            if save > 1.0 || read > 1.5 {
                let s = app.scan_state();
                eprintln!("{:>5.0}s SLOW pages {read:.2}s save {save:.2}s during: {} {}/{}", t0.elapsed().as_secs_f64(), s.stage, s.done, s.total);
            }
            for (w, x) in worst.iter_mut().zip([read, rel, save]) {
                *w = w.max(x);
            }
            n += 1;
            if n % 20 == 0 {
                let s = app.scan_state();
                eprintln!("{:>5.0}s {} {}/{}  worst so far: pages {:.2}s, releases {:.2}s, save {:.2}s", t0.elapsed().as_secs_f64(), s.stage, s.done, s.total, worst[0], worst[1], worst[2]);
            }
        }
        scan.join().unwrap().unwrap();
        eprintln!("scan {:.0}s, {n} rounds; worst: pages {:.2}s, releases {:.2}s, save {:.2}s", t0.elapsed().as_secs_f64(), worst[0], worst[1], worst[2]);
    }
}

/// The last arrangements read (a song's sheet shows one), kept while their set is unchanged.
static KEPT: std::sync::Mutex<Vec<(PathBuf, std::time::SystemTime, std::sync::Arc<als::Arrangement>)>> = std::sync::Mutex::new(Vec::new());

/// A sample an arrangement just read plays (the project's own audio, wherever it lies on
/// this machine): heard by "Play the project", read only.
fn in_kept_arrangement(p: &Path) -> bool {
    let Ok(real) = p.canonicalize() else { return false };
    let kept = KEPT.lock().unwrap();
    kept.iter()
        .flat_map(|(_, _, a)| {
            // the audio clips' samples, and those the instruments play (a Simpler's, a Drum Rack's pads)
            a.tracks.iter().flat_map(|t| {
                t.clips.iter().filter_map(|c| c.sample.as_deref()).chain(t.sound.iter().flat_map(|s| s.zones.iter().map(|z| z.path.as_str())))
            })
        })
        .any(|s| Path::new(s).canonicalize().is_ok_and(|r| r == real))
}

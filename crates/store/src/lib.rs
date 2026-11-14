//! The app's base (SQLite): numbered schema migrations, JSON settings, and the import of the
//! Python prototype's base so nothing the user decided is lost in the move.
//!
//! The tables keep the prototype's shape: song rows with the fields the user edited listed
//! in `user_fields` (a scan never overwrites those), files with the reader's output as JSON,
//! suggestions with their state, the undo journal.
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub use rusqlite;

mod roots;
pub use roots::{Root, Roots};

/// Each entry moves the base one version up. Never edit a shipped one: add the next.
const MIGRATIONS: &[&str] = &[
    // 1: the prototype's schema
    r#"
    create table files (
      rel text primary key,            -- path relative to the root, forward slashes
      song_id integer,
      size integer, mtime real, ctime real,
      song_dir text, vkey text,
      in_version integer default 0,    -- superseded (old folder, replaced by a repaired copy)
      promoted integer default 0,      -- newest version of a folder with no active file
      archived integer default 0,
      data text,                       -- the reader's output, JSON
      plugstate text,                  -- per plugin reference: status, candidates, old (JSON)
      sev integer default 0, read_at real
    );
    create index files_song on files(song_id);
    create table songs (
      id integer primary key autoincrement,
      song_dir text, vkey text,
      title text, artist text, style text, key text, catalog text, ft text, folder_date text,
      stage text, stage_source text,
      temoin text, temoin_source text,
      notes text, notion text,
      user_fields text default '[]',   -- fields the user edited: never overwritten by a scan
      archived integer default 0, gone integer default 0,
      created real, updated real,
      checklist text, checkauto text, tempo real, date text, cover text
    );
    create unique index songs_key on songs(song_dir, vkey) where gone = 0;
    create table suggestions (
      id integer primary key autoincrement,
      song_id integer, kind text, value text, reasons text, sig text,
      state text default 'open',       -- open | accepted | refused | stale
      created real, decided real
    );
    create index sugg_song on suggestions(song_id);
    create table stage_log (song_id integer, stage text, at real);
    create table scans (id integer primary key autoincrement, at real, summary text);
    create table audio (rel text primary key, size integer, mtime real);
    create table settings (k text primary key, v text);
    create table backups (dir text primary key, n integer);
    create table repairs (fixed text primary key, source text, at real, size integer, mtime real, actions text);
    create table undo_log (id integer primary key autoincrement, at real, label text, data text);
    "#,
    // 2: a song's rating (1-5 stars) and types ("Original", "Remix"..., JSON array), the
    // fields a Notion base like a label's keeps
    r#"
    alter table songs add column rating integer;
    alter table songs add column kinds text;
    "#,
    // 3: the MIDI files of the projects folders, with what each holds (midi::MidiInfo)
    r#"
    create table midi (rel text primary key, size integer, mtime real, data text);
    "#,
    // 4: the audio analysis of reference renders (opt-in, crates/audio): the CLAP embedding
    // (512 f32, little-endian), measured tempo and key, per render and model; a render whose
    // size, date or model changed is analysed again. `error` set: it could not be read.
    r#"
    create table audio_analysis (
      rel text primary key, size integer, mtime real, model text,
      embedding blob, bpm real, key text, duration real, analyzed_at real, error text
    );
    "#,
];

/// Tables carried over from the prototype, in an order that keeps ids.
const TABLES: &[&str] =
    &["settings", "songs", "files", "suggestions", "stage_log", "scans", "audio", "backups", "repairs", "undo_log"];

pub struct Store {
    pub con: Connection,
}

fn refused(msg: String) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(msg.into())
}

/// The copies made before updates: the newest `keep` stay (never the user's other files).
fn prune_before(dir: &Path, keep: usize) {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("before-v") && n.ends_with(".sqlite")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort();
    let n = v.len().saturating_sub(keep);
    for (_, p) in v.into_iter().take(n) {
        let _ = std::fs::remove_file(p);
    }
}

impl Store {
    /// Opens the base. A base made by a newer Spool is refused (an older version would not
    /// know what it holds). A base an update is about to change is copied first, whole, into
    /// "backups" next to it ("before-v12-<date>.sqlite"): an update that goes wrong can be
    /// undone by putting it back and running the previous version.
    pub fn open(path: &Path) -> rusqlite::Result<Store> {
        let con = Connection::open(path)?;
        let v: usize = con.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if v > MIGRATIONS.len() {
            return Err(refused(i18n::tr!("store.newer", v = v, max = MIGRATIONS.len())));
        }
        if v > 0 && v < MIGRATIONS.len() {
            let dir = path.parent().unwrap_or(Path::new(".")).join("backups");
            std::fs::create_dir_all(&dir).map_err(|e| refused(i18n::tr!("store.backupFailed", e = e)))?;
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let to = dir.join(format!("before-v{}-{stamp}.sqlite", MIGRATIONS.len()));
            if !to.exists() {
                // a consistent copy (the journal folded in), made by SQLite itself; without it
                // the base is not changed
                con.execute("vacuum into ?", [to.to_string_lossy()])?;
            }
            // each copy is the whole base (hundreds of MB with a large MIDI library): the two
            // newest are enough to step back over the last update
            prune_before(&dir, 2);
        }
        Self::init(con)
    }

    /// The format of the base at `path`, read without changing anything (no journal folded,
    /// nothing created): `None` when there is no base yet.
    pub fn peek(path: &Path) -> Option<usize> {
        use rusqlite::OpenFlags;
        if !path.exists() {
            return None;
        }
        let con = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()?;
        con.pragma_query_value(None, "user_version", |r| r.get(0)).ok()
    }

    /// The format this version writes (what `user_version` reaches once open).
    pub fn format() -> usize {
        MIGRATIONS.len()
    }

    pub fn memory() -> rusqlite::Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(con: Connection) -> rusqlite::Result<Store> {
        con.pragma_update(None, "journal_mode", "WAL")?;
        con.pragma_update(None, "synchronous", "NORMAL")?;
        // the journal is emptied as it goes but its file kept its largest size (43 MB seen
        // after a full scan): cut back to 8 MB at each checkpoint
        con.pragma_update(None, "journal_size_limit", 8 * 1024 * 1024)?;
        let v: usize = con.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(v) {
            let tx = con.unchecked_transaction()?;
            tx.execute_batch(m)?;
            tx.pragma_update(None, "user_version", i + 1)?;
            tx.commit()?;
        }
        Ok(Store { con })
    }

    pub fn version(&self) -> usize {
        self.con.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap_or(0)
    }

    /// A setting (stored as JSON), None when absent or unreadable.
    pub fn get<T: DeserializeOwned>(&self, k: &str) -> Option<T> {
        let v: Option<String> =
            self.con.query_row("select v from settings where k=?", [k], |r| r.get(0)).optional().ok()??;
        serde_json::from_str(&v?).ok()
    }

    pub fn set<T: Serialize>(&self, k: &str, v: &T) -> rusqlite::Result<()> {
        let s = serde_json::to_string(v).expect("serialisable setting");
        self.con.execute(
            "insert into settings(k, v) values(?, ?) on conflict(k) do update set v=excluded.v",
            params![k, s],
        )?;
        Ok(())
    }

    /// Copies the prototype's base into this (empty) base: every row of every table, ids
    /// kept. Returns the rows copied per table. Refuses a base that already has songs.
    pub fn import_prototype(&self, proto: &Path) -> rusqlite::Result<Vec<(String, usize)>> {
        let have: i64 = self.con.query_row("select count(*) from songs", [], |r| r.get(0))?;
        if have > 0 {
            return Err(rusqlite::Error::InvalidParameterName("the base already holds songs".into()));
        }
        self.con.execute("attach database ?1 as proto", [proto.to_string_lossy()])?;
        let res = (|| {
            let tx = self.con.unchecked_transaction()?;
            let mut out = Vec::new();
            for t in TABLES {
                let exists: i64 =
                    tx.query_row("select count(*) from proto.sqlite_master where type='table' and name=?", [t], |r| {
                        r.get(0)
                    })?;
                if exists == 0 {
                    continue;
                }
                let cols = |db: &str| -> rusqlite::Result<Vec<String>> {
                    let mut st = tx.prepare(&format!("pragma {db}.table_info({t})"))?;
                    st.query_map([], |r| r.get::<_, String>(1))?.collect()
                };
                let (mine, theirs) = (cols("main")?, cols("proto")?);
                let common: Vec<&String> = mine.iter().filter(|c| theirs.contains(c)).collect();
                let list = common.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
                tx.execute(&format!("delete from main.{t}"), [])?;
                let n = tx.execute(
                    &format!("insert into main.{t}({list}) select {list} from proto.{t} order by rowid"),
                    [],
                )?;
                out.push((t.to_string(), n));
            }
            tx.commit()?;
            Ok(out)
        })();
        self.con.execute("detach database proto", [])?;
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_keeps_settings() {
        let s = Store::memory().unwrap();
        assert_eq!(s.version(), MIGRATIONS.len());
        s.set("artists", &vec!["Oda Keel".to_string()]).unwrap();
        assert_eq!(s.get::<Vec<String>>("artists"), Some(vec!["Oda Keel".to_string()]));
        assert_eq!(s.get::<Vec<String>>("nope"), None);
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn an_update_copies_the_base_first_and_a_newer_base_is_refused() {
        let dir = std::env::temp_dir().join(format!("spool-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("base.sqlite");
        // a base left by an older version (two formats behind), with something in it
        {
            let con = Connection::open(&p).unwrap();
            for m in &MIGRATIONS[..MIGRATIONS.len() - 2] {
                con.execute_batch(m).unwrap();
            }
            con.pragma_update(None, "user_version", MIGRATIONS.len() - 2).unwrap();
            con.execute("insert into settings(k, v) values('root', '\"C:/x\"')", []).unwrap();
        }
        // what the start screen reads first: the format, nothing changed by reading it
        assert_eq!(Store::peek(&dir.join("none.sqlite")), None);
        assert_eq!(Store::peek(&p), Some(MIGRATIONS.len() - 2));
        assert_eq!(Store::peek(&p), Some(MIGRATIONS.len() - 2));
        assert!(!dir.join("backups").exists());
        let st = Store::open(&p).unwrap();
        assert_eq!(st.version(), MIGRATIONS.len());
        drop(st);
        assert_eq!(Store::peek(&p), Some(MIGRATIONS.len()));
        let copies: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().flatten().collect();
        assert_eq!(copies.len(), 1);
        let old = Connection::open(copies[0].path()).unwrap();
        let v: usize = old.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len() - 2, "the copy is the base as it was");
        let root: String = old.query_row("select v from settings where k='root'", [], |r| r.get(0)).unwrap();
        assert_eq!(root, "\"C:/x\"");
        drop(old);
        // up to date: no copy
        drop(Store::open(&p).unwrap());
        assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 1);
        // made by a newer Spool: refused, untouched
        Connection::open(&p).unwrap().pragma_update(None, "user_version", MIGRATIONS.len() + 1).unwrap();
        let e = Store::open(&p).err().unwrap().to_string();
        assert!(e.contains("plus récente"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Bases made by released versions must open in every later version: a user's catalogue is
/// years of their work (and one day it may sync with an account). These tests hold that line.
#[cfg(test)]
mod compat_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    /// A shipped migration is never edited: a base that already ran it would never run the
    /// new text. Add a new entry instead. (The snapshot grows at each release, never shrinks.)
    #[test]
    fn shipped_migrations_are_frozen() {
        let shipped: Vec<String> = serde_json::from_str(&std::fs::read_to_string(fixtures().join("migrations-0.1.0.json")).unwrap()).unwrap();
        assert!(MIGRATIONS.len() >= shipped.len(), "a shipped migration was removed");
        for (i, m) in shipped.iter().enumerate() {
            assert_eq!(MIGRATIONS[i], m, "migration {} changed after it shipped in 0.1.0: add a new one instead", i + 1);
        }
    }

    /// A base written by Spool 0.1.0, with a bit of everything, opens, migrates and keeps it all.
    #[test]
    fn a_base_from_0_1_0_opens_with_everything_in_it() {
        let dir = std::env::temp_dir().join(format!("spool-compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("plugin-fixer.sqlite");
        std::fs::copy(fixtures().join("base-0.1.0.sqlite"), &p).unwrap();
        let st = Store::open(&p).unwrap();
        assert_eq!(st.version(), MIGRATIONS.len());
        let n = |q: &str| -> i64 { st.con.query_row(q, [], |r| r.get(0)).unwrap() };
        assert_eq!(n("select count(*) from songs where title='Glass Hour' and stage='mix' and rating=4"), 1);
        assert_eq!(n("select count(*) from files where rel='Glass Hour Project/Glass Hour.als'"), 1);
        assert_eq!(n("select count(*) from repairs"), 1);
        assert_eq!(n("select count(*) from undo_log"), 1);
        assert_eq!(n("select count(*) from midi"), 1);
        assert_eq!(st.get::<String>("lang").as_deref(), Some("en"));
        assert_eq!(st.get::<bool>("physical"), Some(true));
        assert!(st.get::<serde_json::Value>("releases").is_some_and(|v| v.is_array()));
        drop(st);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Writes the fixtures (run once at a release: SPOOL_MAKE_FIXTURE=1 cargo test -p store make_fixture).
    #[test]
    fn make_fixture() {
        if std::env::var_os("SPOOL_MAKE_FIXTURE").is_none() {
            return;
        }
        let p = fixtures().join("base-0.1.0.sqlite");
        let _ = std::fs::remove_file(&p);
        let st = Store::open(&p).unwrap();
        st.set("root", &"C:/Music/Projects").unwrap();
        st.set("artists", &vec!["Ivo Sand"]).unwrap();
        st.set("physical", &true).unwrap();
        st.set("lang", &"en").unwrap();
        st.set("releases", &serde_json::json!([{"id": "r1", "title": "Paper Tides", "kind": "ep", "songs": ["Glass Hour Project|glass hour"]}])).unwrap();
        st.con.execute_batch(
            "insert into songs(song_dir, vkey, title, artist, style, key, stage, tempo, rating, kinds, user_fields, created, updated)
               values('Glass Hour Project', 'glass hour', 'Glass Hour', 'Ivo Sand', 'dnb', 'F#m', 'mix', 174, 4, '[\"Original\"]', '[\"stage\"]', 1700000000, 1700000000);
             insert into files(rel, song_id, size, mtime, song_dir, vkey, data, plugstate, sev)
               values('Glass Hour Project/Glass Hour.als', 1, 123456, 1700000000, 'Glass Hour Project', 'glass hour', '{\"plugins\":[]}', '[]', 0);
             insert into repairs(fixed, source, at, size, mtime, actions)
               values('Glass Hour Project/Glass Hour [R].als', 'Glass Hour Project/Glass Hour.als', 1700000100, 120000, 1700000100, '{\"codes\":\"R\"}');
             insert into undo_log(at, label, data) values(1700000200, 'Stage', '{\"kind\":\"setting\",\"k\":\"physical\",\"v\":\"false\"}');
             insert into midi(rel, size, mtime, data) values('Glass Hour Project/bass.mid', 512, 1700000000, '{}');
             insert into stage_log(song_id, stage, at) values(1, 'mix', 1700000000);",
        )
        .unwrap();
        drop(st);
        let shipped: Vec<&str> = MIGRATIONS.to_vec();
        std::fs::write(fixtures().join("migrations-0.1.0.json"), serde_json::to_string_pretty(&shipped).unwrap()).unwrap();
    }
}

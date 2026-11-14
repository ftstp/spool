//! Audio analysis of reference renders (Settings > Spool > Audio analysis).
//!
//! On by default (since 0.1.3; a user who turned it off keeps it off): once the first setup
//! is done, Spool downloads in the background, once, into `<data>/models/<MODEL_TAG>/`, the
//! CLAP audio model (LAION `larger_clap_general`, Apache-2.0, exported by
//! `tools/clap/export.py`) and Microsoft's ONNX Runtime library, every file checked against
//! a SHA-256 written below. Then, in the background, one render at a time on one low
//! priority thread, never while a scan or a repair runs, it measures each song's reference
//! render: an embedding (what the render sounds like), tempo and key, kept in the base
//! (`audio_analysis`) and computed again only when the file, its size or the model changes.
//! Nothing is sent anywhere; renders are only read.
//!
//! ONNX Runtime is not part of Spool.exe: `crates/audio` loads the downloaded library
//! only when the analysis is on and its files are there. A library that does not load
//! leaves the analysis off with the reason, and the rest of the app as it was.

use crate::{App, Result, ScanState, err, now};
use i18n::tr;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use store::rusqlite::{self, Connection, OptionalExtension, params};
use ts_rs::TS;

/// The model's folder name, also the tag of the GitHub release that hosts it.
pub const MODEL_TAG: &str = "clap-general-ada0c23";
/// Written with every analysis: rows of another model are analysed again.
pub const MODEL: &str = index::AUDIO_MODEL;
/// Where the model files are published (a release of Spool's public repository).
pub const RELEASE_URL: &str = "https://github.com/ftstp/spool/releases/download/clap-general-ada0c23/";
/// A folder holding the files already (development): read from there instead of the network.
pub const DIR_ENV: &str = "SPOOL_MODEL_DIR";

/// A file to fetch, with what it must be.
pub struct Remote {
    pub name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// From Spool's release.
pub const MODEL_FILES: [Remote; 3] = [
    Remote {
        name: "audio_model_int8.onnx",
        sha256: "3933ab99b99060bf6348e917585f71c88c86f2668680118243144bb89db396ce",
        size: 75_343_320,
    },
    Remote {
        name: "manifest.json",
        sha256: "e0d540a9844ccd9e768effa1b397bd59720be9c33be4f9d161b2feb72aae9543",
        size: 456,
    },
    Remote {
        name: "NOTICE.txt",
        sha256: "ced63e29fd8a0be4ab73f6cdb9a885754489903cd4db2344725704f7543df7f3",
        size: 959,
    },
];

/// ONNX Runtime, from Microsoft's own release (the version `ort` 2.0.0-rc.15 is built
/// for): only the library is taken out of the archive, by ranges, about 6 MB of 83.
pub const ORT_VERSION: &str = "1.31.1";
pub const ORT_ZIP_URL: &str =
    "https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-win-x64-1.31.1.zip";
pub const ORT_ZIP_SHA256: &str = "5869e26bb25314375f2d5ef1877e3ea06b1543d6c710e6e4a1ec2f7177ab4b2c";
const ORT_ZIP_ENTRY: &str = "onnxruntime-win-x64-1.31.1/lib/onnxruntime.dll";
/// Compressed size of the library inside Microsoft's archive (what is downloaded).
const ORT_COMPRESSED: u64 = 6_178_062;
pub const ORT_DLL: Remote = Remote {
    name: "onnxruntime.dll",
    sha256: "cb2fa518b18356617f6f9f460c012501da2819ec7e5c1d80bd8f5de20492db0c",
    size: 17_253_728,
};

/// Neighbours shown under "Sounds like".
pub const SIMILAR_SHOWN: usize = 5;

/// What the audio analysis is doing, for Settings and the status line.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct AudioState {
    /// The user turned it on.
    pub enabled: bool,
    /// Its files are downloaded and checked.
    pub ready: bool,
    pub paused: bool,
    pub downloading: bool,
    #[ts(type = "number")]
    pub dl_done: u64,
    #[ts(type = "number")]
    pub dl_total: u64,
    /// Analysing a render now.
    pub running: bool,
    /// Renders analysed in this round, out of `total` waiting when it began.
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    /// Renders with an analysis in the base.
    #[ts(type = "number")]
    pub analysed: u64,
    /// Reference renders the base knows.
    #[ts(type = "number")]
    pub renders: u64,
    /// The render being analysed.
    pub detail: String,
    pub error: Option<String>,
    /// About how much is downloaded when it is turned on, in MB.
    #[ts(type = "number")]
    pub download_mb: u64,
    /// What the downloaded files take on disk, in MB (0: none).
    #[ts(type = "number")]
    pub disk_mb: u64,
}

/// A song that sounds like the one shown.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SimilarSong {
    #[ts(type = "number")]
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub style: Option<String>,
    /// Cosine similarity of the two renders (about 0.5 unrelated, 0.9 and more very close).
    pub sim: f64,
}

/// What the analysis of a song's reference render says, on its sheet.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SoundInfo {
    pub rel: String,
    /// Measured tempo (folded into 80-180 bpm): the sheet shows it beside the project's
    /// when they differ by more than 3 %.
    pub bpm: Option<f64>,
    pub similar: Vec<SimilarSong>,
}

/// The analysis' share of App.
#[derive(Default)]
pub struct Sound {
    analyzer: Mutex<Option<Arc<audio::Analyzer>>>,
    /// Loading ONNX Runtime failed in this session: not tried again until turned off and on.
    load_failed: Mutex<Option<String>>,
    downloading: AtomicBool,
    stop_download: AtomicBool,
    started: AtomicBool,
}

pub fn model_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join(MODEL_TAG)
}

fn all_files() -> impl Iterator<Item = &'static Remote> {
    MODEL_FILES.iter().chain(std::iter::once(&ORT_DLL))
}

/// Every file there, at its size (hashes are checked when written and when loaded).
pub fn files_present(dir: &Path) -> bool {
    all_files().all(|f| dir.join(f.name).metadata().is_ok_and(|m| m.len() == f.size))
}

fn sha256_file(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn download_bytes() -> u64 {
    MODEL_FILES.iter().map(|f| f.size).sum::<u64>() + ORT_COMPRESSED
}

fn disk_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
}

// ------------------------------------------------------------------ download

/// Progress of a download: bytes done, bytes in all; false = stop.
type Tick<'a> = &'a dyn Fn(u64) -> bool;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .user_agent("Spool")
        .build()
        .into()
}

/// GET `url` (from `from` on, when the server allows ranges), the body appended to `out`.
/// Returns whether the server honoured the range.
fn fetch_into(
    agent: &ureq::Agent,
    url: &str,
    from: u64,
    out: &mut std::fs::File,
    done0: u64,
    tick: Tick,
) -> Result<bool> {
    let mut req = agent.get(url);
    if from > 0 {
        req = req.header("Range", &format!("bytes={from}-"));
    }
    let mut resp = req.call().map_err(|e| tr!("sound.network", e = e))?;
    let ranged = resp.status().as_u16() == 206;
    if from > 0 && !ranged {
        out.set_len(0).map_err(err)?;
    }
    out.seek(SeekFrom::End(0)).map_err(err)?;
    let mut done = if ranged { from } else { 0 };
    let mut r = resp.body_mut().as_reader();
    let mut buf = vec![0u8; 256 << 10];
    loop {
        let n = r.read(&mut buf).map_err(|e| tr!("sound.network", e = e))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(err)?;
        done += n as u64;
        if !tick(done0 + done) {
            return Err(tr!("sound.stopped"));
        }
    }
    Ok(ranged)
}

/// Bytes `start..start+len` of `url` (a ranged request; None if the server ignores ranges).
fn fetch_range(agent: &ureq::Agent, url: &str, start: u64, len: u64) -> Result<Option<Vec<u8>>> {
    let mut resp = agent
        .get(url)
        .header("Range", &format!("bytes={start}-{}", start + len - 1))
        .call()
        .map_err(|e| tr!("sound.network", e = e))?;
    if resp.status().as_u16() != 206 {
        return Ok(None);
    }
    let mut v = Vec::with_capacity(len as usize);
    resp.body_mut().as_reader().take(len).read_to_end(&mut v).map_err(|e| tr!("sound.network", e = e))?;
    Ok(Some(v))
}

/// The last `n` bytes of `url`, and its whole size (None if the server ignores ranges).
/// Asked as two explicit ranges: GitHub's file storage refuses suffix ranges (`bytes=-n`).
fn fetch_tail(agent: &ureq::Agent, url: &str, n: u64) -> Result<Option<(Vec<u8>, u64)>> {
    let resp = agent.get(url).header("Range", "bytes=0-0").call().map_err(|e| tr!("sound.network", e = e))?;
    if resp.status().as_u16() != 206 {
        return Ok(None);
    }
    let Some(total) = resp
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return Ok(None);
    };
    drop(resp);
    let n = n.min(total);
    Ok(fetch_range(agent, url, total - n, n)?.map(|v| (v, total)))
}

/// Moves `part` to `dest` if its SHA-256 is `want`; otherwise deletes it and says so.
fn accept(part: &Path, dest: &Path, f: &Remote) -> Result<()> {
    let got = sha256_file(part).map_err(err)?;
    if got != f.sha256 {
        let _ = std::fs::remove_file(part);
        return Err(tr!("sound.badHash", name = f.name));
    }
    std::fs::rename(part, dest).map_err(err)
}

fn le16(b: &[u8], i: usize) -> u64 {
    u16::from_le_bytes([b[i], b[i + 1]]) as u64
}

fn le32(b: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64
}

/// (local header offset, compressed size, method) of `name` in a zip's central directory.
fn zip_entry(cd: &[u8], name: &str) -> Option<(u64, u64, u64)> {
    let mut i = 0;
    while i + 46 <= cd.len() && le32(cd, i) == 0x0201_4b50 {
        let (method, comp) = (le16(cd, i + 10), le32(cd, i + 20));
        let (nlen, xlen, clen) = (le16(cd, i + 28) as usize, le16(cd, i + 30) as usize, le16(cd, i + 32) as usize);
        let off = le32(cd, i + 42);
        if cd.get(i + 46..i + 46 + nlen) == Some(name.as_bytes()) {
            return Some((off, comp, method));
        }
        i += 46 + nlen + xlen + clen;
    }
    None
}

/// The central directory's (offset, size), from the archive's last bytes.
fn zip_directory(tail: &[u8]) -> Option<(u64, u64)> {
    let i = (0..tail.len().saturating_sub(21)).rev().find(|&i| le32(tail, i) == 0x0605_4b50)?;
    Some((le32(tail, i + 16), le32(tail, i + 12)))
}

/// Inflates one stored entry (`data` starts at its local header) into `out`.
fn unzip_entry(data: &[u8], comp: u64, method: u64, out: &Path) -> Result<()> {
    if data.len() < 30 || le32(data, 0) != 0x0403_4b50 {
        return Err(tr!("sound.badArchive"));
    }
    let start = 30 + le16(data, 26) as usize + le16(data, 28) as usize;
    let body = data.get(start..start + comp as usize).ok_or_else(|| tr!("sound.badArchive"))?;
    let mut f = std::fs::File::create(out).map_err(err)?;
    match method {
        8 => {
            let mut d = flate2::read::DeflateDecoder::new(body);
            std::io::copy(&mut d, &mut f).map_err(|_| tr!("sound.badArchive"))?;
        }
        0 => f.write_all(body).map_err(err)?,
        _ => return Err(tr!("sound.badArchive")),
    }
    Ok(())
}

/// ONNX Runtime's library out of Microsoft's archive: by ranges when the server allows it
/// (about 6 MB), else the whole archive (checked against its own SHA-256) then taken out.
fn fetch_runtime(agent: &ureq::Agent, dir: &Path, done0: u64, tick: Tick) -> Result<()> {
    let part = dir.join("onnxruntime.dll.part");
    let dest = dir.join(ORT_DLL.name);
    if let Some((tail, total)) = fetch_tail(agent, ORT_ZIP_URL, 64 << 10)? {
        let (cd_off, cd_len) = zip_directory(&tail).ok_or_else(|| tr!("sound.badArchive"))?;
        let tail_start = total - tail.len() as u64;
        let cd = if cd_off >= tail_start {
            tail[(cd_off - tail_start) as usize..].to_vec()
        } else {
            fetch_range(agent, ORT_ZIP_URL, cd_off, cd_len)?.ok_or_else(|| tr!("sound.badArchive"))?
        };
        let (off, comp, method) = zip_entry(&cd, ORT_ZIP_ENTRY).ok_or_else(|| tr!("sound.badArchive"))?;
        // the local header's name and extra field: 30 bytes + a margin
        let want = 30 + ORT_ZIP_ENTRY.len() as u64 + 1024 + comp;
        let mut data = Vec::with_capacity(want as usize);
        let mut at = off;
        while (data.len() as u64) < want && at < total {
            let n = (4u64 << 20).min(want - data.len() as u64).min(total - at);
            let chunk = fetch_range(agent, ORT_ZIP_URL, at, n)?.ok_or_else(|| tr!("sound.badArchive"))?;
            at += chunk.len() as u64;
            data.extend_from_slice(&chunk);
            if !tick(done0 + data.len() as u64) {
                return Err(tr!("sound.stopped"));
            }
        }
        unzip_entry(&data, comp, method, &part)?;
        return accept(&part, &dest, &ORT_DLL);
    }
    // no ranges: the whole archive
    let zpart = dir.join("onnxruntime.zip.part");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&zpart).map_err(err)?;
    let have = f.metadata().map(|m| m.len()).unwrap_or(0);
    fetch_into(agent, ORT_ZIP_URL, have, &mut f, done0, tick)?;
    drop(f);
    if sha256_file(&zpart).map_err(err)? != ORT_ZIP_SHA256 {
        let _ = std::fs::remove_file(&zpart);
        return Err(tr!("sound.badHash", name = "onnxruntime"));
    }
    let zip = std::fs::read(&zpart).map_err(err)?;
    let (cd_off, _) =
        zip_directory(&zip[zip.len().saturating_sub(64 << 10)..]).ok_or_else(|| tr!("sound.badArchive"))?;
    let (off, comp, method) =
        zip_entry(&zip[cd_off as usize..], ORT_ZIP_ENTRY).ok_or_else(|| tr!("sound.badArchive"))?;
    unzip_entry(&zip[off as usize..], comp, method, &part)?;
    let _ = std::fs::remove_file(&zpart);
    accept(&part, &dest, &ORT_DLL)
}

/// Fetches what is missing into `dir`. A file half downloaded is continued (`.part`), a
/// file whose SHA-256 differs is refused and deleted.
fn download(dir: &Path, tick: Tick) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(err)?;
    let mut done = 0u64;
    if let Some(src) = std::env::var_os(DIR_ENV).map(PathBuf::from) {
        // development: the same files from a folder, checked the same way
        for f in all_files() {
            let dest = dir.join(f.name);
            if !dest.metadata().is_ok_and(|m| m.len() == f.size) {
                let part = dir.join(format!("{}.part", f.name));
                std::fs::copy(src.join(f.name), &part).map_err(|e| tr!("sound.network", e = e))?;
                accept(&part, &dest, f)?;
            }
            done += f.size;
            if !tick(done) {
                return Err(tr!("sound.stopped"));
            }
        }
        return Ok(());
    }
    let agent = agent();
    for f in MODEL_FILES.iter() {
        let dest = dir.join(f.name);
        if dest.metadata().is_ok_and(|m| m.len() == f.size) {
            done += f.size;
            continue;
        }
        let part = dir.join(format!("{}.part", f.name));
        let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&part).map_err(err)?;
        let have = out.metadata().map(|m| m.len()).unwrap_or(0).min(f.size);
        if have == f.size {
            drop(out);
        } else {
            fetch_into(&agent, &format!("{RELEASE_URL}{}", f.name), have, &mut out, done, tick)?;
            drop(out);
        }
        accept(&part, &dest, f)?;
        done += f.size;
    }
    if !dir.join(ORT_DLL.name).metadata().is_ok_and(|m| m.len() == ORT_DLL.size) {
        fetch_runtime(&agent, dir, done, tick)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ the base

/// Embedding blob (f32 little-endian) to values.
pub fn embedding_of(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn blob_of(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// A reference render to look at: its name and the song's id.
struct Pending {
    rel: String,
    path: PathBuf,
    size: i64,
    mtime: f64,
}

/// The reference renders still to analyse (new, changed, or analysed by another model),
/// and how many renders there are in all and with an analysis.
fn pending(con: &Connection, roots: &store::Roots) -> rusqlite::Result<(Vec<Pending>, u64, u64)> {
    let mut s = con.prepare(
        "select distinct s.temoin, a.size, a.mtime, a.model from songs s left join audio_analysis a on a.rel = s.temoin \
         where s.gone = 0 and s.temoin is not null and s.temoin != '' order by s.temoin",
    )?;
    let rows: Vec<(String, Option<i64>, Option<f64>, Option<String>)> =
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
    let total = rows.len() as u64;
    let mut out = Vec::new();
    let mut ok = 0;
    for (rel, size, mtime, model) in rows {
        let Some(path) = roots.abs(&rel) else { continue };
        let Ok(meta) = path.metadata() else { continue };
        let fsize = meta.len() as i64;
        let fmtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        if size == Some(fsize) && mtime.is_some_and(|m| (m - fmtime).abs() < 1e-3) && model.as_deref() == Some(MODEL) {
            ok += 1;
        } else {
            out.push(Pending { rel, path, size: fsize, mtime: fmtime });
        }
    }
    Ok((out, total, ok))
}

fn save(con: &Connection, p: &Pending, res: std::result::Result<audio::Analysis, String>) -> rusqlite::Result<()> {
    let (emb, bpm, key, dur, error) = match res {
        Ok(a) => (Some(blob_of(&a.embedding)), a.bpm.map(|b| b as f64), a.key, Some(a.duration_s as f64), None),
        Err(e) => (None, None, None, None, Some(e)),
    };
    con.execute(
        "insert into audio_analysis(rel, size, mtime, model, embedding, bpm, key, duration, analyzed_at, error) \
         values(?,?,?,?,?,?,?,?,?,?) on conflict(rel) do update set size=excluded.size, mtime=excluded.mtime, \
         model=excluded.model, embedding=excluded.embedding, bpm=excluded.bpm, key=excluded.key, \
         duration=excluded.duration, analyzed_at=excluded.analyzed_at, error=excluded.error",
        params![p.rel, p.size, p.mtime, MODEL, emb, bpm, key, dur, now(), error],
    )?;
    Ok(())
}

/// The sheet's "Sounds like" and measured tempo, when the song's render was analysed.
pub fn song_sound(con: &Connection, sid: i64) -> rusqlite::Result<Option<SoundInfo>> {
    let row: Option<(String, Option<String>, Vec<u8>, Option<f64>)> = con
        .query_row(
            "select s.temoin, s.song_dir, a.embedding, a.bpm from songs s join audio_analysis a on a.rel = s.temoin \
             where s.id = ? and a.model = ? and a.embedding is not null",
            params![sid, MODEL],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((rel, dir, blob, bpm)) = row else { return Ok(None) };
    let me = embedding_of(&blob);
    let mut s = con.prepare(
        "select s.id, s.title, s.artist, s.style, s.song_dir, s.temoin, a.embedding from songs s \
         join audio_analysis a on a.rel = s.temoin where s.gone = 0 and s.archived = 0 and s.id != ? \
         and a.model = ? and a.embedding is not null",
    )?;
    let mut sims: Vec<SimilarSong> = s
        .query_map(params![sid, MODEL], |r| {
            Ok((
                SimilarSong { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, style: r.get(3)?, sim: 0.0 },
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Vec<u8>>(6)?,
            ))
        })?
        .filter_map(|x| x.ok())
        // versions of the same song (same folder, same render) are not "another song"
        .filter(|(_, d, t, _)| *t != rel && (d.is_none() || *d != dir))
        .map(|(mut o, _, _, b)| {
            o.sim = audio::cosine(&me, &embedding_of(&b)) as f64;
            o
        })
        .collect();
    sims.sort_by(|a, b| b.sim.total_cmp(&a.sim));
    sims.truncate(SIMILAR_SHOWN);
    Ok(Some(SoundInfo { rel, bpm: bpm.map(|b| (b * 10.0).round() / 10.0), similar: sims }))
}

// ------------------------------------------------------------------ the app's side

#[cfg(windows)]
fn lower_priority() {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_LOWEST};
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

impl App {
    /// On unless the user turned it off (the setting is written only by that switch).
    pub fn audio_enabled(&self) -> bool {
        self.reader.lock().unwrap().get::<bool>("audio_analysis").unwrap_or(true)
    }

    fn audio_paused(&self) -> bool {
        self.reader.lock().unwrap().get::<bool>("audio_paused").unwrap_or(false)
    }

    fn set_audio(&self, f: impl FnOnce(&mut crate::sound::AudioState)) {
        {
            let mut s = self.state.lock().unwrap();
            f(&mut s.audio);
        }
        self.notify();
    }

    /// Recomputes what the state says of the files and the setting (cheap: no hashing).
    fn audio_refresh_state(&self) {
        let dir = model_dir(&self.data_dir);
        let (enabled, paused) = (self.audio_enabled(), self.audio_paused());
        let ready = files_present(&dir);
        let (renders, analysed) = {
            let r = self.reader.lock().unwrap();
            let renders: i64 = r
                .con
                .query_row(
                    "select count(distinct temoin) from songs where gone=0 and temoin is not null and temoin != ''",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let analysed: i64 = r
                .con
                .query_row(
                    "select count(*) from audio_analysis where model=? and rel in (select temoin from songs where gone=0)",
                    [MODEL],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            (renders as u64, analysed as u64)
        };
        let failed = self.sound.load_failed.lock().unwrap().clone();
        self.set_audio(|a| {
            a.enabled = enabled;
            a.paused = paused;
            a.ready = ready;
            a.renders = renders;
            a.analysed = analysed;
            a.download_mb = download_bytes().div_ceil(1 << 20);
            a.disk_mb = if ready { disk_bytes(&dir).div_ceil(1 << 20) } else { 0 };
            if a.error.is_none() {
                a.error = failed;
            }
        });
    }

    pub fn audio_state(&self) -> AudioState {
        self.audio_refresh_state();
        self.state.lock().unwrap().audio.clone()
    }

    /// Turns the analysis on (downloading what is missing, in the background) or off.
    pub fn audio_enable(self: &Arc<Self>, on: bool) -> Result<AudioState> {
        self.store.lock().unwrap().set("audio_analysis", &on).map_err(err)?;
        // the reader sees it at once (WAL: a new read)
        *self.sound.load_failed.lock().unwrap() = None;
        self.set_audio(|a| a.error = None);
        if on {
            self.audio_download();
        } else {
            self.sound.stop_download.store(true, Ordering::SeqCst);
        }
        Ok(self.audio_state())
    }

    pub fn audio_pause(&self, paused: bool) -> Result<AudioState> {
        self.store.lock().unwrap().set("audio_paused", &paused).map_err(err)?;
        Ok(self.audio_state())
    }

    /// Deletes the downloaded files (the analyses already made stay in the base). ONNX
    /// Runtime, once loaded, stays in memory until Spool closes: its library is then
    /// deleted at the next start.
    pub fn audio_remove(&self) -> Result<AudioState> {
        self.store.lock().unwrap().set("audio_analysis", &false).map_err(err)?;
        self.sound.stop_download.store(true, Ordering::SeqCst);
        *self.sound.analyzer.lock().unwrap() = None;
        let dir = model_dir(&self.data_dir);
        let mut left = false;
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if std::fs::remove_file(e.path()).is_err() {
                left = true;
            }
        }
        if left {
            let _ = std::fs::write(dir.join("remove-on-start"), b"");
        } else {
            let _ = std::fs::remove_dir(&dir);
        }
        self.set_audio(|a| a.error = None);
        Ok(self.audio_state())
    }

    /// A removal that could not finish (the library was in use) finishes at start.
    pub(crate) fn audio_cleanup_on_start(data_dir: &Path) {
        let dir = model_dir(data_dir);
        if dir.join("remove-on-start").exists() {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let _ = std::fs::remove_file(e.path());
            }
            let _ = std::fs::remove_dir(&dir);
        }
        // never anything else than our own folder of models
    }

    fn audio_download(self: &Arc<Self>) {
        let dir = model_dir(&self.data_dir);
        if files_present(&dir) || self.sound.downloading.swap(true, Ordering::SeqCst) {
            return;
        }
        self.sound.stop_download.store(false, Ordering::SeqCst);
        let app = Arc::clone(self);
        std::thread::spawn(move || {
            let total = download_bytes();
            app.set_audio(|a| {
                a.downloading = true;
                a.dl_done = 0;
                a.dl_total = total;
                a.error = None;
            });
            let last = Mutex::new(std::time::Instant::now());
            let tick = |done: u64| {
                let mut l = last.lock().unwrap();
                if l.elapsed() >= Duration::from_millis(200) {
                    *l = std::time::Instant::now();
                    drop(l);
                    app.set_audio(|a| a.dl_done = done.min(total));
                }
                !app.sound.stop_download.load(Ordering::SeqCst)
            };
            let res = download(&dir, &tick);
            app.sound.downloading.store(false, Ordering::SeqCst);
            app.set_audio(|a| {
                a.downloading = false;
                a.error = res.err().filter(|e| *e != tr!("sound.stopped"));
            });
            app.audio_refresh_state();
        });
    }

    /// The analyzer, loaded on first use (ONNX Runtime's library and the model).
    fn analyzer(&self) -> Option<Arc<audio::Analyzer>> {
        if let Some(a) = self.sound.analyzer.lock().unwrap().clone() {
            return Some(a);
        }
        if self.sound.load_failed.lock().unwrap().is_some() {
            return None;
        }
        let dir = model_dir(&self.data_dir);
        // every file checked once per session before anything of it is loaded
        let bad = all_files().find(|f| sha256_file(&dir.join(f.name)).ok().as_deref() != Some(f.sha256));
        let res = match bad {
            Some(f) => Err(tr!("sound.badHash", name = f.name)),
            None => audio::Analyzer::load(&dir.join(ORT_DLL.name), &dir.join(MODEL_FILES[0].name), 1)
                .map_err(|e| tr!("sound.loadFailed", e = e)),
        };
        match res {
            Ok(a) => {
                let a = Arc::new(a);
                *self.sound.analyzer.lock().unwrap() = Some(Arc::clone(&a));
                Some(a)
            }
            Err(e) => {
                *self.sound.load_failed.lock().unwrap() = Some(e.clone());
                self.set_audio(|s| s.error = Some(e));
                None
            }
        }
    }

    /// Whether the analysis may run now: on, ready, not paused, and nothing heavier going.
    fn audio_may_run(&self) -> bool {
        self.audio_enabled()
            && !self.audio_paused()
            && files_present(&model_dir(&self.data_dir))
            && !self.state.lock().unwrap().running
            && !self.repair.lock().unwrap().running
            && self.sound.load_failed.lock().unwrap().is_none()
    }

    /// The background analysis: started once; idles while off.
    pub fn start_audio(self: &Arc<Self>) {
        if self.sound.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let app = Arc::clone(self);
        std::thread::Builder::new()
            .name("audio-analysis".into())
            .spawn(move || {
                lower_priority();
                app.audio_refresh_state();
                // on by default: what is missing downloads once per session, after the first
                // setup (offline: tried again at the next start, not in a loop)
                let mut fetched = false;
                loop {
                    std::thread::sleep(Duration::from_secs(3));
                    if !app.setup_done() {
                        continue;
                    }
                    if !fetched && app.audio_enabled() && !files_present(&model_dir(&app.data_dir)) {
                        fetched = true;
                        app.audio_download();
                    }
                    if !app.audio_may_run() {
                        continue;
                    }
                    let _ = app.audio_round();
                }
            })
            .expect("spawn the audio analysis thread");
    }

    /// One round: every render waiting, one at a time, until done, paused or interrupted.
    /// Returns how many were analysed.
    pub fn audio_round(&self) -> Result<u64> {
        let roots = self.roots();
        let (todo, _, _) = pending(&self.reader.lock().unwrap().con, &roots).map_err(err)?;
        if todo.is_empty() {
            return Ok(0);
        }
        let Some(an) = self.analyzer() else { return Ok(0) };
        let total = todo.len() as u64;
        self.set_audio(|a| {
            a.running = true;
            a.done = 0;
            a.total = total;
        });
        let mut done = 0u64;
        for p in &todo {
            if !self.audio_may_run() {
                break;
            }
            self.set_audio(|a| a.detail = p.rel.clone());
            // only read: the analyzer opens the file read-only
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| an.analyze(&p.path)))
                .map_err(|_| tr!("sound.crashed"))
                .and_then(|r| r.map_err(|e| e.to_string()));
            {
                let st = self.store.lock().unwrap();
                save(&st.con, p, res).map_err(err)?;
            }
            done += 1;
            self.set_audio(|a| {
                a.done = done;
                a.analysed += 1;
            });
        }
        // the style suggestions take the new analyses into account, and the pages reload
        {
            let st = self.store.lock().unwrap();
            if let Ok(tx) = st.con.unchecked_transaction() {
                if index::refresh_styles(&tx).is_ok() {
                    let _ = tx.commit();
                }
            }
        }
        {
            let mut s = self.state.lock().unwrap();
            s.audio.running = false;
            s.audio.detail.clear();
            s.version += 1;
        }
        self.notify();
        self.audio_refresh_state();
        Ok(done)
    }
}

/// For the state line in tests and tools.
pub fn describe(s: &ScanState) -> String {
    let a = &s.audio;
    format!(
        "audio: on {} ready {} running {} {}/{} analysed {}/{}",
        a.enabled, a.ready, a.running, a.done, a.total, a.analysed, a.renders
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_parsing_finds_and_inflates_an_entry() {
        // a zip made by hand: one deflated entry
        use flate2::{Compression, write::DeflateEncoder};
        let name = "dir/lib/x.dll";
        let content = b"hello hello hello hello onnx".repeat(50);
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&content).unwrap();
        let comp = enc.finish().unwrap();
        let mut z = Vec::new();
        let local = |z: &mut Vec<u8>| {
            z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            z.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            z.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            z.extend_from_slice(&(content.len() as u32).to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes());
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(name.as_bytes());
        };
        local(&mut z);
        z.extend_from_slice(&comp);
        let cd_off = z.len();
        z.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        z.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        z.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        z.extend_from_slice(&(content.len() as u32).to_le_bytes());
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0u8; 12]);
        z.extend_from_slice(&0u32.to_le_bytes()); // local header at 0
        z.extend_from_slice(name.as_bytes());
        let cd_len = z.len() - cd_off;
        z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        z.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend_from_slice(&(cd_len as u32).to_le_bytes());
        z.extend_from_slice(&(cd_off as u32).to_le_bytes());
        z.extend_from_slice(&[0, 0]);
        let (o, l) = zip_directory(&z).unwrap();
        assert_eq!((o as usize, l as usize), (cd_off, cd_len));
        let (off, c, m) = zip_entry(&z[cd_off..], name).unwrap();
        assert_eq!((off, c as usize, m), (0, comp.len(), 8));
        let dir = std::env::temp_dir().join(format!("spool-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("x.dll");
        unzip_entry(&z, c, m, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), content);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Network: ONNX Runtime's library out of Microsoft's archive by ranges, hash-checked.
    /// `cargo test -p core -- --ignored runtime_comes_out_of_microsofts_archive`
    #[test]
    #[ignore]
    fn runtime_comes_out_of_microsofts_archive() {
        let dir = std::env::temp_dir().join(format!("spool-ort-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fetched = std::sync::atomic::AtomicU64::new(0);
        let tick = |n: u64| {
            fetched.store(n, Ordering::SeqCst);
            true
        };
        fetch_runtime(&agent(), &dir, 0, &tick).unwrap();
        assert_eq!(sha256_file(&dir.join(ORT_DLL.name)).unwrap(), ORT_DLL.sha256);
        let got = fetched.load(Ordering::SeqCst);
        assert!(got < 8 << 20, "fetched {got} bytes, not just the library");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Network: everything the opt-in downloads, from the real release and from Microsoft,
    /// each file hash-checked. `cargo test -p core -- --ignored the_whole_download_works`
    #[test]
    #[ignore]
    fn the_whole_download_works() {
        assert!(std::env::var_os(DIR_ENV).is_none(), "unset {DIR_ENV}: this checks the network");
        let dir = std::env::temp_dir().join(format!("spool-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        download(&dir, &|_| true).unwrap();
        for f in all_files() {
            assert_eq!(sha256_file(&dir.join(f.name)).unwrap(), f.sha256, "{}", f.name);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn embedding_blob_round_trip() {
        let v = vec![0.5f32, -1.25, 3.0];
        assert_eq!(embedding_of(&blob_of(&v)), v);
    }

    #[test]
    fn a_wrong_file_is_refused_and_deleted() {
        let dir = std::env::temp_dir().join(format!("spool-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("manifest.json.part");
        std::fs::write(&part, b"not the manifest").unwrap();
        let e = accept(&part, &dir.join("manifest.json"), &MODEL_FILES[1]).unwrap_err();
        assert!(e.contains("manifest.json"));
        assert!(!part.exists() && !dir.join("manifest.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// What Spool detects of every song by itself, for judging it on a real catalogue: the
/// reference renders analysed (on a COPY of a data folder, SPOOL_EVAL_DATA, never the user's
/// own), the tempo and key written in the projects, the styles from the projects alone (no
/// style the user wrote) and from the audio alone (zero-shot). Writes SPOOL_EVAL_OUT (JSON).
///   SPOOL_EVAL_DATA=<copy> SPOOL_EVAL_MODELS=<model dir> SPOOL_EVAL_OUT=<file.json> \
///   cargo test --release -p core --lib detection_report -- --ignored --nocapture
#[cfg(test)]
mod eval {
    use std::collections::HashMap;
    use super::*;

    #[test]
    #[ignore]
    fn detection_report() {
        let (Some(d), Some(m), Some(out)) =
            (std::env::var_os("SPOOL_EVAL_DATA"), std::env::var_os("SPOOL_EVAL_MODELS"), std::env::var_os("SPOOL_EVAL_OUT"))
        else {
            return;
        };
        let app = crate::App::open(Path::new(&d)).unwrap();
        let an = audio::Analyzer::new(PathBuf::from(&m)).unwrap();
        // the projects read with this version's reader (a new one reads them all again)
        eprintln!("scanning (only reads the projects and MIDI files)");
        app.scan(false).unwrap();
        // 1. the reference renders (only read)
        let roots = app.roots();
        let (todo, total, _) = pending(&app.reader.lock().unwrap().con, &roots).unwrap();
        eprintln!("{} renders to analyse ({total} referenced)", todo.len());
        for (i, p) in todo.iter().enumerate() {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| an.analyze(&p.path)))
                .map_err(|_| "crashed".to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            save(&app.store.lock().unwrap().con, p, res).unwrap();
            if i % 25 == 0 {
                eprintln!("  {i}/{}", todo.len());
            }
        }
        let st = app.reader.lock().unwrap();
        let con = &st.con;
        // 2. the audio side: tempo, key, zero-shot styles centred on this catalogue
        let mut emb: HashMap<i64, (Vec<f32>, Option<f64>, Option<String>, String)> = HashMap::new();
        {
            let mut s = con
                .prepare(
                    "select s.id, a.embedding, a.bpm, a.key, a.rel from songs s join audio_analysis a on a.rel = s.temoin \
                     where s.gone = 0 and a.embedding is not null",
                )
                .unwrap();
            for r in s
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
                .unwrap()
            {
                let (id, b, bpm, key, rel) = r.unwrap();
                emb.insert(id, (embedding_of(&b), bpm, key, rel));
            }
        }
        let mut mean = vec![0f32; audio::EMBEDDING_DIM];
        for (e, ..) in emb.values() {
            for (m, x) in mean.iter_mut().zip(e) {
                *m += x / emb.len() as f32;
            }
        }
        let set = audio::styles::builtin();
        // 3. the projects side: styles without any written style, tempo and key of the newest project
        let blind: HashMap<i64, Vec<catalog::StyleOption>> = index::blind_styles(con).unwrap().into_iter().collect();
        let mut s = con
            .prepare(
                "select s.id, s.title, s.artist, s.style, s.key, s.song_dir, \
                 (select f.data from files f where f.song_id = s.id and f.in_version = 0 order by f.mtime desc limit 1) \
                 from songs s where s.gone = 0 and s.archived = 0 order by s.id",
            )
            .unwrap();
        let rows: Vec<serde_json::Value> = s
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .unwrap()
            .map(|r| {
                let (id, title, artist, style, key, dir, data) = r.unwrap();
                let p: serde_json::Value = data.as_deref().and_then(|d| serde_json::from_str(d).ok()).unwrap_or_default();
                let a = emb.get(&id);
                let heard = crate::detect::AudioMeasure { bpm: a.and_then(|x| x.1), key: a.and_then(|x| x.2.clone()) };
                let shown = serde_json::json!({
                    "key": crate::detect::key(None, &p, &heard),
                    "tempo": crate::detect::tempo(None, &p, &heard),
                });
                serde_json::json!({
                    "id": id, "title": title, "artist": artist, "dir": dir,
                    "label": {"style": style, "key": key},
                    "shown": shown,
                    "project": {"tempo": p["tempo"], "key": p["key"], "key_est": p["key_est"], "key_conf": p["key_conf"],
                                "styles": blind.get(&id).map(|o| o.iter().take(3).map(|x| serde_json::json!([x.style, x.pct, x.why])).collect::<Vec<_>>())},
                    "audio": a.map(|(e, bpm, key, rel)| serde_json::json!({
                        "render": rel, "bpm": bpm, "key": key,
                        "styles": set.score_centered(e, &mean).into_iter().take(3).map(|(n, p)| serde_json::json!([n, (p * 100.0).round()])).collect::<Vec<_>>(),
                    })),
                })
            })
            .collect();
        std::fs::write(&out, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        eprintln!("{} songs, {} with an analysed render -> {}", rows.len(), emb.len(), Path::new(&out).display());
    }
}

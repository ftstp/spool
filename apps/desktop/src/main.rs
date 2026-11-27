// Desktop shell: exposes the core to the interface and forwards its changes. No logic here.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod midi_in;
use app_core::detail::SongDetail;
use app_core::{App, Overview, ScanState, SongItem, Stats, UndoState};
use i18n::tr;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

type Res<T> = Result<T, String>;

/// Runs a core call on a worker thread: the window never waits for the base (a scan holds
/// the writer for its whole length).
async fn run<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn overview(app: State<'_, Arc<App>>) -> Res<Overview> {
    let app = Arc::clone(&app);
    run(move || {
        app.overview()
    })
    .await
}

#[tauri::command]
async fn songs(app: State<'_, Arc<App>>) -> Res<Vec<SongItem>> {
    let app = Arc::clone(&app);
    run(move || {
        app.songs()
    })
    .await
}

#[tauri::command]
async fn stats(app: State<'_, Arc<App>>) -> Res<Stats> {
    let app = Arc::clone(&app);
    run(move || {
        app.stats()
    })
    .await
}

#[tauri::command]
async fn scan_state(app: State<'_, Arc<App>>) -> Res<ScanState> {
    let app = Arc::clone(&app);
    run(move || Ok(app.scan_state())).await
}

/// The audio analysis (opt-in): its state, turning it on (downloads the model once) or off,
/// pausing it, removing the downloaded files.
#[tauri::command]
async fn audio_state(app: State<'_, Arc<App>>) -> Res<app_core::sound::AudioState> {
    let app = Arc::clone(&app);
    run(move || Ok(app.audio_state())).await
}

#[tauri::command]
async fn audio_enable(app: State<'_, Arc<App>>, on: bool) -> Res<app_core::sound::AudioState> {
    let app = Arc::clone(&app);
    run(move || app.audio_enable(on)).await
}

#[tauri::command]
async fn audio_pause(app: State<'_, Arc<App>>, paused: bool) -> Res<app_core::sound::AudioState> {
    let app = Arc::clone(&app);
    run(move || app.audio_pause(paused)).await
}

#[tauri::command]
async fn audio_remove(app: State<'_, Arc<App>>) -> Res<app_core::sound::AudioState> {
    let app = Arc::clone(&app);
    run(move || app.audio_remove()).await
}

/// A scan pass, off the interface's thread.
#[tauri::command]
async fn scan(app: State<'_, Arc<App>>, full: bool, projects_only: Option<bool>) -> Res<bool> {
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn_blocking(move || if projects_only == Some(true) { app.scan_projects() } else { app.scan(full) })
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn root(app: State<'_, Arc<App>>) -> Res<Option<String>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.root())).await
}

#[tauri::command]
async fn set_root(app: State<'_, Arc<App>>, root: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.set_root(&root)
    })
    .await
}

#[tauri::command]
async fn import_prototype(app: State<'_, Arc<App>>, path: String) -> Res<Vec<(String, usize)>> {
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn_blocking(move || app.import_prototype(&PathBuf::from(path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn reset_base(app: State<'_, Arc<App>>) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || {
        app.reset()
    })
    .await
}

#[tauri::command]
async fn song(app: State<'_, Arc<App>>, id: i64) -> Res<Option<SongDetail>> {
    let app = Arc::clone(&app);
    run(move || {
        app.song(id)
    })
    .await
}

#[tauri::command]
async fn create_plan(app: State<'_, Arc<App>>, req: app_core::create::CreateReq) -> Res<app_core::create::CreatePlan> {
    let app = Arc::clone(&app);
    run(move || app.create_plan(req)).await
}

#[tauri::command]
async fn releases(app: State<'_, Arc<App>>) -> Res<Vec<app_core::releases::Release>> {
    let app = Arc::clone(&app);
    run(move || app.releases()).await
}

#[tauri::command]
async fn release_hints(app: State<'_, Arc<App>>) -> Res<Vec<app_core::releases::ReleaseHint>> {
    let app = Arc::clone(&app);
    run(move || app.release_hints()).await
}

#[tauri::command]
async fn release_save(app: State<'_, Arc<App>>, release: app_core::releases::Release) -> Res<(String, UndoState)> {
    let app = Arc::clone(&app);
    run(move || app.release_save(release)).await
}

#[tauri::command]
async fn release_delete(app: State<'_, Arc<App>>, id: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.release_delete(&id)).await
}

#[tauri::command]
async fn release_arrange_plan(app: State<'_, Arc<App>>, id: String, parent: Option<String>) -> Res<app_core::releases::ArrangePlan> {
    let app = Arc::clone(&app);
    run(move || app.release_arrange_plan(&id, parent)).await
}

#[tauri::command]
async fn release_arrange(app: State<'_, Arc<App>>, id: String, parent: Option<String>) -> Res<app_core::fsops::MoveResult> {
    let app = Arc::clone(&app);
    run(move || app.release_arrange(&id, parent)).await
}

#[tauri::command]
async fn board_link(app: State<'_, Arc<App>>, id: i64, items: Vec<String>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.board_link(id, items)).await
}

#[tauri::command]
async fn board_unlink(app: State<'_, Arc<App>>, id: i64, rel: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.board_unlink(id, &rel)).await
}

#[tauri::command]
async fn track_view(app: State<'_, Arc<App>>, rel: String, track: String) -> Res<Vec<repair::track_view::DevView>> {
    let app = Arc::clone(&app);
    run(move || app.track_view(&rel, &track)).await
}

#[tauri::command]
async fn create_song(app: State<'_, Arc<App>>, req: app_core::create::CreateReq) -> Res<Value> {
    let app = Arc::clone(&app);
    run(move || app.create_song(req)).await
}

#[tauri::command]
async fn style_guess(app: State<'_, Arc<App>>, id: i64) -> Res<Vec<app_core::StyleOption>> {
    let app = Arc::clone(&app);
    run(move || app.style_guess(id)).await
}

#[tauri::command]
async fn song_edit(app: State<'_, Arc<App>>, id: i64, body: Map<String, Value>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || {
        app.song_edit(id, body)
    })
    .await
}

#[tauri::command]
async fn check_set(app: State<'_, Arc<App>>, id: i64, key: String, value: Value) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || {
        app.check_set(id, &key, value)
    })
    .await
}

#[tauri::command]
async fn suggestion_act(app: State<'_, Arc<App>>, id: i64, accept: bool, choice: Option<String>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || {
        app.suggestion_act(id, accept, choice)
    })
    .await
}

#[tauri::command]
async fn songs_edit(app: State<'_, Arc<App>>, edits: Vec<(i64, Map<String, Value>)>, label: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.songs_edit(edits, label)).await
}

#[tauri::command]
async fn accept_all(app: State<'_, Arc<App>>, ids: Vec<i64>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || {
        app.accept_all(ids)
    })
    .await
}

#[tauri::command]
async fn undo_state(app: State<'_, Arc<App>>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || {
        app.undo_state()
    })
    .await
}

/// (what was undone, what can still be undone)
#[tauri::command]
async fn song_resources(app: State<'_, Arc<App>>, id: i64, rel: Option<String>) -> Res<app_core::resources::Resources> {
    let app = Arc::clone(&app);
    run(move || app.resources(id, rel)).await
}

#[tauri::command]
async fn media_list(app: State<'_, Arc<App>>) -> Res<Vec<app_core::media_view::MediaSong>> {
    let app = Arc::clone(&app);
    run(move || app.media()).await
}

#[tauri::command]
async fn loose_audio(app: State<'_, Arc<App>>) -> Res<Vec<app_core::media_view::LooseAudio>> {
    let app = Arc::clone(&app);
    run(move || app.loose_audio()).await
}

#[tauri::command]
async fn set_cover(app: State<'_, Arc<App>>, id: i64, rel: Option<String>) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.set_cover(id, rel)).await
}

#[tauri::command]
async fn reading_report(app: State<'_, Arc<App>>) -> Res<app_core::reading::ReadingReport> {
    let app = Arc::clone(&app);
    run(move || app.reading_report()).await
}

#[tauri::command]
async fn group_preview(app: State<'_, Arc<App>>, all: bool, dirs: Vec<String>, title_from: String) -> Res<app_core::reading::GroupPreview> {
    let app = Arc::clone(&app);
    run(move || app.group_preview(all, dirs, title_from)).await
}

#[tauri::command]
async fn folder_neighbours(app: State<'_, Arc<App>>, dir: String) -> Res<Vec<String>> {
    let app = Arc::clone(&app);
    run(move || app.folder_neighbours(&dir)).await
}

#[tauri::command]
async fn set_grouping(app: State<'_, Arc<App>>, all: bool, dirs: Vec<String>, title_from: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.set_grouping(all, dirs, title_from)).await
}

#[tauri::command]
async fn diagnostic_report(app: State<'_, Arc<App>>, version: String) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.diagnostic_report(&version)).await
}

#[tauri::command]
async fn redo(app: State<'_, Arc<App>>) -> Res<(Option<String>, UndoState)> {
    let app = Arc::clone(&app);
    run(move || app.redo()).await
}

#[tauri::command]
async fn undo(app: State<'_, Arc<App>>) -> Res<(Option<String>, UndoState)> {
    let app = Arc::clone(&app);
    run(move || {
        app.undo()
    })
    .await
}

#[tauri::command]
async fn board_note(app: State<'_, Arc<App>>, id: i64, text: String, lyrics: bool, name: Option<String>) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || {
        app.board_note(id, &text, lyrics, name.as_deref())
    })
    .await
}

/// A file added to the board: raw bytes as the body, song id and file name in headers.
#[tauri::command]
async fn board_add(app: State<'_, Arc<App>>, request: tauri::ipc::Request<'_>) -> Res<String> {
    // The bytes come raw when the webview's fast channel works. When it falls back to
    // messages (it did on a French Windows: "file expected"), the same bytes arrive as JSON, as an
    // array or as an object of numbered keys: all three are taken.
    let data: Vec<u8> = match request.body() {
        tauri::ipc::InvokeBody::Raw(d) => d.clone(),
        tauri::ipc::InvokeBody::Json(Value::Array(a)) => a.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect(),
        tauri::ipc::InvokeBody::Json(Value::Object(m)) if !m.is_empty() => {
            let mut v = vec![0u8; m.len()];
            for (k, x) in m {
                let i: usize = k.parse().map_err(|_| tr!("desktop.unreadableFile"))?;
                *v.get_mut(i).ok_or_else(|| tr!("desktop.unreadableFile"))? = x.as_u64().unwrap_or(0) as u8;
            }
            v
        }
        _ => return Err(tr!("desktop.fileExpected")),
    };
    let h = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let name = h("x-name").map(|n| app_core::media::percent_decode(&n)).ok_or_else(|| tr!("err.noName"))?;
    let app = Arc::clone(&app);
    // a picture for a cover (Spool's pictures folder)
    if h("x-picture").is_some() {
        return run(move || app.picture_add(&name, &data)).await;
    }
    // an artist's board (the Artists page), else a song's
    if let Some(artist) = h("x-artist").map(|a| app_core::media::percent_decode(&a)) {
        return run(move || app.artist_board_add(&artist, &name, &data)).await;
    }
    let id: i64 = h("x-song").and_then(|v| v.parse().ok()).ok_or_else(|| tr!("desktop.songMissing"))?;
    run(move || app.board_add(id, &name, &data)).await
}

#[tauri::command]
async fn artists(app: State<'_, Arc<App>>) -> Res<Vec<app_core::ArtistCard>> {
    let app = Arc::clone(&app);
    run(move || app.artists()).await
}

#[tauri::command]
async fn artist(app: State<'_, Arc<App>>, name: String) -> Res<app_core::ArtistPage> {
    let app = Arc::clone(&app);
    run(move || app.artist(&name)).await
}

#[tauri::command]
async fn artist_save(app: State<'_, Arc<App>>, name: String, patch: Map<String, Value>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.artist_save(&name, patch)).await
}

#[tauri::command]
async fn artist_board_note(app: State<'_, Arc<App>>, name: String, text: String, lyrics: bool, old: Option<String>) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.artist_board_note(&name, &text, lyrics, old.as_deref())).await
}

#[tauri::command]
async fn artist_board_copy(app: State<'_, Arc<App>>, name: String, path: String, photo: bool) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.artist_board_copy(&name, &path, photo)).await
}

#[tauri::command]
async fn artist_board_remove(app: State<'_, Arc<App>>, name: String, file: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.artist_board_remove(&name, &file)).await
}

#[tauri::command]
async fn artist_board_meta(app: State<'_, Arc<App>>, name: String, patch: Map<String, Value>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.artist_board_meta(&name, patch)).await
}

/// The sidebar's "Analyse" (automatic analysis off in Settings).
#[tauri::command]
fn analyse_now(app: State<'_, Arc<App>>) {
    app.analyse_now();
}

#[tauri::command]
async fn refs(app: State<'_, Arc<App>>, kind: String, key: String) -> Res<Vec<app_core::RefItem>> {
    let app = Arc::clone(&app);
    run(move || app.refs(&kind, &key)).await
}

#[tauri::command]
async fn ref_mark(app: State<'_, Arc<App>>, path: String, kind: String, key: String, on: bool) -> Res<Vec<app_core::RefItem>> {
    let app = Arc::clone(&app);
    run(move || app.ref_mark(&path, &kind, &key, on)).await
}

/// A web page or an e-mail address (an artist's links), opened by the system. Nothing else:
/// no file, no program.
#[tauri::command]
fn open_link(url: String) -> Res<()> {
    let u = url.trim();
    let ok = (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:"))
        && !u.chars().any(|c| c.is_control() || c == '"' || c == ' ');
    if !ok {
        return Err(tr!("desktop.badLink"));
    }
    std::process::Command::new("explorer").arg(u).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn board_remove(app: State<'_, Arc<App>>, id: i64, name: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.board_remove(id, &name)
    })
    .await
}

#[tauri::command]
async fn board_meta(app: State<'_, Arc<App>>, id: i64, patch: Map<String, Value>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.board_meta(id, patch)
    })
    .await
}

#[tauri::command]
async fn merge_song(app: State<'_, Arc<App>>, id: i64, into: i64) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || {
        app.merge_song(id, into)
    })
    .await
}

#[tauri::command]
async fn split_song(app: State<'_, Arc<App>>, id: i64, key: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.split_song(id, &key)
    })
    .await
}

#[tauri::command]
async fn fixable(app: State<'_, Arc<App>>) -> Res<Vec<app_core::repairs::Fixable>> {
    let app = Arc::clone(&app);
    run(move || {
        app.fixable()
    })
    .await
}

#[tauri::command]
async fn repair_plan(app: State<'_, Arc<App>>, rel: String) -> Res<Option<repair::fix::Plan>> {
    let app = Arc::clone(&app);
    run(move || {
        app.repair_plan(&rel)
    })
    .await
}

#[tauri::command]
async fn repair_state(app: State<'_, Arc<App>>) -> Res<app_core::repairs::RepairState> {
    let app = Arc::clone(&app);
    run(move || Ok(app.repair_state())).await
}

#[tauri::command]
async fn repair_run(app: State<'_, Arc<App>>, rels: Vec<String>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.repair_run(rels)
    })
    .await
}

#[tauri::command]
async fn repair_view(app: State<'_, Arc<App>>, years: Option<i64>) -> Res<app_core::repair_view::RepairView> {
    let app = Arc::clone(&app);
    run(move || {
        app.repair_view(years)
    })
    .await
}

#[tauri::command]
async fn set_ignored(app: State<'_, Arc<App>>, name: String, ignored: bool, keep: bool) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.set_ignored(&name, ignored, keep)
    })
    .await
}

#[tauri::command]
async fn set_trial(app: State<'_, Arc<App>>, name: String, format: String, to: Option<String>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.set_trial(&name, &format, to.as_deref())
    })
    .await
}

#[tauri::command]
async fn atelier_presets(app: State<'_, Arc<App>>) -> Res<Vec<Value>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.atelier_presets())).await
}

#[tauri::command]
fn atelier_default() -> Value {
    repair::studio::default_preset()
}

#[tauri::command]
async fn atelier_save_presets(app: State<'_, Arc<App>>, presets: Vec<Value>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.atelier_save_presets(presets)
    })
    .await
}

#[tauri::command]
async fn atelier_preview(app: State<'_, Arc<App>>, song: i64, preset: Value) -> Res<Value> {
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn_blocking(move || app.atelier_preview(song, preset)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn atelier_run(app: State<'_, Arc<App>>, songs: Vec<i64>, preset: Value) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.atelier_run(songs, preset)
    })
    .await
}

#[tauri::command]
async fn config(app: State<'_, Arc<App>>) -> Res<app_core::Config> {
    let app = Arc::clone(&app);
    run(move || Ok(app.config())).await
}

#[tauri::command]
async fn set_config(app: State<'_, Arc<App>>, key: String, value: Value) -> Res<app_core::Config> {
    let app = Arc::clone(&app);
    run(move || app.set_config(&key, value)).await
}

/// One event of the usage statistics (`tool_opened`, `tool_feature_used`, `tool_error`). The
/// core checks the user's choice and what may go out (crates/core/src/telemetry.rs); nothing
/// is sent unless they said yes. Never fails, never waits for the network.
#[tauri::command]
async fn telemetry(app: State<'_, Arc<App>>, event: String, props: Option<Map<String, Value>>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        app.telemetry(&event, &props.unwrap_or_default());
        Ok(())
    })
    .await
}

#[tauri::command]
async fn cancel_scan(app: State<'_, Arc<App>>) -> Res<()> {
    app.cancel_scan();
    Ok(())
}

#[tauri::command]
async fn folder_peek(app: State<'_, Arc<App>>, path: String) -> Res<app_core::FolderPeek> {
    let app = Arc::clone(&app);
    run(move || app.folder_peek(&path)).await
}

#[tauri::command]
async fn add_root(app: State<'_, Arc<App>>, path: String) -> Res<app_core::Config> {
    let app = Arc::clone(&app);
    run(move || app.add_root(&path).map(|_| app.config())).await
}

#[tauri::command]
async fn remove_root(app: State<'_, Arc<App>>, path: String) -> Res<app_core::Config> {
    let app = Arc::clone(&app);
    run(move || app.remove_root(&path).map(|_| app.config())).await
}

/// Spool's own disk use, the shell's two parts included: the installed application (its
/// folder, when installed; a development build counts its exe alone) and WebView2's cache.
fn shell_sizes(handle: &tauri::AppHandle) -> (u64, u64) {
    let app = std::env::current_exe()
        .ok()
        .map(|exe| match exe.parent() {
            Some(dir) if dir.join("uninstall.exe").exists() => app_core::size_of(dir),
            _ => app_core::size_of(&exe),
        })
        .unwrap_or(0);
    let webview = handle.path().app_local_data_dir().map(|d| app_core::size_of(&d.join("EBWebView"))).unwrap_or(0);
    (app, webview)
}

#[tauri::command]
async fn storage(app: State<'_, Arc<App>>, handle: tauri::AppHandle) -> Res<app_core::Storage> {
    let app = Arc::clone(&app);
    run(move || {
        let mut s = app.storage();
        (s.app, s.webview) = shell_sizes(&handle);
        Ok(s)
    })
    .await
}

#[tauri::command]
async fn trim_backups(app: State<'_, Arc<App>>, handle: tauri::AppHandle) -> Res<app_core::Storage> {
    let app = Arc::clone(&app);
    run(move || {
        let mut s = app.trim_backups()?;
        (s.app, s.webview) = shell_sizes(&handle);
        Ok(s)
    })
    .await
}

/// Opens Spool's data folder in the Explorer (only that folder: nothing chosen by the page).
#[tauri::command]
fn open_data_dir(app: State<'_, Arc<App>>) -> Res<()> {
    let dir = PathBuf::from(app.storage().dir);
    #[cfg(windows)]
    std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    let _ = dir;
    Ok(())
}

#[tauri::command]
async fn written_copies(app: State<'_, Arc<App>>) -> Res<Vec<app_core::repairs::WrittenCopy>> {
    let app = Arc::clone(&app);
    run(move || app.written_copies()).await
}

/// Text saved where the user picks (the system's "Save as", which asks before replacing a
/// file). Written whole: a temporary file next to it, then put in place. None: cancelled.
#[tauri::command]
async fn save_text(window: tauri::Window, title: String, name: String, ext: String, text: String) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    window.dialog().file().set_title(title).set_parent(&window).set_file_name(&name).add_filter(ext.to_uppercase(), &[ext.as_str()]).save_file(move |f| {
        let _ = tx.send(f.and_then(|f| f.into_path().ok()));
    });
    let Some(path) = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    run(move || {
        let tmp = path.with_extension(format!("{ext}.tmp"));
        std::fs::write(&tmp, text.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

/// The system's folder picker (None: cancelled).
#[tauri::command]
async fn pick_folder(window: tauri::Window, title: String) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    window.dialog().file().set_title(title).set_parent(&window).pick_folder(move |f| {
        let _ = tx.send(f.and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned()));
    });
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn artist_suggestions(app: State<'_, Arc<App>>) -> Res<Vec<app_core::ArtistGuess>> {
    let app = Arc::clone(&app);
    run(move || app.artist_suggestions()).await
}

#[tauri::command]
async fn home_stats(app: State<'_, Arc<App>>) -> Res<app_core::insights::HomeStats> {
    let app = Arc::clone(&app);
    run(move || app.home_stats()).await
}

#[tauri::command]
async fn progress(app: State<'_, Arc<App>>) -> Res<app_core::insights::Progress> {
    let app = Arc::clone(&app);
    run(move || app.progress()).await
}

#[tauri::command]
async fn repair_run_preset(app: State<'_, Arc<App>>, rels: Vec<String>, preset: Value) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.repair_run_preset(rels, preset)).await
}

/// The MIDI keyboard (midi_in): the inputs plugged in, one opened, or closed (free for Live).
#[tauri::command]
async fn midi_inputs() -> Res<Vec<String>> {
    tauri::async_runtime::spawn_blocking(midi_in::inputs).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn midi_open(app: tauri::AppHandle, name: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || midi_in::open(&app, &name)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn midi_close() -> Res<()> {
    tauri::async_runtime::spawn_blocking(midi_in::close).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn take_save(app: State<'_, Arc<App>>, at: f64, notes: Vec<app_core::takes::Note>) -> Res<i64> {
    let app = Arc::clone(&app);
    run(move || app.take_save(at, notes)).await
}

#[tauri::command]
async fn takes(app: State<'_, Arc<App>>) -> Res<Vec<app_core::takes::Take>> {
    let app = Arc::clone(&app);
    run(move || app.takes()).await
}

#[tauri::command]
async fn take_delete(app: State<'_, Arc<App>>, id: i64) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.take_delete(id)).await
}

#[tauri::command]
async fn take_keep(app: State<'_, Arc<App>>, id: i64, name: String, bpm: f64) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.take_keep(id, &name, bpm)).await
}

#[tauri::command]
async fn take_smf(app: State<'_, Arc<App>>, id: i64, name: String, bpm: f64) -> Res<tauri::ipc::Response> {
    let app = Arc::clone(&app);
    let b = run(move || app.take_smf(id, &name, bpm)).await?;
    Ok(tauri::ipc::Response::new(b))
}

#[tauri::command]
async fn project_files(app: State<'_, Arc<App>>) -> Res<Vec<app_core::projects_view::ProjectFile>> {
    let app = Arc::clone(&app);
    run(move || app.project_files()).await
}

#[tauri::command]
async fn drives(app: State<'_, Arc<App>>) -> Res<Vec<app_core::drives::Drive>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.drives())).await
}

#[tauri::command]
async fn deep_search(app: State<'_, Arc<App>>, drive: Option<String>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.deep_search(drive)).await
}

#[tauri::command]
async fn fs_folders(app: State<'_, Arc<App>>) -> Res<Vec<String>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.fs_folders())).await
}

#[tauri::command]
async fn fs_media(app: State<'_, Arc<App>>) -> Res<Vec<app_core::fsops::MediaFolder>> {
    let app = Arc::clone(&app);
    run(move || app.fs_media()).await
}

#[tauri::command]
async fn fs_reorg(app: State<'_, Arc<App>>, template: String) -> Res<app_core::fsops::ReorgPlan> {
    let app = Arc::clone(&app);
    run(move || app.fs_reorg(&template)).await
}

#[tauri::command]
async fn fs_plan(app: State<'_, Arc<App>>, src: String, dest: String) -> Res<app_core::fsops::MovePlan> {
    let app = Arc::clone(&app);
    run(move || app.fs_plan(&src, &dest)).await
}

#[tauri::command]
async fn fs_mkdir(app: State<'_, Arc<App>>, rel: String) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.fs_mkdir(&rel)).await
}

#[tauri::command]
async fn fs_move(app: State<'_, Arc<App>>, moves: Vec<(String, String)>) -> Res<app_core::fsops::MoveResult> {
    let app = Arc::clone(&app);
    run(move || app.fs_move(moves)).await
}

#[tauri::command]
async fn fs_to_board(app: State<'_, Arc<App>>, id: i64, items: Vec<String>) -> Res<(usize, usize)> {
    let app = Arc::clone(&app);
    run(move || app.fs_to_board(id, items)).await
}

/// The system's file picker for one file of these extensions (None: cancelled).
#[tauri::command]
async fn pick_file(window: tauri::Window, title: String, ext: Vec<String>) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    let exts: Vec<&str> = ext.iter().map(String::as_str).collect();
    window.dialog().file().set_title(title).set_parent(&window).add_filter("Fichier", &exts).pick_file(move |f| {
        let _ = tx.send(f.and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned()));
    });
    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn notion_preview(app: State<'_, Arc<App>>, path: String, columns: Option<Vec<app_core::notion::Column>>) -> Res<app_core::notion::ImportPreview> {
    let app = Arc::clone(&app);
    run(move || app.notion_preview(&path, columns)).await
}

#[tauri::command]
async fn notion_apply(
    app: State<'_, Arc<App>>,
    path: String,
    columns: Vec<app_core::notion::Column>,
    stages: std::collections::HashMap<String, String>,
    only_empty: bool,
) -> Res<(usize, UndoState)> {
    let app = Arc::clone(&app);
    run(move || app.notion_apply(&path, columns, stages, only_empty)).await
}

#[tauri::command]
async fn sample_waves(app: State<'_, Arc<App>>, paths: Vec<String>) -> Res<std::collections::HashMap<String, Vec<u8>>> {
    let app = Arc::clone(&app);
    run(move || app.sample_waves(paths)).await
}

#[tauri::command]
async fn audio_info(app: State<'_, Arc<App>>, rel: String) -> Res<app_core::audio::AudioInfo> {
    let app = Arc::clone(&app);
    run(move || app.audio_info(&rel)).await
}

#[tauri::command]
async fn set_root_artist(app: State<'_, Arc<App>>, path: String, artist: String) -> Res<app_core::Config> {
    let app = Arc::clone(&app);
    run(move || app.set_root_artist(&path, &artist)).await
}

#[tauri::command]
async fn set_vst3_many(app: State<'_, Arc<App>>, list: Vec<(String, Option<String>)>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.set_vst3_many(&list)).await
}

#[tauri::command]
async fn set_vst3(app: State<'_, Arc<App>>, name: String, uid: Option<String>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.set_vst3(&name, uid.as_deref())).await
}

#[tauri::command]
async fn midi_summary(app: State<'_, Arc<App>>) -> Res<app_core::midi_view::MidiSummary> {
    let app = Arc::clone(&app);
    run(move || app.midi_summary()).await
}

/// The full path of a project file (to copy it).
#[tauri::command]
async fn abs_path(app: State<'_, Arc<App>>, rel: String) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.project_path(&rel).map(|p| p.to_string_lossy().into_owned()).ok_or_else(|| tr!("fs.notFoundShort"))).await
}

/// A page of Fatstep for Artists in the browser. Only that site: the path is checked, the
/// host is fixed here.
#[tauri::command]
async fn open_fatstep(path: String) -> Res<()> {
    run(move || {
        let path = if path.starts_with('/') { path } else { format!("/{path}") };
        if !path.chars().all(|c| c.is_ascii_alphanumeric() || "/-_.?=&".contains(c)) {
            return Err("bad path".into());
        }
        std::process::Command::new("explorer").arg(format!("https://artists.fatstep.com{path}")).spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// Shows a file or folder of the projects in the Explorer.
#[tauri::command]
async fn samples(app: State<'_, Arc<App>>, q: app_core::samples_view::SampleQuery) -> Res<app_core::samples_view::SamplePage> {
    let app = Arc::clone(&app);
    run(move || app.samples(q)).await
}

/// The picture shown under the pointer while a file is dragged out (Spool's icon, written
/// once into the data folder: the drag needs a file).
#[tauri::command]
fn drag_icon(app: State<'_, Arc<App>>) -> Res<String> {
    let p = PathBuf::from(app.storage().dir).join("drag-icon.png");
    if !p.exists() {
        std::fs::write(&p, include_bytes!("../icons/32x32.png")).map_err(|e| e.to_string())?;
    }
    Ok(p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn als_diff(app: State<'_, Arc<App>>, a: String, b: String) -> Res<app_core::als_diff::AlsDiff> {
    let app = Arc::clone(&app);
    run(move || app.als_diff(&a, &b)).await
}

#[tauri::command]
async fn midi_export(app: State<'_, Arc<App>>, rel: String, semis: i32, rate: f64, target: Option<f64>, drums: bool) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.midi_export(&rel, semis, rate, target, drums)).await
}

#[tauri::command]
async fn harmony(app: State<'_, Arc<App>>, rel: String) -> Res<app_core::midi::Harmony> {
    let app = Arc::clone(&app);
    run(move || app.harmony(&rel)).await
}

#[tauri::command]
async fn write_midi(app: State<'_, Arc<App>>, name: String, notes: Vec<(u8, f64, f64)>, bpm: f64) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.write_midi(&name, &notes, bpm)).await
}

#[tauri::command]
async fn arrangement(app: State<'_, Arc<App>>, rel: String) -> Res<als::Arrangement> {
    let app = Arc::clone(&app);
    run(move || app.arrangement(&rel)).await
}

#[tauri::command]
async fn clip_midi(app: State<'_, Arc<App>>, hash: String) -> Res<tauri::ipc::Response> {
    let app = Arc::clone(&app);
    run(move || app.clip_midi(&hash).map(|(b, _)| tauri::ipc::Response::new(b))).await
}

#[tauri::command]
async fn clip_uses(app: State<'_, Arc<App>>, hash: String) -> Res<Vec<app_core::midi_view::ClipUse>> {
    let app = Arc::clone(&app);
    run(move || app.clip_uses(&hash)).await
}

#[tauri::command]
async fn sample_dupes(app: State<'_, Arc<App>>) -> Res<Vec<app_core::samples_view::Dupes>> {
    let app = Arc::clone(&app);
    run(move || app.sample_dupes()).await
}

#[tauri::command]
async fn sort_samples(app: State<'_, Arc<App>>, paths: Vec<String>, family: Option<String>, cat: Option<String>) -> Res<usize> {
    let app = Arc::clone(&app);
    run(move || app.sort_samples(&paths, family.as_deref(), cat.as_deref())).await
}

#[tauri::command]
async fn ui_pref(app: State<'_, Arc<App>>, key: String) -> Res<Option<Value>> {
    let app = Arc::clone(&app);
    run(move || app.ui_pref(&key)).await
}

#[tauri::command]
async fn set_ui_pref(app: State<'_, Arc<App>>, key: String, value: Value) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.set_ui_pref(&key, value)).await
}

#[tauri::command]
async fn style_review(app: State<'_, Arc<App>>) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.style_review()).await
}

#[tauri::command]
async fn style_keep(app: State<'_, Arc<App>>, heard: String, mine: String) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.style_keep(&heard, &mine)).await
}

#[tauri::command]
async fn style_word_yes(app: State<'_, Arc<App>>, word: String, name: String, heard: Option<String>) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.style_word_yes(&word, &name, heard.as_deref())).await
}

#[tauri::command]
async fn style_word_no(app: State<'_, Arc<App>>, word: String) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.style_word_no(&word)).await
}

#[tauri::command]
async fn style_answer(app: State<'_, Arc<App>>, id: i64, style: Option<String>) -> Res<app_core::style_review::StyleAnswer> {
    let app = Arc::clone(&app);
    run(move || app.style_answer(id, style)).await
}

#[tauri::command]
async fn style_skipped(app: State<'_, Arc<App>>) -> Res<Vec<i64>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.style_skipped())).await
}

#[tauri::command]
async fn style_dismiss(app: State<'_, Arc<App>>, mine: String, heard: String) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.style_dismiss(&mine, &heard)).await
}

#[tauri::command]
async fn set_style_vocab(
    app: State<'_, Arc<App>>,
    words: Vec<String>,
    links: Vec<(String, String)>,
    names: Option<Vec<(String, String)>>,
) -> Res<app_core::style_review::StyleReview> {
    let app = Arc::clone(&app);
    run(move || app.set_style_vocab(words, links, names)).await
}

#[tauri::command]
async fn favorites(app: State<'_, Arc<App>>) -> Res<Vec<app_core::favorites::Favorite>> {
    let app = Arc::clone(&app);
    run(move || app.favorites()).await
}

#[tauri::command]
async fn set_favorite(app: State<'_, Arc<App>>, fav: app_core::favorites::Favorite, on: bool) -> Res<Vec<app_core::favorites::Favorite>> {
    let app = Arc::clone(&app);
    run(move || app.set_favorite(fav, on)).await
}

#[tauri::command]
async fn sample_rows(app: State<'_, Arc<App>>, paths: Vec<String>) -> Res<Vec<app_core::samples_view::SampleRow>> {
    let app = Arc::clone(&app);
    run(move || app.sample_rows(&paths)).await
}

#[tauri::command]
async fn sample_questions(app: State<'_, Arc<App>>, skip: Vec<String>) -> Res<app_core::sample_ask::Questions> {
    let app = Arc::clone(&app);
    run(move || app.sample_questions(&skip)).await
}

#[tauri::command]
async fn samples_alike_unsorted(app: State<'_, Arc<App>>, paths: Vec<String>) -> Res<Vec<app_core::samples_view::SampleRow>> {
    let app = Arc::clone(&app);
    run(move || app.samples_alike_unsorted(&paths)).await
}

#[tauri::command]
async fn sample_folder(app: State<'_, Arc<App>>, dir: String) -> Res<Vec<app_core::samples_view::SampleRow>> {
    let app = Arc::clone(&app);
    run(move || app.sample_folder(&dir)).await
}

#[tauri::command]
async fn trash_samples(app: State<'_, Arc<App>>, paths: Vec<String>) -> Res<usize> {
    let app = Arc::clone(&app);
    run(move || app.trash_samples(&paths)).await
}

#[tauri::command]
async fn unsorted_folders(app: State<'_, Arc<App>>, skip: Vec<String>) -> Res<Vec<app_core::samples_view::FolderToSort>> {
    let app = Arc::clone(&app);
    run(move || app.unsorted_folders(&skip)).await
}

#[tauri::command]
async fn sort_sample_folder(app: State<'_, Arc<App>>, dir: String, family: String, cat: Option<String>) -> Res<usize> {
    let app = Arc::clone(&app);
    run(move || app.sort_sample_folder(&dir, &family, cat.as_deref())).await
}

#[tauri::command]
async fn sample_songs(app: State<'_, Arc<App>>, path: String) -> Res<Vec<i64>> {
    let app = Arc::clone(&app);
    run(move || app.sample_songs(&path)).await
}

#[tauri::command]
async fn comments(app: State<'_, Arc<App>>, id: i64) -> Res<Vec<app_core::Comment>> {
    let app = Arc::clone(&app);
    run(move || app.comments(id)).await
}

#[tauri::command]
async fn comment_edit(app: State<'_, Arc<App>>, id: i64, c: app_core::Comment, remove: bool) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.comment_edit(id, c, remove)).await
}

#[tauri::command]
async fn ref_tracks(app: State<'_, Arc<App>>) -> Res<Vec<String>> {
    let app = Arc::clone(&app);
    run(move || Ok(app.ref_tracks())).await
}

#[tauri::command]
async fn ref_track(app: State<'_, Arc<App>>, path: String, add: bool) -> Res<Vec<String>> {
    let app = Arc::clone(&app);
    run(move || app.ref_track(&path, add)).await
}

#[tauri::command]
async fn file_to_song(app: State<'_, Arc<App>>, rel: String) -> Res<i64> {
    let app = Arc::clone(&app);
    run(move || app.file_to_song(&rel)).await
}

#[tauri::command]
async fn merge_artists(app: State<'_, Arc<App>>, from: String, into: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.merge_artists(&from, &into)).await
}

#[tauri::command]
async fn artist_to_feat(app: State<'_, Arc<App>>, name: String, main: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.artist_to_feat(&name, &main)).await
}

#[tauri::command]
async fn feat_back(app: State<'_, Arc<App>>, name: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.feat_back(&name)).await
}

#[tauri::command]
async fn artist_role(app: State<'_, Arc<App>>, name: String, role: String, target: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.artist_role(&name, &role, &target)).await
}

#[tauri::command]
async fn unmerge_artist(app: State<'_, Arc<App>>, from: String) -> Res<UndoState> {
    let app = Arc::clone(&app);
    run(move || app.unmerge_artist(&from)).await
}

#[tauri::command]
async fn artist_board_drop(app: State<'_, Arc<App>>, name: String, paths: Vec<String>) -> Res<app_core::BoardDrop> {
    let app = Arc::clone(&app);
    run(move || app.artist_board_drop(&name, paths)).await
}

#[tauri::command]
async fn release_board_drop(app: State<'_, Arc<App>>, paths: Vec<String>) -> Res<(Vec<String>, app_core::BoardDrop)> {
    let app = Arc::clone(&app);
    run(move || app.release_board_drop(paths)).await
}

#[tauri::command]
async fn board_drop(app: State<'_, Arc<App>>, id: i64, paths: Vec<String>) -> Res<app_core::BoardDrop> {
    let app = Arc::clone(&app);
    run(move || app.board_drop(id, paths)).await
}

#[tauri::command]
async fn midi_page(app: State<'_, Arc<App>>, q: app_core::midi_view::MidiQuery) -> Res<app_core::midi_view::MidiPage> {
    let app = Arc::clone(&app);
    run(move || app.midi_page(q)).await
}

#[tauri::command]
async fn similar_samples(app: State<'_, Arc<App>>, path: String) -> Res<app_core::samples_view::Similar> {
    let app = Arc::clone(&app);
    run(move || app.similar_samples(&path, 40)).await
}

#[tauri::command]
async fn reveal(app: State<'_, Arc<App>>, rel: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        // a project's file, or a library's (a sample, a MIDI file: absolute)
        let p = app.project_path(&rel).or_else(|| app.library_path(&rel)).or_else(|| app.export_path(&rel)).filter(|p| p.exists()).ok_or_else(|| tr!("fs.notFoundShort"))?;
        // Explorer reads only backslashes (a "C:/..." path opens Documents instead), and wants
        // /select,"path" with the quotes around the path alone
        let path = p.to_string_lossy().replace('/', "\\");
        let arg = if p.is_dir() { format!("\"{path}\"") } else { format!("/select,\"{path}\"") };
        let mut cmd = std::process::Command::new("explorer");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.raw_arg(arg);
        }
        #[cfg(not(windows))]
        cmd.arg(arg);
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// media://localhost/<path>: audio and pictures under the projects root, with byte ranges.
/// Readable by the page's own `fetch` (a MIDI file played by the synth, a sample decoded),
/// not only by `<audio>`/`<img>`: the page is another origin (tauri.localhost), so CORS.
fn media(app: &Arc<App>, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let mut r = media_answer(app, req);
    r.headers_mut().insert(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, tauri::http::HeaderValue::from_static("*"));
    // the page reads a file's size from the answer to its first byte (a long recording is
    // then played as it is read, never loaded whole: see midiPlayer's STREAM_BYTES)
    r.headers_mut().insert(tauri::http::header::ACCESS_CONTROL_EXPOSE_HEADERS, tauri::http::HeaderValue::from_static("Content-Range, Content-Length"));
    r
}

fn media_answer(app: &Arc<App>, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, header};
    let rel = app_core::media::percent_decode(req.uri().path().trim_start_matches('/'));
    // ?thumb=N: a small JPEG of a picture (made once, then kept)
    if let Some(n) = req.uri().query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("thumb="))).and_then(|n| n.parse::<u32>().ok()) {
        return match app.thumb(&rel, n) {
            Some(b) => Response::builder().status(200).header(header::CONTENT_TYPE, "image/jpeg").header(header::CACHE_CONTROL, "max-age=86400").body(b).unwrap(),
            None => Response::builder().status(404).body(Vec::new()).unwrap(),
        };
    }
    let range = req.headers().get(header::RANGE).and_then(|v| v.to_str().ok());
    let a = match app.media_path(&rel) {
        Some(p) => app_core::media::serve(&app.playable(p), range),
        None => app_core::media::Answer { status: 404, mime: "text/plain", range: None, body: Vec::new() },
    };
    let mut r = Response::builder()
        .status(a.status)
        .header(header::CONTENT_TYPE, a.mime)
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some(cr) = a.range {
        r = r.header(header::CONTENT_RANGE, cr);
    }
    r.body(a.body).unwrap()
}

/// Markers changed in Spool's arrangement, kept per project until a copy carries them.
#[tauri::command]
async fn markers_get(app: State<'_, Arc<App>>, rel: String) -> Res<Option<Vec<(f64, String)>>> {
    let app = Arc::clone(&app);
    run(move || app.markers_get(&rel)).await
}

#[tauri::command]
async fn markers_set(app: State<'_, Arc<App>>, rel: String, list: Option<Vec<(f64, String)>>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.markers_set(&rel, list)).await
}

/// A copy with what was ticked before opening in Live (repairs, markers, cleanup); its path.
#[tauri::command]
async fn live_copy(app: State<'_, Arc<App>>, rel: String, preset: Value) -> Res<String> {
    let app = Arc::clone(&app);
    run(move || app.live_copy(&rel, preset)).await
}

/// Opens a project in Live (the system's handler for .als), only under the projects root.
#[tauri::command]
async fn open_in_live(app: State<'_, Arc<App>>, rel: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        let p = app.project_path(&rel).ok_or_else(|| tr!("err.projectNotFound"))?;
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("als")) || !p.is_file() {
            return Err(tr!("err.projectNotFound"));
        }
        // through Explorer (the .als handler, Live), not cmd: a name with % or ^ stays a name
        let path = p.to_string_lossy().replace('/', "\\");
        let mut cmd = std::process::Command::new("explorer");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.raw_arg(format!("\"{path}\""));
        }
        #[cfg(not(windows))]
        cmd.arg(path);
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// A newer Spool, as the update server announces it.
#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
    date: Option<String>,
}

/// Is there a newer version? (None: up to date.) Only an update signed with Spool's key is
/// ever offered: the public key is in tauri.conf.json, the private one never leaves the publisher's machine.
#[tauri::command]
async fn update_check(app: tauri::AppHandle) -> Res<Option<UpdateInfo>> {
    use tauri_plugin_updater::UpdaterExt;
    let up = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(up.map(|u| UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone(), date: u.date.map(|d| d.to_string()) }))
}

/// Downloads the new version, checks its signature, installs it and restarts, first: a scan
/// under way is stopped (what it read is kept, the base is written in transactions, the rest is
/// read again after the restart); a repair writing a copy is waited for (a copy cut short
/// would be a broken file in the user's folder), each project takes seconds.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, core: State<'_, Arc<App>>) -> Res<()> {
    let core = Arc::clone(&core);
    // a repair: until its copies are written (a long batch: at most 10 minutes, then refused)
    for _ in 0..1200 {
        if !core.repair_state().running {
            break;
        }
        tokio_sleep(500).await;
    }
    if core.repair_state().running {
        return Err(tr!("update.busy"));
    }
    // a scan: stopped, waited for a moment (it stops between two files), then the update goes
    // on even if it has not (the base is safe: each write is one transaction)
    if core.scan_state().running {
        core.cancel_scan();
        for _ in 0..40 {
            if !core.scan_state().running {
                break;
            }
            tokio_sleep(250).await;
        }
    }
    install_update(&app).await?;
    app.restart();
}

async fn tokio_sleep(ms: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(Duration::from_millis(ms))).await;
}

/// Checks, downloads, verifies the signature and runs the installer (the same path for the
/// button and for the self-test below).
async fn install_update(app: &tauri::AppHandle) -> Res<String> {
    use tauri_plugin_updater::UpdaterExt;
    let up = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(u) = up else { return Err(tr!("update.upToDate")) };
    let v = u.version.clone();
    u.download_and_install(|_, _| {}, || {}).await.map_err(|e| tr!("update.failed", e = e))?;
    Ok(v)
}

/// SPOOL_UPDATE_SELFTEST=1: installs the newer version at start, without a click, and writes
/// what happened to update-selftest.log in the data folder. Only for testing the update path
/// end to end on a throwaway build (tools/update_selftest.py); nothing else sets it.
fn update_selftest(app: tauri::AppHandle, dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let log = |line: String| {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("update-selftest.log")) {
                let _ = writeln!(f, "{} {line}", app.package_info().version);
            }
        };
        log("start".into());
        match install_update(&app).await {
            Ok(v) => {
                log(format!("installed {v}, restarting"));
                app.restart();
            }
            Err(e) => log(format!("refused: {e}")),
        }
    });
}

/// Where the start is: the window shows at once and says it while the base opens (an
/// update of an old catalogue copies it first, then converts it: a minute on a large one).
#[derive(Clone, serde::Serialize)]
struct BootStatus {
    /// "opening", "creating", "upgrading", "ready" or "error"
    stage: &'static str,
    /// the base's size, in MB, when there is one
    mb: u64,
    from: usize,
    to: usize,
    error: Option<String>,
    secs: f64,
}

struct Boot {
    status: std::sync::Mutex<BootStatus>,
    started: std::time::Instant,
}

#[tauri::command]
fn boot_status(boot: State<'_, Boot>) -> BootStatus {
    let mut s = boot.status.lock().unwrap().clone();
    s.secs = boot.started.elapsed().as_secs_f64();
    s
}

// The window's `additionalBrowserArgs` (tauri.conf.json) add `CalculateNativeWinOcclusion` to
// the features wry turns off by default (they are restated there: setting the args replaces
// wry's). With it on, WebView2 stops drawing a window it judges hidden (screens off for the
// night) and can fail to start again: Spool, left open overnight, answered but no longer
// redrew, and kept its old size when moved to another screen.
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // samples and MIDI files dragged out of the window, into Live or Explorer
        .plugin(tauri_plugin_drag::init())
        // audio and pictures: answered off the window's thread. Synchronous, every range
        // request of a playing file ran on the UI thread and waited for the base's reader,
        // so a long read elsewhere froze the window (a right-click took seconds).
        .register_asynchronous_uri_scheme_protocol("media", |ctx, req, responder| {
            let handle = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                // nothing to serve while the base opens
                let Some(app) = handle.try_state::<Arc<App>>() else {
                    responder.respond(tauri::http::Response::builder().status(503).body(Vec::new()).unwrap());
                    return;
                };
                responder.respond(media(&app, &req));
            });
        })
        .setup(|app| {
            // PLUGIN_FIXER_DATA: another data folder (tests on a scratch copy, never the user's base)
            let dir = match std::env::var_os("PLUGIN_FIXER_DATA") {
                Some(d) => PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            // The base opens on its own thread: done here, before the window's first paint, a
            // large old catalogue (a copy, then the conversion) left a black window for a
            // minute with no word. The page asks `boot_status` until "ready".
            let (found, to) = App::base_formats(&dir);
            let stage = match found {
                None => "creating",
                Some(v) if v > 0 && v < to => "upgrading",
                Some(_) => "opening",
            };
            app.manage(Boot {
                status: std::sync::Mutex::new(BootStatus { stage, mb: App::base_bytes(&dir) / 1_000_000, from: found.unwrap_or(0), to, error: None, secs: 0.0 }),
                started: std::time::Instant::now(),
            });
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let boot = handle.state::<Boot>();
                match App::open(&dir) {
                    Ok(core) => {
                        app_core::diag::log(format!("start: Spool {} on {} {}", handle.package_info().version, std::env::consts::OS, std::env::consts::ARCH));
                        let h = handle.clone();
                        core.on_change(Arc::new(move |st: &ScanState| {
                            let _ = h.emit("changed", st.clone());
                        }));
                        core.start_background(Duration::from_secs(60));
                        handle.manage(core);
                        boot.status.lock().unwrap().stage = "ready";
                        if std::env::var_os("SPOOL_UPDATE_SELFTEST").is_some() {
                            update_selftest(handle.clone(), dir.clone());
                        }
                    }
                    Err(e) => {
                        {
                            let mut s = boot.status.lock().unwrap();
                            s.stage = "error";
                            s.error = Some(e.clone());
                        }
                        // said plainly, not a silent crash (a base from a newer Spool, a disk full...)
                        use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                        handle.dialog().message(tr!("desktop.cannotOpenBase", e = e)).kind(MessageDialogKind::Error).title("Spool").blocking_show();
                        handle.exit(1);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            songs,
            style_guess,
            stats,
            scan_state,
            audio_state,
            audio_enable,
            audio_pause,
            audio_remove,
            scan,
            root,
            set_root,
            import_prototype,
            config,
            set_config,
            telemetry,
            cancel_scan,
            folder_peek,
            add_root,
            remove_root,
            pick_folder,
            reveal,
            abs_path,
            midi_summary,
            set_vst3,
            set_vst3_many,
            set_root_artist,
            audio_info,
            sample_waves,
            pick_file,
            notion_preview,
            notion_apply,
            fs_folders,
            fs_media,
            fs_reorg,
            fs_plan,
            fs_mkdir,
            fs_move,
            fs_to_board,
            artist_suggestions,
            home_stats,
            repair_run_preset,
            deep_search,
            drives,
            project_files,
            midi_inputs,
            take_save,
            takes,
            take_delete,
            take_keep,
            take_smf,
            midi_open,
            midi_close,
            progress,
            reset_base,
            open_in_live,
            markers_get,
            markers_set,
            live_copy,
            song,
            song_edit,
            check_set,
            suggestion_act,
            accept_all,
            songs_edit,
            undo_state,
            undo,
            board_note,
            board_add,
            artists,
            artist,
            artist_save,
            artist_board_note,
            artist_board_remove,
            artist_board_copy,
            artist_board_meta,
            open_link,
            refs,
            ref_mark,
            analyse_now,
            redo,
            media_list,
            loose_audio,
            song_resources,
            diagnostic_report,
            reading_report,
            group_preview,
            folder_neighbours,
            set_grouping,
            set_cover,
            board_remove,
            board_meta,
            merge_song,
            split_song,
            fixable,
            repair_plan,
            repair_state,
            written_copies,
            samples,
            similar_samples,
            midi_page,
            board_drop,
            artist_board_drop,
            release_board_drop,
            merge_artists,
            file_to_song,
            comments,
            comment_edit,
            als_diff,
            sample_dupes,
            clip_midi,
            arrangement,
            harmony,
            write_midi,
            clip_uses,
            midi_export,
            sample_songs,
            sort_samples,
            sort_sample_folder,
            unsorted_folders,
            sample_questions,
            favorites,
            style_review,
            style_keep,
            style_answer,
            style_skipped,
            style_dismiss,
            style_word_yes,
            style_word_no,
            set_style_vocab,
            ui_pref,
            set_ui_pref,
            set_favorite,
            sample_rows,
            samples_alike_unsorted,
            sample_folder,
            trash_samples,
            ref_tracks,
            ref_track,
            unmerge_artist,
            artist_role,
            artist_to_feat,
            feat_back,
            drag_icon,
            boot_status,
            storage,
            trim_backups,
            open_data_dir,
            update_check,
            open_fatstep,
            update_install,
            save_text,
            repair_run,
            repair_view,
            set_ignored,
            set_trial,
            create_plan,
            create_song,
            track_view,
            release_arrange,
            release_arrange_plan,
            board_link,
            board_unlink,
            releases,
            release_hints,
            release_save,
            release_delete,
            atelier_presets,
            atelier_default,
            atelier_save_presets,
            atelier_preview,
            atelier_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}

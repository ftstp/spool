// Desktop shell: exposes the core to the interface and forwards its changes. No logic here.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
async fn scan(app: State<'_, Arc<App>>, full: bool) -> Res<bool> {
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn_blocking(move || app.scan(full)).await.map_err(|e| e.to_string())?
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
    let id: i64 = h("x-song").and_then(|v| v.parse().ok()).ok_or_else(|| tr!("desktop.songMissing"))?;
    let name = h("x-name").map(|n| app_core::media::percent_decode(&n)).ok_or_else(|| tr!("err.noName"))?;
    let app = Arc::clone(&app);
    run(move || app.board_add(id, &name, &data)).await
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

#[tauri::command]
async fn deep_search(app: State<'_, Arc<App>>) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || app.deep_search()).await
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
async fn reveal(app: State<'_, Arc<App>>, rel: String) -> Res<()> {
    let app = Arc::clone(&app);
    run(move || {
        let p = app.project_path(&rel).filter(|p| p.exists()).ok_or_else(|| tr!("fs.notFoundShort"))?;
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
fn media(app: &Arc<App>, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
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
        Some(p) => app_core::media::serve(&p, range),
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

/// Downloads the new version, checks its signature, installs it and restarts. Refused while
/// a scan or a repair is under way (it would be cut short).
#[tauri::command]
async fn update_install(app: tauri::AppHandle, core: State<'_, Arc<App>>) -> Res<()> {
    if core.scan_state().running || core.repair_state().running {
        return Err(tr!("update.busy"));
    }
    install_update(&app).await?;
    app.restart();
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // audio and pictures: answered off the window's thread. Synchronous, every range
        // request of a playing file ran on the UI thread and waited for the base's reader,
        // so a long read elsewhere froze the window (a right-click took seconds).
        .register_asynchronous_uri_scheme_protocol("media", |ctx, req, responder| {
            let handle = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let app: State<Arc<App>> = handle.state();
                responder.respond(media(&app, &req));
            });
        })
        .setup(|app| {
            // PLUGIN_FIXER_DATA: another data folder (tests on a scratch copy, never the user's base)
            let dir = match std::env::var_os("PLUGIN_FIXER_DATA") {
                Some(d) => PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            let core = match App::open(&dir) {
                Ok(c) => c,
                Err(e) => {
                    // said plainly, not a silent crash (a base from a newer Spool, a disk full...)
                    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                    app.dialog().message(tr!("desktop.cannotOpenBase", e = e)).kind(MessageDialogKind::Error).title("Spool").blocking_show();
                    return Err(e.into());
                }
            };
            let handle = app.handle().clone();
            core.on_change(Arc::new(move |st: &ScanState| {
                let _ = handle.emit("changed", st.clone());
            }));
            core.start_background(Duration::from_secs(60));
            app.manage(core);
            if std::env::var_os("SPOOL_UPDATE_SELFTEST").is_some() {
                update_selftest(app.handle().clone(), dir.clone());
            }
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
            progress,
            reset_base,
            open_in_live,
            song,
            song_edit,
            check_set,
            suggestion_act,
            accept_all,
            undo_state,
            undo,
            board_note,
            board_add,
            board_remove,
            board_meta,
            merge_song,
            split_song,
            fixable,
            repair_plan,
            repair_state,
            written_copies,
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

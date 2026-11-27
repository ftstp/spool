// The only place that knows how the interface talks to the core: Tauri today, HTTP for the
// web version later. Pages import these functions, never `invoke` or `listen`.
import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, fmtNum } from "../i18n/index.svelte";
import type { Overview } from "./bindings/Overview";
import type { ScanState } from "./bindings/ScanState";
import type { SongItem } from "./bindings/SongItem";
import type { Stats } from "./bindings/Stats";
import type { SongDetail } from "./bindings/SongDetail";
import type { StyleReview } from "./bindings/StyleReview";
import type { StyleAnswer } from "./bindings/StyleAnswer";
import type { StyleOpt } from "./bindings/StyleOpt";
import type { UndoState } from "./bindings/UndoState";

import type { Config } from "./bindings/Config";
import type { ArtistGuess } from "./bindings/ArtistGuess";
import type { HomeStats } from "./bindings/HomeStats";
import type { Progress } from "./bindings/Progress";
export type { ArtistGuess, HomeStats, Progress };

export type { Overview, ScanState, SongItem, Stats, SongDetail, UndoState, Config, StyleOpt };
export type { Check } from "./bindings/Check";
export type { CreatePlan } from "./bindings/CreatePlan";
export type { CreatePart } from "./bindings/CreatePart";
export type { CreateReq } from "./bindings/CreateReq";
export type { DevView } from "./bindings/DevView";
export type { Release } from "./bindings/Release";
export type { ReleaseHint } from "./bindings/ReleaseHint";
export type { ChainView } from "./bindings/ChainView";
export type { Candidate } from "./bindings/Candidate";
export type { FileItem } from "./bindings/FileItem";
export type { Item } from "./bindings/Item";
export type { Fixable } from "./bindings/Fixable";
export type { RepairState } from "./bindings/RepairState";
export type { WrittenCopy } from "./bindings/WrittenCopy";
export type UpdateInfo = { version: string; current: string; notes: string | null; date: string | null };
/** A newer Spool (null: up to date). */
export const updateCheck = () => (inTauri ? invoke<UpdateInfo | null>("update_check") : Promise.resolve(null));
/** Downloads, checks the signature, installs, restarts. */
export const updateInstall = () => (inTauri ? invoke<void>("update_install") : readOnly());
export const appVersion = () => (inTauri ? import("@tauri-apps/api/app").then((m) => m.getVersion()) : Promise.resolve("dev"));
import type { WrittenCopy } from "./bindings/WrittenCopy";
import type { Storage } from "./bindings/Storage";
import type { SamplePage } from "./bindings/SamplePage";
import type { SampleQuery } from "./bindings/SampleQuery";
export type { SamplePage, SampleQuery };
export type { SampleRow } from "./bindings/SampleRow";
import type { Similar } from "./bindings/Similar";
import type { MidiPage } from "./bindings/MidiPage";
import type { BoardDrop } from "./bindings/BoardDrop";
/** One artist merged into another (songs, releases, folders' artist; a rule for the scan). */
export const mergeArtists = (from: string, into: string) => (inTauri ? invoke<UndoState>("merge_artists", { from, into }) : readOnly());
export const unmergeArtist = (from: string) => (inTauri ? invoke<UndoState>("unmerge_artist", { from }) : readOnly());
/** A name taken for one of the user's artists that is a featured artist: its songs go to `main`. */
export const artistToFeat = (name: string, main: string) => (inTauri ? invoke<UndoState>("artist_to_feat", { name, main }) : readOnly());
export const featBack = (name: string) => (inTauri ? invoke<UndoState>("feat_back", { name }) : readOnly());
/** The Artists page's "who is it?": "me", "alias" (of `target`), "guest" (on `target`'s songs,
 *  empty: the projects folder's artist), "none" (not an artist). */
export type ArtistRole = "me" | "alias" | "guest" | "none";
export const artistRole = (name: string, role: ArtistRole, target = "") => (inTauri ? invoke<UndoState>("artist_role", { name, role, target }) : readOnly());
/** Files dropped on a song, by their paths: linked when in the projects, copied when not. */
export const boardDrop = (id: number, paths: string[]) => (inTauri ? invoke<BoardDrop>("board_drop", { id, paths }) : readOnly());
/** A file dragged out of the window (into Live, Explorer...): always as a copy, so a drop on a
 * folder in Explorer never moves a sample out of its library. `path`: absolute. */
let dragIcon: Promise<string> | null = null;
export async function dragOut(path: string) {
  if (!inTauri) return;
  const { startDrag } = await import("@crabnebula/tauri-plugin-drag");
  const icon = await (dragIcon ??= invoke<string>("drag_icon"));
  await startDrag({ item: [path.replace(/\//g, "\\")], icon, mode: "copy" });
}

/** Files dropped from the system onto the window: (kind, paths, position in CSS pixels). */
export async function onFileDrop(f: (kind: "over" | "drop" | "leave", paths: string[], x: number, y: number) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    const r = window.devicePixelRatio || 1;
    if (p.type === "leave") return f("leave", [], 0, 0);
    const x = p.position.x / r, y = p.position.y / r;
    if (p.type === "drop") f("drop", p.paths, x, y);
    else f("over", p.type === "enter" ? p.paths : [], x, y);
  });
}
import type { MidiQuery } from "./bindings/MidiQuery";
export type { MidiPage, MidiQuery };
export type { MidiRow } from "./bindings/MidiRow";
/** A page of MIDI files (MIDI page). The browser preview reads public/dev-midi.json. */
export const midiPage = (q: MidiQuery) =>
  inTauri
    ? invoke<MidiPage>("midi_page", { q })
    : fetch("/dev-midi.json").then((r) => (r.ok ? (r.json() as Promise<MidiPage>) : ({ total: 0, all: 0, rows: [], counts: [] } as MidiPage)));
export type { Similar };
/** The samples that sound like one (it is listened to first if it was not yet). */
export const similarSamples = (path: string) => (inTauri ? invoke<Similar>("similar_samples", { path }) : readOnly());
/** A page of samples (Samples page). The browser preview reads public/dev-samples.json. */
export const samples = (q: SampleQuery) =>
  inTauri
    ? invoke<SamplePage>("samples", { q })
    : fetch("/dev-samples.json").then((r) => (r.ok ? (r.json() as Promise<SamplePage>) : ({ total: 0, all: 0, rows: [], counts: [] } as SamplePage)));
export type { Storage };
/** What Spool keeps on this computer (its own folder only). */
export const storage = () => (inTauri ? invoke<Storage>("storage") : Promise.resolve(devStorage()));
/** Removes the safety copies but the newest. */
export const trimBackups = () => (inTauri ? invoke<Storage>("trim_backups") : readOnly());
export const openDataDir = () => (inTauri ? invoke<void>("open_data_dir") : readOnly());
const devStorage = (): Storage => ({ dir: "C:\Users\you\AppData\Roaming\app.pluginfixer.desktop", base: 309e6, journal: 43e6, samples: 48e6, app: 31e6, webview: 101e6, midi: 180e6, backups: 577e6, backup_count: 4, trimmable: 302e6, cache: 8e6, model: 0, total: 894e6 });
/** Where the start is: the base opens (or is converted) while the window shows. */
export type BootStatus = { stage: "opening" | "creating" | "upgrading" | "ready" | "error"; mb: number; from: number; to: number; error: string | null; secs: number };
export const bootStatus = () => (inTauri ? invoke<BootStatus>("boot_status") : Promise.resolve(devBoot()));
// the browser preview: "?boot=upgrading" (or opening, error) shows the start screen for 8 s
const bootT0 = Date.now();
function devBoot(): BootStatus {
  const want = new URLSearchParams(location.search).get("boot") as BootStatus["stage"] | null;
  const secs = (Date.now() - bootT0) / 1000;
  const stage = want && (secs < 8 || want === "error") ? want : "ready";
  return { stage, mb: 275, from: 3, to: 4, error: stage === "error" ? "base made by a newer Spool (format 9, this one reads up to 4)" : null, secs };
}
export const writtenCopies = () => (inTauri ? invoke<WrittenCopy[]>("written_copies") : Promise.resolve([] as WrittenCopy[]));
/** "Save as": the text written where the user picks. Null: cancelled. */
export const saveText = (title: string, name: string, ext: string, text: string) =>
  inTauri ? invoke<string | null>("save_text", { title, name, ext, text }) : readOnly();
export type { RepairResult } from "./bindings/RepairResult";
export type { RepairView } from "./bindings/RepairView";
export type { PluginNeed } from "./bindings/PluginNeed";
import type { RepairView } from "./bindings/RepairView";
import type { Fixable } from "./bindings/Fixable";
import type { RepairState } from "./bindings/RepairState";

// Every call to the core, watched for the usage statistics (lib/telemetry.ts): a command
// that fails reports its error, one that succeeds may count a feature. The watcher decides;
// the core sends nothing unless the user said yes.
type Watcher = (cmd: string, args: InvokeArgs | undefined, error: unknown | undefined, failed: boolean) => void;
let watcher: Watcher | null = null;
export const watchCommands = (w: Watcher) => (watcher = w);
function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  return tauriInvoke<T>(cmd, args, options).then(
    (v) => {
      try { watcher?.(cmd, args, undefined, false); } catch { /* never in the way */ }
      return v;
    },
    (e) => {
      try { watcher?.(cmd, args, e, true); } catch { /* never in the way */ }
      throw e;
    },
  );
}
/** One event of the usage statistics. Never fails, never waited for: the core checks the
 *  user's choice and what may go out (crates/core/src/telemetry.rs). */
export function telemetry(event: "tool_opened" | "tool_feature_used" | "tool_error", props?: Record<string, string>) {
  if (inTauri) tauriInvoke("telemetry", { event, props }).catch(() => {});
}

// Outside the desktop app (a browser preview while designing), the data comes from a
// frozen copy written by `core-dump` into public/dev-data.json (never committed).
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
let dev: Promise<{ songs: SongItem[]; overview: Overview; stats: Stats; home: HomeStats; progress: Progress }> | null = null;
const devData = () => (dev ??= fetch("/dev-data.json").then((r) => r.json()));

export const overview = () => (inTauri ? invoke<Overview>("overview") : devData().then((d) => d.overview));
export const songs = () => (inTauri ? invoke<SongItem[]>("songs") : devData().then((d) => d.songs));
export const stats = () => (inTauri ? invoke<Stats>("stats") : devData().then((d) => d.stats));
/** The home page's statistics block and the Progression page (heavier: computed on demand). */
export const homeStats = () => (inTauri ? invoke<HomeStats>("home_stats") : devData().then((d) => d.home));
export const progress = () => (inTauri ? invoke<Progress>("progress") : devData().then((d) => d.progress));
export const scanState = () =>
  inTauri
    ? invoke<ScanState>("scan_state")
    : Promise.resolve({ running: false, stage: "", done: 0, total: 0, detail: "", last: Date.now() / 1000, error: null, cancelled: false, version: 0, audio: devAudio() } as ScanState);
export const scan = (full = false) => invoke<boolean>("scan", { full });
/** Every project read again, the MIDI files and samples left as they are. */
export const scanProjects = () => invoke<boolean>("scan", { full: true, projectsOnly: true });
// The audio analysis of reference renders (opt-in): its state, on/off (on downloads the
// model once), pause, removing the downloaded files.
import type { AudioState } from "./bindings/AudioState";
import type { SoundInfo } from "./bindings/SoundInfo";
import type { SimilarSong } from "./bindings/SimilarSong";
export type { AudioState, SoundInfo, SimilarSong };
// the browser preview: off, or "?audio" shows a samples' analysis under way
export const devAudio = (): AudioState =>
  typeof location !== "undefined" && location.search.includes("audio")
    ? { enabled: true, ready: true, paused: false, downloading: false, dl_done: 0, dl_total: 0, running: true, done: 1234, total: 32697, analysed: 177, renders: 200,
        detail: "D:/Samples/Pack/Drums/Percussion/Perc Loop 5 128.wav", error: null, download_mb: 78, disk_mb: 88, samples_todo: 31463, samples_done: 412, samples_rest: 114262,
        samples_heard: 1646, eta_s: 2580, ms_per_file: 82, phase: "unsorted", copies: 4, busy_wait: false }
    : { enabled: false, ready: false, paused: false, downloading: false, dl_done: 0, dl_total: 0, running: false, done: 0, total: 0, analysed: 0, renders: 0, detail: "", error: null,
        download_mb: 78, disk_mb: 0, samples_todo: 0, samples_done: 0, samples_rest: 0, samples_heard: 0, eta_s: 0, ms_per_file: 0, phase: "", copies: 0, busy_wait: false };
export const audioState = () => (inTauri ? invoke<AudioState>("audio_state") : Promise.resolve(devAudio()));
export const audioEnable = (on: boolean) => (inTauri ? invoke<AudioState>("audio_enable", { on }) : readOnly());
export const audioPause = (paused: boolean) => (inTauri ? invoke<AudioState>("audio_pause", { paused }) : readOnly());
export const audioRemove = () => (inTauri ? invoke<AudioState>("audio_remove") : readOnly());
/** Stops the analysis in progress (what it read is kept). */
export const cancelScan = () => (inTauri ? invoke<void>("cancel_scan") : Promise.resolve());
import type { FolderPeek } from "./bindings/FolderPeek";
export type { FolderPeek };
/** A quick look at a folder before adding it: files, MIDI files, time to read them. */
export const folderPeek = (path: string) => invoke<FolderPeek>("folder_peek", { path });
/** A warning when a folder's MIDI files will take a while to read the first time, else null. */
export async function bigFolderWarning(path: string): Promise<string | null> {
  if (!inTauri) return null;
  let p: FolderPeek;
  try {
    p = await folderPeek(path);
  } catch {
    return null; // the adding itself says what is wrong
  }
  if (p.complete && p.midi < 5000 && p.seconds < 60) return null;
  const one = { minimumFractionDigits: 1, maximumFractionDigits: 1 };
  const size = p.midi_bytes > 1e9 ? t("app.api.gb", { n: fmtNum(p.midi_bytes / 1e9, one) }) : t("app.api.mb", { n: Math.max(1, Math.round(p.midi_bytes / 1e6)) });
  const time = p.seconds < 60 ? t("app.api.lessThanMinute") : p.seconds < 3600 ? t("app.api.aboutMin", { n: Math.round(p.seconds / 60) }) : t("app.api.aboutHours", { n: fmtNum(p.seconds / 3600, one) });
  return t(p.complete ? "app.api.bigFolder" : "app.api.bigFolderMore", { midi: fmtNum(p.midi), size, files: fmtNum(p.files), time });
}
export const root = () => (inTauri ? invoke<string | null>("root") : Promise.resolve(t("app.api.preview")));
export const setRoot = (root: string) => invoke<void>("set_root", { root });
export const importPrototype = (path: string) => invoke<[string, number][]>("import_prototype", { path });
// Setup and Settings: projects folders, sample folders, artists... Outside the app (the
// design preview) the setup counts as done, on a folder named after the app.api.preview text.
// the preview shows the first-run setup with ?setup in the address (?setup=share: its last
// step), and the usage statistics' question asked at start with ?ask
// the preview's settings; its artists' (names, aliases, guests) from public/dev-artists.json,
// read once at start (see artists() below)
let devArtistCfg: Partial<Config> = {};
const devConfig = (): Config => ({ roots: [{ label: "", path: t("app.api.preview"), artist: "" }], sample_dirs: [], midi_dirs: [], old_drive: "", artists: [], artist_aliases: {}, artist_feats: {}, artist_ignored: [], work_words: [], setup_done: !location.search.includes("setup"), live_db: true, lang: "", telemetry: location.search.includes("ask") ? "" : "no", ...devArtistCfg } as Config);
export const config = () =>
  inTauri ? invoke<Config>("config") : devJson<{ config?: Partial<Config> }>("dev-artists.json", {}).then((d) => ((devArtistCfg = d.config ?? {}), devConfig()));
/** The sidebar's "Analyse" (automatic analysis off): the projects read now, the sounds after. */
export const analyseNow = () => (inTauri ? invoke<void>("analyse_now") : readOnly());
export const setConfig = (key: string, value: unknown) => (inTauri ? invoke<Config>("set_config", { key, value }) : readOnly());
export const addRoot = (path: string) => invoke<Config>("add_root", { path });
export const removeRoot = (path: string) => invoke<Config>("remove_root", { path });
export const setRootArtist = (path: string, artist: string) => invoke<Config>("set_root_artist", { path, artist });
/** The system's folder picker; null when cancelled. */
export const pickFolder = (title: string) => (inTauri ? invoke<string | null>("pick_folder", { title }) : readOnly());
export const artistSuggestions = () => (inTauri ? invoke<ArtistGuess[]>("artist_suggestions") : devJson<{ sugg?: ArtistGuess[] }>("dev-artists.json", {}).then((d) => d.sugg ?? []));
// Files and folders (the user's real folders; every operation can be undone with Ctrl+Z).
import type { MovePlan } from "./bindings/MovePlan";
import type { MoveResult } from "./bindings/MoveResult";
import type { MediaFolder } from "./bindings/MediaFolder";
import type { ReorgPlan } from "./bindings/ReorgPlan";
export type { MovePlan, MoveResult, MediaFolder, ReorgPlan };
export const fsFolders = () => (inTauri ? invoke<string[]>("fs_folders") : Promise.resolve([] as string[]));
export const fsMedia = () => (inTauri ? invoke<MediaFolder[]>("fs_media") : Promise.resolve([] as MediaFolder[]));
export const fsReorg = (template: string) => (inTauri ? invoke<ReorgPlan>("fs_reorg", { template }) : readOnly());
export const fsPlan = (src: string, dest: string) => (inTauri ? invoke<MovePlan>("fs_plan", { src, dest }) : readOnly());
export const fsMkdir = (rel: string) => (inTauri ? invoke<string>("fs_mkdir", { rel }) : readOnly());
/** [source, destination folder] pairs. */
export const fsMove = (moves: [string, string][]) => (inTauri ? invoke<MoveResult>("fs_move", { moves }) : readOnly());
export const fsToBoard = (id: number, items: string[]) => (inTauri ? invoke<[number, number]>("fs_to_board", { id, items }) : readOnly());
// Notion import (a base exported as CSV)
import type { Column as NotionColumn } from "./bindings/Column";
import type { ImportPreview as NotionPreview } from "./bindings/ImportPreview";
export type { NotionColumn, NotionPreview };
export const pickFile = (title: string, ext: string[]) => (inTauri ? invoke<string | null>("pick_file", { title, ext }) : readOnly());
export const notionPreview = (path: string, columns: NotionColumn[] | null) => invoke<NotionPreview>("notion_preview", { path, columns });
export const notionApply = (path: string, columns: NotionColumn[], stages: Record<string, string>, onlyEmpty: boolean) =>
  invoke<[number, UndoState]>("notion_apply", { path, columns, stages, onlyEmpty });
import type { AudioInfo } from "./bindings/AudioInfo";
export type { AudioInfo };
/** A render's waveform and loudness, measured by the app (the window may not read files). */
export const audioInfo = (rel: string) => invoke<AudioInfo>("audio_info", { rel });
export const inApp = inTauri;
/** Samples' shapes for their rows (48 values 0..255 each), by path; unknown paths left out. */
export const sampleWaves = (paths: string[]) =>
  inTauri ? invoke<Record<string, number[]>>("sample_waves", { paths }) : Promise.resolve({} as Record<string, number[]>);
import type { MidiSummary } from "./bindings/MidiSummary";
export type { MidiSummary };
/** The MIDI files of the projects folders: kinds, keys, chord progressions. */
export const midiSummary = () => (inTauri ? invoke<MidiSummary>("midi_summary") : Promise.resolve({ files: 0, mine: { files: 0, kinds: [], keys: [], minor: 0, major: 0, tempos: [], progressions: [] }, library: { files: 0, kinds: [], keys: [], minor: 0, major: 0, tempos: [], progressions: [] }, items: [] } as MidiSummary));
export const resetBase = () => invoke<string>("reset_base");
export const reveal = (rel: string) => invoke<void>("reveal", { rel });
/** A page of Fatstep for Artists in the browser (the preview: a new tab). */
export const openFatstep = (path = "/spool") =>
  inTauri ? invoke<void>("open_fatstep", { path }) : Promise.resolve(void window.open("https://artists.fatstep.com" + path, "_blank"));
export const absPath = (rel: string) => invoke<string>("abs_path", { rel });
/** Copies a project's full path; says so (or why not). */
export async function copyPath(rel: string): Promise<string> {
  const p = await absPath(rel);
  await navigator.clipboard.writeText(p);
  return p;
}
export const openInLive = (rel: string) => invoke<void>("open_in_live", { rel });
/** Markers changed in Spool's arrangement for a project ([beat, name]), null: the set's own. */
export const markersGet = (rel: string) => (inTauri ? invoke<[number, string][] | null>("markers_get", { rel }) : Promise.resolve(null));
export const markersSet = (rel: string, list: [number, string][] | null) => (inTauri ? invoke<void>("markers_set", { rel, list }) : readOnly());
/** A copy with what was ticked (a preset of R / M / P...), written next to the project; its path. */
export const liveCopy = (rel: string, preset: Record<string, unknown>) => (inTauri ? invoke<string>("live_copy", { rel, preset }) : readOnly());
/** Every project file (.als), the newest first. */
export const projectFiles = () =>
  inTauri ? invoke<import("./bindings/ProjectFile").ProjectFile[]>("project_files") : Promise.resolve([] as import("./bindings/ProjectFile").ProjectFile[]);

// In the browser preview a sheet comes from public/dev-detail.json (core-dump --detail) and
// changes are refused: the preview is read-only.
let devDetail: Promise<Record<string, SongDetail>> | null = null;
const readOnly = () => Promise.reject(t("app.api.readOnly"));
export const song = (id: number) =>
  inTauri
    ? invoke<SongDetail | null>("song", { id })
    : (devDetail ??= fetch("/dev-detail.json").then((r) => (r.ok ? r.json() : {}) as Promise<Record<string, SongDetail>>)).then((d) => d[id] ?? null);
/** The styles the app would suggest for a song, with why, even when one is noted. */
export const styleGuess = (id: number) => (inTauri ? invoke<StyleOpt[]>("style_guess", { id }) : Promise.resolve([] as StyleOpt[]));
export const songEdit = (id: number, body: Record<string, unknown>) =>
  inTauri ? invoke<UndoState>("song_edit", { id, body }) : readOnly();
export const checkSet = (id: number, key: string, value: boolean | "na" | "off" | null) =>
  inTauri ? invoke<UndoState>("check_set", { id, key, value }) : readOnly();
export const suggestionAct = (id: number, accept: boolean, choice?: string) =>
  inTauri ? invoke<UndoState>("suggestion_act", { id, accept, choice: choice ?? null }) : readOnly();
/** Several songs edited at once, one undo named `label`. */
export const songsEdit = (edits: [number, Record<string, unknown>][], label: string) =>
  inTauri ? invoke<UndoState>("songs_edit", { edits, label }) : readOnly();
export const acceptAll = (ids: number[]) => (inTauri ? invoke<UndoState>("accept_all", { ids }) : readOnly());
export const undoState = () => (inTauri ? invoke<UndoState>("undo_state") : Promise.resolve({ label: null, left: 0, files: false }));
/** Every song's media (the Media page), and a song's picture chosen by hand (null: the app's). */
/** A song's resources: the samples its project plays, its Samples folders' files. */
export const songResources = (id: number, rel?: string) => (inTauri ? invoke<import("./bindings/Resources").Resources>("song_resources", { id, rel: rel ?? null }) : Promise.resolve({ project: "", items: [], more: 0 }));
export const looseAudio = () => (inTauri ? invoke<import("./bindings/LooseAudio").LooseAudio[]>("loose_audio") : Promise.resolve([]));
export const mediaList = () => (inTauri ? invoke<import("./bindings/MediaSong").MediaSong[]>("media_list") : Promise.resolve([]));
export const setCover = (id: number, rel: string | null) => (inTauri ? invoke<UndoState>("set_cover", { id, rel }) : readOnly());
/** How the app reads the folders, what other rules would make, and those rules changed. */
export const readingReport = () => invoke<import("./bindings/ReadingReport").ReadingReport>("reading_report");
export const groupPreview = (all: boolean, dirs: string[], titleFrom: string) =>
  invoke<import("./bindings/GroupPreview").GroupPreview>("group_preview", { all, dirs, titleFrom });
export const setGrouping = (all: boolean, dirs: string[], titleFrom: string) => (inTauri ? invoke<UndoState>("set_grouping", { all, dirs, titleFrom }) : readOnly());
export const folderNeighbours = (dir: string) => invoke<string[]>("folder_neighbours", { dir });
/** A diagnostic report written in the app's exports folder (nothing is sent). */
export const diagnosticReport = async () => invoke<string>("diagnostic_report", { version: await appVersion() });
export const redo = () => (inTauri ? invoke<[string | null, UndoState]>("redo") : readOnly());
export const undo = () => (inTauri ? invoke<[string | null, UndoState]>("undo") : readOnly());
// Mood board: written straight into the song's _moodboard folder (not part of Ctrl+Z; a
// removed item goes to its .corbeille folder).
export const boardNote = (id: number, text: string, lyrics = false, name?: string) =>
  inTauri ? invoke<string>("board_note", { id, text, lyrics, name: name ?? null }) : readOnly();
export const boardAdd = (id: number, file: File) =>
  inTauri
    ? file.arrayBuffer().then((b) =>
        invoke<string>("board_add", new Uint8Array(b), { headers: { "x-song": String(id), "x-name": encodeURIComponent(file.name) } }),
      )
    : readOnly();
// the virtual mood board: a file shown on a song's board where it is (nothing moves)
export const boardLink = (id: number, items: string[]) => (inTauri ? invoke<UndoState>("board_link", { id, items }) : readOnly());
export const boardUnlink = (id: number, rel: string) => (inTauri ? invoke<UndoState>("board_unlink", { id, rel }) : readOnly());
// "Ranger sur le disque" for a release: what would move (nothing touched), then the moves
export const releaseArrangePlan = (id: string, parent: string | null) =>
  inTauri ? invoke<import("./bindings/ArrangePlan").ArrangePlan>("release_arrange_plan", { id, parent }) : readOnly();
export const releaseArrange = (id: string, parent: string | null) =>
  inTauri ? invoke<import("./bindings/MoveResult").MoveResult>("release_arrange", { id, parent }) : readOnly();
/** A web page or an e-mail address, opened by the system (http, https and mailto only). */
export const openLink = (url: string) => (inTauri ? invoke<void>("open_link", { url }) : Promise.resolve(void window.open(url, "_blank")));
// References: audio to compare with, attached to a song, an artist or a release (only read)
export const refs = (kind: string, key: string) =>
  inTauri
    ? invoke<import("./bindings/RefItem").RefItem[]>("refs", { kind, key })
    : devJson<Record<string, import("./bindings/RefItem").RefItem[]>>("dev-song-refs.json", {}).then((m) => m[`${kind}|${key}`] ?? []);
export const refMark = (path: string, kind: string, key: string, on: boolean) =>
  inTauri ? invoke<import("./bindings/RefItem").RefItem[]>("ref_mark", { path, kind, key, on }) : readOnly();
// Artists: their profiles (a setting), their photo and mood board (in Spool's data folder)
export const artists = () => (inTauri ? invoke<import("./bindings/ArtistCard").ArtistCard[]>("artists") : devJson<{ cards?: import("./bindings/ArtistCard").ArtistCard[] }>("dev-artists.json", {}).then((d) => d.cards ?? []));
export const artist = (name: string) => (inTauri ? invoke<import("./bindings/ArtistPage").ArtistPage>("artist", { name }) : readOnly());
export const artistSave = (name: string, patch: Record<string, string>) => (inTauri ? invoke<void>("artist_save", { name, patch }) : readOnly());
export const artistBoardNote = (name: string, text: string, lyrics = false, old?: string) =>
  inTauri ? invoke<string>("artist_board_note", { name, text, lyrics, old: old ?? null }) : readOnly();
export const artistBoardAdd = (name: string, file: File) =>
  inTauri
    ? file.arrayBuffer().then((b) =>
        invoke<string>("board_add", new Uint8Array(b), { headers: { "x-artist": encodeURIComponent(name), "x-name": encodeURIComponent(file.name) } }),
      )
    : readOnly();
/** A file the app shows copied onto an artist's board (made their photo when `photo`). */
export const artistBoardCopy = (name: string, path: string, photo: boolean) => (inTauri ? invoke<string>("artist_board_copy", { name, path, photo }) : readOnly());
/** A picture picked from the disk for a cover, copied into Spool's pictures folder: its path. */
export const pictureAdd = (file: File) =>
  inTauri
    ? file.arrayBuffer().then((b) => invoke<string>("board_add", new Uint8Array(b), { headers: { "x-picture": "1", "x-name": encodeURIComponent(file.name) } }))
    : readOnly();
/** Files dropped from the system on an artist's board: copied onto it. */
export const artistBoardDrop = (name: string, paths: string[]) => (inTauri ? invoke<BoardDrop>("artist_board_drop", { name, paths }) : readOnly());
/** Files dropped from the system on a release's board: (the paths the board keeps, what became of them). */
export const releaseBoardDrop = (paths: string[]) => (inTauri ? invoke<[string[], BoardDrop]>("release_board_drop", { paths }) : readOnly());
export const artistBoardRemove = (name: string, file: string) => (inTauri ? invoke<void>("artist_board_remove", { name, file }) : readOnly());
export const artistBoardMeta = (name: string, patch: { palette?: string[] }) => (inTauri ? invoke<void>("artist_board_meta", { name, patch }) : readOnly());
export const boardRemove = (id: number, name: string) => (inTauri ? invoke<void>("board_remove", { id, name }) : readOnly());
export const boardMeta = (id: number, patch: { palette?: string[]; order?: string[]; captions?: Record<string, string> }) =>
  inTauri ? invoke<void>("board_meta", { id, patch }) : readOnly();

/** Song `id` becomes part of song `into` (its files and future saves). Undone by splitSong. */
export const mergeSong = (id: number, into: number) => (inTauri ? invoke<string>("merge_song", { id, into }) : readOnly());
/** A project made a song of its own (it was grouped with others by its name). Returns its id. */
export const fileToSong = (rel: string) => (inTauri ? invoke<number>("file_to_song", { rel }) : readOnly());
/** A project's arrangement as a picture: tracks, placed clips (with their notes), locators. */
export const arrangement = (rel: string) =>
  inTauri
    ? invoke<import("./bindings/Arrangement").Arrangement>("arrangement", { rel })
    : devJson<import("./bindings/Arrangement").Arrangement | null>("dev-arrangement.json", null).then((a) => a ?? Promise.reject(new Error("preview")));
/** A project's harmony bar by bar (key, chords), and notes written as a MIDI file to drag. */
export const harmony = (rel: string) =>
  inTauri
    ? invoke<import("./bindings/Harmony").Harmony>("harmony", { rel })
    : devJson<import("./bindings/Harmony").Harmony | null>("dev-harmony.json", null).then((h) => h ?? Promise.reject(new Error("preview")));
/** A song's arrangement and chords can show: in the app, and in the browser preview while
 *  developing (one fictional project from public/dev-arrangement.json and dev-harmony.json,
 *  for screenshots; nothing shows without them). */
export const withArrangement = inTauri || import.meta.env.DEV;
export const writeMidi = (name: string, notes: [number, number, number][], bpm: number) => invoke<string>("write_midi", { name, notes, bpm });
/** A clip of a project as a MIDI file (its bytes), and every project holding it. */
export const clipMidi = (hash: string) => invoke<ArrayBuffer>("clip_midi", { hash });
/** The MIDI keyboard read by the app (WinMM): null in the browser preview (Web MIDI there). */
export const nativeMidi = inTauri;
export const midiInputs = () => invoke<string[]>("midi_inputs");
export const midiOpen = (name: string) => invoke<void>("midi_open", { name });
export const midiClose = () => invoke<void>("midi_close");
export const onMidiIn = (f: (d: [number, number, number]) => void) => listen<[number, number, number]>("midi-in", (e) => f(e.payload));
/** What was played on the MIDI keyboard: takes kept 30 days, or for good as a MIDI file. */
export type Take = import("./bindings/Take").Take;
export const takeSave = (at: number, notes: [number, number, number, number][]) => (inTauri ? invoke<number>("take_save", { at, notes }) : Promise.resolve(0));
export const takes = () => (inTauri ? invoke<Take[]>("takes") : Promise.resolve([] as Take[]));
export const takeDelete = (id: number) => (inTauri ? invoke<void>("take_delete", { id }) : readOnly());
export const takeKeep = (id: number, name: string, bpm: number) => (inTauri ? invoke<string>("take_keep", { id, name, bpm }) : readOnly());
export const takeSmf = (id: number, name: string, bpm: number) => (inTauri ? invoke<ArrayBuffer>("take_smf", { id, name, bpm }) : readOnly());
export const clipUses = (hash: string) => invoke<import("./bindings/ClipUse").ClipUse[]>("clip_uses", { hash });
/** A MIDI file transposed / at another tempo, written as a new file to drag into Live. */
export const midiExport = (rel: string, semis: number, rate: number, target: number | null, drums: boolean) =>
  inTauri ? invoke<string>("midi_export", { rel, semis, rate, target, drums }) : readOnly();
/** The samples kept twice or more, and the songs whose projects use a sample. */
export const sampleDupes = () => (inTauri ? invoke<import("./bindings/Dupes").Dupes[]>("sample_dupes") : Promise.resolve([]));
/** Samples sorted by hand (family null: back to the automatic sort); a folder's unsorted ones at once. */
export const sortSamples = (paths: string[], family: string | null, cat: string | null) =>
  inTauri ? invoke<number>("sort_samples", { paths, family, cat }) : Promise.resolve(paths.length);
export const sortSampleFolder = (dir: string, family: string, cat: string | null) =>
  inTauri ? invoke<number>("sort_sample_folder", { dir, family, cat }) : Promise.resolve(0);
/** The folders with the most unsorted samples (the quick sort by folder), but those skipped. */
import type { Questions } from "./bindings/Questions";
import type { Question } from "./bindings/Question";
export type { Questions, Question };
/** The quick sort's next questions (a folder and what sounds like it), but the folders skipped. */
export const sampleQuestions = (skip: string[]) =>
  inTauri ? invoke<Questions>("sample_questions", { skip }) : Promise.resolve({ items: [], folders: 0, unsorted: 0 } as Questions);
/** The unsorted samples that sound like these, just sorted. */
export const samplesAlikeUnsorted = (paths: string[]) => (inTauri ? invoke<SampleRow[]>("samples_alike_unsorted", { paths }) : Promise.resolve([] as SampleRow[]));
/** Every sample of a folder, sorted or not. */
export const sampleFolder = (dir: string) => (inTauri ? invoke<SampleRow[]>("sample_folder", { dir }) : Promise.resolve([] as SampleRow[]));
/** Sends samples to the Recycle Bin; how many went. */
export const trashSamples = (paths: string[]) => invoke<number>("trash_samples", { paths });
import type { Favorite } from "./bindings/Favorite";
export type { Favorite };
/** A choice of the interface kept in the base ("ui." keys): survives the web view's storage. */
export const uiPref = <T>(key: string) => (inTauri ? invoke<T | null>("ui_pref", { key }) : Promise.resolve(null));
export const setUiPref = (key: string, value: unknown) => (inTauri ? invoke<void>("set_ui_pref", { key, value }) : Promise.resolve());
/** The favourites, the newest first. */
const noReview: StyleReview = { questions: [], candidates: [], groups: [], words: [], links: [], names: [], known: [], heard: 0, renders: 0 };
export const styleReview = () => (inTauri ? invoke<StyleReview>("style_review") : Promise.resolve(noReview));
/** The answer to "Spool thinks this is X. Right?": the song's style, or null (mixed, don't know: not asked again). */
export const styleAnswer = (id: number, style: string | null) => (inTauri ? invoke<StyleAnswer>("style_answer", { id, style }) : readOnly());
/** The songs not asked about again. */
export const styleSkipped = () => (inTauri ? invoke<number[]>("style_skipped") : Promise.resolve([] as number[]));
export const styleKeep = (heard: string, mine: string) => (inTauri ? invoke<StyleReview>("style_keep", { heard, mine }) : readOnly());
/** `word` of the user's folders is a style, called `name`; with `heard`, what Spool hears as that counts for it too. */
export const styleWordYes = (word: string, name: string, heard: string | null) =>
  inTauri ? invoke<StyleReview>("style_word_yes", { word, name, heard }) : readOnly();
export const styleWordNo = (word: string) => (inTauri ? invoke<StyleReview>("style_word_no", { word }) : readOnly());
export const styleDismiss = (mine: string, heard: string) => (inTauri ? invoke<StyleReview>("style_dismiss", { mine, heard }) : readOnly());
export const setStyleVocab = (words: string[], links: [string, string][], names: [string, string][]) =>
  inTauri ? invoke<StyleReview>("set_style_vocab", { words, links, names }) : readOnly();
export const favorites = () => (inTauri ? invoke<Favorite[]>("favorites") : Promise.resolve([] as Favorite[]));
/** Stars or unstars one; the list as it is now. */
export const setFavorite = (fav: Favorite, on: boolean) => invoke<Favorite[]>("set_favorite", { fav, on });
/** These samples' rows, in the order given. */
export const sampleRows = (paths: string[]) => (inTauri ? invoke<SampleRow[]>("sample_rows", { paths }) : Promise.resolve([] as SampleRow[]));
export const unsortedFolders = (skip: string[]) =>
  inTauri ? invoke<import("./bindings/FolderToSort").FolderToSort[]>("unsorted_folders", { skip }) : Promise.resolve([]);
export const sampleSongs = (path: string) =>
  inTauri ? invoke<number[]>("sample_songs", { path }) : devJson<Record<string, number[]>>("dev-sample-songs.json", {}).then((m) => m[path] ?? []);
/** What changed between two saves of a project (older first). */
export const alsDiff = (a: string, b: string) => (inTauri ? invoke<import("./bindings/AlsDiff").AlsDiff>("als_diff", { a, b }) : readOnly());
/** A song's timed comments; one added (id 0), changed or removed. */
// the browser preview: optional public/dev-comments.json, dev-refs.json, dev-audio.json
const devJson = <T>(f: string, empty: T): Promise<T> => fetch("/" + f).then((r) => (r.ok ? (r.json() as Promise<T>) : empty)).catch(() => empty);
export const comments = (id: number) =>
  inTauri ? invoke<import("./bindings/Comment").Comment[]>("comments", { id }) : devJson<Record<string, import("./bindings/Comment").Comment[]>>("dev-comments.json", {}).then((m) => m[id] ?? []);
/** The preview's waveform and loudness of a file (null: none in dev-audio.json). */
export const devAudioInfo = (rel: string) => devJson<Record<string, AudioInfo>>("dev-audio.json", {}).then((m) => m[rel] ?? null);
export const commentEdit = (id: number, c: Partial<import("./bindings/Comment").Comment>, remove = false) =>
  inTauri ? invoke<UndoState>("comment_edit", { id, c: { id: 0, rel: "", at: 0, text: "", done: false, created: 0, ...c }, remove }) : readOnly();
/** Released songs to compare a render with (absolute paths, only read). */
export const refTracks = () => (inTauri ? invoke<string[]>("ref_tracks") : devJson<string[]>("dev-refs.json", []));
export const refTrack = (path: string, add: boolean) => (inTauri ? invoke<string[]>("ref_track", { path, add }) : readOnly());
export const splitSong = (id: number, key: string) => (inTauri ? invoke<void>("split_song", { id, key }) : readOnly());

// Repairs: a copy "Name [R].als" next to each project, the original never touched.
export const fixable = () =>
  inTauri ? invoke<Fixable[]>("fixable") : devData().then((d) => ((d as { fixable?: Fixable[] }).fixable ?? []));
export const repairState = () =>
  inTauri ? invoke<RepairState>("repair_state") : Promise.resolve({ running: false, done: 0, total: 0, results: [], at: null } as RepairState);
/** What a repair of one project would do (null: unknown project). */
export type RepairPlan = { useful: boolean; complete: boolean; renames: unknown[]; copies: unknown[]; removes: unknown[]; trials: unknown[]; lost: string[] };
export const repairPlan = (rel: string) => (inTauri ? invoke<RepairPlan | null>("repair_plan", { rel }) : Promise.resolve(null));
/** The Repair page; `years`: only the projects saved in the last N years. */
export const repairView = (years?: number) =>
  inTauri ? invoke<RepairView>("repair_view", { years: years ?? null }) : devData().then((d) => (d as unknown as { repair: RepairView }).repair);
/** A plugin no longer used (keep: left in repaired copies to replace by hand). */
export const setIgnored = (name: string, ignored: boolean, keep = false) =>
  inTauri ? invoke<void>("set_ignored", { name, ignored, keep }) : readOnly();
/** Replace a version found nowhere by an installed VST2 (to: null removes the choice). */
export const setTrial = (name: string, format: string, to: string | null) =>
  inTauri ? invoke<void>("set_trial", { name, format, to }) : readOnly();
/** A VST2 replaced by an installed VST3 in repaired copies (uid: its class id), or not (null). */
// Create: what a new song of a style would be made of, then writing it (a NEW folder only)
export const createPlan = (req: Partial<import("./bindings/CreateReq").CreateReq>) =>
  inTauri
    ? invoke<import("./bindings/CreatePlan").CreatePlan>("create_plan", { req: { overrides: {}, parts: [], tempo: null, name: "", style: null, live: null, seed: null, seconds: null, folder: null, ...req } })
    : fetch("/dev-create.json").then((r) => r.json() as Promise<import("./bindings/CreatePlan").CreatePlan>);
// a track's devices, racks opened (read from the project, never written)
let devTracks: Promise<Record<string, import("./bindings/DevView").DevView[]>> | null = null;
export const trackView = (rel: string, track: string) =>
  inTauri
    ? invoke<import("./bindings/DevView").DevView[]>("track_view", { rel, track })
    : (devTracks ??= fetch("/dev-tracks.json").then((r) => r.json())).then((m) => m[rel + "|" + track] ?? []);
// releases (EP, LP...): kept in the app only, nothing moves on disk
export const releases = () => (inTauri ? invoke<import("./bindings/Release").Release[]>("releases") : Promise.resolve(devReleases));
export const releaseHints = () =>
  inTauri
    ? invoke<import("./bindings/ReleaseHint").ReleaseHint[]>("release_hints")
    : devData().then((d) => {
        // the preview: the same rule as the app (a folder whose name says EP / LP / album)
        const kw = /(^|[^a-z])(ep|lp|album|mixtape)([^a-z]|$)/i;
        const g = new Map<string, number[]>();
        for (const s of d.songs) {
          const parts = s.dir.split("/");
          const i = parts.findLastIndex((p) => kw.test(p));
          if (i >= 0) g.set(parts.slice(0, i + 1).join("/"), [...(g.get(parts.slice(0, i + 1).join("/")) ?? []), s.id]);
        }
        return [...g].filter(([f, ids]) => ids.length > 1 && !devReleases.some((r) => r.folder === f)).map(([folder, songs]) => {
          const name = folder.split("/").pop()!;
          return { folder, songs, artist: "", kind: /\bep\b/i.test(name) ? "ep" : "lp", title: name.replace(kw, " ").replace(/^[\s-]+|[\s-]+$/g, "") };
        });
      });
export const releaseSave = (release: import("./bindings/Release").Release) =>
  inTauri
    ? invoke<[string, UndoState]>("release_save", { release })
    : Promise.resolve(((devReleases = [...devReleases.filter((r) => r.id !== release.id), { ...release, id: release.id || "r" + Date.now() }]), [release.id || devReleases.at(-1)!.id, { label: null, left: 0, files: false }]) as [string, UndoState]);
export const releaseDelete = (id: string) => (inTauri ? invoke<UndoState>("release_delete", { id }) : readOnly());
let devReleases: import("./bindings/Release").Release[] = [];
export const createSong = (req: import("./bindings/CreateReq").CreateReq) => (inTauri ? invoke<Record<string, unknown>>("create_song", { req }) : readOnly());
export const setVst3Many = (list: [string, string | null][]) => (inTauri ? invoke<void>("set_vst3_many", { list }) : readOnly());
export const setVst3 = (name: string, uid: string | null) => (inTauri ? invoke<void>("set_vst3", { name, uid }) : readOnly());
export const repairRun = (rels: string[]) => (inTauri ? invoke<void>("repair_run", { rels }) : readOnly());
/** Repair AND a treatments preset in one copy. */
export const repairRunPreset = (rels: string[], preset: Preset) => (inTauri ? invoke<void>("repair_run_preset", { rels, preset }) : readOnly());
/** The old drive's user folder searched for the lost samples (in the background). */
/** The lost samples looked for on a drive ("E"; null: the old Windows one, left as set). */
export const deepSearch = (drive: string | null = null) => (inTauri ? invoke<void>("deep_search", { drive }) : readOnly());
/** The drives plugged in now (not the system's), to pick the previous system's one. */
export const drives = () =>
  inTauri
    ? invoke<import("./bindings/Drive").Drive[]>("drives")
    : Promise.resolve([{ letter: "D", label: "Ancien SSD", total: 512e9, windows: true }, { letter: "E", label: "Samples", total: 2e12, windows: false }]);

// Treatments: presets (C colours, N names, O order, P cleanup, M markers,
// G template, R repair), a preview on one song, copies "Name [codes].als" written in the
// background. Presets are plain JSON, as the prototype kept them.
export type Preset = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
export type Track = { name: string; kind: string; role: string; color: number; depth: number };
export type AtelierRun = Record<string, any> & { ok: boolean; error?: string; out?: string; before: Track[]; after: Track[]; source: string; rel: string }; // eslint-disable-line @typescript-eslint/no-explicit-any
export const atelierPresets = () => (inTauri ? invoke<Preset[]>("atelier_presets") : atelierDefault().then((d) => [d]));
export const atelierDefault = () =>
  inTauri ? invoke<Preset>("atelier_default") : fetch("/atelier-default.json").then((r) => (r.ok ? r.json() : {}));
export const atelierSavePresets = (presets: Preset[]) => (inTauri ? invoke<void>("atelier_save_presets", { presets }) : readOnly());
export const atelierPreview = (song: number, preset: Preset) => (inTauri ? invoke<AtelierRun>("atelier_preview", { song, preset }) : readOnly());
export const atelierRun = (songs: number[], preset: Preset) => (inTauri ? invoke<void>("atelier_run", { songs, preset }) : readOnly());

/** URL of a file under the projects root (audio, pictures), served by the app. */
/** A picture's thumbnail, `size` px on its long side (made once by the app, then kept). */
export const thumbUrl = (rel: string, size = 240) => {
  const u = mediaUrl(rel);
  return u ? `${u}?thumb=${size}` : "";
};
export const mediaUrl = (rel: string) =>
  inTauri ? "http://media.localhost/" + rel.split("/").map(encodeURIComponent).join("/") : "";

/** Called whenever the core changed something (a scan found news, an import...). */
export const onChanged = (f: (s: ScanState) => void) =>
  inTauri ? listen<ScanState>("changed", (e) => f(e.payload)) : Promise.resolve(() => {});

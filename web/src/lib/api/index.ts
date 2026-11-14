// The only place that knows how the interface talks to the core: Tauri today, HTTP for the
// web version later. Pages import these functions, never `invoke` or `listen`.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, fmtNum } from "../i18n/index.svelte";
import type { Overview } from "./bindings/Overview";
import type { ScanState } from "./bindings/ScanState";
import type { SongItem } from "./bindings/SongItem";
import type { Stats } from "./bindings/Stats";
import type { SongDetail } from "./bindings/SongDetail";
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
// The audio analysis of reference renders (opt-in): its state, on/off (on downloads the
// model once), pause, removing the downloaded files.
import type { AudioState } from "./bindings/AudioState";
import type { SoundInfo } from "./bindings/SoundInfo";
import type { SimilarSong } from "./bindings/SimilarSong";
export type { AudioState, SoundInfo, SimilarSong };
export const devAudio = (): AudioState => ({ enabled: false, ready: false, paused: false, downloading: false, dl_done: 0, dl_total: 0, running: false, done: 0, total: 0, analysed: 0, renders: 0, detail: "", error: null, download_mb: 78, disk_mb: 0 });
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
// the preview shows the first-run setup with ?setup in the address
const devConfig = (): Config => ({ roots: [{ label: "", path: t("app.api.preview"), artist: "" }], sample_dirs: [], midi_dirs: [], old_drive: "", artists: [], work_words: [], setup_done: !location.search.includes("setup"), live_db: true, lang: "" });
export const config = () => (inTauri ? invoke<Config>("config") : Promise.resolve(devConfig()));
export const setConfig = (key: string, value: unknown) => (inTauri ? invoke<Config>("set_config", { key, value }) : readOnly());
export const addRoot = (path: string) => invoke<Config>("add_root", { path });
export const removeRoot = (path: string) => invoke<Config>("remove_root", { path });
export const setRootArtist = (path: string, artist: string) => invoke<Config>("set_root_artist", { path, artist });
/** The system's folder picker; null when cancelled. */
export const pickFolder = (title: string) => (inTauri ? invoke<string | null>("pick_folder", { title }) : readOnly());
export const artistSuggestions = () => (inTauri ? invoke<ArtistGuess[]>("artist_suggestions") : Promise.resolve([] as ArtistGuess[]));
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
export const checkSet = (id: number, key: string, value: boolean | "na" | null) =>
  inTauri ? invoke<UndoState>("check_set", { id, key, value }) : readOnly();
export const suggestionAct = (id: number, accept: boolean, choice?: string) =>
  inTauri ? invoke<UndoState>("suggestion_act", { id, accept, choice: choice ?? null }) : readOnly();
export const acceptAll = (ids: number[]) => (inTauri ? invoke<UndoState>("accept_all", { ids }) : readOnly());
export const undoState = () => (inTauri ? invoke<UndoState>("undo_state") : Promise.resolve({ label: null, left: 0, files: false }));
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
export const boardRemove = (id: number, name: string) => (inTauri ? invoke<void>("board_remove", { id, name }) : readOnly());
export const boardMeta = (id: number, patch: { palette?: string[]; order?: string[]; captions?: Record<string, string> }) =>
  inTauri ? invoke<void>("board_meta", { id, patch }) : readOnly();

/** Song `id` becomes part of song `into` (its files and future saves). Undone by splitSong. */
export const mergeSong = (id: number, into: number) => (inTauri ? invoke<string>("merge_song", { id, into }) : readOnly());
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
          return { folder, songs, artist: "", kind: /ep/i.test(name) ? "ep" : "lp", title: name.replace(kw, " ").replace(/^[\s-]+|[\s-]+$/g, "") };
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
export const deepSearch = () => (inTauri ? invoke<void>("deep_search") : readOnly());

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

// Usage statistics and errors: what the interface reports to the core, which sends it ONLY
// if the user said yes (asked in the setup or once at start; Settings > Spool to change it).
// The gate and the closed lists of what may go out are in the core
// (crates/core/src/telemetry.rs): here, only fixed identifiers are named, never a song, a
// project, a file or a path. Each feature is reported once per launch.
import * as api from "./api";

/** The features counted, the same closed list as the core's `FEATURES`. */
export type Feature =
  | "song_sheet_opened"
  | "statistics_opened"
  | "samples_opened"
  | "midi_opened"
  | "releases_opened"
  | "artists_opened"
  | "media_opened"
  | "repair_opened"
  | "starter_pack_opened"
  | "folder_rules_opened"
  | "scan_finished"
  | "repair_run"
  | "treatment_run"
  | "starter_pack_created"
  | "midi_exported"
  | "project_played"
  | "project_opened_in_live"
  | "artists_merged"
  | "folders_arranged"
  | "notion_imported"
  | "catalogue_exported"
  | "audio_analysis_enabled";

/** A feature used (the core checks the choice and sends each one once per launch). */
export function feature(f: Feature, extra?: { catalogue_size?: string }) {
  api.telemetry("tool_feature_used", { feature: f, ...extra });
}

/** The start (sent once per launch, if the user said yes: called again after a yes). */
export const opened = () => api.telemetry("tool_opened");

/** A catalogue's size as a range, never its count. */
export function sizeBucket(n: number) {
  return n < 100 ? "<100" : n < 1000 ? "100-1k" : n < 10000 ? "1k-10k" : ">10k";
}

/** An error: where (a fixed identifier) and its message; the core takes out paths and names. */
export function error(where: string, e: unknown) {
  const message = e instanceof Error ? `${e.name}: ${e.message}` : typeof e === "string" ? e : (() => { try { return JSON.stringify(e); } catch { return String(e); } })();
  api.telemetry("tool_error", { where, message: String(message).slice(0, 2000) });
}

/** The pages counted when opened (their address, first part). */
const PAGES: Record<string, Feature> = {
  stats: "statistics_opened",
  samples: "samples_opened",
  midi: "midi_opened",
  releases: "releases_opened",
  release: "releases_opened",
  artists: "artists_opened",
  artist: "artists_opened",
  media: "media_opened",
  repair: "repair_opened",
  atelier: "repair_opened",
  create: "starter_pack_opened",
  reading: "folder_rules_opened",
};
export function page(p: string) {
  const f = PAGES[p.split(/[/?]/)[0]];
  if (f) feature(f);
}

/** The core's commands that count as a feature when they succeed. */
const COMMANDS: Record<string, Feature> = {
  repair_run: "repair_run",
  repair_run_preset: "repair_run",
  atelier_run: "treatment_run",
  create_song: "starter_pack_created",
  midi_export: "midi_exported",
  open_in_live: "project_opened_in_live",
  merge_artists: "artists_merged",
  fs_move: "folders_arranged",
  notion_apply: "notion_imported",
};

let installed = false;
/** Watches the core's commands and the page's unhandled errors (once, at start). */
export function install() {
  if (installed) return;
  installed = true;
  api.watchCommands((cmd, args, err, failed) => {
    if (cmd === "telemetry") return;
    if (failed) return error(`cmd:${cmd}`, err);
    const f = COMMANDS[cmd];
    if (f) feature(f);
    else if (cmd === "audio_enable" && (args as { on?: boolean } | undefined)?.on) feature("audio_analysis_enabled");
  });
  addEventListener("error", (e) => error("ui_error", e.error ?? e.message));
  addEventListener("unhandledrejection", (e) => {
    // a command's failure (a string from the core) was already reported by the watcher
    if (typeof e.reason === "string") return;
    error("ui_rejection", e.reason);
  });
}

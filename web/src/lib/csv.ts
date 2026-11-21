// The catalogue as a CSV for a spreadsheet (";" in French, "," in English, a BOM so
// the accents read right). The songs given are exported as they are shown.
import * as api from "./api";
import type { SongItem } from "./api";
import { stageLabel } from "./format";
import { app } from "./state.svelte";
import { t, i18n } from "./i18n/index.svelte";

// the severities, worst last (worded at export time, in the user's language)
const SEV = ["ok", "fix", "lost", "bad"];

// Excel opens a CSV with the list separator of the system's language: ";" in French, ","
// in English
const sep = () => (i18n.lang === "fr" ? ";" : ",");

function cell(v: unknown): string {
  const s = v === null || v === undefined ? "" : String(v);
  return /[";,\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

const date = (t: number) => (t ? new Date(t * 1000).toISOString().slice(0, 10) : "");

export function songsCsv(songs: SongItem[]): string {
  const head = ["title", "artist", "style", "key", "tempo", "stage", "rating", "kinds", "catalog", "files", "plugins", "audio", "live", "created", "modified", "dir", "project"].map((k) => t(`app.csv.head.${k}`));
  const rows = songs.map((s) => [
    s.title, s.artist, s.style, s.key, s.tempo ? Math.round(s.tempo * 100) / 100 : "", stageLabel(s.stage), s.rating ?? "",
    s.kinds.join(", "), s.catalog, s.files, t(`app.csv.sev.${SEV[Math.min(3, s.sev)]}`), s.temoin ? t("app.csv.yes") : t("app.csv.no"), s.live, date(s.created), date(s.mtime), s.dir, s.rel,
  ]);
  return "﻿" + [head, ...rows].map((r) => r.map(cell).join(sep())).join("\r\n") + "\r\n";
}

/** Asks where, then writes it. */
export async function exportSongs(songs: SongItem[], name = t("app.csv.fileName")) {
  try {
    const day = new Date().toISOString().slice(0, 10);
    const at = await api.saveText(t("app.csv.dialogTitle"), `${name} ${day}.csv`, "csv", songsCsv(songs));
    if (at) app.say(t("app.csv.exported", { n: songs.length, at }));
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

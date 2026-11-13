// What the screens say about a release: its songs, how far it is, what is left to do.
import type { Release, SongItem } from "./api";
import { t, fmtDate } from "./i18n/index.svelte";

export const KIND_KEYS = ["single", "ep", "lp", "compil"];
/** The kinds with their names in the user's language (built when called: follows a change of language). */
export const kinds = (): [string, string][] => KIND_KEYS.map((k) => [k, t(`releases.kind.${k}`)]);
export const kindLabel = (k: string) => (KIND_KEYS.includes(k) ? t(`releases.kind.${k}`) : k);

// how far a stage is from a finished song
const WEIGHT: Record<string, number> = { draft: 0.15, arrangement: 0.4, mix: 0.7, master: 0.9, done: 1 };
const READY = ["master", "done"];

export function tracks(r: Release, songs: SongItem[]): (SongItem | null)[] {
  const by = new Map(songs.map((s) => [s.id, s]));
  return r.songs.map((id) => (id == null ? null : (by.get(id) ?? null)));
}

export function summary(r: Release, songs: SongItem[]) {
  const list = tracks(r, songs);
  const present = list.filter((s): s is SongItem => !!s);
  const ready = present.filter((s) => READY.includes(s.stage ?? "")).length;
  const progress = present.length ? present.reduce((a, s) => a + (WEIGHT[s.stage ?? ""] ?? 0), 0) / present.length : 0;
  const seconds = present.reduce((a, s) => a + (Number(s.len) || 0), 0);
  const blocked = present.filter((s) => s.sev >= 3).length;
  const noAudio = present.filter((s) => !s.temoin).length;
  const cover =
    (r.cover_song != null && songs.find((s) => s.id === r.cover_song)?.cover) || present.find((s) => s.cover)?.cover || null;
  // what is left, the most urgent first
  const todo: string[] = [];
  if (blocked) todo.push(t("releases.todo.blocked", { n: blocked }));
  const late = present.filter((s) => !READY.includes(s.stage ?? ""));
  if (late.length)
    todo.push(
      late.length === 1
        ? t("releases.todo.finishOne", { a: late[0].title })
        : late.length === 2
          ? t("releases.todo.finishTwo", { a: late[0].title, b: late[1].title })
          : t("releases.todo.toFinish", { n: late.length }),
    );
  if (noAudio) todo.push(t("releases.todo.noAudio", { n: noAudio }));
  if (list.length > present.length) todo.push(t("releases.todo.missing", { n: list.length - present.length }));
  return { tracks: list, present, ready, progress, seconds, blocked, cover, todo, count: list.length };
}

/** Days from today to `date` (local days), null without a date. */
export function daysUntil(date: string | null): number | null {
  if (!date) return null;
  const d = new Date(date + "T00:00:00");
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.round((d.getTime() - today.getTime()) / 86400000);
}

/** "in 12 days", "out today", "released 3 days ago" (dates are local days). */
export function countdown(date: string | null): string {
  const n = daysUntil(date);
  if (n === null) return "";
  if (n === 0) return t("releases.countdown.today");
  if (n === 1) return t("releases.countdown.tomorrow");
  if (n > 0) return t("releases.countdown.in", { n });
  return t("releases.countdown.ago", { n: -n });
}

export const longDate = (date: string | null) =>
  date ? fmtDate(new Date(date + "T00:00:00"), { day: "numeric", month: "long", year: "numeric" }) : "";

export const minutes = (s: number) =>
  s >= 3600
    ? t("releases.hours", { h: Math.floor(s / 3600), m: String(Math.round((s % 3600) / 60)).padStart(2, "0") })
    : t("releases.minutes", { n: Math.round(s / 60) });

/** Titles in natural order ("2 - x" before "10 - y"). */
export const natural = (a: string, b: string) => a.localeCompare(b, "fr", { numeric: true, sensitivity: "base" });

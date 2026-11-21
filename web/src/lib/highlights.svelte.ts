// What the pill at the top of the window says, in turn: one useful thing about the catalogue
// at a time (the next release, the song nearly finished, an idea left aside...), each one a
// link to where it is dealt with. Worked out from what the app already holds; nothing is
// stored. A highlight that has nothing to say today is left out.
import type { SongItem } from "./api";
import type { Release } from "./api/bindings/Release";
import { summary } from "./releases";
import { t } from "./i18n/index.svelte";
import { stageLabel } from "./format";

export type Highlight = { key: string; text: string; href: string; tone?: "warn" };

const DAY = 86400;
const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const stars = (n: number) => "★".repeat(Math.max(0, Math.min(5, n)));
const daysTo = (date: string, now: Date) => Math.round((new Date(date + "T12:00:00").getTime() - new Date(iso(now) + "T12:00:00").getTime()) / (DAY * 1000));
const months = (s: number) => Math.max(1, Math.round(s / (30 * DAY)));
const name = (s: SongItem) => s.title || t("songs.song");
// a choice that holds for the day (the same idea all day, another tomorrow)
function ofTheDay<T>(xs: T[], now: Date, salt: number): T | undefined {
  if (!xs.length) return undefined;
  const d = Math.floor(now.getTime() / (DAY * 1000)) + salt;
  return xs[((d * 2654435761) >>> 0) % xs.length];
}
const MIX = ["mix", "master"];

export function highlights(songs: SongItem[], releases: Release[], now = new Date()): Highlight[] {
  const live = songs.filter((s) => !s.archived);
  const ts = now.getTime() / 1000;
  const today = iso(now);
  const out: Highlight[] = [];

  // ---- releases: the next one, one whose date went by, finished songs without a date
  const upcoming = releases
    .filter((r) => r.date && daysTo(r.date, now) >= 0)
    .sort((a, b) => (a.date ?? "").localeCompare(b.date ?? ""));
  const next = upcoming[0];
  if (next?.date) {
    const s = summary(next, songs);
    const n = daysTo(next.date, now);
    out.push({
      key: "release",
      text: t(n === 0 ? "app.hl.releaseToday" : "app.hl.release", { title: next.title, n, ready: s.ready, count: s.count }),
      href: `#/release/${next.id}`,
      tone: n <= 14 && s.ready < s.count ? "warn" : undefined,
    });
  }
  const late = releases.find((r) => r.date && daysTo(r.date, now) < 0 && daysTo(r.date, now) > -60 && summary(r, songs).ready < summary(r, songs).count);
  if (late?.date) out.push({ key: "late", text: t("app.hl.late", { title: late.title, n: -daysTo(late.date, now) }), href: `#/release/${late.id}`, tone: "warn" });
  const inRelease = new Set(releases.flatMap((r) => r.songs.filter((x): x is number => x != null)));
  const doneNoDate = live.filter((s) => s.stage === "done" && !s.date && !inRelease.has(s.id));
  if (doneNoDate.length) out.push({ key: "doneNoDate", text: t("app.hl.doneNoDate", { n: doneNoDate.length }), href: "#/songs" });

  // ---- songs to finish
  const near = live
    .filter((s) => MIX.includes(s.stage ?? "") && s.checks > 0 && s.check < s.checks)
    .sort((a, b) => b.check / b.checks - a.check / a.checks || b.mtime - a.mtime)[0];
  if (near) out.push({ key: "near", text: t("app.hl.near", { title: name(near), done: near.check, n: near.checks, stage: stageLabel(near.stage ?? "") }), href: `#/song/${near.id}` });
  const noRender = live.filter((s) => MIX.includes(s.stage ?? "") && !s.temoin).sort((a, b) => b.mtime - a.mtime)[0];
  if (noRender) out.push({ key: "noRender", text: t("app.hl.noRender", { title: name(noRender), stage: stageLabel(noRender.stage ?? "") }), href: `#/song/${noRender.id}` });
  const resume = [...live].sort((a, b) => b.mtime - a.mtime)[0];
  if (resume && ts - resume.mtime < 3 * DAY) out.push({ key: "resume", text: t("app.hl.resume", { title: name(resume) }), href: `#/song/${resume.id}` });

  // ---- ideas left aside
  const forgotten = live.filter((s) => (s.rating ?? 0) >= 3 && s.stage !== "done" && ts - s.mtime > 180 * DAY);
  const f = ofTheDay(forgotten, now, 1);
  if (f) out.push({ key: "forgotten", text: t("app.hl.forgotten", { title: name(f), n: months(ts - f.mtime), stars: stars(f.rating ?? 0) }), href: `#/song/${f.id}` });
  const gems = live.filter((s) => (s.rating ?? 0) >= 4);
  const g = ofTheDay(gems, now, 2);
  if (g && g !== f) out.push({ key: "gem", text: t("app.hl.gem", { title: name(g), stars: stars(g.rating ?? 0) }), href: `#/song/${g.id}` });
  const drafts = live.filter((s) => (s.stage === "draft" || !s.stage) && s.temoin && ts - s.mtime > 60 * DAY && ts - s.mtime < 400 * DAY);
  const dr = ofTheDay(drafts, now, 3);
  if (dr) out.push({ key: "draft", text: t("app.hl.draft", { title: name(dr), n: months(ts - dr.mtime) }), href: `#/song/${dr.id}` });

  // ---- the work itself: this week, a streak, a year ago, the month's colour
  const week = new Set<number>();
  const days = new Set<string>();
  const weekAgo = iso(new Date(now.getTime() - 6 * DAY * 1000));
  for (const s of live) for (const d of s.days) {
    days.add(d);
    if (d >= weekAgo && d <= today) week.add(s.id);
  }
  if (week.size) out.push({ key: "week", text: t("app.hl.week", { n: week.size }), href: "#/songs" });
  let streak = 0;
  for (let d = new Date(now); days.has(iso(d)); d = new Date(d.getTime() - DAY * 1000)) streak++;
  if (streak >= 3) out.push({ key: "streak", text: t("app.hl.streak", { n: streak }), href: "#/songs" });
  const yearAgo = new Date(now.getTime() - 365 * DAY * 1000);
  const anniv = live.find((s) => Math.abs(s.created - yearAgo.getTime() / 1000) < 3 * DAY);
  if (anniv) out.push({ key: "anniv", text: t("app.hl.anniv", { title: name(anniv) }), href: `#/song/${anniv.id}` });
  const month = live.filter((s) => ts - s.created < 30 * DAY);
  if (month.length >= 2) {
    const count = (xs: (string | null)[]) => {
      const m = new Map<string, number>();
      for (const x of xs) if (x) m.set(x, (m.get(x) ?? 0) + 1);
      return [...m.entries()].sort((a, b) => b[1] - a[1])[0];
    };
    const k = count(month.map((s) => s.key));
    const tp = count(month.map((s) => (s.tempo ? String(Math.round(s.tempo)) : null)));
    if (k && k[1] >= 2) out.push({ key: "colour", text: t("app.hl.colour", { n: month.length, key: k[0], bpm: tp && tp[1] >= 2 ? tp[0] : "?" }), href: "#/stats" });
    else out.push({ key: "started", text: t("app.hl.started", { n: month.length }), href: "#/songs" });
  }

  // ---- what blocks: recent projects that do not open as saved
  const broken = live.filter((s) => s.sev >= 2 && ts - s.mtime < 30 * DAY);
  if (broken.length) out.push({ key: "broken", text: t("app.hl.broken", { n: broken.length }), href: "#/repair", tone: "warn" });
  return out;
}

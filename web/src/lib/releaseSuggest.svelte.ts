// Which releases a song most likely belongs to, for "send to a release": a release whose
// folder holds the song, one already holding songs of the same folder, the same artist, one
// not out yet. Plus the folder that looks like a release ("- EP - Night Shift") holding it,
// not made one yet. The menus are synchronous: the folder hints are fetched once per change.
import type { DragItem } from "./pdrag.svelte";
import * as api from "./api";
import type { Release } from "./api/bindings/Release";
import type { ReleaseHint } from "./api/bindings/ReleaseHint";
import { app } from "./state.svelte";
import { t } from "./i18n/index.svelte";

const hints = $state<{ v: number; list: ReleaseHint[] }>({ v: -1, list: [] });

/** The folder hints, fetched again after every change (one made a release). */
export function loadHints() {
  if (hints.v === app.version) return;
  hints.v = app.version;
  api
    .releaseHints()
    .then((h) => (hints.list = h))
    .catch(() => (hints.list = []));
}

const parent = (d: string) => d.split("/").slice(0, -1).join("/");
const today = () => new Date().toISOString().slice(0, 10);
const DAY = 86400;

export type Suggested = { sugg: Release[]; rest: Release[]; hint: ReleaseHint | null };

/** The releases the song is not in, the likely ones first (at most 3), and a folder hint. */
export function suggestReleases(id: number): Suggested {
  const s = app.songs.find((x) => x.id === id);
  const out = app.releases.filter((r) => !r.songs.includes(id));
  if (!s) return { sugg: [], rest: out, hint: null };
  const byId = new Map(app.songs.map((x) => [x.id, x]));
  const dir = s.dir ?? "";
  const artist = (s.artist ?? "").trim().toLowerCase();
  const now = Date.now() / 1000;
  const score = (r: Release) => {
    let n = 0;
    if (r.folder && dir && (dir === r.folder || dir.startsWith(r.folder + "/"))) n += 10;
    const members = r.songs.flatMap((x) => (x != null && byId.get(x) ? [byId.get(x)!] : []));
    if (dir && members.some((m) => m.dir === dir)) n += 6;
    else if (parent(dir) && members.some((m) => parent(m.dir ?? "") === parent(dir))) n += 3;
    const ra = (r.artist || "").trim().toLowerCase();
    if (artist && (ra === artist || members.some((m) => (m.artist ?? "").trim().toLowerCase() === artist))) n += 2;
    if (!r.date || r.date >= today()) n += 1;
    if (r.created && now - r.created < 60 * DAY) n += 1;
    return n;
  };
  const scored = out.map((r) => [r, score(r)] as const);
  const sugg = scored
    .filter(([, n]) => n >= 4)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([r]) => r);
  const rest = out.filter((r) => !sugg.includes(r));
  const hint = hints.list.find((h) => h.songs.includes(id) && !app.releases.some((r) => r.folder === h.folder)) ?? null;
  return { sugg, rest, hint };
}

/** A song dragged from a list (onto a release of the sidebar, a stage, a folder): see lib/pdrag. */
export const songDrag = (id: number, title: string | null | undefined): DragItem => ({ kind: "song", data: id, label: title || "" });

/** A song added at the end of a release (once: already in it, it is said). */
export async function addToRelease(rid: string, id: number) {
  const r = app.releases.find((x) => x.id === rid);
  const s = app.songs.find((x) => x.id === id);
  if (!r || !s) return;
  if (r.songs.includes(id)) return app.say(t("releases.dnd.already", { song: s.title ?? "", release: r.title }));
  try {
    const [, st] = await api.releaseSave({ ...r, songs: [...r.songs, id] });
    app.undo = st;
    app.say(t("releases.dnd.added", { song: s.title ?? "", release: r.title }), { undo: true });
    app.releases = await api.releases();
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

/** The folders that look like releases (fetched with `loadHints`). */
export const folderHints = () => hints.list;

/** A folder as it stands for the releases: the release made from it, the hint it gives, or
 *  why it cannot be one (it holds releases already: its EP inside an artist's folder...). */
export function folderRelease(dir: string): { release: Release | null; hint: ReleaseHint | null; inside: string[] } {
  const release = app.releases.find((r) => r.folder === dir) ?? null;
  const hint = hints.list.find((h) => h.folder === dir) ?? null;
  const inside = [
    ...app.releases.filter((r) => r.folder && r.folder.startsWith(dir + "/")).map((r) => r.title),
    ...hints.list.filter((h) => h.folder.startsWith(dir + "/")).map((h) => h.title),
  ];
  return { release, hint, inside: [...new Set(inside)] };
}

/** A release made from a folder that does not say it is one: its songs, its name, the
 *  format guessed from how many songs (1 a single, up to 6 an EP, more an album). */
export async function releaseFromFolder(dir: string, ids: number[]) {
  const kind = ids.length <= 1 ? "single" : ids.length <= 6 ? "ep" : "lp";
  return releaseFromHint({ folder: dir, title: dir.split("/").pop() || dir, kind, artist: "", songs: ids });
}

/** A release made from a folder hint (all its songs), opened. */
export async function releaseFromHint(h: ReleaseHint) {
  const byId = new Map(app.songs.map((x) => [x.id, x]));
  const count = new Map<string, number>();
  for (const id of h.songs) {
    const a = byId.get(id)?.artist;
    if (a) count.set(a, (count.get(a) ?? 0) + 1);
  }
  const artist = h.artist || ([...count].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "");
  const ids = [...h.songs].sort((a, b) => (byId.get(a)?.title ?? "").localeCompare(byId.get(b)?.title ?? "", undefined, { numeric: true }));
  try {
    const [rid, st] = await api.releaseSave({ id: "", title: h.title, kind: h.kind, artist, date: null, cover_song: null, cover: null, notes: "", songs: ids, folder: h.folder, created: 0, board: [] });
    app.undo = st;
    if (st.label) app.say(st.label, { undo: true });
    app.releases = await api.releases();
    location.hash = `#/release/${rid}`;
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

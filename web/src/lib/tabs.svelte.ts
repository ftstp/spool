// The songs open on the right of the Songs page, as tabs (kept across launches). Every
// link to a song ("#/song/<id>") opens it here: the list stays on the left. Tabs can be
// gathered in named, coloured groups that fold.
import { t } from "./i18n/index.svelte";

const KEY = "pf:tabs";
const MAX = 16;

export type Group = { id: number; name: string; color: string; folded: boolean; ids: number[] };
export const GROUP_COLORS = ["#3987e5", "#d55181", "#199e70", "#c98500", "#d95926", "#9085e9"];

function load(): { ids: number[]; active: number | null; groups: Group[] } {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && Array.isArray(v.ids)) {
      return { ids: v.ids.filter((x: unknown) => typeof x === "number"), active: v.active ?? null, groups: Array.isArray(v.groups) ? v.groups : [] };
    }
  } catch {
    /* nothing kept */
  }
  return { ids: [], active: null, groups: [] };
}

export const tabs = $state(load());

/** The address names the song shown; with no song left, it goes back to the list. */
function address() {
  const want = tabs.active === null ? "#/songs" : `#/song/${tabs.active}`;
  if (location.hash.startsWith("#/song") && location.hash !== want) history.replaceState(null, "", want);
}

function save() {
  // a group keeps only open tabs, and goes when empty
  tabs.groups = tabs.groups.map((g) => ({ ...g, ids: g.ids.filter((i) => tabs.ids.includes(i)) })).filter((g) => g.ids.length);
  address();
  try {
    localStorage.setItem(KEY, JSON.stringify({ ids: tabs.ids, active: tabs.active, groups: tabs.groups }));
  } catch {
    /* private window: not kept */
  }
}

/** Shows a song: its tab if open, else a new tab (the oldest ungrouped one goes past 16). */
export function openSong(id: number) {
  if (!tabs.ids.includes(id)) {
    let ids = [...tabs.ids, id];
    while (ids.length > MAX) {
      const drop = ids.find((x) => x !== id && !groupOf(x)) ?? ids[0];
      ids = ids.filter((x) => x !== drop);
    }
    tabs.ids = ids;
  }
  const g = groupOf(id);
  if (g?.folded) g.folded = false;
  tabs.active = id;
  save();
}

export function closeSongs(ids: number[]) {
  if (!ids.length) return;
  const i = tabs.active === null ? -1 : tabs.ids.indexOf(tabs.active);
  const left = tabs.ids.filter((x) => !ids.includes(x));
  if (tabs.active !== null && ids.includes(tabs.active)) {
    tabs.active = left[Math.min(Math.max(i, 0), left.length - 1)] ?? null;
  }
  tabs.ids = left;
  save();
}

export const closeSong = (id: number) => closeSongs([id]);
export const closeAll = () => closeSongs([...tabs.ids]);
export const closeOthers = (id: number) => closeSongs(tabs.ids.filter((x) => x !== id));
export const closeLeft = (id: number) => closeSongs(tabs.ids.slice(0, tabs.ids.indexOf(id)));
export const closeRight = (id: number) => closeSongs(tabs.ids.slice(tabs.ids.indexOf(id) + 1));

/** Drops tabs of songs that no longer exist. */
export function prune(exists: (id: number) => boolean) {
  const gone = tabs.ids.filter((x) => !exists(x));
  if (gone.length) closeSongs(gone);
}

// ---- groups

export const groupOf = (id: number) => tabs.groups.find((g) => g.ids.includes(id));

/** Tabs in display order: each group's tabs together, where its first tab was. */
export function ordered(): { group: Group | null; ids: number[] }[] {
  const out: { group: Group | null; ids: number[] }[] = [];
  const seen = new Set<number>();
  for (const id of tabs.ids) {
    if (seen.has(id)) continue;
    const g = groupOf(id);
    if (g) {
      const ids = tabs.ids.filter((x) => g.ids.includes(x));
      ids.forEach((x) => seen.add(x));
      out.push({ group: g, ids });
    } else {
      seen.add(id);
      const last = out.at(-1);
      if (last && !last.group) last.ids.push(id);
      else out.push({ group: null, ids: [id] });
    }
  }
  return out;
}

export function newGroup(id: number, name: string) {
  ungroup(id);
  const gid = Math.max(0, ...tabs.groups.map((g) => g.id)) + 1;
  tabs.groups = [...tabs.groups, { id: gid, name: name || t("menu.defaultGroup", { n: gid }), color: GROUP_COLORS[(gid - 1) % GROUP_COLORS.length], folded: false, ids: [id] }];
  save();
  return gid;
}

export function addToGroup(id: number, gid: number) {
  tabs.groups = tabs.groups.map((g) => ({ ...g, ids: g.id === gid ? [...g.ids.filter((x) => x !== id), id] : g.ids.filter((x) => x !== id) }));
  save();
}

export function ungroup(id: number) {
  tabs.groups = tabs.groups.map((g) => ({ ...g, ids: g.ids.filter((x) => x !== id) }));
  save();
}

export function editGroup(gid: number, patch: Partial<Pick<Group, "name" | "color" | "folded">>) {
  tabs.groups = tabs.groups.map((g) => (g.id === gid ? { ...g, ...patch } : g));
  save();
}

/** The group goes, its tabs stay. */
export function dissolve(gid: number) {
  tabs.groups = tabs.groups.filter((g) => g.id !== gid);
  save();
}

export function closeGroup(gid: number) {
  const g = tabs.groups.find((x) => x.id === gid);
  if (g) closeSongs([...g.ids]);
}

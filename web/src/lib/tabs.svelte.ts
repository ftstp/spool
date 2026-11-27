// The songs open on the right of the Songs page, as tabs (kept across launches). Every
// link to a song ("#/song/<id>") opens it here: the list stays on the left. Tabs can be
// gathered in named, coloured groups that fold.
import { t } from "./i18n/index.svelte";
import { feature } from "./telemetry";

const KEY = "pf:tabs";
const MAX = 16;

export type Group = { id: number; name: string; color: string; folded: boolean; ids: number[] };
export const GROUP_COLORS = ["#3987e5", "#d55181", "#199e70", "#c98500", "#d95926", "#9085e9"];

/** `open`: per open tab, the sections of its sheet unfolded (all folded otherwise). */
function load(): { ids: number[]; active: number | null; groups: Group[]; open: Record<string, string[]> } {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && Array.isArray(v.ids)) {
      return { ids: v.ids.filter((x: unknown) => typeof x === "number"), active: v.active ?? null, groups: Array.isArray(v.groups) ? v.groups : [], open: v.open && typeof v.open === "object" ? v.open : {} };
    }
  } catch {
    /* nothing kept */
  }
  return { ids: [], active: null, groups: [], open: {} };
}

export const tabs = $state(load());

/** Tabs or not (Settings > Spool): without, a song opened takes the place of the one shown. */
const SOLO = "spool.tabs.off";
export const tabMode = $state({
  off: (() => {
    try {
      return localStorage.getItem(SOLO) === "1";
    } catch {
      return false;
    }
  })(),
});
export function setTabsOff(off: boolean) {
  tabMode.off = off;
  try {
    if (off) localStorage.setItem(SOLO, "1");
    else localStorage.removeItem(SOLO);
  } catch { /* not kept */ }
  // turned off: only the song shown stays
  if (off) {
    tabs.ids = tabs.active === null ? [] : [tabs.active];
    save();
  }
}

/** The address names the song shown; with no song left, it goes back to the list. Another
 *  tab brought to the front is a step of the history (Back returns to the one before); a tab
 *  closed is not. */
function address(step = false) {
  const want = tabs.active === null ? "#/songs" : `#/song/${tabs.active}`;
  if (!location.hash.startsWith("#/song") || location.hash === want) return;
  if (step) location.hash = want;
  else history.replaceState(null, "", want);
}

function save(step = false) {
  // a group keeps only open tabs, and goes when empty
  tabs.groups = tabs.groups.map((g) => ({ ...g, ids: g.ids.filter((i) => tabs.ids.includes(i)) })).filter((g) => g.ids.length);
  // a closed tab forgets its sections: opened again, its sheet starts folded
  for (const k of Object.keys(tabs.open)) if (!tabs.ids.includes(Number(k))) delete tabs.open[k];
  address(step);
  try {
    localStorage.setItem(KEY, JSON.stringify({ ids: tabs.ids, active: tabs.active, groups: tabs.groups, open: tabs.open }));
  } catch {
    /* private window: not kept */
  }
}

/** Shows a song: its tab if open, else a new tab (the oldest ungrouped one goes past 16). */
export function openSong(id: number) {
  feature("song_sheet_opened");
  if (tabMode.off) tabs.ids = [id];
  else if (!tabs.ids.includes(id)) {
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
  save(true);
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

/** The sections open on a song's sheet: the user's own layout, the same for every song and
 *  kept between starts. The first time: the arrangement, its chords, the checklist and notes. */
const LAYOUT = "spool.sheet.open";
export const SHEET_DEFAULT = ["session", "chords", "checks", "board"];
const layout = $state<{ open: string[] }>({
  open: (() => {
    try {
      const v = JSON.parse(localStorage.getItem(LAYOUT) ?? "null");
      // the notes are in the mood board now: open where they were open
      if (Array.isArray(v)) return [...new Set(v.filter((x): x is string => typeof x === "string").map((x) => (x === "notes" ? "board" : x)))];
    } catch { /* the default */ }
    return [...SHEET_DEFAULT];
  })(),
});
export const sectionOpen = (_sid: number, key: string) => layout.open.includes(key);
export function setSectionOpen(_sid: number, key: string, on: boolean) {
  const cur = layout.open.filter((k) => k !== key);
  layout.open = on ? [...cur, key] : cur;
  try {
    localStorage.setItem(LAYOUT, JSON.stringify(layout.open));
  } catch { /* not kept */ }
}
/** The order of the sheet's sections chosen by the user (empty: the app's own), for the
 *  buttons at the top and the sections below alike, the same for every song. */
const ORDER = "spool.sheet.order";
export const sheetOrder = $state<{ keys: string[] }>({
  keys: (() => {
    try {
      const v = JSON.parse(localStorage.getItem(ORDER) ?? "null");
      if (Array.isArray(v)) return v.filter((x): x is string => typeof x === "string");
    } catch { /* the app's order */ }
    return [];
  })(),
});
export function setSheetOrder(keys: string[]) {
  sheetOrder.keys = keys;
  try {
    if (keys.length) localStorage.setItem(ORDER, JSON.stringify(keys));
    else localStorage.removeItem(ORDER);
  } catch { /* not kept */ }
}
/** `keys` (the app's order) in the user's: the ones it has not placed yet (a new section)
 *  stay after the one they follow in the app's order. */
export function inSheetOrder<T extends { key: string }>(list: T[]): T[] {
  const mine = sheetOrder.keys;
  if (!mine.length) return list;
  const out = list.filter((x) => mine.includes(x.key)).sort((a, b) => mine.indexOf(a.key) - mine.indexOf(b.key));
  list.forEach((x, i) => {
    if (mine.includes(x.key)) return;
    const before = list.slice(0, i).reverse().find((y) => mine.includes(y.key));
    out.splice(before ? out.indexOf(before) + 1 : 0, 0, x);
  });
  return out;
}

/** A tab moved by hand: before (or after) another one. */
export function moveTab(id: number, target: number, after: boolean) {
  if (id === target || !tabs.ids.includes(id) || !tabs.ids.includes(target)) return;
  const ids = tabs.ids.filter((x) => x !== id);
  ids.splice(ids.indexOf(target) + (after ? 1 : 0), 0, id);
  tabs.ids = ids;
  save();
}
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

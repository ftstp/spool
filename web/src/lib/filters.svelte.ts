// The Songs page's filters and view, kept while a song sheet is open (and the view across
// launches).
const saved = (() => {
  try {
    const v = localStorage.getItem("pf:view") ?? "list";
    // the grid view is gone: a launch that remembered it opens the list
    return ["list", "folders", "board", "cal", "projects"].includes(v) ? v : "list";
  } catch {
    return "list";
  }
})();

export const f = $state({
  view: saved as "list" | "folders" | "board" | "cal" | "projects",
  stages: [] as string[],
  /** Several values each: a song passes when it has one of them (none chosen: every song). */
  artist: [] as string[],
  style: [] as string[],
  key: [] as string[],
  bmin: null as number | null,
  bmax: null as number | null,
  half: false,
  sort: "recent",
  /** The sort the other way round (songs with no value for it stay last). */
  rev: false,
  archived: false,
  /** Plugins state of the song's projects: "ok" opens, "fix" the app repairs, "lost"
   *  settings lost, "bad" something to install. */
  /** "ok" opens, "fix" the app repairs, "lost" settings lost, "bad" something to install. */
  plugins: [] as string[],
  /** "recent": saved in the last 6 months; "asleep": nothing saved for 3 months; "idle":
   *  nothing saved for a year. */
  activity: "" as "" | "recent" | "asleep" | "idle",
  /** "yes": has an audio render (reference render); "no": none. */
  audio: "" as "" | "yes" | "no",
  /** At least this many stars (0: any). */
  rating: 0,
  /** Types ("Remix"...), none for any. */
  kind: [] as string[],
  /** Only the archived songs. */
  onlyArchived: false,
  /** Folders view: grouped by folder, artist or style. */
  by: "folder" as "folder" | "artist" | "style",
  /** Calendar: the month shown ("2026-11"), and the days projects were saved. */
  month: "",
  work: true,
  /** Calendar: the songs placed by their date (release, deadline), their first project's
   *  save (created) or their newest save (modified). */
  calBy: "date" as "date" | "created" | "mtime",
});

export function setView(v: typeof f.view) {
  f.view = v;
  try {
    localStorage.setItem("pf:view", v);
  } catch {
    /* private window: not remembered */
  }
}

// ---- presets: the filters, the sort and the view, kept under a name. The one marked as the
// default is put in place when Spool opens.
const PRESET_KEYS = ["view", "stages", "artist", "style", "key", "bmin", "bmax", "half", "sort", "rev", "archived", "plugins", "activity", "audio", "rating", "kind", "onlyArchived"] as const;
type PresetKey = (typeof PRESET_KEYS)[number];
export type Preset = { name: string; v: Partial<Pick<typeof f, PresetKey>>; def?: boolean };
const PRESETS = "spool.songs.presets";

function loadPresets(): Preset[] {
  try {
    const v = JSON.parse(localStorage.getItem(PRESETS) || "[]");
    return Array.isArray(v) ? v.filter((p) => p && typeof p.name === "string" && p.v) : [];
  } catch {
    return [];
  }
}
export const presets = $state({ list: loadPresets() });
function keep() {
  try {
    localStorage.setItem(PRESETS, JSON.stringify(presets.list));
  } catch {
    /* private window: not kept */
  }
}
/** What the filters are now, as a preset holds them. */
function snapshot(): Preset["v"] {
  const v: Record<string, unknown> = {};
  for (const k of PRESET_KEYS) v[k] = Array.isArray(f[k]) ? [...(f[k] as string[])] : f[k];
  return v as Preset["v"];
}
/** The filters as they are, saved under `name` (a preset of that name is replaced). */
export function savePreset(name: string) {
  const n = name.trim();
  if (!n) return;
  const old = presets.list.find((p) => p.name.toLowerCase() === n.toLowerCase());
  const p: Preset = { name: n, v: snapshot(), def: old?.def };
  presets.list = old ? presets.list.map((x) => (x === old ? p : x)) : [...presets.list, p];
  keep();
}
const BLANK: Preset["v"] = { rev: false, stages: [], artist: [], style: [], key: [], bmin: null, bmax: null, half: false, archived: false, plugins: [], activity: "", audio: "", rating: 0, kind: [], onlyArchived: false };
// a preset kept before a filter took several values held one, as text
const many = (v: unknown) => (Array.isArray(v) ? [...v] : typeof v === "string" && v ? [v] : []);
/** A preset put in place: every filter it does not set goes back to none. */
export function applyPreset(p: Preset) {
  const v = p.v as Record<string, unknown>;
  Object.assign(f, BLANK, p.v, {
    stages: many(v.stages),
    artist: many(v.artist),
    style: many(v.style),
    key: many(v.key),
    kind: many(v.kind),
    plugins: many(v.plugins),
  });
  if (p.v.view) setView(p.v.view);
}
export function removePreset(name: string) {
  presets.list = presets.list.filter((p) => p.name !== name);
  keep();
}
/** The default preset (none: `name` null), put in place when Spool opens. */
export function setDefaultPreset(name: string | null) {
  presets.list = presets.list.map((p) => ({ ...p, def: p.name === name ? true : undefined }));
  keep();
}
/** The preset the filters match now, if any. */
export function currentPreset(): Preset | null {
  const now = JSON.stringify(snapshot());
  return presets.list.find((p) => {
    const v = p.v as Record<string, unknown>;
    const as = { ...BLANK, ...p.v, stages: many(v.stages), artist: many(v.artist), style: many(v.style), key: many(v.key), kind: many(v.kind), plugins: many(v.plugins) };
    return JSON.stringify({ ...snapshot(), ...as }) === now;
  }) ?? null;
}
// at launch: the default one
{
  const d = presets.list.find((p) => p.def);
  if (d) applyPreset(d);
}

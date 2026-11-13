// The Songs page's filters and view, kept while a song sheet is open (and the view across
// launches).
const saved = (() => {
  try {
    return localStorage.getItem("pf:view") ?? "list";
  } catch {
    return "list";
  }
})();

export const f = $state({
  view: saved as "list" | "folders" | "board" | "cal" | "grid",
  stages: [] as string[],
  artist: "",
  style: "",
  key: "",
  bmin: null as number | null,
  bmax: null as number | null,
  half: false,
  sort: "recent",
  archived: false,
  /** Plugins state of the song's projects: "ok" opens, "fix" the app repairs, "lost"
   *  settings lost, "bad" something to install. */
  plugins: "" as "" | "ok" | "fix" | "lost" | "bad",
  /** "recent": saved in the last 6 months; "idle": nothing saved for a year. */
  activity: "" as "" | "recent" | "idle",
  /** "yes": has an audio render (reference render); "no": none. */
  audio: "" as "" | "yes" | "no",
  /** At least this many stars (0: any). */
  rating: 0,
  /** A type ("Remix"...), "" for any. */
  kind: "",
  /** Only the archived songs. */
  onlyArchived: false,
  /** Folders view: grouped by folder, artist or style. */
  by: "folder" as "folder" | "artist" | "style",
  /** Calendar: the month shown ("2026-11"), and the days projects were saved. */
  month: "",
  work: true,
});

export function setView(v: typeof f.view) {
  f.view = v;
  try {
    localStorage.setItem("pf:view", v);
  } catch {
    /* private window: not remembered */
  }
}

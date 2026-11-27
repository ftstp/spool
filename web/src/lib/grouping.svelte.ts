// A folder made one song from a song's sheet: the rule kept ("one_song_dirs"), and what it
// proposes next, the folders beside it that hold several songs too (shown on the sheet of
// the song the folder became).
import * as api from "./api";
import { app } from "./state.svelte";
import { t } from "./i18n/index.svelte";

export const after = $state<{ dir: string; dirs: string[] }>({ dir: "", dirs: [] });

/** `dirs` read as one song each, kept with the rules in force; one Ctrl+Z. */
export async function oneSong(dirs: string[]) {
  const r = await api.readingReport();
  const st = await api.setGrouping(r.group_mode === "folder", [...new Set([...r.one_song_dirs, ...dirs])], r.title_from);
  app.undo = st;
  if (st.label) app.say(st.label, { undo: true });
  await app.reload();
}

/** One folder made one song, then its neighbours looked for. */
export async function gatherFolder(dir: string) {
  await oneSong([dir]);
  after.dir = dir;
  after.dirs = await api.folderNeighbours(dir).catch(() => []);
}

export async function gatherNeighbours() {
  const ds = after.dirs;
  after.dirs = [];
  try {
    await oneSong(ds);
  } catch (e) {
    app.say(String(e), { error: true });
  }
  void t;
}

// What a right-click offers, by what is under the pointer: a song (any element with
// data-song), an open song's tab (data-tab), a group of tabs (data-group), a folder
// (data-folder) or a loose media file (data-media). App.svelte listens once for the whole
// window; the dialogs these items open live in `ui`.
import * as api from "./api";
import { app } from "./state.svelte";
import { player } from "./player.svelte";
import { STAGES, stageLabel } from "./format";
import { openMenu, type MenuItem } from "./menu.svelte";
import { t } from "./i18n/index.svelte";
import {
  tabs, openSong, closeSong, closeOthers, closeLeft, closeRight, closeAll, groupOf, newGroup, addToGroup, ungroup,
  editGroup, dissolve, closeGroup, GROUP_COLORS,
} from "./tabs.svelte";

/** Dialogs a menu opens: a folder to move, files for a mood board, a name to type. */
export const ui = $state<{
  move: string | null;
  toBoard: string[] | null;
  prompt: { title: string; value: string; ok: (v: string) => void } | null;
}>({ move: null, toBoard: null, prompt: null });

const go = (h: string) => (location.hash = h);
async function copy(rel: string) {
  try {
    app.say(t("menu.pathCopied", { path: await api.copyPath(rel) }));
  } catch (e) {
    app.say(String(e), { error: true });
  }
}
const reveal = (rel: string) => api.reveal(rel).catch((e) => app.say(String(e), { error: true }));
const edit = (id: number, body: Record<string, unknown>) => app.act(api.songEdit(id, body));
const leaf = (p: string) => p.split("/").pop() || p;
const COLOR_KEYS = ["blue", "pink", "green", "yellow", "orange", "violet"];

// a song in a release, or out of it (the release saved whole, one Ctrl+Z)
async function toggleRelease(rid: string, id: number) {
  const r = app.releases.find((x) => x.id === rid);
  if (!r) return;
  const songs = r.songs.includes(id) ? r.songs.filter((x) => x !== id) : [...r.songs, id];
  try {
    const [, st] = await api.releaseSave({ ...r, songs });
    app.undo = st;
    if (st.label) app.say(st.label, { undo: true });
  } catch (e) {
    app.say(String(e), { error: true });
  }
}
async function newRelease(id: number, title: string) {
  try {
    const [rid, st] = await api.releaseSave({ id: "", title: title || t("menu.newReleaseTitle"), kind: "single", artist: "", date: null, cover_song: id, notes: "", songs: [id], folder: null, created: 0 });
    app.undo = st;
    if (st.label) app.say(st.label, { undo: true });
    go(`#/release/${rid}`);
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

/** A song's menu, also from its sheet's "..." button. */
export function openSongMenu(e: MouseEvent, id: number) {
  songMenu(e, id);
}

function songMenu(e: MouseEvent, id: number) {
  const s = app.songs.find((x) => x.id === id);
  if (!s) return;
  const own = !!s.dir && app.songs.filter((x) => x.dir === s.dir && !x.archived).length <= 1;
  const open = tabs.active === id && location.hash.startsWith("#/song");
  const items: MenuItem[] = [
    ...(open ? [] : [{ label: t("menu.song.open"), run: () => { openSong(id); go(`#/song/${id}`); } }]),
    { label: t("menu.song.openInLive"), run: () => api.openInLive(s.rel).catch((er) => app.say(String(er), { error: true })) },
    { label: t("menu.song.listenTemoin"), disabled: !s.temoin, run: () => player.playSong(id, s.title ?? "") },
    { sep: true },
    {
      label: t("menu.song.stage"),
      sub: [
        ...STAGES.map((k) => ({ label: stageLabel(k), checked: s.stage === k, run: () => edit(id, { stage: k }) })),
        { sep: true } as MenuItem,
        { label: stageLabel(null), checked: !s.stage, run: () => edit(id, { stage: null }) },
      ],
    },
    {
      label: t("menu.song.rating"),
      sub: [
        ...[5, 4, 3, 2, 1].map((n) => ({ label: "★".repeat(n), checked: s.rating === n, run: () => edit(id, { rating: n }) })),
        { sep: true } as MenuItem,
        { label: t("menu.song.noRating"), checked: !s.rating, run: () => edit(id, { rating: null }) },
      ],
    },
    {
      label: t("menu.song.release"),
      sub: [
        ...app.releases.map((r) => ({ label: r.title, checked: r.songs.includes(id), run: () => toggleRelease(r.id, id) })),
        ...(app.releases.length ? [{ sep: true } as MenuItem] : []),
        { label: t("menu.song.newRelease"), run: () => newRelease(id, s.title ?? "") },
      ],
    },
    { sep: true },
    { label: t("menu.song.reveal"), run: () => reveal(s.rel) },
    { label: t("menu.song.copyProjectPath"), run: () => copy(s.rel) },
    { label: t("menu.song.copyFolderPath"), disabled: !s.dir, run: () => copy(s.dir) },
    { sep: true },
    {
      label: s.archived ? t("menu.song.unarchive") : t("menu.song.archive"),
      hint: s.archived ? "" : t("menu.song.archiveHint"),
      run: () => edit(id, { archived: !s.archived }),
    },
    ...(app.physical ? [{ label: t("menu.song.moveFolder"), disabled: !own, hint: own ? "" : t("menu.song.sharedFolder"), run: () => (ui.move = s.dir) }] : []),
  ];
  openMenu(e, items, s.title ?? "");
}

function tabMenu(e: MouseEvent, id: number) {
  const i = tabs.ids.indexOf(id);
  const g = groupOf(id);
  const items: MenuItem[] = [
    { label: t("menu.tab.close"), run: () => closeSong(id) },
    { label: t("menu.tab.closeOthers"), disabled: tabs.ids.length < 2, run: () => closeOthers(id) },
    { label: t("menu.tab.closeLeft"), disabled: i <= 0, run: () => closeLeft(id) },
    { label: t("menu.tab.closeRight"), disabled: i >= tabs.ids.length - 1, run: () => closeRight(id) },
    { label: t("menu.tab.closeAll"), run: () => closeAll() },
    { sep: true },
    {
      label: t("menu.tab.group"),
      sub: [
        { label: t("menu.tab.newGroup"), run: () => (ui.prompt = { title: t("menu.tab.groupName"), value: "", ok: (v) => newGroup(id, v) }) },
        ...tabs.groups.map((x) => ({ label: x.name, checked: g?.id === x.id, run: () => addToGroup(id, x.id) })),
        ...(g ? [{ sep: true } as MenuItem, { label: t("menu.tab.ungroup"), run: () => ungroup(id) }] : []),
      ],
    },
  ];
  openMenu(e, items, app.songs.find((x) => x.id === id)?.title ?? "");
}

function groupMenu(e: MouseEvent, gid: number) {
  const g = tabs.groups.find((x) => x.id === gid);
  if (!g) return;
  openMenu(
    e,
    [
      { label: g.folded ? t("menu.group.unfold") : t("menu.group.fold"), run: () => editGroup(gid, { folded: !g.folded }) },
      { label: t("menu.group.rename"), run: () => (ui.prompt = { title: t("menu.tab.groupName"), value: g.name, ok: (v) => editGroup(gid, { name: v || g.name }) }) },
      { label: t("menu.group.color"), sub: GROUP_COLORS.map((c, n) => ({ label: t(`menu.group.colors.${COLOR_KEYS[n]}`), checked: g.color === c, run: () => editGroup(gid, { color: c }) })) },
      { sep: true },
      { label: t("menu.group.dissolve"), run: () => dissolve(gid) },
      { label: t("menu.group.closeTabs"), danger: true, run: () => closeGroup(gid) },
    ],
    g.name,
  );
}

function folderMenu(e: MouseEvent, dir: string) {
  openMenu(
    e,
    [
      { label: t("menu.folder.reveal"), run: () => reveal(dir) },
      { label: t("menu.folder.copyPath"), run: () => copy(dir) },
      ...(app.physical
        ? [
            { sep: true } as MenuItem,
            { label: t("menu.folder.move"), run: () => (ui.move = dir) },
            {
              label: t("menu.folder.newSub"),
              run: () => (ui.prompt = { title: t("menu.folder.newIn", { name: leaf(dir) }), value: "", ok: (v) => v && app.fileAct(api.fsMkdir(`${dir}/${v}`)) }),
            },
          ]
        : []),
    ],
    leaf(dir),
  );
}

function mediaMenu(e: MouseEvent, rel: string, kind: string) {
  openMenu(
    e,
    [
      { label: t("menu.media.toBoard"), run: () => (ui.toBoard = [rel]) },
      ...(kind === "audio" ? [{ label: t("menu.media.listen"), run: () => player.play(rel, null, leaf(rel)) }] : []),
      { sep: true },
      { label: t("menu.folder.reveal"), run: () => reveal(rel) },
      { label: t("menu.folder.copyPath"), run: () => copy(rel) },
    ],
    leaf(rel),
  );
}

/** The window's right-click: the nearest thing that has a menu, else the system's (in a
 *  text field) or nothing. */
export function onContextMenu(e: MouseEvent) {
  const t = e.target as HTMLElement | null;
  if (!t?.closest) return;
  if (t.closest("input, textarea, [contenteditable=true]")) return;
  const el = t.closest<HTMLElement>("[data-tab], [data-group], [data-song], [data-folder], [data-media], a[href^='#/song/']");
  if (!el) {
    e.preventDefault();
    return;
  }
  const d = el.dataset;
  // any link to a song is the song
  if (!d.tab && !d.group && !d.song && !d.folder && !d.media) {
    const id = +(el.getAttribute("href") ?? "").slice(7);
    if (id) songMenu(e, id);
    else e.preventDefault();
    return;
  }
  if (d.tab) tabMenu(e, +d.tab);
  else if (d.group) groupMenu(e, +d.group);
  else if (d.song) songMenu(e, +d.song);
  else if (d.folder) folderMenu(e, d.folder);
  else if (d.media) mediaMenu(e, d.media, d.kind ?? "");
}

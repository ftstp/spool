// What a right-click offers, by what is under the pointer: a song (any element with
// data-song), an open song's tab (data-tab), a group of tabs (data-group), a folder
// (data-folder) or a loose media file (data-media). App.svelte listens once for the whole
// window; the dialogs these items open live in `ui`.
import { liveOpen } from "./liveOpen.svelte";
import * as api from "./api";
import { app } from "./state.svelte";
import { player } from "./player.svelte";
import { midi } from "./midiPlayer.svelte";
import { KIND_KEYS, kindLabel } from "./releases";
import { STAGES, stageLabel } from "./format";
import { openMenu, type MenuItem } from "./menu.svelte";
import { t } from "./i18n/index.svelte";
import { suggestReleases, releaseFromHint, loadHints } from "./releaseSuggest.svelte";
import {
  tabs, openSong, closeSong, closeOthers, closeLeft, closeRight, closeAll, groupOf, newGroup, addToGroup, ungroup,
  editGroup, dissolve, closeGroup, GROUP_COLORS,
} from "./tabs.svelte";

/** Dialogs a menu opens: a folder to move, files for a mood board, a name to type. */
export const ui = $state<{
  move: string | null;
  toBoard: string[] | null;
  /** A picture to make the main image of a song, a release or an artist. */
  cover: string | null;
  prompt: { title: string; value: string; ok: (v: string) => void } | null;
  /** The song whose sheet opens its "attach another song" picker. */
  mergeInto: number | null;
  /** A change that reshapes the catalogue, said before it is done (Ctrl+Z still undoes it). */
  confirm: { title: string; text: string; ok: string; run: () => void } | null;
}>({ move: null, toBoard: null, cover: null, prompt: null, mergeInto: null, confirm: null });

const go = (h: string) => (location.hash = h);
async function copy(rel: string) {
  try {
    app.say(t("menu.pathCopied", { path: await api.copyPath(rel) }));
  } catch (e) {
    app.say(String(e), { error: true });
  }
}
const reveal = (rel: string) => api.reveal(rel).catch((e) => app.say(String(e), { error: true }));
const edit = (id: number, body: Record<string, unknown>) => app.editSong(id, body);
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
    const [rid, st] = await api.releaseSave({ id: "", title: title || t("menu.newReleaseTitle"), kind: "single", artist: "", date: null, cover_song: id, cover: null, notes: "", songs: [id], folder: null, created: 0, board: [] });
    app.undo = st;
    if (st.label) app.say(st.label, { undo: true });
    // the page finds it at once, without waiting for the whole catalogue to reload
    app.releases = await api.releases();
    go(`#/release/${rid}`);
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

/** A song's menu, also from its sheet's "..." button. */
/** The Samples or MIDI page, filtered on a song's key and tempo. */
function findLink(page: "samples" | "midi", s: { key: string | null; tempo: number | null; title: string | null }) {
  const p = new URLSearchParams();
  if (s.key) p.set("key", s.key);
  if (s.tempo) p.set("bpm", String(Math.round(s.tempo * 10) / 10));
  p.set("from", s.title ?? "");
  return `#/${page}?` + p.toString();
}

export function openSongMenu(e: MouseEvent, id: number) {
  songMenu(e, id);
}

function songMenu(e: MouseEvent, id: number, extra: MenuItem[] = []) {
  const s = app.songs.find((x) => x.id === id);
  if (!s) return;
  const own = !!s.dir && app.songs.filter((x) => x.dir === s.dir && !x.archived).length <= 1;
  const open = tabs.active === id && location.hash.startsWith("#/song");
  // the releases: the ones it is in, then the likely ones, then the rest
  loadHints();
  const rs = suggestReleases(id);
  const inR = app.releases.filter((r) => r.songs.includes(id));
  const relItem = (r: (typeof app.releases)[number]) => ({ label: r.title, checked: r.songs.includes(id), run: () => toggleRelease(r.id, id) });
  const relSub: MenuItem[] = [
    ...inR.map(relItem),
    ...(inR.length && (rs.sugg.length || rs.hint) ? [{ sep: true } as MenuItem] : []),
    ...(rs.sugg.length || rs.hint ? [{ label: t("menu.song.suggested"), disabled: true }] : []),
    ...rs.sugg.map(relItem),
    ...(rs.hint ? [{ label: t("menu.song.fromFolder", { title: rs.hint.title, n: rs.hint.songs.length }), run: () => releaseFromHint(rs.hint!) }] : []),
    ...(rs.rest.length && (inR.length || rs.sugg.length || rs.hint) ? [{ sep: true } as MenuItem] : []),
    ...rs.rest.map(relItem),
    ...(app.releases.length || rs.hint ? [{ sep: true } as MenuItem] : []),
    { label: t("menu.song.newRelease"), run: () => newRelease(id, s.title ?? "") },
  ];
  const items: MenuItem[] = [
    ...(open ? [] : [{ label: t("menu.song.open"), run: () => { openSong(id); go(`#/song/${id}`); } }]),
    { label: t("menu.song.openInLive"), run: () => void liveOpen.open(s.rel) },
    { label: t("menu.song.listenTemoin"), disabled: !s.temoin, run: () => player.playSong(id, s.title ?? "") },
    // your samples and MIDI files in its key and at its tempo
    { label: t("menu.song.findSamples"), run: () => go(findLink("samples", s)) },
    { label: t("menu.song.findMidi"), run: () => go(findLink("midi", s)) },
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
      sub: relSub,
    },
    { label: t("menu.song.mergeWith"), run: () => { ui.mergeInto = id; openSong(id); go(`#/song/${id}`); } },
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
  openMenu(e, extra.length ? [...extra, { sep: true }, ...items] : items, s.title ?? "");
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

const PICTURE = /\.(png|jpe?g|webp|gif|bmp|avif|tiff?)$/i;

function mediaMenu(e: MouseEvent, rel: string, kind: string) {
  const picture = kind === "image" || kind === "gif" || (!kind && PICTURE.test(rel));
  const playing = kind === "midi" ? midi.is(rel) : player.is(rel) && player.playing;
  openMenu(
    e,
    [
      ...(kind === "audio" || kind === "midi"
        ? [{ label: playing ? t("menu.media.stop") : t("menu.media.listen"), run: () => (kind === "midi" ? midi.play(rel) : player.is(rel) ? player.toggle() : player.play(rel, null, leaf(rel))) }]
        : []),
      ...(picture ? [{ label: t("menu.media.asCover"), run: () => (ui.cover = rel) }] : []),
      { label: t("menu.media.toBoard"), run: () => (ui.toBoard = [rel]) },
      { sep: true },
      { label: t("menu.folder.reveal"), run: () => reveal(rel) },
      { label: t("menu.folder.copyPath"), run: () => copy(rel) },
      { label: t("menu.media.copyName"), run: () => copyText(leaf(rel).replace(/\.[^.]+$/, "")) },
    ],
    leaf(rel),
  );
}

async function copyText(s: string) {
  try {
    await navigator.clipboard.writeText(s);
    app.say(t("menu.copied", { text: s.length > 60 ? s.slice(0, 57) + "..." : s }));
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

// a release, saved whole (one Ctrl+Z)
async function saveRelease(r: (typeof app.releases)[number]) {
  try {
    const [, st] = await api.releaseSave(r);
    app.undo = st;
    if (st.label) app.say(st.label, { undo: true });
    app.releases = await api.releases();
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

/** A release's menu: from its card in Releases, its page, anything carrying data-release. */
function releaseMenu(e: MouseEvent, rid: string) {
  const r = app.releases.find((x) => x.id === rid);
  if (!r) return;
  const titles = r.songs.map((id, i) => `${i + 1}. ${app.songs.find((s) => s.id === id)?.title ?? "?"}`).join("\n");
  const here = location.hash.startsWith(`#/release/${rid}`);
  openMenu(
    e,
    [
      ...(here ? [] : [{ label: t("menu.release.open"), run: () => go(`#/release/${rid}`) }]),
      { label: t("menu.release.rename"), run: () => (ui.prompt = { title: t("menu.release.rename"), value: r.title, ok: (v) => v && saveRelease({ ...r, title: v }) }) },
      { label: t("menu.release.kind"), sub: KIND_KEYS.map((k) => ({ label: kindLabel(k), checked: r.kind === k, run: () => saveRelease({ ...r, kind: k }) })) },
      { label: t("menu.release.copyTracklist"), disabled: !r.songs.length, run: () => copyText(`${r.title}\n${titles}`) },
      { sep: true },
      {
        label: t("menu.release.delete"),
        hint: t("menu.release.deleteHint"),
        danger: true,
        run: async () => {
          try {
            app.undo = await api.releaseDelete(rid);
            if (app.undo.label) app.say(app.undo.label, { undo: true });
            app.releases = await api.releases();
            if (here) go("#/releases");
          } catch (er) {
            app.say(String(er), { error: true });
          }
        },
      },
    ],
    r.title,
  );
}

/** A track of a release: the song's own menu, and its place in the release on top. */
function trackMenu(e: MouseEvent, rid: string, i: number) {
  const r = app.releases.find((x) => x.id === rid);
  const id = r?.songs[i];
  if (!r) return;
  const move = (to: number) => {
    const songs = [...r.songs];
    const [m] = songs.splice(i, 1);
    songs.splice(to, 0, m);
    saveRelease({ ...r, songs });
  };
  const own: MenuItem[] = [
    { label: t("menu.track.up"), disabled: i === 0, run: () => move(i - 1) },
    { label: t("menu.track.down"), disabled: i >= r.songs.length - 1, run: () => move(i + 1) },
    { label: t("menu.track.cover"), disabled: id == null || (r.cover_song === id && !r.cover), run: () => saveRelease({ ...r, cover_song: id ?? null, cover: null }) },
    { label: t("menu.track.remove"), danger: true, run: () => saveRelease({ ...r, songs: r.songs.filter((_, j) => j !== i) }) },
  ];
  if (id == null) return openMenu(e, own, r.title);
  songMenu(e, id, own);
}

/** The window's right-click: the nearest thing that has a menu, else the system's (in a
 *  text field) or nothing. */
export function onContextMenu(e: MouseEvent) {
  const t = e.target as HTMLElement | null;
  if (!t?.closest) return;
  if (t.closest("input, textarea, [contenteditable=true]")) return;
  const el = t.closest<HTMLElement>("[data-track], [data-release], [data-tab], [data-group], [data-song], [data-folder], [data-media], a[href^='#/song/']");
  if (!el) {
    e.preventDefault();
    return;
  }
  const d = el.dataset;
  if (d.track && d.release) return trackMenu(e, d.release, +d.track);
  if (d.release) return releaseMenu(e, d.release);
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

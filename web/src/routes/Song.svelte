<script lang="ts">
  import { outerHeightOf } from "../lib/size";
  // One song's sheet: its fields, stage, reference render, projects, finishing checks, mood board and
  // devices. Every change goes through the core and can be undone (Ctrl+Z).
  import * as api from "../lib/api";
  import type { SongDetail } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { STAGES, ago, bpm, dur, hue, stageLabel } from "../lib/format";
  import { t, fmtDate, compare } from "../lib/i18n/index.svelte";
  import Field from "../lib/components/Field.svelte";
  import StyleConfirm from "../lib/components/StyleConfirm.svelte";
  import Stars from "../lib/components/Stars.svelte";
  import FavStar from "../lib/components/FavStar.svelte";
  import { focus } from "../lib/focus.svelte";
  import type { Device } from "../lib/api/bindings/Device";
  import { statusSeverity } from "../lib/severity";
  // a plugin's state, as Repair counts it: 1 the app repairs it, 2 a version to install,
  // 3 not installed (an ignored one counts for nothing)
  const PSAY_KEYS = ["ok", "renamed", "trial", "vst3", "other_vst2", "newer", "to_vst3", "other_vst3", "missing"];
  const psay = (st?: string | null) => (st && PSAY_KEYS.includes(st) ? t(`song.psay.${st}`) : st);
  const devSev = (x: Device) => (x.native || x.ignored ? 0 : statusSeverity(x.status));
  const devTip = (x: Device) =>
    x.native ? t("song.dev.native") : [x.name, x.ignored ? t("song.dev.ignored") : psay(x.status), x.instead && t("song.dev.here", { name: x.instead }), x.count > 1 && t("song.dev.instances", { n: x.count }), t("song.dev.clickHint")].filter(Boolean).join("\n");
  function openPlugin(x: Device) {
    if (x.native || !x.key) return;
    focus.plugin = x.key;
    location.hash = "#/repair";
  }
  import Player from "../lib/components/Player.svelte";
  import MoodBoard from "../lib/components/MoodBoard.svelte";
  import Resources from "../lib/components/Resources.svelte";
  import Merges from "../lib/components/Merges.svelte";
  import Compare from "../lib/components/Compare.svelte";
  import Comments from "../lib/components/Comments.svelte";
  import AlsDiffView from "../lib/components/AlsDiffView.svelte";
  import MiniSession from "../lib/components/MiniSession.svelte";
  import LiveButton from "../lib/components/LiveButton.svelte";
  import { ui } from "../lib/contextMenus.svelte";
  import { openSong } from "../lib/tabs.svelte";
  import Unstuck from "../lib/components/Unstuck.svelte";
  import { masonry } from "../lib/masonry";
  import Checks from "../lib/components/Checks.svelte";
  import Notes from "../lib/components/Notes.svelte";
  import RefList from "../lib/components/RefList.svelte";
  import PlayIcon from "../lib/components/PlayIcon.svelte";
  import FoldHead from "../lib/components/FoldHead.svelte";
  import SheetTransport from "../lib/components/SheetTransport.svelte";
  import { sheetTransport } from "../lib/sheetTransport.svelte";
  import { tick } from "svelte";
  import Skeleton from "../lib/components/Skeleton.svelte";
  import { sectionOpen, setSectionOpen, sheetOrder, setSheetOrder, inSheetOrder } from "../lib/tabs.svelte";
  import { openMenu } from "../lib/menu.svelte";
  import { after as gathered, gatherFolder, gatherNeighbours } from "../lib/grouping.svelte";
  import { playingFile, playOrPause } from "../lib/player.svelte";
  import { commentsOf } from "../lib/comments.svelte";
  import { player } from "../lib/player.svelte";
  import { openSongMenu } from "../lib/contextMenus.svelte";
  import { suggestReleases, releaseFromHint, loadHints } from "../lib/releaseSuggest.svelte";
  import { onMount } from "svelte";

  // files dropped from Explorer anywhere on the sheet: onto its mood board (linked when they
  // are in the projects folders, copied when they come from elsewhere). Only the sheet under
  // the pointer takes them (several can be open in tabs).
  let sheet = $state<HTMLElement>();
  let dropping = $state(false);
  const under = (x: number, y: number) => {
    const r = sheet?.getBoundingClientRect();
    return !!r && r.width > 0 && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  };
  onMount(() => {
    let off: (() => void) | undefined;
    let gone = false;
    api.onFileDrop(async (kind, paths, x, y) => {
      if (kind === "leave") return void (dropping = false);
      // a card that takes the files itself (the references) keeps them
      const ref = [...(sheet?.querySelectorAll<HTMLElement>("[data-refdrop]") ?? [])].some((el) => {
        const r = el.getBoundingClientRect();
        return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
      });
      if (kind === "over") return void (dropping = under(x, y) && !ref);
      dropping = false;
      if (!under(x, y) || ref || !paths.length || !s) return;
      try {
        const r = await api.boardDrop(s.id, paths);
        const parts = [
          r.linked.length && t("song.board.linkedN", { n: r.linked.length }),
          r.copied.length && t("song.board.copiedN", { n: r.copied.length }),
          r.refused.length && t("song.board.refusedN", { n: r.refused.length, names: r.refused.slice(0, 3).join(", ") }),
        ].filter(Boolean);
        app.say(parts.join(" · "), { error: !r.linked.length && !r.copied.length });
        await app.reload();
      } catch (e) {
        app.say(String(e), { error: true });
      }
    }).then((f) => (gone ? f() : (off = f)));
    return () => {
      gone = true;
      off?.();
    };
  });

  /** `embedded`: shown in a tab beside the songs list (no way back: the list is there). */
  let { id, embedded = false }: { id: number; embedded?: boolean } = $props();
  let d = $state<SongDetail | null>(null);
  let missing = $state(false);
  // the song the sheet holds: another tab's song shows its skeleton until its own comes,
  // never the last one's sheet
  let loadedId = $state<number | null>(null);

  // the newest answer only: two fetches close together may come back out of order
  let seqSong = 0;
  let seqGuess = 0;
  $effect(() => {
    void app.version; // fetched again after every change
    const want = id;
    const n = ++seqSong;
    api.song(want).then((r) => {
      if (want !== id || n !== seqSong) return;
      d = r;
      missing = !r;
      loadedId = want;
    });
  });

  const s = $derived(d?.song);
  // the audio analysis of the reference render (opt-in): measured tempo beside the project's
  // when they differ by more than 3 %, and the songs that sound alike
  // measured on a real catalogue: two unrelated renders score about 0.55, 1 pair in 100
  // reaches 0.87; a song's closest other render is about 0.87
  const SIM_VERY = 0.9;
  const SIM_CLOSE = 0.8;
  const simWord = (x: number) => t(x >= SIM_VERY ? "song.sound.veryClose" : x >= SIM_CLOSE ? "song.sound.close" : "song.sound.related");
  /** What the app would suggest for the style and the key, shown on hover even once noted. */
  let guess = $state<api.StyleOpt[]>([]);
  $effect(() => {
    void app.version;
    const want = id;
    const n = ++seqGuess;
    api.styleGuess(want).then((g) => want === id && n === seqGuess && (guess = g), () => (guess = []));
  });
  // a song whose style Spool is unsure of (none written, its best guess under 60 %, not set
  // aside): the confirmation in one click, as on the styles page
  let skipped = $state<number[]>([]);
  $effect(() => {
    void app.version;
    api.styleSkipped().then((x) => (skipped = x), () => {});
  });
  const unsure = $derived(!!d && !d.song.style?.trim() && !!d.song.temoin && guess.length > 0 && guess[0].pct < 60 && !skipped.includes(id));
  const styleHint = $derived.by(() => {
    if (!guess.length) return t("song.hint.styleNone");
    return t("song.hint.head") + "\n" + guess.map((o) => `${t("song.hint.stylePct", { style: o.style, pct: o.pct })}${o.why.length ? "\n   " + o.why.join("\n   ") : ""}`).join("\n");
  });
  const keyHintText = $derived.by(() => {
    const l: string[] = [];
    if (main?.key) l.push(t("song.hint.liveKey", { key: main.key }));
    if (main?.key_raw) {
      const pct = Math.round((main.key_conf ?? 0) * 100);
      const sure = !!main.key_est;
      l.push(
        t(main.key_bass ? "song.hint.midiBass" : "song.hint.midi", { key: main.key_raw, pct }) +
          (sure ? "" : "\n   " + t("song.hint.unsure")),
      );
    }
    if (!l.length) return t("song.hint.keyNone");
    return t("song.hint.head") + "\n" + l.join("\n");
  });
  /** Which of its projects a repair would improve. */
  let plans = $state<Record<string, api.RepairPlan | null>>({});
  $effect(() => {
    const files = d?.files ?? [];
    Promise.all(files.map((f) => api.repairPlan(f.rel).then((p) => [f.rel, p] as const))).then((l) => (plans = Object.fromEntries(l)));
  });
  async function repairNow(rel: string) {
    try {
      await api.repairRun([rel]);
      app.say(t("song.repairStarted"));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // shown on the sheet at once, then saved (the sheet is read again after the save)
  const edit = (body: Record<string, unknown>) => {
    if (d) {
      const cur = d.song as unknown as Record<string, unknown>;
      const next = { ...cur };
      for (const [k, v] of Object.entries(body)) if (k in cur) next[k] = v;
      d = { ...d, song: next as unknown as typeof d.song };
    }
    return app.editSong(id, body);
  };
  // what the fields offer: the artists known (settings, folders, songs), the styles used,
  // every key
  const uniq = (xs: (string | null | undefined)[]) =>
    [...new Set(xs.map((x) => (x ?? "").trim()).filter(Boolean))].sort(compare);
  const artistOpts = $derived(uniq([...(app.config?.artists ?? []), ...(app.config?.roots ?? []).map((r) => r.artist), ...app.songs.map((x) => x.artist)]));
  // collaborators: the artists the app knows and everyone already written as "with" on a song,
  // the song's own artist left out
  const ftOpts = $derived(uniq([...artistOpts, ...app.songs.flatMap((x) => (x.ft ?? "").split(/\s*,\s*/))]).filter((n) => n !== s?.artist));
  const styleOpts = $derived(uniq(app.songs.flatMap((x) => (x.style ?? "").split(/\s*,\s*/))));
  const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
  const keyOpts = $derived(uniq([...NOTES.map((n) => n + "m"), ...NOTES, ...app.songs.map((x) => x.key)]));
  const kindOpts = $derived(uniq(["Original", "Single", "Remix", "Collab", "Remake", "Cover", "Edit", "VIP", "Live", ...app.songs.flatMap((x) => x.kinds)]));
  async function copy(rel: string) {
    try {
      app.say(t("song.pathCopied", { path: await api.copyPath(rel) }));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // a project grouped with others by its name made a song of its own (one Ctrl+Z puts it back)
  // asked first (it reshapes the catalogue), then opened in a tab of its own
  function ownSong(rel: string, name: string) {
    ui.confirm = {
      title: t("song.projects.ownConfirmTitle", { name }),
      text: t("song.projects.ownConfirmText", { title: s?.title ?? "" }),
      ok: t("song.projects.own"),
      run: async () => {
        try {
          const nid = await api.fileToSong(rel);
          await app.reload();
          app.say(t("song.projects.ownDone", { name }), { undo: true });
          openSong(nid);
          location.hash = `#/song/${nid}`;
        } catch (e) {
          app.say(String(e), { error: true });
        }
      },
    };
  }
  async function show(rel: string) {
    try {
      await api.reveal(rel);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const main = $derived(d?.files.find((f) => f.latest) ?? d?.files[0]);
  const temoin = $derived(
    d && s?.temoin ? [...d.temoins, ...d.other_audio].find((c) => c.rel === s.temoin || s.temoin!.replace(/\\/g, "/").endsWith("/" + c.rel)) : undefined,
  );
  // the song's own renders, and those of the shared "- MP3" folder found only by a close name
  // (offered apart, never taken by the app)
  const ownRenders = $derived(d ? d.temoins.filter((c) => c.where !== "MP3") : []);
  const byName = $derived(d ? d.temoins.filter((c) => c.where === "MP3") : []);
  // the sheet's picture shown at most 512 px: its thumbnail (the original only for a GIF)
  const cover = $derived(s?.cover ? (/\.gif$/i.test(s.cover) ? api.mediaUrl(s.cover) : api.thumbUrl(s.cover, 512)) : "");
  // the song's comments (pinned on its waveforms, its post-its): one list shared by the sheet
  const comments = $derived(commentsOf(id));
  // where the song lives: its folder, and the releases it is part of (it moves from one to
  // another in one go; each release saved whole, so Ctrl+Z puts each back)
  const inReleases = $derived(s ? app.releases.filter((r) => r.songs.includes(s.id)) : []);
  $effect(() => loadHints());
  const relSugg = $derived(s ? suggestReleases(s.id) : null);
  async function moveToRelease(target: string) {
    if (!s || !target) return;
    if (target === "hint" && relSugg?.hint) return releaseFromHint(relSugg.hint);
    const id = s.id;
    try {
      for (const r of app.releases.filter((x) => x.songs.includes(id) && x.id !== target)) {
        await api.releaseSave({ ...r, songs: r.songs.filter((x) => x !== id) });
      }
      const to = app.releases.find((x) => x.id === target);
      if (to && !to.songs.includes(id)) {
        const [, st] = await api.releaseSave({ ...to, songs: [...to.songs, id] });
        app.undo = st;
      }
      app.releases = await api.releases();
      app.say(t("song.place.moved", { title: to?.title ?? "" }));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // the Samples page in this song's key and at its tempo (as shown: written or detected)
  // the song's key and tempo as shown (written, else detected): what fits it
  const fitKey = $derived((s?.key || d?.detected?.key.shown?.value) ?? null);
  const fitBpm = $derived(s?.tempo || Number(d?.detected?.tempo.shown?.value) || 0);
  // the other songs Spool made of the same folder (projects of different names): with this
  // one, by date, the newest first. A small folder is one song for most people: gathered in
  // one click into the newest (merges, each undone by Ctrl+Z; the folder's name titles it)
  const sameFolder = $derived.by(() => {
    const me = app.songs.find((x) => x.id === id);
    if (!me?.dir) return [];
    const all = app.songs.filter((x) => x.dir === me.dir && !x.archived);
    return all.length > 1 && all.length <= 20 ? [...all].sort((a, b) => b.mtime - a.mtime) : [];
  });
  let gathering = $state(false);
  // the folder becomes a rule: read as one song (now and at every analysis), and the folders
  // beside it that look the same are proposed
  async function gather() {
    const [target] = sameFolder;
    const dir = app.songs.find((x) => x.id === id)?.dir;
    if (!target || !dir || gathering) return;
    gathering = true;
    try {
      await gatherFolder(dir);
      const into = app.songs.find((x) => x.dir === dir && !x.archived);
      if (into) {
        openSong(into.id);
        location.hash = `#/song/${into.id}`;
      }
    } catch (e) {
      app.say(String(e), { error: true });
    } finally {
      gathering = false;
    }
  }
  // the songs whose main project is this one's file copied (same name, size and date)
  const copies = $derived((app.songs.find((x) => x.id === id)?.copies ?? []).map((c) => app.songs.find((x) => x.id === c)).filter((x) => !!x));
  const doneChecks = $derived(d ? d.checklist.filter((c) => c.done).length : 0);

  const STATUS_KEYS = ["renamed", "other_vst2", "trial", "vst3", "newer", "to_vst3", "other_vst3", "missing"];
  const statusLabel = (st: string) => (STATUS_KEYS.includes(st) ? t(`song.status.${st}`) : st);
  const sevLabel = (n: number) => (n >= 0 && n <= 3 ? t(`song.sev.s${n}`) : undefined);

  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "numeric" });
  // when the song was started (its oldest project saved) and last worked on (its newest save)
  const when = $derived(s ? app.songs.find((x) => x.id === s.id) : undefined);

  const temoinRel = $derived(s?.temoin ?? temoin?.rel ?? null);

  // the stages as a line the song moves along; "on hold" stands apart
  const FLOW = STAGES.filter((x) => x !== "pause");
  const stageAt = $derived(s?.stage ? FLOW.indexOf(s.stage as (typeof FLOW)[number]) : -1);

  // the sheet's sections, in the order they show: each button opens or closes one (kept as the
  // user's layout for every song), and says what its section holds while it is closed
  const tr = $derived(sheetTransport.by[id] ?? null);
  // the bar kept at the top: what sticks below it (the sections' titles) starts under it
  let topH = $state(0);
  const latestFile = $derived(d ? [...d.files].sort((a, b) => Number(!!b.latest) - Number(!!a.latest) || b.mtime - a.mtime)[0] : undefined);
  const sections = $derived.by(() => {
    if (!d || !s) return [];
    const toFix = d.files.filter((f) => f.issues.some((i) => !i.ignored) || f.samples_lost.length || f.error).length;
    const lost = latestFile?.samples_lost.length ?? 0;
    const plugs = d.devices.instr.length + d.devices.fx.length + d.devices.midi.length;
    const boardN = d.board.items.length + d.board.found.length;
    const renders = d.temoins.length + d.other_audio.length;
    const out: { key: string; sum: string; warn?: boolean }[] = [];
    if (api.withArrangement) {
      out.push({ key: "session", sum: tr?.summary ?? "" });
      out.push({ key: "chords", sum: tr?.key ?? "" });
    }
    if (api.inApp || import.meta.env.DEV) out.push({ key: "unstuck", sum: "" });
    out.push({ key: "temoin", sum: renders ? String(renders) : "" });
    const pinned = comments.filter((c) => !!c.rel);
    out.push({ key: "comments", sum: pinned.length ? `${pinned.filter((c) => !c.done).length}/${pinned.length}` : "" });
    out.push({ key: "checks", sum: `${doneChecks}/${d.checklist.length}` });
    out.push({ key: "projects", sum: d.files.length ? (toFix ? t("song.sections.toFix", { n: toFix }) : t("song.sections.allHere")) : "", warn: toFix > 0 });
    out.push({ key: "resources", sum: lost ? t("song.sections.lost", { n: lost }) : "", warn: lost > 0 });
    out.push({ key: "devices", sum: plugs ? String(plugs) : "" });
    // the notes are in the mood board
    const nNotes = comments.filter((c) => !c.rel).length + (s.notes?.trim() ? 1 : 0);
    out.push({ key: "board", sum: boardN + nNotes ? String(boardN + nNotes) : "" });
    return inSheetOrder(out);
  });
  // ---- the sections in the order the user wants (right-click on the bar): moved one step, to
  // an end, or dragged along the bar; the sections below follow
  function moveSection(key: string, to: number) {
    const keys = sections.map((x) => x.key).filter((k) => k !== key);
    keys.splice(Math.max(0, Math.min(keys.length, to)), 0, key);
    setSheetOrder(keys);
  }
  let arranging = $state(false);
  function secMenu(e: MouseEvent, key?: string) {
    const keys = sections.map((x) => x.key);
    const i = key ? keys.indexOf(key) : -1;
    const moves = key
      ? [
          { label: t("song.sections.order.left"), disabled: i <= 0, run: () => moveSection(key, i - 1) },
          { label: t("song.sections.order.right"), disabled: i >= keys.length - 1, run: () => moveSection(key, i + 1) },
          { label: t("song.sections.order.first"), disabled: i <= 0, run: () => moveSection(key, 0) },
          { label: t("song.sections.order.last"), disabled: i >= keys.length - 1, run: () => moveSection(key, keys.length - 1) },
          { sep: true as const },
        ]
      : [];
    openMenu(e, [
      ...moves,
      { label: t("song.sections.order.arrange"), run: () => (arranging = true) },
      { label: t("song.sections.order.reset"), disabled: !sheetOrder.keys.length, run: () => setSheetOrder([]) },
    ], key ? t(`song.sections.label.${key}`) : t("song.sections.bar"));
  }
  // dragged along the bar while arranging: its place is where the pointer passes the middle
  // of the others
  let secbarEl = $state<HTMLElement>();
  let dragKey = $state<string | null>(null);
  function dragDown(e: PointerEvent, key: string) {
    if (!arranging || e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragKey = key;
  }
  function dragMove(e: PointerEvent) {
    if (!dragKey || !secbarEl) return;
    const others = [...secbarEl.querySelectorAll<HTMLElement>("[data-key]")].filter((b) => b.dataset.key !== dragKey);
    const to = others.filter((b) => { const r = b.getBoundingClientRect(); return r.left + r.width / 2 < e.clientX; }).length;
    if (sections.findIndex((x) => x.key === dragKey) !== to) moveSection(dragKey, to);
  }
  // the sections below, in the same order (none set: the sheet's own layout)
  function sectionsOrder(node: HTMLElement, keys: string[]) {
    let k = keys;
    const apply = () => {
      for (const el of node.children) {
        const key = (el as HTMLElement).dataset.sec;
        const i = key ? k.indexOf(key) : -1;
        (el as HTMLElement).style.order = i >= 0 ? String(i) : "";
      }
    };
    const mo = new MutationObserver(apply);
    mo.observe(node, { childList: true });
    apply();
    return {
      update(n: string[]) {
        k = n;
        apply();
      },
      destroy: () => mo.disconnect(),
    };
  }
  async function toggleSection(key: string) {
    const on = !sectionOpen(id, key);
    setSectionOpen(id, key, on);
    if (!on) return;
    await tick();
    showSection(key);
  }
  // a section just opened (or asked for) comes up under the bar kept in sight, unless it is
  // already in view
  function showSection(key: string) {
    const el = sheet?.querySelector<HTMLElement>(`[data-sec="${key}"]`);
    const bar = sheet?.querySelector<HTMLElement>(".top");
    if (!el || !bar) return;
    const top = bar.getBoundingClientRect().bottom;
    const r = el.getBoundingClientRect();
    if (r.top >= top && r.top < innerHeight - 120) return;
    el.style.scrollMarginTop = `${bar.offsetHeight + 12}px`;
    el.scrollIntoView({ block: "start", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  }
  // 1 to 9 and 0: the sections in their order (not while typing)
  function sectionKey(e: KeyboardEvent) {
    if (arranging && e.key === "Escape") return void (arranging = false);
    if (e.ctrlKey || e.metaKey || e.altKey || !/^[0-9]$/.test(e.key) || !sheet?.isConnected || !sheet.offsetParent) return;
    const el = e.target as HTMLElement | null;
    if (el?.closest("input, textarea, select, [contenteditable]")) return;
    const sec = sections[e.key === "0" ? 9 : Number(e.key) - 1];
    if (!sec) return;
    e.preventDefault();
    toggleSection(sec.key);
  }
</script>

<svelte:window onkeydown={sectionKey} />

{#if missing}
  <div class="page"><p class="muted">{t("song.gone")} <a href="#/songs">{t("song.backToSongs")}</a></p></div>
{:else if d && s && loadedId === id}
  <!-- data-song: a right-click anywhere on the sheet opens the song's menu -->
  <div class="page" data-song={s.id} bind:this={sheet} style:--sheet-top="{topH}px">
    {#if dropping}<div class="dropzone" aria-hidden="true"><div>{t("song.board.dropHere")}<span class="small muted">{t("song.board.dropHow")}</span></div></div>{/if}
    {#if s.archived}
      <div class="archived small">{t("song.archived")}
        <button class="btn sm" onclick={() => edit({ archived: false })}>{t("song.unarchive")}</button></div>
    {/if}
    {#if !s.archived && sameFolder.length}
      <div class="samef small">
        <div class="grow">
          <b>{t("song.folder.title", { n: sameFolder.length, folder: (app.songs.find((x) => x.id === id)?.dir ?? "").split("/").pop() ?? "" })}</b>
          <div class="sfl">
            {#each sameFolder as o, i (o.id)}
              <span class:me={o.id === id}>{#if o.id === id}<b>{o.title}</b>{:else}<a href="#/song/{o.id}">{o.title}</a>{/if}
                <span class="faint">{ago(o.mtime)}</span>{#if i === 0}<span class="newest">{t("song.folder.newest")}</span>{/if}</span>
            {/each}
          </div>
        </div>
        <button class="btn sm" disabled={gathering} title={t("song.folder.gatherTip", { title: sameFolder[0].title ?? "" })} onclick={gather}>{t("song.folder.gather")}</button>
      </div>
    {/if}
    {#if gathered.dirs.length && app.songs.find((x) => x.id === id)?.dir === gathered.dir}
      <!-- a folder just made one song: the folders beside it that hold several songs too -->
      <div class="samef small">
        <div class="grow">
          <b>{t("song.folder.neighbours", { n: gathered.dirs.length })}</b>
          <div class="sfl">{#each gathered.dirs.slice(0, 8) as d (d)}<span>{d.split("/").pop()}</span>{/each}{#if gathered.dirs.length > 8}<span class="faint">+{gathered.dirs.length - 8}</span>{/if}</div>
        </div>
        <button class="btn sm" onclick={gatherNeighbours}>{t("song.folder.neighboursDo")}</button>
        <button class="btn sm" onclick={() => (gathered.dirs = [])}>{t("song.folder.neighboursNo")}</button>
        <a class="small" href="#/reading">{t("song.folder.rules")}</a>
      </div>
    {/if}
    {#if s.archived}
    {:else if copies.length}
      <!-- the same project file copied in other folders: a duplicated folder, most likely -->
      <div class="copies small">
        <span>{t("song.copies.text", { n: copies.length })}
          {#each copies as c, i (c.id)}{i ? ", " : " "}<a href="#/song/{c.id}" title={c.dir}>{c.title || t("songs.song")} <span class="faint">({c.dir.split("/").pop()})</span></a>{/each}.
          <span class="faint">{t("song.copies.why")}</span></span>
        <button class="btn sm" title={t("song.copies.archiveTip")} onclick={() => edit({ archived: true })}>{t("song.copies.archive")}</button>
      </div>
    {/if}
    <header class="hero">
      <div class="art" style="--h:{hue(s.title ?? '')}">
        {#if cover}<img src={cover} alt="" decoding="async" onerror={(e) => { const i = e.currentTarget as HTMLImageElement; if (s?.cover && !i.dataset.full) { i.dataset.full = "1"; i.src = api.mediaUrl(s.cover); } }} />{:else}{(s.title || "?").slice(0, 1).toUpperCase()}{/if}
      </div>
      <div class="id">
        {#if !embedded}<a class="back small" href="#/songs">{t("song.back")}</a>{/if}
        <Field big value={s.title} placeholder={t("song.ph.title")} save={(v) => edit({ title: v })} />
        <div class="meta">
          <Field value={s.artist} placeholder={t("song.ph.artist")} options={artistOpts} save={(v) => edit({ artist: v })} />
          <span class="dot">·</span>
          <Field value={s.style} placeholder={t("song.ph.style")} options={styleOpts} hint={styleHint} detected={d.detected?.style} save={(v) => edit({ style: v })} />
          <span class="dot">·</span>
          <Field value={s.key} placeholder={t("song.ph.key")} options={keyOpts} hint={keyHintText} detected={d.detected?.key} typed={(s.user_fields ?? []).includes("key")} save={(v) => edit({ key: v })} />
          <span class="dot">·</span>
          <Field kind="tempo" value={s.tempo} placeholder={t("song.ph.tempo")} detected={d.detected?.tempo} save={(v) => edit({ tempo: v })} />
          <span class="dot">·</span>
          <Field kind="date" value={s.date} placeholder={t("song.ph.date")} save={(v) => edit({ date: v })} />
        </div>
        {#if unsure}
          <StyleConfirm compact sid={id} options={guess.slice(0, 3).map((o) => o.style)} done={() => (guess = [])} />
        {/if}
        <div class="meta">
          <Stars value={s.rating} set={(n) => edit({ rating: n })} />
          <span class="dot">·</span>
          <Field value={(s.kinds ?? []).join(", ")} placeholder={t("song.ph.kind")} options={kindOpts} save={(v) => edit({ kinds: v.split(",").map((x) => x.trim()).filter(Boolean) })} />
          <span class="dot">·</span>
          <Field multi value={s.ft} placeholder={t("song.ph.ft")} options={ftOpts} save={(v) => edit({ ft: v })} />
        </div>
        <div class="place small">
          {#if s.song_dir}
            <button class="link folder" title={t("song.place.folderTip")} onclick={() => api.reveal(s.song_dir!).catch((e) => app.say(String(e), { error: true }))}>
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg>
              <span>{s.song_dir}</span></button>
          {/if}
          {#if when && when.mtime > 0}
            <span class="dates" title={t("song.place.datesTip", { created: day(when.created), modified: day(when.mtime) })}>
              {#if Number.isFinite(when.created) && when.created > 0 && day(when.created) !== day(when.mtime)}{t("song.place.created", { date: day(when.created) })}<span class="faint">{" · "}</span>{/if}{t("song.place.modified", { date: day(when.mtime), ago: ago(when.mtime) })}
            </span>
          {/if}
          <span class="rel">
            {#each inReleases as r (r.id)}<a class="relchip" href="#/release/{r.id}">{r.title}</a>{:else}<span class="faint">{t("song.place.noRelease")}</span>{/each}
            {#if relSugg && (relSugg.sugg.length || relSugg.rest.length || relSugg.hint)}
              <select aria-label={t("song.place.moveTo")} class:hasSugg={relSugg.sugg.length || relSugg.hint} onchange={(e) => { moveToRelease(e.currentTarget.value); e.currentTarget.value = ""; }}>
                <option value="">{inReleases.length ? t("song.place.moveTo") : t("song.place.addTo")}</option>
                {#if relSugg.sugg.length || relSugg.hint}
                  <optgroup label={t("song.place.suggested")}>
                    {#each relSugg.sugg as r (r.id)}<option value={r.id}>{r.title}</option>{/each}
                    {#if relSugg.hint}<option value="hint">{t("song.place.fromFolder", { title: relSugg.hint.title, n: relSugg.hint.songs.length })}</option>{/if}
                  </optgroup>
                  {#if relSugg.rest.length}
                    <optgroup label={t("song.place.others")}>
                      {#each relSugg.rest as r (r.id)}<option value={r.id}>{r.title}</option>{/each}
                    </optgroup>
                  {/if}
                {:else}
                  {#each relSugg.rest as r (r.id)}<option value={r.id}>{r.title}</option>{/each}
                {/if}
              </select>
            {/if}
          </span>
        </div>
      </div>
      <div class="acts">
        <FavStar kind="song" id={String(s.id)} label={s.title ?? ""} size={19} />
        {#if main}<LiveButton rel={main.rel} label size="md" />{/if}
        <!-- the rest (folder, paths, release, archive) in the song's menu: here, or a right-click anywhere on the sheet -->
        <button class="btn more" title={t("song.moreTip")} aria-label={t("song.more")} onclick={(e) => openSongMenu(e, s.id)}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg></button>
      </div>
      <!-- the stages: a line the song moves along, the one Spool guesses dotted -->
      <div class="flow">
        <div class="stages" role="radiogroup" aria-label={t("song.stage")}>
          {#each FLOW as st, i (st)}
            {@const g = d.suggestions.find((x) => x.value === st)}
            <button role="radio" aria-checked={s.stage === st} class="st s-{st}" class:on={s.stage === st} class:past={stageAt > i} class:guess={!!g}
              title={g ? t("song.probableStage", { stage: stageLabel(st) }) + "\n" + g.reasons.join("\n") : undefined}
              onclick={() => edit({ stage: s.stage === st ? null : st })}><i></i>{stageLabel(st)}</button>
          {/each}
          <button role="radio" aria-checked={s.stage === "pause"} class="st hold s-pause" class:on={s.stage === "pause"}
            onclick={() => edit({ stage: s.stage === "pause" ? null : "pause" })}><i></i>{stageLabel("pause")}</button>
        </div>
        {#each d.suggestions as g (g.id)}
          <div class="guessline small">
            <span class="why" title={g.reasons.join("\n")}>{t("song.probableStage", { stage: stageLabel(g.value) })}<span class="faint"> · {g.reasons.join(" · ")}</span></span>
            <button class="btn sm" onclick={() => app.act(api.suggestionAct(g.id, true))}>{t("song.accept")}</button>
            <button class="btn sm" onclick={() => app.act(api.suggestionAct(g.id, false))}>{t("song.decline")}</button>
          </div>
        {/each}
      </div>
    </header>

    <!-- kept in sight while the sheet scrolls: the render (or the project), and the sections -->
    <div class="top" use:outerHeightOf={(x) => (topH = x)}>
      <SheetTransport sid={id} title={s.title ?? ""} temoin={temoinRel}
        chosenBy={s.temoin_source === "user" ? t("song.temoin.byYou") : temoin ? t("song.temoin.byApp") : ""}
        openRenders={() => (sectionOpen(id, "temoin") ? showSection("temoin") : toggleSection("temoin"))} />
      <nav class="secbar" class:arranging bind:this={secbarEl} aria-label={t("song.sections.bar")} oncontextmenu={(e) => secMenu(e)} onwheel={(e) => { const el = e.currentTarget; if (el.scrollWidth > el.clientWidth && Math.abs(e.deltaY) > Math.abs(e.deltaX)) { el.scrollLeft += e.deltaY; e.preventDefault(); } }}>
        {#each sections as x, i (x.key)}
          {@const on = sectionOpen(id, x.key)}
          <button class="sec" class:on class:drag={dragKey === x.key} data-key={x.key} aria-pressed={on}
            title={arranging ? t("song.sections.order.dragTip") : t("song.sections.tip", { n: i === 9 ? 0 : i + 1 })}
            onclick={() => { if (!arranging) toggleSection(x.key); }} oncontextmenu={(e) => secMenu(e, x.key)}
            onpointerdown={(e) => dragDown(e, x.key)} onpointermove={dragMove} onpointerup={() => (dragKey = null)} onpointercancel={() => (dragKey = null)}>
            {t(`song.sections.label.${x.key}`)}{#if x.sum}<span class="sum" class:warn={x.warn}>{x.sum}</span>{/if}
          </button>
        {/each}
        {#if arranging}<button class="sec done" onclick={() => (arranging = false)}>{t("song.sections.order.done")}</button>{/if}
      </nav>
    </div>

    <div class="panels" use:masonry={380} use:sectionsOrder={sheetOrder.keys.length ? sections.map((x) => x.key) : []}>
      <MiniSession sid={id} temoin={s.temoin ?? temoin?.rel ?? null} title={s.title ?? ""}
        files={[...d.files.map((f) => ({ rel: f.rel, name: f.name, mtime: f.mtime, latest: f.latest })), ...d.versions.map((v) => ({ rel: v.rel, name: v.name, mtime: v.mtime, old: true }))]} />

      <Unstuck sid={id} title={s.title ?? ""} songKey={fitKey} bpm={fitBpm} temoin={s.temoin ?? temoin?.rel ?? null} />

      {#if sectionOpen(id, "temoin")}
        <section class="card wide" data-sec="temoin">
          <FoldHead sid={id} key="temoin" title={t("song.sections.label.temoin")}>
            <span class="small faint">{s.temoin_source === "user" ? t("song.temoin.byYou") : temoin ? t("song.temoin.byApp") : ""}</span></FoldHead>
          {#if temoin}
            {@const rr = s.temoin ?? temoin.rel}
            <Compare sid={id} title={s.title ?? ""} a={rr} versions={[...d.temoins, ...d.other_audio].map((c) => ({ rel: c.rel, mtime: c.mtime }))} />
          {:else}
            <p class="small faint">{t("song.temoin.none")}</p>
          {/if}
          <RefList kind="song" key={String(id)} against={s.temoin ?? temoin?.rel ?? null} againstLabel={t("refs.yourRender")} sid={id} title={s.title ?? ""} />
          {#if ownRenders.length > 1}
            <!-- every render of the song in sight (a WAV and its MP3, older bounces): open
                 unless there are many -->
            <details class="more" open={ownRenders.length <= 8}>
              <summary class="small muted">{t("song.temoin.renders", { n: ownRenders.length })}</summary>
              {#each ownRenders as c (c.rel)}
                <div class="line">
                  <div class="grow wrapnm" title={c.rel}>{c.rel.split("/").pop()}
                    <span class="small faint">{c.kind} {ago(c.mtime)}</span></div>
                  <button class="icon" title={t("song.temoin.listen")} aria-label={t("song.temoin.listen")} onclick={() => playOrPause(c.rel, () => player.play(c.rel, id, s.title ?? ""))}>
                    <PlayIcon on={playingFile(c.rel)} /></button>
                  {#if c === temoin}<span class="small tag">{t("song.temoin.tag")}</span>
                  {:else if c.pickable}<button class="btn sm" onclick={() => edit({ temoin: c.rel })}>{t("song.temoin.choose")}</button>{/if}
                  <button class="btn sm" title={t("refs.markTip")} onclick={() => api.refMark(c.rel, "song", String(id), true).then(() => app.say(t("refs.marked", { name: c.rel.split("/").pop() ?? c.rel }))).catch((e) => app.say(String(e), { error: true }))}>{t("refs.mark")}</button>
                </div>
              {/each}
            </details>
          {/if}
          {#if byName.length}
            <!-- renders of the shared "- MP3" folder whose NAME is close to the song's: only
                 offered (another song can bear a close name), linked by a click -->
            <details class="more" open={!temoin || byName.includes(temoin)}>
              <summary class="small muted">{t("song.temoin.byName", { n: byName.length })}</summary>
              <p class="small faint">{t("song.temoin.byNameHint")}</p>
              {#each byName as c (c.rel)}
                <div class="line">
                  <div class="grow wrapnm" title={c.rel}>{c.rel.split("/").pop()}
                    <span class="small faint">{c.rel.split("/").slice(0, -1).join("/")} {ago(c.mtime)}</span></div>
                  <button class="icon" title={t("song.temoin.listen")} aria-label={t("song.temoin.listen")} onclick={() => playOrPause(c.rel, () => player.play(c.rel, id, s.title ?? ""))}>
                    <PlayIcon on={playingFile(c.rel)} /></button>
                  {#if c === temoin}<span class="small tag">{t("song.temoin.tag")}</span>
                  {:else}<button class="btn sm" onclick={() => edit({ temoin: c.rel })}>{t("song.temoin.choose")}</button>{/if}
                </div>
              {/each}
            </details>
          {/if}
          {#if s.temoin_source === "user"}<button class="link small" onclick={() => edit({ temoin: null })}>{t("song.temoin.letApp")}</button>{/if}
          {#if d.other_audio.length}
            <details class="more" open={d.other_audio.length <= 8}>
              <summary class="small muted">{t("song.temoin.otherAudio", { n: d.other_audio.length })}</summary>
              <p class="small faint">{t("song.temoin.otherAudioHint")}</p>
              {#each d.other_audio as c (c.rel)}
                <div class="line" data-media={c.rel} data-kind="audio">
                  <div class="grow wrapnm" title={c.rel}>{c.rel.split("/").pop()} <span class="small faint">{ago(c.mtime)}</span></div>
                  <button class="icon" title={t("song.temoin.listen")} aria-label={t("song.temoin.listen")} onclick={() => playOrPause(c.rel, () => player.play(c.rel, id, s.title ?? ""))}>
                    <PlayIcon on={playingFile(c.rel)} /></button>
                  {#if c === temoin}<span class="small tag">{t("song.temoin.tag")}</span>
                  {:else}<button class="btn sm" onclick={() => edit({ temoin: c.rel })}>{t("song.temoin.choose")}</button>{/if}
                  <button class="btn sm" title={t("refs.markTip")} onclick={() => api.refMark(c.rel, "song", String(id), true).then(() => app.say(t("refs.marked", { name: c.rel.split("/").pop() ?? c.rel }))).catch((e) => app.say(String(e), { error: true }))}>{t("refs.mark")}</button>
                </div>
              {/each}
            </details>
          {/if}
          {#if d.sound?.heard.length}
            <p class="small heard" title={t("song.sound.heardTip")}>
              <span class="faint">{t("song.sound.heard")}</span>
              {#each d.sound.heard as [name, act], i (name)}{i ? ", " : " "}<b>{name}</b> <span class="faint">{Math.round(act * 100)} %</span>{/each}
            </p>
          {/if}
          {#if d.sound?.similar.length}
            <div class="sounds">
              <h3 class="small">{t("song.sound.title")}</h3>
              <ul>
                {#each d.sound.similar as o (o.id)}
                  <li><a href="#/song/{o.id}">{o.title || t("song.sound.untitled")}</a>{#if o.artist}<span class="faint"> · {o.artist}</span>{/if}{#if o.style}<span class="faint"> · {o.style}</span>{/if}<span class="small sim" title={t("song.sound.simTitle", { pct: Math.round(o.sim * 100) })}>{simWord(o.sim)}</span></li>
                {/each}
              </ul>
            </div>
          {/if}
          {#if d.near_temoin.length}
            <div class="small faint near">{t("song.temoin.near")}
              {#each d.near_temoin as n, i}{i ? ", " : " "}<span title={n.rel}>{n.name} ({[t("song.temoin.nearDays", { n: n.days, d: (n.days > 0 ? "+" : "") + n.days }), n.old && t("song.temoin.oldVersion"), !n.same && n.title].filter(Boolean).join(", ")})</span>{/each}
            </div>
          {/if}
        </section>
      {/if}
      <!-- the projects first, two columns wide where there are several: what matters most -->
      {#if sectionOpen(id, "comments")}<Comments sid={id} title={s.title ?? ""} rel={temoinRel} />{/if}
      {#if sectionOpen(id, "projects")}
        <section class="card" data-sec="projects">
          <FoldHead sid={id} key="projects" title={t("song.projects.title")}><span class="small faint">{d.hidden ? t("song.projects.oldVersions", { n: d.hidden }) : ""}{d.backups ? ` · ${t("song.projects.backups", { n: d.backups })}` : ""}</span></FoldHead>
          {#each d.files as f (f.rel)}
            <div class="file">
              <div class="line">
                <span class="sev s{f.sev}" title={sevLabel(f.sev)}></span>
                <div class="grow">
                  <div class="ell" title={f.rel}><b>{f.name}</b>{#if f.latest} <span class="tag small">{t("song.projects.latest")}</span>{/if}</div>
                  <div class="small faint ell">{[sevLabel(f.sev), f.live && "Live " + f.live, f.tempo && bpm(f.tempo), dur(f.len), ago(f.mtime)].filter(Boolean).join(" · ")}</div>
                </div>
                {#if plans[f.rel]?.useful}<button class="btn sm" title={plans[f.rel]?.complete ? t("song.projects.repairComplete") : t("song.projects.repairPartial")} onclick={() => repairNow(f.rel)}>{t("song.projects.repair")}</button>{/if}
                <FavStar kind="project" id={f.rel} label={f.name} sub={s.title ?? ""} song={s.id} size={14} />
                <button class="icon" title={t("song.projects.copyFullPath")} aria-label={t("song.projects.copyPath")} onclick={() => copy(f.rel)}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V5a1 1 0 0 1 1-1h10" /></svg>
                </button>
                <button class="icon" title={t("song.projects.reveal")} aria-label={t("song.projects.reveal")} onclick={() => show(f.rel)}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg>
                </button>
                <LiveButton rel={f.rel} />
                {#if d.files.length + d.versions.length > 1}
                  <button class="icon" title={t("song.projects.ownTip")} aria-label={t("song.projects.own")} onclick={() => ownSong(f.rel, f.name)}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M8 21H5a2 2 0 0 1-2-2v-3M16 21h3a2 2 0 0 0 2-2v-3M12 8v8M8 12h8" /></svg>
                  </button>
                {/if}
              </div>
              {#if f.error}<div class="small bad">{t("song.projects.unreadable", { error: f.error })}</div>{/if}
              {#if f.issues.length || f.samples_lost.length}
                <div class="issues small">
                  {#each f.issues as i}
                    {@const c = (i.candidates as { name: string }[])[0]}
                    <button class="iss v{i.ignored ? 0 : statusSeverity(i.status)}" class:ign={i.ignored}
                      title={[psay(i.status), c && t("song.dev.here", { name: c.name }), i.need_label, t("song.dev.clickHint")].filter(Boolean).join("\n")}
                      onclick={() => { focus.plugin = `${i.format}|${i.name}|${i.status}`; location.hash = "#/repair"; }}>{i.name}{i.n > 1 ? ` ×${i.n}` : ""} <span class="faint">{i.ignored ? t("song.statusIgnored", { status: statusLabel(i.status) }) : statusLabel(i.status)}</span></button>
                  {/each}
                  {#if f.samples_lost.length}<span class="iss">{t("song.projects.samplesLost", { n: f.samples_lost.length })}</span>{/if}
                </div>
              {:else if !f.error}
                <div class="small allhere">✓ {t("song.projects.allHere")}</div>
              {/if}
            </div>
          {/each}
          {#if d.versions.length}
            <details class="more">
              <summary class="small muted">{t("song.projects.oldVersions", { n: d.versions.length })}</summary>
              {#each d.versions as v (v.rel)}<div class="line small"><span class="grow ell" title={v.rel}>{v.name}</span><span class="faint">{ago(v.mtime)}</span>
                <LiveButton rel={v.rel} /> <button class="link own" title={t("song.projects.ownTip")} onclick={() => ownSong(v.rel, v.name)}>{t("song.projects.own")}</button></div>{/each}
            </details>
          {/if}
          <AlsDiffView files={[...d.files, ...d.versions].map((f) => ({ rel: f.rel, name: f.name, mtime: f.mtime }))} />
          <Merges sid={id} {d} />
        </section>
      {/if}
      {#if sectionOpen(id, "checks")}<Checks sid={id} list={d.checklist} />{/if}
      {#if sectionOpen(id, "resources")}
        <section class="card" data-sec="resources">
          <FoldHead sid={id} key="resources" title={t("song.resources.title")} />
          <Resources sid={id} rel={[...d.files].sort((a, b) => Number(!!b.latest) - Number(!!a.latest) || b.mtime - a.mtime)[0]?.rel ?? ""} />
        </section>
      {/if}
      {#if sectionOpen(id, "devices")}
        <section class="card" data-sec="devices">
          <FoldHead sid={id} key="devices" title={t("song.devices.title")} />
          {#each [[t("song.devices.instr"), d.devices.instr], [t("song.devices.fx"), d.devices.fx], [t("song.devices.midi"), d.devices.midi]] as const as [label, list]}
            {#if list.length}
              <h3 class="small muted">{label}</h3>
              <div class="devs">{#each list.slice(0, 14) as x}
                {#if x.native}<span class="dev" title={devTip(x)}>{x.name}{x.count > 1 ? ` ×${x.count}` : ""}</span>
                {:else}<button class="dev plug v{devSev(x)}" class:ign={x.ignored} title={devTip(x)} onclick={() => openPlugin(x)}>{x.name}{x.count > 1 ? ` ×${x.count}` : ""}</button>{/if}
              {/each}</div>
            {/if}
          {/each}
          {#if d.main_chain.length}<p class="small faint">{t("song.devices.chain", { chain: d.main_chain.join(" → ") })}</p>{/if}
        </section>
      {/if}
      <!-- the notes (post-its) live in the mood board -->
      <MoodBoard sid={id} board={d.board} title={s.title ?? ""}>
        <Notes sid={id} title={s.title ?? ""} temoin={temoinRel} legacy={s.notes?.trim() || null}>
          {#snippet log()}
            {#if d.log.length}
              <div class="small faint log">{#each d.log as l, i}{i ? " → " : ""}{stageLabel(l.stage)} <span title={day(l.at)}>({ago(l.at)})</span>{/each}</div>
            {/if}
          {/snippet}
        </Notes>
      </MoodBoard>
    </div>
  </div>
{:else}
  <Skeleton kind="sheet" />
{/if}

<style>
  .page { padding: 20px 28px 60px; container-type: inline-size; position: relative; }
  /* where the song lives: its folder, its releases */
  .place { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 14px; margin: 2px 0 8px; color: var(--muted); }
  .place .folder { display: inline-flex; align-items: center; gap: 6px; max-width: 100%; color: var(--muted); border: 0; background: none; padding: 0; cursor: pointer; font: inherit; }
  .place .folder span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 46ch; }
  .place .folder:hover { color: var(--ink); text-decoration: underline; }
  .place .dates { white-space: nowrap; }
  .place .rel { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .relchip { padding: 1px 8px; border-radius: 6px; background: var(--accent-bg); color: var(--ink); text-decoration: none; }
  .relchip:hover { text-decoration: underline; }
  .place select { border: 1px solid var(--line); background: var(--panel); color: var(--muted); border-radius: 7px; padding: 2px 6px; font: inherit; max-width: 220px; }
  .place select.hasSugg { border-color: color-mix(in srgb, var(--accent) 55%, var(--line)); color: var(--ink); }
  /* files held over the sheet: the whole sheet takes them */
  .dropzone { position: absolute; inset: 8px; z-index: 40; border: 2px dashed var(--ink); border-radius: var(--radius); background: color-mix(in srgb, var(--bg) 82%, transparent);
    display: grid; place-items: center; pointer-events: none; }
  .dropzone div { display: flex; flex-direction: column; align-items: center; gap: 6px; font-weight: 600; text-align: center; max-width: 46ch; position: sticky; top: 40%; }
  .hero { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 14px 20px; align-items: start; margin-bottom: 12px; }
  @container (max-width: 760px) { .hero { grid-template-columns: auto minmax(0, 1fr); } .hero .acts { grid-column: 1 / -1; } .art { width: 72px; height: 72px; font-size: 28px; } }
  .art { width: 96px; height: 96px; border-radius: 14px; overflow: hidden; display: grid; place-items: center; font-size: 40px; font-weight: 650; color: #fff;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .id { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .back { color: var(--muted); width: fit-content; }
  .back:hover { color: var(--ink); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 12px; color: var(--muted); }
  .dot { color: var(--faint); }
  /* the stages: one line the song moves along; the stage it is at, those behind it, and the
     one Spool guesses (dotted); "on hold" apart at the end */
  .flow { grid-column: 1 / -1; }
  .stages { display: flex; gap: 4px; align-items: stretch; }
  .st { flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; padding: 7px 2px 0; border: 0; border-top: 3px solid var(--line); background: none;
    color: var(--faint); font: inherit; font-size: 12px; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: color .15s, border-color .15s; }
  .st i { width: 7px; height: 7px; border-radius: 50%; flex: none; background: var(--s-none); }
  .st:hover { color: var(--ink); }
  .st.past { color: var(--muted); border-top-color: color-mix(in srgb, var(--ink) 35%, var(--line)); }
  .st.on { color: var(--ink); font-weight: 650; border-top-color: var(--c, var(--ink)); }
  .st.guess:not(.on) { border-top-style: dashed; border-top-color: var(--c, var(--accent)); color: var(--muted); }
  .st.hold { flex: 0 0 auto; margin-left: 10px; padding-left: 8px; padding-right: 4px; }
  .s-draft { --c: var(--s-draft); } .s-arrangement { --c: var(--s-arrangement); } .s-mix { --c: var(--s-mix); }
  .s-master { --c: var(--s-master); } .s-done { --c: var(--s-done); } .s-pause { --c: var(--s-pause); }
  .st i { background: var(--c, var(--s-none)); }
  .st:focus-visible, .sec:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .guessline { display: flex; align-items: center; gap: 8px; margin-top: 8px; color: var(--muted); min-width: 0; }
  .guessline .why { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .guessline .btn { flex: none; }

  /* the render and the sections, in sight while the sheet scrolls */
  .top { position: sticky; top: 0; z-index: 6; display: flex; flex-direction: column; gap: 7px; margin: 0 -28px 14px; padding: 7px 28px 8px;
    background: color-mix(in srgb, var(--bg) 94%, transparent); backdrop-filter: blur(6px); border-bottom: 1px solid var(--line); }
  /* one row: it scrolls sideways when the sheet is narrow (the wheel too) */
  .secbar { display: flex; gap: 6px; overflow-x: auto; scrollbar-width: none; margin: 0 -4px; padding: 0 4px; }
  .secbar::-webkit-scrollbar { display: none; }
  .secbar { mask-image: linear-gradient(to right, #000 calc(100% - 28px), transparent); padding-right: 28px; }
  .sec { flex: none; white-space: nowrap; }
  .sec { display: inline-flex; align-items: baseline; gap: 6px; padding: 4px 11px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel);
    color: var(--muted); font: inherit; font-size: 12.5px; cursor: pointer; transition: border-color .15s, color .15s, background-color .15s; }
  .sec:hover { color: var(--ink); border-color: var(--faint); }
  /* arranging: the buttons are dragged along the bar */
  .secbar.arranging .sec:not(.done) { cursor: grab; border-style: dashed; touch-action: none; }
  .secbar.arranging .sec.drag { cursor: grabbing; border-color: var(--accent); border-style: solid; }
  .sec.done { border-color: var(--accent); color: var(--accent); font-weight: 600; }
  .sec.on { background: var(--ink); border-color: var(--ink); color: var(--bg); font-weight: 600; }
  .sec .sum { font-size: 11px; font-weight: 500; color: var(--faint); max-width: 18ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sec.on .sum { color: color-mix(in srgb, var(--bg) 70%, var(--ink)); }
  .sec .sum.warn { color: var(--lost); }
  .sec.on .sum.warn { color: color-mix(in srgb, var(--lost) 60%, var(--bg)); }

  /* the open sections: the big ones across the sheet, the others side by side */
  /* balanced at every width: rows of 2 px, each card spanning as many as its height (see
     masonry), so a card goes where the columns are shortest and no hole stays under a short
     one. The wide ones first, across the sheet; then "unblock" and the projects over two
     columns, the mood board in one beside them, the small ones filling the rest */
  .panels { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 380px), 1fr)); grid-auto-rows: 2px; column-gap: 14px; row-gap: 0; align-items: start; grid-auto-flow: row dense; }
  .panels > :global(.wide) { grid-column: 1 / -1; order: -1; }
  .panels > :global([data-sec="unstuck"]) { order: 1; }
  .panels > :global([data-sec="board"]) { order: 2; }
  .panels > :global([data-sec="projects"]) { order: 3; }
  .panels > :global([data-sec="checks"]) { order: 4; }
  .panels > :global([data-sec="resources"]) { order: 5; }
  .panels > :global([data-sec="devices"]) { order: 6; }
  /* as soon as there are two columns (masonry says how many) */
  /* (global: Svelte drops a selector on an attribute the markup does not write) */
  :global(.panels[data-cols]:not([data-cols="1"]) > [data-sec="projects"]), :global(.panels[data-cols]:not([data-cols="1"]) > [data-sec="unstuck"]) { grid-column: span 2; }
  .panels > :global(.card) { min-width: 0; margin: 0; }
  @media (prefers-reduced-motion: reduce) { .st, .sec { transition: none; } }
  .acts { display: flex; gap: 8px; }
  .chip { font-size: 12px; padding: 3px 10px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; }
  .chip:hover { border-color: var(--accent); }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; }
  .line { display: flex; align-items: center; gap: 10px; padding: 5px 0; min-width: 0; }
  .grow { flex: 1; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* a render's whole name, on two lines if it must: its buttons keep their place */
  .wrapnm { overflow-wrap: anywhere; line-height: 1.35; }
  .line > :global(button), .line > .tag { flex: none; }
  .tag { padding: 0 7px; border-radius: 99px; background: var(--accent-bg); color: var(--accent); font-size: 11px; font-weight: 600; }
  .more { margin-top: 8px; }
  .more summary { cursor: pointer; padding: 3px 0; }
  .near { margin-top: 8px; }
  .sounds { margin-top: 12px; }
  .sounds h3 { margin: 0 0 4px; font-weight: 600; color: var(--muted); }
  .sounds ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 3px; }
  .sounds li { display: flex; align-items: baseline; gap: 2px; min-width: 0; }
  .sounds a { color: var(--ink); }
  .sounds .sim { margin-left: auto; padding-left: 10px; color: var(--muted); flex: none; }
  .measured { color: var(--muted); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .file { padding: 4px 0; border-bottom: 1px solid var(--line); }
  .file:last-of-type { border-bottom: 0; }
  .allhere { color: var(--ok); margin-top: 2px; }
  .samef { display: flex; gap: 12px; align-items: center; padding: 8px 12px; margin-bottom: 12px; border-radius: 9px; border: 1px solid var(--line); background: var(--panel2); }
  .samef .grow { flex: 1; min-width: 0; }
  .sfl { display: flex; flex-wrap: wrap; gap: 4px 14px; margin-top: 3px; }
  .sfl a { color: var(--ink); text-decoration: underline; text-underline-offset: 2px; }
  .newest { margin-left: 6px; padding: 0 6px; border-radius: 99px; background: var(--accent-bg); color: var(--ink); font-size: 11px; }
  .copies { display: flex; gap: 12px; align-items: center; justify-content: space-between; padding: 8px 12px; margin-bottom: 12px; border-radius: 9px; border: 1px solid var(--line); background: color-mix(in srgb, var(--fix) 10%, transparent); }
  .copies a { color: var(--ink); text-decoration: underline; text-underline-offset: 2px; }
  .sev { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--ok); }
  .sev.s1 { background: var(--fix); } .sev.s2 { background: var(--lost); } .sev.s3 { background: var(--bad); }
  .icon { width: 26px; height: 26px; flex: none; border-radius: 7px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .icon:hover { color: var(--accent); border-color: var(--accent); }
  .issues { display: flex; flex-wrap: wrap; gap: 4px 8px; padding: 0 0 6px 18px; }
  .iss { padding: 1px 8px; border-radius: 6px; background: color-mix(in srgb, var(--lost) 12%, transparent); border: 0; font: inherit; font-size: 12px; color: var(--ink); }
  button.iss { cursor: pointer; }
  button.iss:hover { outline: 1px solid var(--accent); }
  .iss.v1 { background: color-mix(in srgb, var(--fix) 14%, transparent); }
  .iss.v2 { background: color-mix(in srgb, var(--lost) 16%, transparent); }
  .iss.v3 { background: color-mix(in srgb, var(--bad) 16%, transparent); }
  .iss.ign { opacity: .55; }
  .bad { color: var(--bad); }
  .check { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--line); }
  .check:last-child { border-bottom: 0; }
  .check.na { opacity: .5; }
  .check.na .grow > div:first-child { text-decoration: line-through; }
  .box { width: 18px; height: 18px; flex: none; border-radius: 5px; border: 1.5px solid var(--faint); background: none; cursor: pointer; display: grid; place-items: center; color: #111; padding: 0; transition: background-color .15s, border-color .15s; }
  .box.on { background: var(--ok); border-color: var(--ok); }
  textarea { width: 100%; resize: vertical; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; color: inherit; font: inherit; outline: 0; }
  textarea:focus { border-color: var(--accent); }
  .log { margin-top: 8px; }
  h3 { margin: 10px 0 4px; font-weight: 600; }
  .devs { display: flex; flex-wrap: wrap; gap: 4px; }
  .dev { font-size: 12px; padding: 1px 8px; border-radius: 6px; background: var(--panel2); border: 1px solid var(--line); }
  .dev.plug { border-color: color-mix(in srgb, var(--accent) 35%, var(--line)); cursor: pointer; color: var(--ink); font: inherit; font-size: 12px; }
  .dev.plug:hover { border-color: var(--accent); }
  .dev.v1 { border-color: var(--fix); }
  .dev.v2 { border-color: var(--lost); color: var(--lost); }
  .dev.v3 { border-color: var(--bad); color: var(--bad); }
  .dev.ign { opacity: .55; text-decoration: line-through; }
  .archived { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 12px; margin-bottom: 12px; border-radius: 9px;
    background: color-mix(in srgb, var(--lost) 14%, transparent); }
  .btn.more { padding: 6px 9px; display: inline-grid; place-items: center; }
  .heard { margin: 8px 0 0; }
</style>

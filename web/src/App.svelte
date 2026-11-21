<script lang="ts">
  import { scanStageText } from "./lib/scanStage";
  import { onMount, untrack } from "svelte";
  import { app } from "./lib/state.svelte";
  import * as api from "./lib/api";
  import { setSongTempo } from "./lib/player.svelte";
  // a song's reference render plays at the song's tempo (for "lock to" a tempo)
  setSongTempo((sid) => app.songs.find((s) => s.id === sid)?.tempo ?? null);
  import { ago } from "./lib/format";
  import { t } from "./lib/i18n/index.svelte";
  import Home from "./routes/Home.svelte";
  import Songs from "./routes/Songs.svelte";
  import Settings from "./routes/Settings.svelte";
  import Repair from "./routes/Repair.svelte";
  import Stats from "./routes/Stats.svelte";
  import Create from "./routes/Create.svelte";
  import Releases from "./routes/Releases.svelte";
  import Samples from "./routes/Samples.svelte";
  import Media from "./routes/Media.svelte";
  import Reading from "./routes/Reading.svelte";
  import Skeleton from "./lib/components/Skeleton.svelte";
  import ActivityStatus from "./lib/components/ActivityStatus.svelte";
  import Midi from "./routes/Midi.svelte";
  import Release from "./routes/Release.svelte";
  import Artists from "./routes/Artists.svelte";
  import Artist from "./routes/Artist.svelte";
  import Art from "./lib/components/Art.svelte";
  import Logo from "./lib/components/Logo.svelte";
  import UpdateBanner from "./lib/components/UpdateBanner.svelte";
  import { updates } from "./lib/update.svelte";
  import { tabs } from "./lib/tabs.svelte";
  import { summary, countdown } from "./lib/releases";
  import { addToRelease } from "./lib/releaseSuggest.svelte";
  import { dropTarget } from "./lib/pdrag.svelte";
  import { tracksOf, releasePlaying, playRelease } from "./lib/releasePlay.svelte";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import { sheetTransport } from "./lib/sheetTransport.svelte";
  import ScanProgress from "./lib/components/ScanProgress.svelte";
  import AudioDetails from "./lib/components/AudioDetails.svelte";
  import Setup from "./routes/Setup.svelte";
  import Search from "./lib/components/Search.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Dialogs from "./lib/components/Dialogs.svelte";
  import { onContextMenu } from "./lib/contextMenus.svelte";
  import { openSong } from "./lib/tabs.svelte";
  import Tip from "./lib/charts/Tip.svelte";
  import HoverTip from "./lib/components/HoverTip.svelte";
  import HighlightPill from "./lib/components/HighlightPill.svelte";
  import KeyboardMenu from "./lib/components/KeyboardMenu.svelte";
  import { keyboard } from "./lib/keyboard.svelte";
  // a MIDI keyboard chosen in Settings plays from the start
  if (keyboard.input) void keyboard.start();
  import { player } from "./lib/player.svelte";
  import { midi } from "./lib/midiPlayer.svelte";

  let page = $state(location.hash.replace(/^#\/?/, "") || "home");
  // "#/song/<id>" opens the song in a tab of the Songs page, beside the list
  $effect(() => {
    if (page.startsWith("song/")) {
      const id = +page.slice(5);
      // untracked: closing that tab later must not open it again
      if (id) untrack(() => openSong(id));
    }
  });
  // back / forward through the pages seen (the webview's history: every page is a #/ address)
  type Nav = { canGoBack?: boolean; canGoForward?: boolean; addEventListener?: (t: string, f: () => void) => void };
  const navApi = (window as unknown as { navigation?: Nav }).navigation;
  let canBack = $state(false);
  let canFwd = $state(false);
  const syncNav = () => {
    canBack = navApi?.canGoBack ?? history.length > 1;
    canFwd = navApi?.canGoForward ?? true;
  };
  onMount(() => {
    updates.start();
    syncNav();
    navApi?.addEventListener?.("currententrychange", syncNav);
    const go = () => {
      page = location.hash.replace(/^#\/?/, "") || "home";
      syncNav();
    };
    addEventListener("hashchange", go);
    // a link to a song opens its tab even when the address already names it (the tab was
    // closed since, or another tab is in front)
    const songLink = (e: MouseEvent) => {
      const a = (e.target as HTMLElement | null)?.closest?.("a[href^='#/song/']") as HTMLAnchorElement | null;
      if (!a || e.button !== 0 || e.ctrlKey || e.metaKey) return;
      const id = +a.getAttribute("href")!.slice(7);
      if (id) openSong(id);
    };
    addEventListener("click", songLink);
    addEventListener("contextmenu", onContextMenu);
    app.start();
    const k = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        document.getElementById("search")?.focus();
      }
      // Ctrl+Z undoes the last change on songs, except while typing (the field keeps its own undo)
      const typing = e.target instanceof HTMLElement && (e.target.matches("input, textarea, select") || e.target.isContentEditable);
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        app.undoLast();
      }
      // Ctrl+Y (or Ctrl+Shift+Z) puts back what Ctrl+Z took
      if ((e.ctrlKey || e.metaKey) && !typing && (e.key.toLowerCase() === "y" || (e.shiftKey && e.key.toLowerCase() === "z"))) {
        e.preventDefault();
        app.redoLast();
      }
      // Space plays / pauses what is heard (or was last), except while typing or on a button
      // (which Space presses): the MIDI playing (a song's chords, a clip, a project), else the
      // song sheet's playback (its render or its project), else the player bar's file
      if (e.key === " " && !typing && !(e.target instanceof HTMLButtonElement)) {
        // the sheet whose render or project is heard (or was last), else the sheet in sight
        // when nothing else waits in the player
        const all = Object.values(sheetTransport.by);
        const st =
          all.find((x) => (!!x.temoin && player.is(x.temoin)) || midi.rel === x.project) ??
          (!player.src && !midi.playing ? all.find((x) => !!document.querySelector<HTMLElement>(`[data-song="${x.sid}"]`)?.offsetParent) : undefined);
        const sheetOwns = !!st;
        if (midi.playing && !(st && midi.rel === st.project)) {
          e.preventDefault();
          midi.stop();
        } else if (st && sheetOwns) {
          e.preventDefault();
          st.playPause();
        } else if (player.src) {
          e.preventDefault();
          player.toggle();
        } else if (midi.shown && midi.rel) {
          e.preventDefault();
          midi.toggle();
        }
      }
    };
    addEventListener("keydown", k);
    return () => {
      removeEventListener("hashchange", go);
      (navApi as { removeEventListener?: (t: string, f: () => void) => void } | undefined)?.removeEventListener?.("currententrychange", syncNav);
      removeEventListener("click", songLink);
      removeEventListener("contextmenu", onContextMenu);
      removeEventListener("keydown", k);
    };
  });
  // the sidebar in sections: the catalogue (what you have), making music (what you draw
  // from), and looking after it; Home alone at the top
  const NAV: [string, [string, string][]][] = [
    ["", [
      ["home", "M3 3h7v9H3zM14 3h7v5h-7zM14 12h7v9h-7zM3 16h7v5H3z"],
    ]],
    ["catalogue", [
      ["songs", "M9 18V5l12-2v13M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 19a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"],
      ["releases", "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4z"],
      ["artists", "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0"],
      ["media", "M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M15.5 9.5a1.5 1.5 0 1 0 0-.01"],
    ]],
    ["make", [
      ["create", "M12 5v14M5 12h14"],
      ["samples", "M3 12h2M7 8v8M11 4v16M15 7v10M19 10v4"],
      ["midi", "M3 5h18v14H3zM8 5v8M13 5v8M18 5v8M3 13h18"],
    ]],
    ["care", [
      ["repair", "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"],
      ["stats", "M4 20V10M10 20V4M16 20v-7M22 20H2"],
    ]],
  ];
  // the labels follow a change of language
  // the sidebar's sections folded by the user, kept
  const FOLD = "spool.side.folded";
  let folded = $state<string[]>((() => { try { return JSON.parse(localStorage.getItem(FOLD) || "[]"); } catch { return []; } })());
  function foldSec(sec: string) {
    folded = folded.includes(sec) ? folded.filter((x) => x !== sec) : [...folded, sec];
    try { localStorage.setItem(FOLD, JSON.stringify(folded)); } catch { /* not kept */ }
  }
  const nav = $derived(NAV.map(([sec, items]) => [sec, items.map(([k, d]) => [k, t(`app.nav.${k}`), d])] as const));
  // the sidebar folds to its icons (kept across launches)
  let mini = $state((() => {
    try {
      return localStorage.getItem("pf:side") === "mini";
    } catch {
      return false;
    }
  })());
  function fold() {
    mini = !mini;
    try {
      localStorage.setItem("pf:side", mini ? "mini" : "full");
    } catch {
      /* not kept */
    }
  }
  const fix = $derived(app.stats?.projects.fix ?? 0);
  const SETTINGS_D = "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.8 1.2V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3 15H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 9 4.6V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1A1.7 1.7 0 0 0 21 9h.1a2 2 0 1 1 0 4H21a1.7 1.7 0 0 0-1.6 2z";
  // releases not out yet (the next first, then the undated): the sidebar's "Upcoming releases"
  const today = new Date().toISOString().slice(0, 10);
  // the releases not out yet, the next first: up to 20, in a list that scrolls (the
  // settings, the analysis and "fold" stay in sight below)
  const current = $derived(app.releases.filter((r) => !r.date || r.date >= today).slice(0, 20).map((r) => ({ r, s: summary(r, app.songs) })));
  // a song dragged from a list onto one of them: added to it
  let dropRel = $state<string | null>(null);
  const status = $derived(
    app.scan?.running ? t("app.status.running") : app.scan?.error ? t("app.status.error", { error: app.scan.error }) : app.scan?.last ? t("app.status.upToDate", { ago: ago(app.scan.last) }) : "",
  );
</script>

{#if app.loaded && app.config && !app.config.setup_done}
  <Setup />
{:else}
<div class="app" class:mini>
  <aside class="side">
    <div class="brand"><Logo size={30} /><span>spool</span></div>
    {#each nav as [sec, items] (sec)}
      {#if sec}{#if mini}<div class="sep" aria-hidden="true"></div>{:else}<button class="cap sec fold" class:folded={folded.includes(sec)} aria-expanded={!folded.includes(sec)} title={t(folded.includes(sec) ? "app.side.secUnfold" : "app.side.secFold")} onclick={() => foldSec(sec)}>{t(`app.side.sec.${sec}`)}<svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.4"><path d="M6 9l6 6 6-6" /></svg></button>{/if}{/if}
    {#each items as [k, label, d] (k)}
      {@const isOn = page === k || page.startsWith(k + "/") || (k === "songs" && page.startsWith("song/")) || (k === "releases" && page.startsWith("release/")) || (k === "artists" && page.startsWith("artist/")) || (k === "samples" && page.startsWith("samples?")) || (k === "midi" && page.startsWith("midi?")) || (k === "repair" && (page === "atelier" || page === "copies"))}
      <!-- a folded section keeps the page open in sight -->
      {#if mini || !sec || !folded.includes(sec) || isOn}
      <a class="nav" title={mini ? (k === "repair" && fix ? t("app.nav.repairTitle", { label, n: fix }) : label) : undefined} class:on={isOn} href="#/{k}">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path {d} /></svg><span>{label}</span>
        {#if k === "repair" && fix}<span class="count fx" class:stale={app.busy}>{fix}</span>{/if}
        {#if k === "home" && app.overview && app.overview.queue.length}<span class="count">{app.overview.queue.length}</span>{/if}
      </a>
      {/if}
    {/each}
    {/each}
    {#if !mini && current.length}
      <div class="cap capfold"><a href="#/releases">{t("app.side.current")}</a>
        <button class="fold" class:folded={folded.includes("current")} aria-expanded={!folded.includes("current")} aria-label={t(folded.includes("current") ? "app.side.secUnfold" : "app.side.secFold")} title={t(folded.includes("current") ? "app.side.secUnfold" : "app.side.secFold")} onclick={() => foldSec("current")}><svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.4"><path d="M6 9l6 6 6-6" /></svg></button></div>
      {#if !folded.includes("current")}
      <div class="rels">
      {#each current as { r, s } (r.id)}
        <a class="mini-rel" class:on={page === "release/" + r.id} class:drop={dropRel === r.id} href="#/release/{r.id}"
          use:dropTarget={{
            accept: (it) => it.kind === "song",
            over: () => (dropRel = r.id),
            leave: () => { if (dropRel === r.id) dropRel = null; },
            drop: (it) => { dropRel = null; addToRelease(r.id, it.data as number); },
          }} title={r.date ? t("app.side.relTitleWhen", { title: r.title, ready: s.ready, count: s.count, when: countdown(r.date) }) : t("app.side.relTitle", { title: r.title, ready: s.ready, count: s.count })}>
          <span class="mcv"><Art title={r.title} cover={s.cover} size={30} radius={6} />
            {#if tracksOf(r).length}
              {@const on = releasePlaying(r)}
              <button class="mplay" class:on aria-label={on ? t("song.player.pause") : t("releases.playAll", { n: tracksOf(r).length })} title={on ? t("song.player.pause") : t("releases.playAll", { n: tracksOf(r).length })} onclick={(e) => playRelease(e, r)}>
                {#if on}<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
                {:else}<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor" aria-hidden="true"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
              </button>
            {/if}
          </span>
          <span class="mr"><b>{r.title}</b><span>{r.date ? t("app.side.readyWhen", { ready: s.ready, count: s.count, when: countdown(r.date) }) : t("app.side.ready", { ready: s.ready, count: s.count })}</span>
            <i class="mp"><i style:width="{Math.round(s.progress * 100)}%" class:warn={s.blocked > 0}></i></i></span>
        </a>
      {/each}
      </div>
      {/if}
    {/if}
    <!-- the songs open are the tabs of the Songs page: not listed here a second time -->
    <div class="grow"></div>
    <UpdateBanner {mini} />
    <!-- the automatic analysis off (Settings): run it by hand, a dot when projects changed -->
    {#if app.config && !app.config.auto_analysis}
      <button class="nav analyse" disabled={app.scan?.running} title={app.scan?.pending ? t("app.analyse.pendingTip") : t("app.analyse.tip")} onclick={() => api.analyseNow()}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" /></svg><span>{t("app.analyse.label")}</span>
        {#if app.scan?.pending}<i class="dot" aria-label={t("app.analyse.pending")}></i>{/if}
      </button>
    {/if}
    <a class="nav" title={mini ? t("app.nav.settings") : undefined} class:on={page === "settings"} href="#/settings">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d={SETTINGS_D} /></svg><span>{t("app.nav.settings")}</span>
    </a>
    <!-- the analysis: its own small block, the fold button below it like a nav entry -->
    <div class="status" class:run={app.scan?.running} class:err={!!app.scan?.error}
      title={mini ? (app.scan?.running ? scanStageText(app.scan.stage) : app.busy ? t("app.status.busy") : status) : undefined}>
      {#if !mini}<ActivityStatus />
      {:else}
        {#if app.busy && !app.scan?.running}<span class="spin"></span>{:else}<i class="sd" class:run={app.scan?.running}></i>{/if}
        {#if !mini}<span class="st" title={app.busy && !app.scan?.running ? t("app.status.busy") : status}>{app.busy && !app.scan?.running ? t("app.status.busy") : status}</span>{/if}
      {/if}
    </div>
    {#if app.audioDetails && !mini}
      <div class="audiopop" role="dialog" aria-label={t("app.activity.title")}>
        <button class="x" onclick={() => (app.audioDetails = false)} aria-label={t("common.close")}>✕</button>
        <b class="poptitle">{t("app.activity.title")}</b>
        <div class="popsec small muted">{app.scan?.running ? (app.scan.full ? t("app.activity.long") : t("app.activity.checking")) : app.scan?.last ? t("app.status.upToDate", { ago: ago(app.scan.last) }) : ""}</div>
        {#if app.scan?.running || app.scan?.last}<ScanProgress full />{/if}
        <hr />
        <AudioDetails />
      </div>
    {/if}
    <button class="nav foldb" onclick={fold} title={mini ? t("app.side.expand") : t("app.side.collapse")} aria-label={mini ? t("app.side.expand") : t("app.side.collapse")}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d={mini ? "M13 6l6 6-6 6M5 6l6 6-6 6" : "M11 6l-6 6 6 6M19 6l-6 6 6 6"} /></svg><span>{t("app.side.fold")}</span>
    </button>
  </aside>
  <div class="col">
    <header class="top">
      <div class="hist">
        <button class="ib" disabled={!canBack} onclick={() => history.back()} title={t("app.top.backTitle")} aria-label={t("app.top.back")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M15 6l-6 6 6 6" /></svg></button>
        <button class="ib" disabled={!canFwd} onclick={() => history.forward()} title={t("app.top.forwardTitle")} aria-label={t("app.top.forward")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 6l6 6-6 6" /></svg></button>
        <button class="ib" disabled={!app.undo.left} onclick={() => app.undoLast()} title={app.undo.label ? t("app.top.undoTitle", { label: app.undo.label }) : t("app.top.nothingToUndo")} aria-label={t("app.top.undo")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 14L4 9l5-5" /><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11" /></svg></button>
        <button class="ib" disabled={!app.undo.redo} onclick={() => app.redoLast()} title={app.undo.redo ? t("app.redo.title", { label: app.undo.redo }) : t("app.redo.nothing")} aria-label={t("app.redo.label")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M15 14l5-5-5-5" /><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13" /></svg></button>
      </div>
      <Search {page} {fold} />
      <a class="btn sm primary newb" href="#/create" title={t("app.top.newSongTitle")}><i class="recdot"></i>{t("app.top.newSong")}</a>
      <HighlightPill {fix} />
      <KeyboardMenu />
    </header>
    <main>
      {#if !app.loaded}<Skeleton kind={location.hash.startsWith("#/song/") ? "sheet" : location.hash.startsWith("#/songs") ? "list" : "page"} />
      {:else if page === "songs" || page.startsWith("song/")}<Songs />
      {:else if page === "settings"}<Settings />
      {:else if page === "repair"}<Repair />
      {:else if page === "atelier"}<Repair initial="atelier" />
      {:else if page === "copies"}<Repair initial="copies" />
      {:else if page === "stats"}<Stats />
      {:else if page === "create"}<Create />
      {:else if page === "releases"}<Releases />
      {:else if page === "samples" || page.startsWith("samples?")}{#key page}<Samples />{/key}
      {:else if page === "midi" || page.startsWith("midi?")}{#key page}<Midi />{/key}
      {:else if page.startsWith("release/")}{#key page}<Release id={page.slice(8)} />{/key}
      {:else if page === "artists"}<Artists />
      {:else if page === "media"}<Media />
      {:else if page === "reading"}<Reading />
      {:else if page.startsWith("artist/")}{#key page}<Artist name={decodeURIComponent(page.slice(7))} />{/key}
      {:else}<Home />{/if}
    </main>
    <PlayerBar />
  </div>
</div>
{/if}

<Tip />
<HoverTip />
<ContextMenu />
<Dialogs />

{#if app.toast}
  <div class="toast" class:err={app.toast.error} role="status">
    <span>{app.toast.text}</span>
    {#if app.toast.undo && app.undo.left}<button onclick={() => app.undoLast()}>{t("app.top.undo")} <kbd>Ctrl Z</kbd></button>{/if}
  </div>
{/if}

<style>
  .app { display: grid; grid-template-columns: var(--side) 1fr; height: 100vh; }
  .side { background: var(--panel2); border-right: 1px solid var(--line); display: flex; flex-direction: column; padding: 14px 10px; gap: 2px; min-height: 0; }
  .brand { display: flex; align-items: center; gap: 10px; padding: 4px 8px 16px; font-weight: 700; font-size: 14.5px; }
  .brand span { font-family: var(--mono); font-size: 19px; font-weight: 600; letter-spacing: -0.03em; }
  .newb { display: inline-flex; align-items: center; gap: 8px; }
  .recdot { width: 8px; height: 8px; border-radius: 50%; background: var(--rec); box-shadow: 0 0 0 2px color-mix(in srgb, var(--rec) 25%, transparent); }
  /* the page you are on: a red tape mark at its edge */
  .nav.on { position: relative; }
  .nav.on::before { content: ""; position: absolute; left: -10px; top: 9px; bottom: 9px; width: 3px; border-radius: 0 3px 3px 0; background: var(--rec); }
  .mini .nav.on::before { left: -8px; }
  .nav { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 8px; color: var(--muted); }
  .nav:hover { background: color-mix(in srgb, var(--line) 55%, transparent); color: var(--ink); }
  .nav.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .nav svg { width: 17px; height: 17px; flex: none; }
  .count { margin-left: auto; font-size: 11px; padding: 0 7px; border-radius: 99px; background: color-mix(in srgb, var(--lost) 20%, transparent); color: var(--lost); font-weight: 600; }
  .count.fx { background: color-mix(in srgb, var(--fix) 18%, transparent); color: var(--fix); }
  .grow { flex: 1; }
  .side { overflow-y: auto; }
  /* nothing in the sidebar is squeezed (the analysis block was, under Réglages, on a short
     window): it scrolls instead */
  .side > :global(*) { flex-shrink: 0; }
  /* the releases list is the one part that gives way: it scrolls */
  .side > .rels { flex: 0 1 auto; min-height: 70px; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
  .mini-rel.drop { outline: 2px dashed var(--accent); outline-offset: -2px; background: var(--accent-bg); }
  .cap { font-size: 11px; color: var(--faint); padding: 14px 10px 4px; font-weight: 600; }
  .cap a { color: inherit; }
  button.nav.analyse { border: 0; background: none; font: inherit; cursor: pointer; text-align: left; width: 100%; position: relative; }
  button.nav.analyse:disabled { opacity: .5; cursor: default; }
  .analyse .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); margin-left: auto; }
  .cap.sec { padding-top: 16px; text-transform: none; letter-spacing: 0; }
  .sep { height: 1px; background: var(--line); margin: 8px 12px; }
  .cap a:hover { color: var(--ink); }
  /* a section's name folds it: the arrow turns, the pages hide (the open one stays) */
  button.cap { display: flex; align-items: center; gap: 5px; width: 100%; border: 0; background: none; font-family: inherit; text-align: left; cursor: pointer; }
  button.cap:hover { color: var(--ink); }
  .capfold { display: flex; align-items: center; gap: 5px; }
  .capfold .fold { border: 0; background: none; color: inherit; padding: 0; cursor: pointer; display: grid; place-items: center; }
  .capfold .fold:hover { color: var(--ink); }
  .fold svg { opacity: .7; transition: transform .15s; }
  .fold.folded svg { transform: rotate(-90deg); }
  @media (prefers-reduced-motion: reduce) { .fold svg { transition: none; } }
  .mini-rel { display: flex; align-items: center; gap: 9px; padding: 5px 10px; border-radius: 8px; color: var(--ink); min-width: 0; }
  .mini-rel:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
  .mini-rel.on { background: var(--accent-bg); }
  .mr { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .mr b { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mr > span { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mr .one { font-size: 12.5px; color: var(--muted); }
  .small-row { padding: 3px 10px; }
  .mp { display: block; height: 3px; border-radius: 3px; background: var(--line); margin-top: 3px; overflow: hidden; }
  .mp i { display: block; height: 100%; background: var(--ok); }
  .mp i.warn { background: var(--lost); }
  .app { transition: grid-template-columns .2s ease; }
  .app.mini { grid-template-columns: 60px 1fr; }
  .mini .side { padding: 14px 8px; }
  .mini .brand { padding: 4px 0 16px; justify-content: center; }
  .mini .brand span, .mini .nav span:not(.count) { display: none; }
  .count.stale { opacity: .4; transition: opacity .2s; }
  /* folded: the number goes under the icon, never over it */
  .mini .nav { flex-direction: column; justify-content: center; gap: 3px; padding: 8px 0 6px; }
  .mini .count { margin: 0; padding: 0 5px; font-size: 10px; line-height: 15px; }
  .upd { display: inline-flex; align-items: center; gap: 8px; }
  .status { display: flex; align-items: center; gap: 10px; margin: 6px 0 4px; padding: 9px 10px; border-radius: 9px; background: color-mix(in srgb, var(--line) 35%, transparent);
    font-size: 12px; color: var(--muted); line-height: 1.35; }
  .status.run { display: block; }
  .status.err { color: var(--bad); }
  .status { min-width: 0; max-width: 100%; overflow: hidden; }
  .st { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mini .status, .mini .status.run { display: flex; justify-content: center; padding: 9px 0; background: none; }
  .foldb { border: 0; background: none; font: inherit; cursor: pointer; width: 100%; text-align: left; color: var(--faint); }
  .sd { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--ok); display: block; }
  .sd.run { background: var(--accent); animation: blink 1s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: .35; } }
  .col { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .top { height: 56px; flex: none; display: flex; align-items: center; gap: 12px; padding: 0 22px 0 14px; border-bottom: 1px solid var(--line); }
  .top .pill { margin-left: auto; }
  .hist { display: flex; gap: 2px; flex: none; }
  .ib { width: 30px; height: 30px; border: 0; border-radius: 7px; background: none; color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .ib svg { width: 17px; height: 17px; }
  .ib:hover:not(:disabled) { color: var(--ink); background: color-mix(in srgb, var(--line) 55%, transparent); }
  .ib:disabled { opacity: .35; cursor: default; }
  .newb { flex: none; white-space: nowrap; color: var(--on-accent); }
  .pill { display: inline-flex; align-items: center; gap: 7px; padding: 4px 11px; border-radius: 999px; border: 1px solid var(--line); color: var(--muted); background: var(--panel); font-size: 12px; }
  .pill i { width: 7px; height: 7px; border-radius: 50%; background: var(--lost); }
  /* the scrollbar's room is kept even when nothing scrolls: a page whose height sits right at
     the window's would otherwise gain and lose it in a loop, its width (and its layout) with it */
  main { flex: 1; min-height: 0; overflow: auto; scrollbar-gutter: stable; }
  .loading { padding: 40px; }
  .toast { position: fixed; left: 50%; bottom: 22px; transform: translateX(-50%); display: flex; align-items: center; gap: 14px; max-width: min(640px, calc(100vw - 40px));
    padding: 9px 10px 9px 16px; border-radius: 10px; background: var(--ink); color: var(--bg); box-shadow: 0 10px 30px rgba(0, 0, 0, .35); font-size: 13px; animation: up .2s ease-out; z-index: 50; }
  .toast.err { background: var(--bad); color: #fff; }
  .toast button { border: 0; border-radius: 7px; padding: 4px 10px; background: color-mix(in srgb, var(--bg) 14%, transparent); color: inherit; cursor: pointer; font-weight: 600; white-space: nowrap; }
  .toast kbd { font: inherit; font-size: 10.5px; opacity: .65; margin-left: 4px; }
  @keyframes up { from { opacity: 0; transform: translate(-50%, 8px); } }
  /* the audio analysis' details, beside the sidebar's status */
  .audiopop { position: fixed; left: calc(var(--side) + 10px); bottom: 14px; z-index: 70; width: 340px; max-width: calc(100vw - var(--side) - 24px);
    padding: 14px 16px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 16px 40px rgba(0, 0, 0, .35); }
  .audiopop { max-height: calc(100vh - 40px); overflow-y: auto; width: 380px; }
  .audiopop .poptitle { display: block; margin-bottom: 4px; }
  .audiopop .popsec { margin-bottom: 10px; }
  .audiopop hr { border: 0; border-top: 1px solid var(--line); margin: 14px 0; }
  .audiopop .x { position: absolute; top: 8px; right: 8px; border: 0; background: none; color: var(--faint); cursor: pointer; padding: 4px 6px; border-radius: 6px; }
  .audiopop .x:hover { color: var(--ink); background: var(--panel2); }
  /* a release in progress played whole: over its cover, on hover, and while it plays */
  .mcv { position: relative; flex: none; display: grid; }
  .mplay { position: absolute; inset: 0; border: 0; border-radius: 6px; background: rgba(0, 0, 0, .5); color: #fff; display: grid; place-items: center; cursor: pointer; padding: 0; opacity: 0; transition: opacity .15s; }
  .mini-rel:hover .mplay, .mplay:focus-visible, .mplay.on { opacity: 1; }
  .mplay.on { background: color-mix(in srgb, var(--accent) 80%, transparent); color: var(--on-accent); }
</style>

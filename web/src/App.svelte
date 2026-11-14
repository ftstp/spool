<script lang="ts">
  import { scanStageText } from "./lib/scanStage";
  import { onMount, untrack } from "svelte";
  import { app } from "./lib/state.svelte";
  import { ago } from "./lib/format";
  import { t } from "./lib/i18n/index.svelte";
  import Home from "./routes/Home.svelte";
  import Songs from "./routes/Songs.svelte";
  import Settings from "./routes/Settings.svelte";
  import Repair from "./routes/Repair.svelte";
  import Stats from "./routes/Stats.svelte";
  import Create from "./routes/Create.svelte";
  import Releases from "./routes/Releases.svelte";
  import Release from "./routes/Release.svelte";
  import Art from "./lib/components/Art.svelte";
  import Logo from "./lib/components/Logo.svelte";
  import UpdateBanner from "./lib/components/UpdateBanner.svelte";
  import { updates } from "./lib/update.svelte";
  import { tabs } from "./lib/tabs.svelte";
  import { summary, countdown } from "./lib/releases";
  import PlayerBar from "./lib/components/PlayerBar.svelte";
  import ScanProgress from "./lib/components/ScanProgress.svelte";
  import Setup from "./routes/Setup.svelte";
  import Search from "./lib/components/Search.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Dialogs from "./lib/components/Dialogs.svelte";
  import { onContextMenu } from "./lib/contextMenus.svelte";
  import { openSong } from "./lib/tabs.svelte";
  import Tip from "./lib/charts/Tip.svelte";
  import { player } from "./lib/player.svelte";

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
      // Space plays / pauses, except while typing or on a button (which Space presses)
      if (e.key === " " && !typing && !(e.target instanceof HTMLButtonElement) && player.src) {
        e.preventDefault();
        player.toggle();
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
  const NAV = [
    ["home", "M3 3h7v9H3zM14 3h7v5h-7zM14 12h7v9h-7zM3 16h7v5H3z"],
    ["songs", "M9 18V5l12-2v13M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 19a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"],
    ["repair", "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"],
    ["releases", "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4z"],
    ["create", "M12 5v14M5 12h14"],
    ["stats", "M4 20V10M10 20V4M16 20v-7M22 20H2"],
  ];
  // the labels follow a change of language
  const nav = $derived(NAV.map(([k, d]) => [k, t(`app.nav.${k}`), d]));
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
  const current = $derived(app.releases.filter((r) => !r.date || r.date >= today).slice(0, 4).map((r) => ({ r, s: summary(r, app.songs) })));
  // the songs open in tabs, the last opened first
  const recent = $derived(
    [...tabs.ids].reverse().map((id) => app.songs.find((s) => s.id === id)).filter((s): s is NonNullable<typeof s> => !!s).slice(0, 4),
  );
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
    {#each nav as [k, label, d]}
      <a class="nav" title={mini ? (k === "repair" && fix ? t("app.nav.repairTitle", { label, n: fix }) : label) : undefined} class:on={page === k || page.startsWith(k + "/") || (k === "songs" && page.startsWith("song/")) || (k === "releases" && page.startsWith("release/")) || (k === "repair" && (page === "atelier" || page === "copies"))} href="#/{k}">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path {d} /></svg><span>{label}</span>
        {#if k === "repair" && fix}<span class="count fx" class:stale={app.busy}>{fix}</span>{/if}
        {#if k === "home" && app.overview && app.overview.queue.length}<span class="count">{app.overview.queue.length}</span>{/if}
      </a>
    {/each}
    {#if !mini && current.length}
      <div class="cap"><a href="#/releases">{t("app.side.current")}</a></div>
      {#each current as { r, s } (r.id)}
        <a class="mini-rel" class:on={page === "release/" + r.id} href="#/release/{r.id}" title={r.date ? t("app.side.relTitleWhen", { title: r.title, ready: s.ready, count: s.count, when: countdown(r.date) }) : t("app.side.relTitle", { title: r.title, ready: s.ready, count: s.count })}>
          <Art title={r.title} cover={s.cover} size={30} radius={6} />
          <span class="mr"><b>{r.title}</b><span>{r.date ? t("app.side.readyWhen", { ready: s.ready, count: s.count, when: countdown(r.date) }) : t("app.side.ready", { ready: s.ready, count: s.count })}</span>
            <i class="mp"><i style:width="{Math.round(s.progress * 100)}%" class:warn={s.blocked > 0}></i></i></span>
        </a>
      {/each}
    {/if}
    {#if !mini && recent.length}
      <div class="cap">{t("app.side.open")}</div>
      {#each recent as s (s.id)}
        <a class="mini-rel small-row" href="#/song/{s.id}"><Art title={s.title} cover={s.cover} size={22} radius={5} /><span class="mr"><span class="one">{s.title}</span></span></a>
      {/each}
    {/if}
    <div class="grow"></div>
    <UpdateBanner {mini} />
    <a class="nav" title={mini ? t("app.nav.settings") : undefined} class:on={page === "settings"} href="#/settings">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d={SETTINGS_D} /></svg><span>{t("app.nav.settings")}</span>
    </a>
    <!-- the analysis: its own small block, the fold button below it like a nav entry -->
    <div class="status" class:run={app.scan?.running} class:err={!!app.scan?.error}
      title={mini ? (app.scan?.running ? scanStageText(app.scan.stage) : app.busy ? t("app.status.busy") : status) : undefined}>
      {#if (app.scan?.running || app.scan?.audio?.running) && !mini}<ScanProgress />
      {:else}
        {#if app.busy && !app.scan?.running}<span class="spin"></span>{:else}<i class="sd" class:run={app.scan?.running}></i>{/if}
        {#if !mini}<span class="st" title={app.busy && !app.scan?.running ? t("app.status.busy") : status}>{app.busy && !app.scan?.running ? t("app.status.busy") : status}</span>{/if}
      {/if}
    </div>
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
      </div>
      <Search {page} {fold} />
      <a class="btn sm primary newb" href="#/create" title={t("app.top.newSongTitle")}><i class="recdot"></i>{t("app.top.newSong")}</a>
      {#if fix}<a class="pill" href="#/repair"><i></i>{t("app.top.fixable", { n: fix })}</a>{/if}
    </header>
    <main>
      {#if !app.loaded}<p class="muted loading">{t("common.loading")}</p>
      {:else if page === "songs" || page.startsWith("song/")}<Songs />
      {:else if page === "settings"}<Settings />
      {:else if page === "repair"}<Repair />
      {:else if page === "atelier"}<Repair initial="atelier" />
      {:else if page === "copies"}<Repair initial="copies" />
      {:else if page === "stats"}<Stats />
      {:else if page === "create"}<Create />
      {:else if page === "releases"}<Releases />
      {:else if page.startsWith("release/")}{#key page}<Release id={page.slice(8)} />{/key}
      {:else}<Home />{/if}
    </main>
    <PlayerBar />
  </div>
</div>
{/if}

<Tip />
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
  .cap { font-size: 11px; color: var(--faint); padding: 14px 10px 4px; font-weight: 600; }
  .cap a { color: inherit; }
  .cap a:hover { color: var(--ink); }
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
  main { flex: 1; min-height: 0; overflow: auto; }
  .loading { padding: 40px; }
  .toast { position: fixed; left: 50%; bottom: 22px; transform: translateX(-50%); display: flex; align-items: center; gap: 14px; max-width: min(640px, calc(100vw - 40px));
    padding: 9px 10px 9px 16px; border-radius: 10px; background: var(--ink); color: var(--bg); box-shadow: 0 10px 30px rgba(0, 0, 0, .35); font-size: 13px; animation: up .2s ease-out; z-index: 50; }
  .toast.err { background: var(--bad); color: #fff; }
  .toast button { border: 0; border-radius: 7px; padding: 4px 10px; background: color-mix(in srgb, var(--bg) 14%, transparent); color: inherit; cursor: pointer; font-weight: 600; white-space: nowrap; }
  .toast kbd { font: inherit; font-size: 10.5px; opacity: .65; margin-left: 4px; }
  @keyframes up { from { opacity: 0; transform: translate(-50%, 8px); } }
</style>

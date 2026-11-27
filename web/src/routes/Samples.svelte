<script lang="ts">
  import Skeleton from "../lib/components/Skeleton.svelte";
  import { cached, keep } from "../lib/pageCache";
  import { onDestroy } from "svelte";
  // The samples of the sample folders: by family and category (from their name and folders),
  // loops or one-shots, in a key or at a tempo (a song's, from its sheet: #/samples?key=Am&bpm=174).
  // One page at a time from the base; a click plays, the arrows move and play the next one.
  import * as api from "../lib/api";
  import type { SamplePage, SampleRow, SampleQuery } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { player, browse } from "../lib/player.svelte";
  import { ui } from "../lib/contextMenus.svelte";
  import SampleDupes from "../lib/components/SampleDupes.svelte";
  import SampleWave from "../lib/components/SampleWave.svelte";
  import FavStar from "../lib/components/FavStar.svelte";
  import { favsOf, loadFavs } from "../lib/favorites.svelte";
  import SampleSortMenu from "../lib/components/SampleSortMenu.svelte";
  import SampleAsk from "../lib/components/SampleAsk.svelte";
  import { FAMILIES } from "../lib/sampleFamilies";
  import { t, fmtNum } from "../lib/i18n/index.svelte";
  import { aboutTime } from "../lib/format";
  const au = $derived(app.scan?.audio);

  /** Shown apart from the unsorted ones: long recordings (not listened to) and files that cannot be read. */
  const APART = ["long", "unreadable"];
  // sorting one sample by hand (under its row), and the quick sort of the unsorted ones
  let sortOf = $state<string | null>(null);
  let quick = $state(false);
  function sorted() {
    sortOf = null;
    load();
  }
  const srcTip = (s: string | null) =>
    t(s === "audio" ? "samples.fromAudio" : s === "folder" ? "samples.fromFolder" : s === "neighbours" ? "samples.fromNeighbours" : s === "user" ? "samples.fromUser" : "samples.fromName");
  const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
  const KEYS = [...NOTES.map((n) => n + "m"), ...NOTES];
  const PAGE = 200;

  // filters, from the address when a song sends here
  const params = new URLSearchParams(location.hash.split("?")[1] ?? "");
  let q = $state("");
  let family = $state("");
  let cat = $state("");
  let kind = $state("");
  /** "", "used" (in a project) or "unused" (in none). */
  let used = $state("");
  let showDupes = $state(false);
  // the songs using a sample, shown under its row
  let useOf = $state<string | null>(null);
  let useSongs = $state<number[]>([]);
  async function toggleUse(r: SampleRow) {
    if (useOf === r.path) return (useOf = null);
    useOf = r.path;
    useSongs = [];
    useSongs = await api.sampleSongs(r.path).catch(() => []);
  }
  const songTitle = (id: number) => app.songs.find((s) => s.id === id)?.title ?? "?";
  let key = $state(params.get("key") ?? "");
  let bpm = $state(params.get("bpm") ?? "");
  const from = params.get("from") ?? "";

  // a song's key written otherwise ("Eb", "G phrygien") stays shown as chosen
  const keyOpts = $derived(!key || KEYS.includes(key) ? KEYS : [key, ...KEYS]);
  let page = $state<SamplePage | null>(null);
  // "sounds alike": the list shows the samples closest to one, until back
  let near = $state<api.Similar | null>(null);
  let nearOf = $state<string | null>(null);
  const shown = $derived(near ? near.rows.map((x) => x.row) : rows);
  const sims = $derived(new Map((near?.rows ?? []).map((x) => [x.row.path, x.sim])));
  async function alike(r: SampleRow) {
    nearOf = r.path;
    try {
      near = await api.similarSamples(r.path);
      sel = -1;
    } catch (e) {
      app.say(String(e), { error: true });
    }
    nearOf = null;
  }
  // opened from an arrangement's clip (or elsewhere): the samples that sound like this one
  const alikeOf = params.get("alike");
  // its own line: the list's loading clears `error` as it starts
  let alikeErr = $state("");
  if (alikeOf) {
    // a file of the project itself (a freeze, a bounce) is not in the library: said plainly
    api.similarSamples(alikeOf).then((x) => (near = x)).catch((e) => {
      const name = alikeOf.split("/").pop() ?? alikeOf;
      alikeErr = String(e) === t("core.fs.notFoundShort") ? t("samples.alikeNotInLibrary", { name }) : t("samples.alikeFailed", { name, error: String(e) });
    });
  }
  // measured on a real library: a clap's closest claps score 0.6 to 0.75
  const simWord = (x: number) => t(x >= 0.8 ? "samples.veryClose" : x >= 0.62 ? "samples.close" : "samples.alike");
  let rows = $state<SampleRow[]>([]);
  let loading = $state(false);
  let error = $state("");
  let sel = $state(-1);
  let seq = 0;

  const query = (offset: number): SampleQuery => ({
    q, family: family || null, cat: cat || null, kind: kind || null, key: key || null,
    bpm: bpm && +bpm > 0 ? +bpm : null, used: used || null, offset, limit: PAGE,
  });
  // a view already seen shows at once (pageCache), and is read again only if the base changed
  let loadingMore = $state(false);
  // the starred samples, alone (the sidebar's "Favourites")
  let favOnly = $state(location.hash.includes("fav=1"));
  async function load(more = false) {
    const n = ++seq;
    if (favOnly) {
      await loadFavs();
      const r = await api.sampleRows(favsOf("sample").map((f) => f.id));
      if (n !== seq) return;
      rows = r;
      sel = -1;
      loading = false;
      return;
    }
    const ck = "samples:" + JSON.stringify(query(0));
    const version = app.version;
    if (!more) {
      const c = cached<{ page: SamplePage; rows: SampleRow[] }>(ck);
      if (c) {
        page = c.value.page;
        rows = c.value.rows;
        sel = -1;
        error = "";
        if (c.version === version) return void (loading = false);
      }
    }
    loading = true;
    loadingMore = more;
    try {
      const p = await api.samples(query(more ? rows.length : 0));
      if (n !== seq) return;
      page = p;
      rows = more ? [...rows, ...p.rows] : p.rows;
      if (!more) sel = -1;
      error = "";
      keep(ck, version, { page: p, rows });
    } catch (e) {
      if (n === seq) error = String(e);
    }
    if (n === seq) {
      loading = false;
      loadingMore = false;
    }
  }
  // typing waits a little; the other filters reload at once
  let timer = 0;
  $effect(() => {
    void [family, cat, kind, key, bpm, used, app.version, favOnly];
    clearTimeout(timer);
    load();
  });
  function typed() {
    clearTimeout(timer);
    timer = window.setTimeout(() => load(), 220);
  }

  // the counts come split by loop / one-shot: the sidebar follows the loop filter, and a
  // family shows both halves
  const isKind = (x: { is_loop: boolean | null }, k: string) => !k || (k === "loop" ? x.is_loop === true : x.is_loop === false);
  const count = (f: string, c?: string, k = kind) =>
    (page?.counts ?? []).filter((x) => (f === "none" ? x.family === null : x.family === f) && (c === undefined || x.cat === c) && isKind(x, k)).reduce((a, x) => a + x.n, 0);
  const total = $derived((page?.counts ?? []).filter((x) => isKind(x, kind)).reduce((a, x) => a + x.n, 0));
  function pickFamily(f: string) {
    if (family === f && !cat) family = "";
    else family = f;
    cat = "";
  }
  function pickCat(f: string, c: string) {
    family = f;
    cat = cat === c ? "" : c;
  }

  // the BPM typed here is the tempo the player can lock to (off when the page is left)
  $effect(() => player.setTarget(+bpm > 0 ? +bpm : null));
  onDestroy(() => player.setTarget(null));
  const playing = (r: SampleRow) => player.playing && player.src === api.mediaUrl(r.path);
  function play(i: number) {
    const r = shown[i];
    if (!r) return;
    sel = i;
    if (playing(r)) return player.toggle();
    player.play(r.path, null, r.name, 0, r.bpm);
    browse.set(r.path, i, shown.length, step);
  }
  // previous / next, from the arrows or the player bar
  function step(d: number) {
    const i = Math.max(0, Math.min(shown.length - 1, sel + d));
    if (i === sel) return;
    play(i);
    document.getElementById("smp-" + i)?.scrollIntoView({ block: "nearest" });
  }
  onDestroy(() => browse.clear(step));
  function keys(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && e.target.matches("input, select, textarea");
    if (typing || quick || !shown.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      step(e.key === "ArrowDown" ? 1 : -1);
    }
  }
  const dur = (s: number | null) => (s == null ? "" : s < 10 ? t("samples.secs", { n: (Math.round(s * 10) / 10).toString() }) : `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, "0")}`);
  const reveal = (r: SampleRow) => api.reveal(r.path).catch((e) => app.say(String(e), { error: true }));
  async function copy(r: SampleRow) {
    try {
      await navigator.clipboard.writeText(r.path.replace(/\//g, "\\"));
      app.say(t("samples.copied"));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const filtered = $derived(!!(q || family || kind || key || bpm));
  function clear() {
    q = ""; family = ""; cat = ""; kind = ""; key = ""; bpm = "";
  }
  // no sample folder set and nothing indexed: say where to set one
  const noDirs = $derived(!(app.config?.sample_dirs ?? []).length && !page?.all);
</script>

<svelte:window onkeydown={keys} />

<div class="page">
  <header class="head">
    <div>
      <h1>{t("samples.title")}</h1>
      <p class="small muted">
        {#if page}{filtered ? t("samples.countOf", { n: fmtNum(page.total), all: fmtNum(page.all) }) : t("samples.count", { n: fmtNum(page.all) })}{/if}
        {#if from}<span class="from">{t("samples.forSong", { title: from })}</span>{/if}
      </p>
    </div>
    <div class="bar">
      <input class="search" type="search" placeholder={t("samples.search")} bind:value={q} oninput={typed} aria-label={t("samples.search")} />
      <div class="seg" role="radiogroup" aria-label={t("samples.kind")}>
        {#each [["", t("samples.all")], ["loop", t("samples.loops")], ["shot", t("samples.shots")]] as [v, l]}
          <button role="radio" aria-checked={kind === v} class:on={kind === v} onclick={() => (kind = v)}>{l}</button>
        {/each}
      </div>
      <select bind:value={key} aria-label={t("samples.key")}>
        <option value="">{t("samples.anyKey")}</option>
        {#each keyOpts as k}<option value={k}>{k}</option>{/each}
      </select>
      <label class="bpm"><input type="number" min="40" max="240" placeholder={t("samples.bpm")} bind:value={bpm} aria-label={t("samples.bpm")} /></label>
      <select bind:value={used} aria-label={t("samples.used.label")} title={t("samples.used.tip")}>
        <option value="">{t("samples.used.any")}</option>
        <option value="used">{t("samples.used.used")}</option>
        <option value="unused">{t("samples.used.unused")}</option>
      </select>
      <button class="btn sm" class:on={showDupes} onclick={() => (showDupes = !showDupes)} title={t("samples.dupes.tip")}>{t("samples.dupes.button")}</button>
      {#if filtered}<button class="btn sm" onclick={clear}>{t("samples.clear")}</button>{/if}
    </div>
  </header>

  {#if noDirs}
    <div class="empty">
      <p>{t("samples.noDirs")}</p>
      <a class="btn" href="#/settings">{t("samples.toSettings")}</a>
    </div>
  {:else if page && page.all === 0}
    <div class="empty"><p>{app.scan?.running ? t("samples.indexing") : t("samples.none")}</p></div>
  {:else}
    <div class="layout">
      <nav class="cats" aria-label={t("samples.families")}>
        <button class="fam" class:on={!family} onclick={() => { family = ""; cat = ""; }}>{t("samples.everything")} <span class="n">{fmtNum(total)}</span></button>
        {#each FAMILIES as [f, cs]}
          {@const n = count(f)}
          <button class="fam" class:on={family === f && !cat} disabled={!n && family !== f} onclick={() => pickFamily(f)} title={t(`samples.famTip.${f}`)}>{t(`samples.fam.${f}`)} <span class="n">{fmtNum(n)}</span></button>
          {#if family === f}
            <!-- the family's loops and one-shots: one track of three, like the other switches;
                 only when it holds both (a drum loop or a demo is never a one-shot) -->
            {@const nl = count(f, cat || undefined, "loop")}
            {@const ns = count(f, cat || undefined, "shot")}
            {#if (nl > 0 && ns > 0) || kind}
              <div class="split" role="radiogroup" aria-label={t("samples.kind")}>
                {#each [["", t("samples.all")], ["loop", t("samples.loops")], ["shot", t("samples.shots")]] as [v, l]}
                  {@const m = count(f, cat || undefined, v)}
                  {#if !v || m > 0 || kind === v}
                    <button role="radio" aria-checked={kind === v} class:on={kind === v} onclick={() => (kind = v)}>
                      <span>{l}</span><span class="sn">{fmtNum(m)}</span>
                    </button>
                  {/if}
                {/each}
              </div>
            {/if}
          {/if}
          {#if family === f && cs.length > 1}
            {#each cs as c}
              {@const m = count(f, c)}
              <button class="sub" class:on={cat === c} disabled={!m} onclick={() => pickCat(f, c)}>{t(`samples.cat.${c}`)} <span class="n">{fmtNum(m)}</span></button>
            {/each}
          {/if}
        {/each}
        <button class="fam" class:on={family === "none"} disabled={!count("none")} onclick={() => pickFamily("none")} title={t("samples.noneTip")}>{t("samples.fam.none")} <span class="n">{fmtNum(count("none"))}</span></button>
        {#if count("none")}
          <button class="sub quickbtn" class:on={quick} onclick={() => (quick = !quick)} title={t("samples.quick.tip")}>{t("samples.quick.open")}</button>
        {/if}
        {#if favsOf("sample").length}
          <button class="fam" class:on={favOnly} onclick={() => (favOnly = !favOnly)}>★ {t("common.fav.title")} <span class="n">{fmtNum(favsOf("sample").length)}</span></button>
        {/if}
        {#each APART as f}
          {#if count(f)}
            <button class="fam" class:on={family === f} onclick={() => pickFamily(f)} title={t(`samples.${f}Tip`)}>{t(`samples.fam.${f}`)} <span class="n">{fmtNum(count(f))}</span></button>
          {/if}
        {/each}
        <p class="small faint hint">{t("samples.howTip")}</p>
        {#if au?.enabled && au.samples_todo}
          <p class="small faint hint">{au.running && au.eta_s
            ? t("samples.listening", { n: fmtNum(au.samples_todo), time: aboutTime(au.eta_s) })
            : t("samples.toListen", { n: fmtNum(au.samples_todo) })}</p>
        {/if}
      </nav>

      <div class="list" class:stale={loading && rows.length > 0 && !loadingMore} aria-busy={loading} role="listbox" aria-label={t("samples.title")}>
        {#if error}<p class="small warn">{error}</p>{/if}
        {#if alikeErr}<p class="small warn">{alikeErr}</p>{/if}
        {#if showDupes}
          <SampleDupes close={() => (showDupes = false)} />
        {:else if quick}
          <SampleAsk close={() => { quick = false; load(); }} />
        {:else}
        {#if near}
          <div class="nearhead">
            <div><b>{t("samples.alikeOf", { name: near.of.name })}</b>
              <div class="small muted">{t("samples.compared", { n: fmtNum(Number(near.compared)) })}</div></div>
            <button class="btn sm" onclick={() => { near = null; sel = -1; }}>{t("samples.back")}</button>
          </div>
        {/if}
        {#if loading && !rows.length}<Skeleton kind="list" rows={10} />{/if}
        {#each shown as r, i (r.path)}
          <div class="row" id="smp-{i}" data-media={r.path} data-kind="audio" role="option" aria-selected={sel === i} class:sel={sel === i} class:on={playing(r)}
            draggable="true" title={t("samples.dragTip")}
            ondragstart={(e) => { e.preventDefault(); api.dragOut(r.path).catch((x) => app.say(String(x), { error: true })); }}>
            <!-- its shape: hit, pad, riser, loop at a glance -->
            <button class="wv" onclick={() => play(i)} tabindex="-1" aria-hidden="true"><SampleWave path={r.path} progress={playing(r) && player.len ? player.t / player.len : null} /></button>
            <button class="pp" onclick={() => play(i)} aria-label={playing(r) ? t("samples.pause") : t("samples.play")}>
              {#if playing(r)}<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
              {:else}<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
            </button>
            <button class="nm" onclick={() => play(i)} title={r.path}>
              <span class="name">{r.name}</span>
              <span class="folder small faint">{r.folder}</span>
            </button>
            <span class="tags small">
              <FavStar kind="sample" id={r.path} label={r.name} sub={r.folder} size={13} />
              {#if r.cat || (r.family && !APART.includes(r.family) && r.cat_src)}
                <button class="tag" class:heard={r.cat_src === "audio" || r.cat_src === "neighbours"} class:mine={r.cat_src === "user"} title="{srcTip(r.cat_src)}. {t('samples.sort.tip')}" onclick={() => (sortOf = sortOf === r.path ? null : r.path)}>{t(r.cat ? `samples.cat.${r.cat}` : `samples.fam.${r.family}`)}</button>
              {:else}
                <button class="tag add" title={t("samples.sort.tip")} onclick={() => (sortOf = sortOf === r.path ? null : r.path)}>{r.guess ? t("samples.sort.maybe", { what: t(r.guess_cat && r.guess_cat !== r.guess ? `samples.cat.${r.guess_cat}` : `samples.fam.${r.guess}`) }) : t("samples.sort.button")}</button>
              {/if}
              {#if r.is_loop !== null}<span class="faint">{r.is_loop ? t("samples.loop") : t("samples.shot")}</span>{/if}
              {#if r.used}<button class="use" class:open={useOf === r.path} title={t("samples.used.rowTip")} onclick={() => toggleUse(r)}>{t("samples.used.inN", { n: r.used })}</button>{/if}
            </span>
            <span class="num small">{r.bpm ? t("common.bpm", { n: Math.round(r.bpm * 10) / 10 }) : ""}</span>
            {#if !r.key && !r.note && r.notes}
              <span class="num small heard notes" title={t("samples.notesHeard", { notes: r.notes })}>{r.notes}</span>
            {:else}
              <span class="num small" class:heard={r.note_heard && !r.key} title={r.note_heard && !r.key ? t("samples.noteHeard") : undefined}>{r.key ?? r.note ?? ""}</span>
            {/if}
            <span class="num small faint">{near ? simWord(sims.get(r.path) ?? 0) : dur(r.dur)}</span>
            <span class="acts">
              <button class="icon" title={t("dialogs.board.send")} aria-label={t("dialogs.board.send")} onclick={() => (ui.toBoard = [r.path])}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="3" /><path d="M8 3v5M3 8h5M12 12v6M9 15h6" /></svg></button>
              <button class="icon" title={t("samples.alikeTip")} aria-label={t("samples.alikeTip")} disabled={nearOf === r.path} onclick={() => alike(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="9" cy="12" r="5" /><circle cx="15" cy="12" r="5" /></svg></button>
              <button class="icon" title={t("samples.reveal")} aria-label={t("samples.reveal")} onclick={() => reveal(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg></button>
              <button class="icon" title={t("samples.copy")} aria-label={t("samples.copy")} onclick={() => copy(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg></button>
            </span>
          </div>
          {#if sortOf === r.path}
            <SampleSortMenu row={r} close={() => (sortOf = null)} done={sorted} />
          {/if}
          {#if useOf === r.path}
            <div class="uses small">{t("samples.used.inSongs")}
              {#each useSongs as id, k (id)}{k ? ", " : " "}<a href="#/song/{id}">{songTitle(id)}</a>{/each}
            </div>
          {/if}
        {:else}
          {#if page && !loading}<p class="small muted pad">{t("samples.nothing")}</p>{/if}
        {/each}
        {#if !near && page && rows.length < page.total}
          <button class="btn more" disabled={loading} onclick={() => load(true)}>{#if loadingMore}<span class="spin" aria-hidden="true"></span>{t("samples.loadingMore")}{:else}{t("samples.more", { n: fmtNum(Math.min(PAGE, page.total - rows.length)) })}{/if}</button>
        {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 80px; display: flex; flex-direction: column; gap: 18px; min-height: 0; }
  .head { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 14px; }
  .head h1 { margin: 0 0 4px; }
  .head p { margin: 0; }
  .from { margin-left: 8px; padding: 1px 8px; border-radius: 6px; background: var(--accent-bg); color: var(--ink); }
  .bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .search { width: 240px; }
  .bar input, .bar select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 6px 10px; font: inherit; }
  .bpm input { width: 86px; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: var(--panel); color: var(--muted); padding: 6px 12px; cursor: pointer; font: inherit; transition: background-color .15s, color .15s; }
  .seg button.on { background: var(--accent-bg); color: var(--ink); }
  .layout { display: grid; grid-template-columns: 210px minmax(0, 1fr); gap: 24px; align-items: start; }
  .cats { position: sticky; top: 0; display: flex; flex-direction: column; gap: 2px; }
  .cats button { display: flex; justify-content: space-between; gap: 8px; border: 0; background: none; color: var(--ink); text-align: left; padding: 6px 10px; border-radius: 8px; cursor: pointer; font: inherit; transition: background-color .15s; }
  .cats button:hover:not(:disabled) { background: var(--panel); }
  .cats button.on { background: var(--accent-bg); font-weight: 600; }
  .cats button:disabled { color: var(--faint); cursor: default; }
  .cats .sub { padding-left: 24px; font-size: 13px; }
  .split { display: grid; grid-auto-flow: column; grid-auto-columns: 1fr; gap: 2px; margin: 2px 0 6px 14px; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .cats .split button { flex-direction: column; align-items: center; justify-content: center; gap: 0; padding: 4px 2px; border-radius: 7px; font-size: 11.5px; line-height: 1.25; color: var(--muted); font-weight: 400; }
  .cats .split button:hover:not(:disabled) { background: var(--panel); color: var(--ink); }
  .cats .split button.on { background: var(--panel); color: var(--ink); font-weight: 600; box-shadow: 0 1px 2px rgb(0 0 0 / .12); }
  .split .sn { font-size: 10.5px; color: var(--faint); font-variant-numeric: tabular-nums; font-weight: 400; }
  .cats .n { color: var(--faint); font-variant-numeric: tabular-nums; font-weight: 400; }
  .cats .hint { margin: 12px 10px 0; line-height: 1.45; }
  .list { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); overflow: hidden; }
  .use { border: 0; background: var(--accent-bg); color: var(--ink); border-radius: 6px; padding: 0 6px; cursor: pointer; font: inherit; white-space: nowrap; }
  .use.open, .use:hover { background: var(--accent); color: var(--on-accent); }
  .uses { padding: 4px 12px 8px 124px; color: var(--muted); }
  .bar .btn.on { border-color: var(--accent); color: var(--ink); }
  .row { display: grid; grid-template-columns: 56px 32px minmax(0, 1fr) auto 70px 64px 48px auto; align-items: center; gap: 12px; padding: 5px 12px; border-top: 1px solid var(--line); }
  .row:first-child { border-top: 0; }
  .wv { border: 0; background: none; padding: 0; cursor: pointer; }
  .row:hover { background: var(--panel2); }
  .row[draggable="true"] { cursor: pointer; }
  .row.sel { background: var(--accent-bg); }
  .pp { width: 28px; height: 28px; border-radius: 50%; border: 1px solid var(--line); background: var(--bg); color: var(--ink); display: grid; place-items: center; cursor: pointer; transition: border-color .15s; }
  .row.on .pp { border-color: var(--ink); }
  .nm { display: flex; flex-direction: column; min-width: 0; border: 0; background: none; color: var(--ink); text-align: left; cursor: pointer; padding: 0; font: inherit; }
  .name, .folder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tags { display: flex; gap: 8px; align-items: center; white-space: nowrap; }
  .tag { border: 1px solid var(--line); border-radius: 6px; padding: 0 6px; background: none; color: var(--ink); font: inherit; cursor: pointer; transition: border-color .15s; }
  .tag:hover { border-color: var(--accent); }
  /* sorted by ear or by its folder, not by name: dashed; by the user: filled */
  .tag.heard { border-style: dashed; }
  .tag.mine { background: var(--accent-bg); }
  .tag.add { border-style: dashed; color: var(--faint); }
  .quickbtn { color: var(--accent) !important; }
  .num { font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
  /* a note heard, not read in the name */
  .num.heard { color: var(--muted); text-decoration: underline dotted; text-underline-offset: 3px; }
  .num.notes { overflow: hidden; text-overflow: ellipsis; font-size: 11.5px; letter-spacing: -.01em; }
  .acts { display: flex; gap: 2px; opacity: 0; transition: opacity .15s; }
  .icon:disabled { opacity: .4; cursor: progress; }
  .row:hover .acts, .row.sel .acts, .acts:focus-within { opacity: 1; }
  .icon { border: 0; background: none; color: var(--muted); padding: 4px; border-radius: 6px; cursor: pointer; }
  .icon:hover { color: var(--ink); background: var(--panel); }
  .more { margin: 10px auto; }
  .list { transition: opacity .12s; }
  .list.stale { opacity: .5; }
  .more .spin { margin: 0 8px 0 0; }
  .nearhead { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 10px 12px; background: var(--accent-bg); }
  .pad { padding: 14px; }
  .empty { padding: 40px 0; display: flex; flex-direction: column; align-items: flex-start; gap: 12px; color: var(--muted); }
  .warn { color: var(--lost); padding: 10px 14px; }
  @media (max-width: 900px) {
    .layout { grid-template-columns: 1fr; }
    .cats { position: static; flex-direction: row; flex-wrap: wrap; }
    .cats .hint { display: none; }
  }
</style>

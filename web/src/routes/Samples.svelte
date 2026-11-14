<script lang="ts">
  // The samples of the sample folders: by family and category (from their name and folders),
  // loops or one-shots, in a key or at a tempo (a song's, from its sheet: #/samples?key=Am&bpm=174).
  // One page at a time from the base; a click plays, the arrows move and play the next one.
  import * as api from "../lib/api";
  import type { SamplePage, SampleRow, SampleQuery } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { player } from "../lib/player.svelte";
  import { t, fmtNum } from "../lib/i18n/index.svelte";

  const FAMILIES: [string, string[]][] = [
    ["drums", ["kick", "snare", "clap", "hihat", "cymbal", "tom", "perc"]],
    ["drumloop", ["drumloop"]],
    ["bass", ["808", "bass"]],
    ["melodic", ["lead", "pad", "pluck", "keys", "guitar"]],
    ["vocal", ["vocal"]],
    ["fx", ["fx"]],
  ];
  const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
  const KEYS = [...NOTES.map((n) => n + "m"), ...NOTES];
  const PAGE = 200;

  // filters, from the address when a song sends here
  const params = new URLSearchParams(location.hash.split("?")[1] ?? "");
  let q = $state("");
  let family = $state("");
  let cat = $state("");
  let kind = $state("");
  let key = $state(params.get("key") ?? "");
  let bpm = $state(params.get("bpm") ?? "");
  const from = params.get("from") ?? "";

  // a song's key written otherwise ("Eb", "G phrygien") stays shown as chosen
  const keyOpts = $derived(!key || KEYS.includes(key) ? KEYS : [key, ...KEYS]);
  let page = $state<SamplePage | null>(null);
  let rows = $state<SampleRow[]>([]);
  let loading = $state(false);
  let error = $state("");
  let sel = $state(-1);
  let seq = 0;

  const query = (offset: number): SampleQuery => ({
    q, family: family || null, cat: cat || null, kind: kind || null, key: key || null,
    bpm: bpm && +bpm > 0 ? +bpm : null, offset, limit: PAGE,
  });
  async function load(more = false) {
    const n = ++seq;
    loading = true;
    try {
      const p = await api.samples(query(more ? rows.length : 0));
      if (n !== seq) return;
      page = p;
      rows = more ? [...rows, ...p.rows] : p.rows;
      if (!more) sel = -1;
      error = "";
    } catch (e) {
      if (n === seq) error = String(e);
    }
    if (n === seq) loading = false;
  }
  // typing waits a little; the other filters reload at once
  let timer = 0;
  $effect(() => {
    void [family, cat, kind, key, bpm, app.version];
    clearTimeout(timer);
    load();
  });
  function typed() {
    clearTimeout(timer);
    timer = window.setTimeout(() => load(), 220);
  }

  const count = (f: string, c?: string) =>
    (page?.counts ?? []).filter((x) => (f === "none" ? x.family === null : x.family === f) && (c === undefined || x.cat === c)).reduce((a, x) => a + x.n, 0);
  const total = $derived((page?.counts ?? []).reduce((a, x) => a + x.n, 0));
  function pickFamily(f: string) {
    if (family === f && !cat) family = "";
    else family = f;
    cat = "";
  }
  function pickCat(f: string, c: string) {
    family = f;
    cat = cat === c ? "" : c;
  }

  const playing = (r: SampleRow) => player.playing && player.src === api.mediaUrl(r.path);
  function play(i: number) {
    const r = rows[i];
    if (!r) return;
    sel = i;
    if (playing(r)) return player.toggle();
    player.play(r.path, null, r.name);
  }
  function keys(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && e.target.matches("input, select, textarea");
    if (typing || !rows.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const i = Math.max(0, Math.min(rows.length - 1, sel + (e.key === "ArrowDown" ? 1 : -1)));
      play(i);
      document.getElementById("smp-" + i)?.scrollIntoView({ block: "nearest" });
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
          <button class="fam" class:on={family === f && !cat} disabled={!n} onclick={() => pickFamily(f)}>{t(`samples.fam.${f}`)} <span class="n">{fmtNum(n)}</span></button>
          {#if family === f && cs.length > 1}
            {#each cs as c}
              {@const m = count(f, c)}
              <button class="sub" class:on={cat === c} disabled={!m} onclick={() => pickCat(f, c)}>{t(`samples.cat.${c}`)} <span class="n">{fmtNum(m)}</span></button>
            {/each}
          {/if}
        {/each}
        <button class="fam" class:on={family === "none"} disabled={!count("none")} onclick={() => pickFamily("none")} title={t("samples.noneTip")}>{t("samples.fam.none")} <span class="n">{fmtNum(count("none"))}</span></button>
        <p class="small faint hint">{t("samples.howTip")}</p>
      </nav>

      <div class="list" role="listbox" aria-label={t("samples.title")}>
        {#if error}<p class="small warn">{error}</p>{/if}
        {#each rows as r, i (r.path)}
          <div class="row" id="smp-{i}" role="option" aria-selected={sel === i} class:sel={sel === i} class:on={playing(r)}>
            <button class="pp" onclick={() => play(i)} aria-label={playing(r) ? t("samples.pause") : t("samples.play")}>
              {#if playing(r)}<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
              {:else}<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M8 5.5v13l11-6.5z" /></svg>{/if}
            </button>
            <button class="nm" onclick={() => play(i)} title={r.path}>
              <span class="name">{r.name}</span>
              <span class="folder small faint">{r.folder}</span>
            </button>
            <span class="tags small">
              {#if r.cat}<span class="tag" title={r.cat_src === "folder" ? t("samples.fromFolder") : t("samples.fromName")}>{t(`samples.cat.${r.cat}`)}</span>{/if}
              {#if r.is_loop !== null}<span class="faint">{r.is_loop ? t("samples.loop") : t("samples.shot")}</span>{/if}
            </span>
            <span class="num small">{r.bpm ? t("common.bpm", { n: Math.round(r.bpm * 10) / 10 }) : ""}</span>
            <span class="num small">{r.key ?? r.note ?? ""}</span>
            <span class="num small faint">{dur(r.dur)}</span>
            <span class="acts">
              <button class="icon" title={t("samples.reveal")} aria-label={t("samples.reveal")} onclick={() => reveal(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg></button>
              <button class="icon" title={t("samples.copy")} aria-label={t("samples.copy")} onclick={() => copy(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg></button>
            </span>
          </div>
        {:else}
          {#if page && !loading}<p class="small muted pad">{t("samples.nothing")}</p>{/if}
        {/each}
        {#if page && rows.length < page.total}
          <button class="btn more" disabled={loading} onclick={() => load(true)}>{t("samples.more", { n: fmtNum(Math.min(PAGE, page.total - rows.length)) })}</button>
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
  .cats .n { color: var(--faint); font-variant-numeric: tabular-nums; font-weight: 400; }
  .cats .hint { margin: 12px 10px 0; line-height: 1.45; }
  .list { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); overflow: hidden; }
  .row { display: grid; grid-template-columns: 32px minmax(0, 1fr) auto 70px 44px 48px auto; align-items: center; gap: 12px; padding: 5px 12px; border-top: 1px solid var(--line); }
  .row:first-child { border-top: 0; }
  .row:hover { background: var(--panel2); }
  .row.sel { background: var(--accent-bg); }
  .pp { width: 28px; height: 28px; border-radius: 50%; border: 1px solid var(--line); background: var(--bg); color: var(--ink); display: grid; place-items: center; cursor: pointer; transition: border-color .15s; }
  .row.on .pp { border-color: var(--ink); }
  .nm { display: flex; flex-direction: column; min-width: 0; border: 0; background: none; color: var(--ink); text-align: left; cursor: pointer; padding: 0; font: inherit; }
  .name, .folder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tags { display: flex; gap: 8px; align-items: center; white-space: nowrap; }
  .tag { border: 1px solid var(--line); border-radius: 6px; padding: 0 6px; }
  .num { font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
  .acts { display: flex; gap: 2px; opacity: 0; transition: opacity .15s; }
  .row:hover .acts, .row.sel .acts, .acts:focus-within { opacity: 1; }
  .icon { border: 0; background: none; color: var(--muted); padding: 4px; border-radius: 6px; cursor: pointer; }
  .icon:hover { color: var(--ink); background: var(--panel); }
  .more { margin: 10px auto; }
  .pad { padding: 14px; }
  .empty { padding: 40px 0; display: flex; flex-direction: column; align-items: flex-start; gap: 12px; color: var(--muted); }
  .warn { color: var(--lost); padding: 10px 14px; }
  @media (max-width: 900px) {
    .layout { grid-template-columns: 1fr; }
    .cats { position: static; flex-direction: row; flex-wrap: wrap; }
    .cats .hint { display: none; }
  }
</style>

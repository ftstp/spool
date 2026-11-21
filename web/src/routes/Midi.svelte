<script lang="ts">
  import Skeleton from "../lib/components/Skeleton.svelte";
  // The MIDI files of the projects and the MIDI folders: by kind, in a key (or its relative),
  // at a tempo (half and double too), with their chords. From a song's sheet:
  // #/midi?key=Am&bpm=174. One page at a time from the base; a file is shown in Explorer, to
  // drag into Live from there.
  import * as api from "../lib/api";
  import type { MidiPage, MidiRow } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { t, fmtNum } from "../lib/i18n/index.svelte";
  import { midi, SOUNDS, type Choice } from "../lib/midiPlayer.svelte";
  import { ui } from "../lib/contextMenus.svelte";
  import { onDestroy } from "svelte";
  import { browse } from "../lib/player.svelte";

  const KINDS = ["chords", "bass", "melody", "drums", "multi"];
  const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
  const KEYS = [...NOTES.map((n) => n + "m"), ...NOTES];
  const PAGE = 200;

  const params = new URLSearchParams(location.hash.split("?")[1] ?? "");
  let q = $state("");
  let kind = $state(params.get("kind") ?? "");
  /** "clips": the MIDI written in the projects; "files": the MIDI files; "": both. */
  let source = $state(params.get("source") ?? "clips");
  // a clip's projects, opened under its row
  let usesOf = $state<string | null>(null);
  let uses = $state<import("../lib/api/bindings/ClipUse").ClipUse[]>([]);
  async function toggleUses(r: MidiRow) {
    if (usesOf === r.rel) return (usesOf = null);
    usesOf = r.rel;
    uses = [];
    uses = await api.clipUses(r.rel.slice(5)).catch(() => []);
  }
  let key = $state(params.get("key") ?? "");
  let bpm = $state(params.get("bpm") ?? "");
  const from = params.get("from") ?? "";
  const keyOpts = $derived(!key || KEYS.includes(key) ? KEYS : [key, ...KEYS]);

  let page = $state<MidiPage | null>(null);
  let rows = $state<MidiRow[]>([]);
  let loading = $state(false);
  let error = $state("");
  let seq = 0;
  async function load(more = false) {
    const n = ++seq;
    loading = true;
    try {
      const p = await api.midiPage({ q, kind: kind || null, key: key || null, bpm: bpm && +bpm > 0 ? +bpm : null, source: source || null, offset: more ? rows.length : 0, limit: PAGE });
      if (n !== seq) return;
      page = p;
      rows = more ? [...rows, ...p.rows] : p.rows;
      error = "";
    } catch (e) {
      if (n === seq) error = String(e);
    }
    if (n === seq) loading = false;
  }
  let timer = 0;
  $effect(() => {
    void [kind, key, bpm, source, app.version];
    clearTimeout(timer);
    load();
  });
  function typed() {
    clearTimeout(timer);
    timer = window.setTimeout(() => load(), 220);
  }
  const count = (k: string) => (page?.counts ?? []).filter((c) => c.label === k).reduce((a, c) => a + Number(c.n), 0);
  const total = $derived((page?.counts ?? []).reduce((a, c) => a + Number(c.n), 0));
  const filtered = $derived(!!(q || kind || key || bpm));
  function clear() {
    q = ""; kind = ""; key = ""; bpm = "";
  }
  const reveal = (r: MidiRow) => api.reveal(r.rel).catch((e) => app.say(String(e), { error: true }));
  async function copy(r: MidiRow) {
    try {
      await navigator.clipboard.writeText(r.rel.replace(/\//g, "\\"));
      app.say(t("midi.copied"));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // leaving the page lets what plays go on (the player bar keeps it, stopped from there)
  // the BPM typed here (or the song's, from its sheet) is the tempo the MIDI can lock to
  $effect(() => midi.setTarget(+bpm > 0 ? +bpm : null));
  // dragged as heard: transposed / at the tempo chosen here, as a new file (the original
  // is never changed); as it is otherwise
  const changed = $derived(midi.semis !== 0 || (midi.lock && !!midi.target) || Math.abs(midi.rate - 1) > 0.001);
  function drag(r: MidiRow) {
    // a clip is always written out (it is no file on disk), named after its song and track
    const p = changed || r.clip
      ? api.midiExport(r.rel, midi.semis, midi.rate, midi.lock ? midi.target : null, r.kind === "drums")
      : !/^[A-Za-z]:|^\//.test(r.rel) ? api.absPath(r.rel) : Promise.resolve(r.rel);
    p.then((x) => api.dragOut(x)).catch((x) => app.say(String(x), { error: true }));
  }
  // a click on a row plays it (again: stops), as on the Samples page; the arrows move to the
  // next one and play it. Dragging the row into Live still works.
  let sel = $state(-1);
  function playAt(i: number) {
    const r = rows[i];
    if (!r) return;
    sel = i;
    midi.play(r.rel, r.kind ?? "", r.name);
    browse.set(r.rel, i, rows.length, step);
  }
  // previous / next, from the arrows or the player bar
  function step(d: number) {
    const i = Math.max(0, Math.min(rows.length - 1, sel + d));
    if (i === sel) return;
    playAt(i);
    document.getElementById("mid-" + i)?.scrollIntoView({ block: "nearest" });
  }
  onDestroy(() => browse.clear(step));
  function keys(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && e.target.matches("input, select, textarea");
    if (typing || !rows.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      step(e.key === "ArrowDown" ? 1 : -1);
    }
  }
  const bars = (b: number) => (b ? t("midi.bars", { n: Math.round(b * 4) / 4 }) : "");
</script>

<svelte:window onkeydown={keys} />

<div class="page">
  <header class="head">
    <div>
      <h1>{t("midi.title")}</h1>
      <p class="small muted">
        {#if page}{source === "clips"
          ? filtered ? t("midi.clipCountOf", { n: fmtNum(Number(page.total)), all: fmtNum(Number(page.all)) }) : t("midi.clipCount", { n: fmtNum(Number(page.all)) })
          : filtered ? t("midi.countOf", { n: fmtNum(Number(page.total)), all: fmtNum(Number(page.all)) }) : t("midi.count", { n: fmtNum(Number(page.all)) })}{/if}
        {#if from}<span class="from">{t("midi.forSong", { title: from })}</span>{/if}
      </p>
    </div>
    <div class="bar">
      <div class="seg" role="radiogroup" aria-label={t("midi.source.label")}>
        {#each [["clips", t("midi.source.clips")], ["files", t("midi.source.files")], ["", t("midi.source.all")]] as [v, l]}
          <button role="radio" aria-checked={source === v} class:on={source === v} title={v === "clips" ? t("midi.source.clipsTip") : undefined} onclick={() => (source = v)}>{l}</button>
        {/each}
      </div>
      <input class="search" type="search" placeholder={source === "clips" ? t("midi.searchClips") : t("midi.search")} bind:value={q} oninput={typed} aria-label={t("midi.search")} />
      <select bind:value={key} aria-label={t("midi.key")}>
        <option value="">{t("midi.anyKey")}</option>
        {#each keyOpts as k}<option value={k}>{k}</option>{/each}
      </select>
      <label class="bpm"><input type="number" min="40" max="240" placeholder={t("midi.bpm")} bind:value={bpm} aria-label={t("midi.bpm")} /></label>
      {#if filtered}<button class="btn sm" onclick={clear}>{t("midi.clear")}</button>{/if}
      <label class="sound" title={t("midi.player.soundTip")}>
        <span class="small muted">{t("midi.player.sound")}</span>
        <select value={midi.sound} onchange={(e) => midi.setSound(e.currentTarget.value as Choice)} aria-label={t("midi.player.sound")}>
          {#each SOUNDS as c}<option value={c}>{t(`midi.player.${c}`)}</option>{/each}
        </select>
      </label>
      <button class="btn sm" class:on={midi.loop} aria-pressed={midi.loop} title={t("midi.player.loopTip")} onclick={() => (midi.loop = !midi.loop)}>{t("midi.player.loop")}</button>
    </div>
    <div class="bar play-opts">
      <span class="small muted">{t("midi.player.transpose")}</span>
      <span class="semis" title={t("midi.player.transposeTip")}>
        <button aria-label={t("midi.player.down")} onclick={() => midi.setSemis(midi.semis - 1)}>−</button>
        <button class="v" class:changed={midi.semis !== 0} onclick={() => midi.setSemis(0)}>{midi.semis > 0 ? "+" : ""}{midi.semis}</button>
        <button aria-label={t("midi.player.up")} onclick={() => midi.setSemis(midi.semis + 1)}>+</button>
      </span>
    </div>
  </header>

  {#if page && Number(page.all) === 0 && source === "clips"}
    <div class="empty"><p>{app.scan?.running ? t("midi.clipsReading") : t("midi.clipsNone")}</p></div>
  {:else if page && Number(page.all) === 0}
    <div class="empty"><p>{app.scan?.running ? t("midi.indexing") : t("midi.none")}</p><a class="btn" href="#/settings">{t("midi.toSettings")}</a></div>
  {:else}
    <div class="layout">
      <nav class="cats" aria-label={t("midi.kinds")}>
        <button class:on={!kind} onclick={() => (kind = "")}>{t("midi.everything")} <span class="n">{fmtNum(total)}</span></button>
        {#each KINDS as k}
          {@const n = count(k)}
          <button class:on={kind === k} disabled={!n} onclick={() => (kind = kind === k ? "" : k)}>{t(`midi.kind.${k}`)} <span class="n">{fmtNum(n)}</span></button>
        {/each}
        <p class="small faint hint">{t("midi.howTip")}</p>
      </nav>
      <div class="list">
        {#if error}<p class="small warn">{error}</p>{/if}
        {#if midi.error}<p class="small warn">{midi.error}</p>{/if}
        {#if loading && !rows.length}<Skeleton kind="list" rows={10} />{/if}
        {#each rows as r, i (r.rel)}
          {@const on = midi.is(r.rel)}
          <div class="row" id="mid-{i}" data-media={r.clip ? undefined : r.rel} data-kind="midi" data-song={r.clip && r.song ? r.song : undefined} class:on class:sel={sel === i} draggable="true" title={changed ? t("midi.dragChanged") : t("midi.dragTip")}
            ondragstart={(e) => { e.preventDefault(); drag(r); }}>
            <button class="play" class:on aria-label={on ? t("midi.player.stop") : t("midi.player.play")} title={on ? t("midi.player.stop") : t("midi.player.play")}
              onclick={() => playAt(i)} ondragstart={(e) => e.stopPropagation()}>
              {#if on}<svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5" /></svg>
              {:else}<svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
            </button>
            {#if on && midi.len}<span class="prog" style="width:{(midi.t / midi.len) * 100}%" aria-hidden="true"></span>{/if}
            <button class="nm" title={r.rel} onclick={() => playAt(i)}>
              <span class="name">{r.name}{#if r.in_project && !r.clip}<span class="mine small">{t("midi.mine")}</span>{/if}</span>
              <span class="folder small faint">{r.folder}</span>
            </button>
            <span class="tag small">{t(`midi.kind.${r.kind || "multi"}`)}</span>
            <span class="chords small" title={r.chords.join(" · ")}>{r.chords.slice(0, 4).join(" · ")}</span>
            <span class="num small">{r.key ?? ""}</span>
            <span class="num small">{r.bpm ? t("common.bpm", { n: Math.round(r.bpm) }) : ""}</span>
            <span class="num small faint">{bars(r.bars)}</span>
            <span class="acts">
              {#if r.clip}
                {#if r.song}<a class="icon songlink" href="#/song/{r.song}" title={t("midi.openSong")} aria-label={t("midi.openSong")}>
                  <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" /></svg></a>{/if}
                <button class="uses" class:open={usesOf === r.rel} title={t("midi.usesTip")} onclick={() => toggleUses(r)}>{t("midi.inProjects", { n: r.projects })}</button>
              {:else}
              <button class="icon" title={t("dialogs.board.send")} aria-label={t("dialogs.board.send")} onclick={() => (ui.toBoard = [r.rel])}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="3" /><path d="M8 3v5M3 8h5M12 12v6M9 15h6" /></svg></button>
              <button class="icon" title={t("midi.reveal")} aria-label={t("midi.reveal")} onclick={() => reveal(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg></button>
              <button class="icon" title={t("midi.copy")} aria-label={t("midi.copy")} onclick={() => copy(r)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg></button>
              {/if}
            </span>
          </div>
          {#if usesOf === r.rel}
            <ul class="uselist small">
              {#each [...new Map(uses.map((u) => [u.rel, u])).values()] as u (u.rel)}
                <li title={u.rel}>{#if u.song}<a href="#/song/{u.song}">{u.title || u.rel.split("/").pop()}</a>{:else}<span>{u.rel.split("/").pop()}</span>{/if}
                  <span class="faint"> · {u.rel.split("/").slice(-2).join("/")}</span></li>
              {/each}
            </ul>
          {/if}
        {:else}
          {#if page && !loading}<p class="small muted pad">{t("midi.nothing")}</p>{/if}
        {/each}
        {#if page && rows.length < Number(page.total)}
          <button class="btn more" disabled={loading} onclick={() => load(true)}>{t("midi.more", { n: fmtNum(Math.min(PAGE, Number(page.total) - rows.length)) })}</button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 80px; display: flex; flex-direction: column; gap: 18px; }
  .head { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 14px; }
  .head h1 { margin: 0 0 4px; }
  .head p { margin: 0; }
  .from { margin-left: 8px; padding: 1px 8px; border-radius: 6px; background: var(--accent-bg); color: var(--ink); }
  .bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .search { width: 240px; }
  .bar input, .bar select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 6px 10px; font: inherit; }
  .bpm input { width: 86px; }
  .layout { display: grid; grid-template-columns: 210px minmax(0, 1fr); gap: 24px; align-items: start; }
  .cats { position: sticky; top: 0; display: flex; flex-direction: column; gap: 2px; }
  .cats button { display: flex; justify-content: space-between; gap: 8px; border: 0; background: none; color: var(--ink); text-align: left; padding: 6px 10px; border-radius: 8px; cursor: pointer; font: inherit; transition: background-color .15s; }
  .cats button:hover:not(:disabled) { background: var(--panel); }
  .cats button.on { background: var(--accent-bg); font-weight: 600; }
  .cats button:disabled { color: var(--faint); cursor: default; }
  .cats .n { color: var(--faint); font-variant-numeric: tabular-nums; font-weight: 400; }
  .cats .hint { margin: 12px 10px 0; line-height: 1.45; }
  .list { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); overflow: hidden; }
  .row { position: relative; display: grid; grid-template-columns: 26px minmax(0, 1fr) auto minmax(0, 220px) 52px 64px 60px auto; align-items: center; gap: 12px; padding: 6px 12px 6px 8px; border-top: 1px solid var(--line); }
  .row.on { background: var(--accent-bg); }
  .prog { position: absolute; left: 0; bottom: 0; height: 2px; background: var(--accent); pointer-events: none; }
  .play { width: 26px; height: 26px; display: grid; place-items: center; border: 0; border-radius: 50%; background: none; color: var(--muted); cursor: pointer; transition: background-color .15s, color .15s; }
  .play:hover { background: var(--panel); color: var(--ink); }
  .play.on { background: var(--accent); color: var(--on-accent); }
  .play-opts { flex-basis: 100%; justify-content: flex-end; gap: 6px; }
  .play-opts .sep { margin-left: 10px; }
  .semis { display: inline-flex; align-items: center; gap: 2px; font-size: 12px; }
  .semis button { border: 0; background: none; color: var(--muted); font: inherit; cursor: pointer; border-radius: 6px; padding: 2px 6px; }
  .semis button:hover { color: var(--ink); background: var(--panel); }
  .semis .v { min-width: 30px; font-variant-numeric: tabular-nums; }
  .semis .v.changed { color: var(--ink); font-weight: 600; }
  .sound { display: inline-flex; align-items: center; gap: 6px; }
  .bar .btn.on { border-color: var(--accent); color: var(--ink); }
  .row:first-child { border-top: 0; }
  .row:hover { background: var(--panel2); }
  .row[draggable="true"] { cursor: pointer; }
  .nm { display: flex; flex-direction: column; min-width: 0; border: 0; background: none; color: var(--ink); text-align: left; cursor: pointer; padding: 0; font: inherit; }
  .row.sel:not(.on) { background: var(--panel2); }
  .name, .folder, .chords { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mine { margin-left: 8px; padding: 0 6px; border-radius: 6px; background: var(--accent-bg); color: var(--muted); }
  .tag { border: 1px solid var(--line); border-radius: 6px; padding: 0 6px; white-space: nowrap; }
  .chords { color: var(--muted); font-variant-numeric: tabular-nums; }
  .num { font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: none; color: var(--muted); padding: 6px 10px; cursor: pointer; font: inherit; }
  .seg button + button { border-left: 1px solid var(--line); }
  .seg button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .uses { border: 0; background: var(--accent-bg); color: var(--ink); border-radius: 6px; padding: 1px 7px; cursor: pointer; font: inherit; font-size: 12px; white-space: nowrap; }
  .uses.open, .uses:hover { background: var(--accent); color: var(--on-accent); }
  .songlink { display: grid; place-items: center; }
  .uselist { list-style: none; margin: 0; padding: 4px 14px 10px 48px; display: grid; gap: 2px; color: var(--muted); border-top: 0; }
  .acts { display: flex; gap: 2px; opacity: 0; transition: opacity .15s; }
  .row:hover .acts, .acts:focus-within { opacity: 1; }
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
    .row { grid-template-columns: 26px minmax(0, 1fr) auto 52px auto; }
    .row .chords, .row .num:nth-of-type(n + 5) { display: none; }
  }
</style>

<script lang="ts">
  // The quick sort as questions (crates/core sample_ask): a folder's unsorted samples, with
  // the ones elsewhere that sound like them, answered at once. Spool says what it thinks (from
  // the samples you sorted that they sound like, or its own guess); Enter confirms, 1 to 8
  // picks the family. Untick a sample that does not belong: it stays to sort. After an answer,
  // what sounds like the samples just sorted is asked next, never sorted silently.
  // Under the question, the whole folder: what is sorted, what your songs use, and the samples
  // to delete (sent to the Recycle Bin, after a warning).
  import { onDestroy, onMount } from "svelte";
  import * as api from "../api";
  import type { SampleRow } from "../api";
  import { app } from "../state.svelte";
  import { player } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { FAMILIES, catsOf, onlyCat } from "../sampleFamilies";
  import SampleWave from "./SampleWave.svelte";

  let { close }: { close: () => void } = $props();

  /** One question: a folder's samples (`dir`), or the ones that sound like an answer just given. */
  type Q = { key: string; dir: string; rows: SampleRow[]; alike: SampleRow[]; guess: string | null; guessCat: string | null; guessN: number; like: string | null; follow: boolean };

  let queue = $state<Q[]>([]);
  let folders = $state(0);
  let unsorted = $state(0);
  let done = $state(0);
  let loading = $state(true);
  let busy = $state(false);
  let all = $state(0);
  const skipped = new Set<string>();
  // what was answered here, newest last (Ctrl+Z puts it back), and what Ctrl+Z took back
  // (each with the samples it sorted, and how many of the app's Ctrl+Z it took: one per category)
  type Done = { q: Q; paths: string[]; steps: number };
  let hist = $state<Done[]>([]);
  let fwd = $state<Done[]>([]);
  // the samples sorted here: dropped from the questions still queued
  const sorted = new Set<string>();
  const cur = $derived(queue[0] ?? null);
  // the samples of the question, in order (the folder's, then the alike ones)
  const items = $derived(cur ? [...cur.rows, ...cur.alike] : []);
  // ticked: answered together (all, at first)
  let off = $state(new Set<string>());
  const ticked = $derived(items.filter((r) => !off.has(r.path)));
  // the sample played, its index in `items`
  let at = $state(0);
  // the family picked, waiting for its category (or Enter: the family only)
  let fam = $state("");

  async function gauge() {
    try {
      all = (await api.samples({ q: "", family: null, cat: null, kind: null, key: null, bpm: null, used: null, offset: 0, limit: 1 })).all;
    } catch {
      /* the gauge waits */
    }
  }
  const share = $derived(all ? (all - unsorted) / all : 0);

  async function fill() {
    loading = true;
    try {
      const r = await api.sampleQuestions([...skipped]);
      folders = r.folders;
      unsorted = r.unsorted;
      const follows = queue.filter((q) => q.follow);
      queue = [...follows, ...r.items.map((x) => ({ key: x.dir, dir: x.dir, rows: x.rows, alike: x.alike, guess: x.guess, guessCat: x.guess_cat, guessN: Number(x.guess_n), like: x.like, follow: false }))];
    } catch (e) {
      app.say(String(e), { error: true });
    }
    loading = false;
  }
  onMount(() => {
    void fill();
    void gauge();
  });
  onDestroy(() => player.playing && player.toggle());

  // a new question: everything ticked, its first sample played, its folder shown below
  let shownKey = "";
  $effect(() => {
    if (cur && cur.key !== shownKey) {
      shownKey = cur.key;
      // ticked: the samples that lean to the answer proposed (or to nothing); the others wait
      // for a question of their own (a folder of basses and synths is not answered at once)
      const g = cur.guess;
      off = new Set(g && !cur.follow ? items.filter((r) => r.guess && r.guess !== g).map((r) => r.path) : []);
      fam = "";
      at = 0;
      play(0);
      void loadFolder(cur.dir || dirOf(items[0]?.path ?? ""));
    }
  });
  const dirOf = (p: string) => p.slice(0, p.lastIndexOf("/"));
  const leafDir = (d: string) => d.slice(d.lastIndexOf("/") + 1);
  const playingRow = (r: SampleRow) => player.playing && player.src === api.mediaUrl(r.path);
  function play(i: number) {
    const r = items[i];
    if (!r) return;
    at = i;
    player.play(r.path, null, r.name, 0, r.bpm);
    if (r.path && dirOf(r.path) !== folderDir) void loadFolder(dirOf(r.path));
    queueMicrotask(() => document.querySelector(`.ask [data-i="${i}"]`)?.scrollIntoView({ block: "nearest" }));
  }
  function tick(p: string) {
    const s = new Set(off);
    if (s.has(p)) s.delete(p);
    else s.add(p);
    off = s;
  }

  async function next() {
    queue = queue.slice(1).map((q) => ({ ...q, rows: q.rows.filter((r) => !sorted.has(r.path)), alike: q.alike.filter((r) => !sorted.has(r.path)) })).filter((q) => q.rows.length + q.alike.length > 0);
    fam = "";
    if (queue.length < 2) await fill();
  }
  // Spool's answer accepted: each sample keeps the category it leans to within the family
  // (a folder of hi-hats and snares: not all hi-hats); none: the family only
  const ownCat = (r: SampleRow, f: string) => (r.guess === f && r.guess_cat && catsOf(f).includes(r.guess_cat) ? r.guess_cat : onlyCat(f));
  const acceptCats = $derived(cur?.guess ? [...new Set(ticked.map((r) => ownCat(r, cur!.guess!)))] : []);
  const guessText = $derived.by(() => {
    if (!cur?.guess) return "";
    if (acceptCats.length <= 1) return label(cur.guess, acceptCats[0] ?? null);
    const cats = acceptCats.filter((c): c is string => !!c).slice(0, 3).map((c) => t(`samples.cat.${c}`).toLowerCase());
    return `${t(`samples.fam.${cur.guess}`)} (${cats.join(", ")}${acceptCats.length > 3 ? "…" : ""})`;
  });
  async function answer(f: string, c: string | null, accept = false) {
    if (!cur || busy) return;
    const paths = ticked.map((r) => r.path);
    if (!paths.length) return app.say(t("samples.ask.noneChecked"));
    busy = true;
    try {
      // one sorting per category (each a Ctrl+Z of the app)
      const groups = new Map<string | null, string[]>();
      for (const r of ticked) {
        const k = accept ? ownCat(r, f) : (c ?? onlyCat(f));
        groups.set(k, [...(groups.get(k) ?? []), r.path]);
      }
      let steps = 0;
      for (const [k, ps] of groups) {
        await api.sortSamples(ps, f, k);
        steps++;
      }
      const cat = groups.size === 1 ? [...groups.keys()][0] : null;
      for (const p of paths) sorted.add(p);
      hist = [...hist, { q: cur, paths, steps }];
      fwd = [];
      done += paths.length;
      unsorted = Math.max(0, unsorted - paths.length);
      // what sounds like them, asked next with the same answer proposed
      const more = (await api.samplesAlikeUnsorted(paths)).filter((r) => !sorted.has(r.path));
      await next();
      if (more.length) {
        const like = items.find((r) => paths.includes(r.path))?.name ?? null;
        queue = [{ key: `follow:${paths[0]}:${Date.now()}`, dir: "", rows: [], alike: more, guess: f, guessCat: cat, guessN: more.length, like, follow: true }, ...queue];
      }
      void gauge();
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
  async function skip() {
    if (!cur) return;
    if (cur.dir) skipped.add(cur.dir);
    await next();
  }
  // the app's own Ctrl+Z / Ctrl+Y (one entry per answer): the question comes back
  async function undo() {
    const h = hist[hist.length - 1];
    if (!h || busy) return;
    busy = true;
    for (let i = 0; i < h.steps; i++) await app.undoLast();
    hist = hist.slice(0, -1);
    fwd = [...fwd, h];
    for (const p of h.paths) sorted.delete(p);
    done = Math.max(0, done - h.paths.length);
    unsorted += h.paths.length;
    shownKey = "";
    queue = [h.q, ...queue.filter((x) => !x.follow || !x.alike.some((r) => h.paths.includes(r.path)))];
    busy = false;
  }
  async function redo() {
    const h = fwd[fwd.length - 1];
    if (!h || busy) return;
    busy = true;
    for (let i = 0; i < h.steps; i++) await app.redoLast();
    fwd = fwd.slice(0, -1);
    hist = [...hist, h];
    for (const p of h.paths) sorted.add(p);
    done += h.paths.length;
    queue = [];
    await fill();
    busy = false;
  }

  // the folder of the sample played, all of it, and what is selected there to delete
  let folderDir = $state("");
  let folder = $state<SampleRow[]>([]);
  let pick = $state(new Set<string>());
  let confirming = $state<string[] | null>(null);
  async function loadFolder(d: string) {
    if (!d) return;
    folderDir = d;
    try {
      const rows = await api.sampleFolder(d);
      if (folderDir === d) {
        folder = rows;
        pick = new Set([...pick].filter((p) => rows.some((r) => r.path === p)));
      }
    } catch {
      folder = [];
    }
  }
  const playingDir = $derived(items[at] ? dirOf(items[at].path) : folderDir);
  function togglePick(p: string) {
    const s = new Set(pick);
    if (s.has(p)) s.delete(p);
    else s.add(p);
    pick = s;
  }
  const allPicked = $derived(folder.length > 0 && folder.every((r) => pick.has(r.path)));
  const confirmUsed = $derived((confirming ?? []).filter((p) => (folder.find((r) => r.path === p)?.used ?? 0) > 0).length);
  async function trash() {
    const paths = confirming;
    if (!paths?.length) return;
    busy = true;
    try {
      if (player.playing && paths.some((p) => player.src === api.mediaUrl(p))) player.toggle();
      const n = await api.trashSamples(paths);
      app.say(t("samples.ask.deleted", { n }));
      for (const p of paths) sorted.add(p);
      unsorted = Math.max(0, unsorted - paths.filter((p) => items.some((r) => r.path === p)).length);
      pick = new Set();
      confirming = null;
      // the question without them (gone, or emptied: the next one)
      queue = queue.map((q) => ({ ...q, rows: q.rows.filter((r) => !paths.includes(r.path)), alike: q.alike.filter((r) => !paths.includes(r.path)) })).filter((q) => q.rows.length + q.alike.length > 0);
      if (!queue.length) await fill();
      await loadFolder(folderDir);
      void gauge();
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }

  function keys(e: KeyboardEvent) {
    if (e.target instanceof HTMLElement && e.target.matches("input, select, textarea")) return;
    if (confirming) {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        confirming = null;
      }
      return;
    }
    const k = e.key;
    if ((e.ctrlKey || e.metaKey) && (k.toLowerCase() === "z" || k.toLowerCase() === "y")) {
      e.preventDefault();
      e.stopPropagation();
      if (k.toLowerCase() === "y" || e.shiftKey) void redo();
      else void undo();
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    let used = true;
    if (k === "Escape") close();
    else if (k === "Enter") {
      if (fam) answer(fam, null);
      else if (cur?.guess) answer(cur.guess, null, true);
    } else if (k === "s" || k === "S" || k === "ArrowRight") skip();
    else if (k === "Backspace") undo();
    else if (k === " ") player.toggle();
    else if ((k === "ArrowDown" || k === "ArrowUp") && items.length) play((at + (k === "ArrowDown" ? 1 : -1) + items.length) % items.length);
    else if ((k === "x" || k === "X") && items[at]) tick(items[at].path);
    else if (/^[1-9]$/.test(k) && FAMILIES[+k - 1]) {
      const f = FAMILIES[+k - 1][0];
      if (catsOf(f).length) fam = f;
      else answer(f, null);
    } else used = false;
    if (used) e.preventDefault();
  }
  const label = (f: string | null, c: string | null) => (f ? t(c && c !== f ? `samples.cat.${c}` : `samples.fam.${f}`) : "");
  const rowTag = (r: SampleRow) => (r.family ? label(r.family, r.cat) : t("samples.ask.unsorted"));
  const usedText = (r: SampleRow) => (r.used ? t("samples.used.inN", { n: r.used }) : t("samples.ask.unused"));
  const last = $derived(hist[hist.length - 1] ?? null);
  const qTitle = (q: Q) => (q.follow ? t("samples.ask.follow", { n: q.alike.length }) : leafDir(q.dir));
</script>

<svelte:window onkeydowncapture={keys} />

<section class="ask" aria-label={t("samples.ask.title")}>
  <header>
    <div>
      <b>{t("samples.ask.title")}</b>
      <div class="small muted">{t("samples.ask.left", { q: fmtNum(folders), n: fmtNum(unsorted) })}{#if done}{" · "}{t("samples.ask.done", { n: done })}{/if}</div>
    </div>
    <button class="btn sm" onclick={close}>{t("samples.quick.stop")}</button>
  </header>
  {#if all}
    <div class="gauge" title={t("samples.quick.gaugeTip", { done: fmtNum(all - unsorted), all: fmtNum(all) })}>
      <div class="track"><i style:width="{(share * 100).toFixed(2)}%"></i></div>
      <span class="small"><b>{t("samples.quick.gaugePct", { p: (share * 100).toFixed(share < 0.1 ? 1 : 0) })}</b></span>
    </div>
  {/if}

  {#if cur}
    <div class="q card">
      <div class="qh">
        <div class="who">
          <div class="name">{qTitle(cur)}</div>
          <div class="small faint path">{cur.follow ? t("samples.ask.followSub") : cur.dir.split("/").slice(-3, -1).join(" / ")}</div>
        </div>
        {#if items[at]}<button class="btn sm" onclick={() => api.reveal(items[at].path).catch((e) => app.say(String(e), { error: true }))} title={t("samples.ask.openTip")}>{t("samples.ask.open")}</button>{/if}
      </div>

      <div class="answer">
        {#if cur.guess}
          <button class="guess" disabled={busy || !ticked.length} onclick={() => cur?.guess && answer(cur.guess, null, true)} title={t("samples.quick.enter")}>
            {t("samples.ask.guess", { what: guessText })} <kbd>{t("samples.quick.enterKey")}</kbd>
          </button>
          <span class="small muted">{#if cur.like}{t("samples.ask.like", { name: cur.like })} · {/if}{t("samples.ask.guessN", { n: cur.guessN, all: items.length })}</span>
        {:else}
          <span class="small muted">{t("samples.ask.noGuess")}</span>
        {/if}
        <span class="small n">{t("samples.ask.answerN", { n: ticked.length })}</span>
      </div>
      <div class="fams">
        {#each FAMILIES as [f], i}
          <button class="chip" class:on={fam === f} disabled={busy || !ticked.length} onclick={() => (catsOf(f).length ? (fam = f) : answer(f, null))} title={t(`samples.famTip.${f}`)}><kbd>{i + 1}</kbd> {t(`samples.fam.${f}`)}</button>
        {/each}
        <button class="chip plain" disabled={busy} onclick={skip}><kbd>S</kbd> {t("samples.quick.skip")}</button>
      </div>
      {#if fam}
        <div class="fams">
          {#each catsOf(fam) as c}
            <button class="chip" disabled={busy} onclick={() => answer(fam, c)}>{t(`samples.cat.${c}`)}</button>
          {/each}
          <button class="chip plain" disabled={busy} onclick={() => answer(fam, null)}><kbd>{t("samples.quick.enterKey")}</kbd> {t("samples.sort.familyOnly")}</button>
        </div>
      {/if}

      <!-- the samples the answer sorts: ticked, each heard with a click -->
      {#snippet list(rows: SampleRow[], from: number, showDir: boolean)}
        {#each rows as r, j (r.path)}
          {@const i = from + j}
          <div class="it" class:on={at === i} class:out={off.has(r.path)} data-i={i}>
            <input type="checkbox" checked={!off.has(r.path)} onchange={() => tick(r.path)} aria-label={r.name} />
            <button class="wv" onclick={() => play(i)} tabindex="-1" aria-hidden="true"><SampleWave path={r.path} progress={playingRow(r) && player.len ? player.t / player.len : null} /></button>
            <button class="nm" onclick={() => play(i)} title={r.path}>
              <span class="ell">{r.name}</span>
              {#if showDir}<span class="small faint ell">{r.folder}</span>{/if}
            </button>
            <span class="small faint">{r.guess ? label(r.guess, r.guess_cat) : ""}</span>
            <span class="small" class:faint={!r.used} class:used={r.used}>{usedText(r)}</span>
          </div>
        {/each}
      {/snippet}
      <div class="lists">
        {#if cur.rows.length}
          <h4 class="small muted">{t("samples.ask.toSort", { n: cur.rows.length })}</h4>
          {@render list(cur.rows, 0, false)}
        {/if}
        {#if cur.alike.length}
          <h4 class="small muted" title={t("samples.ask.alikeTip")}>{cur.follow ? t("samples.ask.toSort", { n: cur.alike.length }) : t("samples.ask.alike", { n: cur.alike.length })}</h4>
          {@render list(cur.alike, cur.rows.length, true)}
        {/if}
      </div>
      <p class="small faint keys">{t("samples.ask.keys")}{#if last}{" · "}<button class="link" onclick={undo}>{t("samples.quick.undo", { name: qTitle(last.q) })}</button>{/if}{#if fwd.length}{" · "}<button class="link" onclick={redo}>{t("samples.quick.redo", { name: qTitle(fwd[fwd.length - 1].q) })}</button>{/if}</p>
    </div>

    <!-- the folder of the sample played: everything in it, used or not, to delete -->
    {#if folder.length}
      <div class="folder card">
        <div class="fh">
          <b class="small" title={t("samples.ask.inFolderTip")}>{t("samples.ask.inFolder", { n: folder.length })} <span class="faint">· {leafDir(playingDir)}</span></b>
          <span class="grow"></span>
          {#if pick.size}<span class="small muted">{t("samples.ask.selected", { n: pick.size })}</span>{/if}
          <button class="btn sm" onclick={() => (pick = allPicked ? new Set() : new Set(folder.map((r) => r.path)))}>{allPicked ? t("samples.ask.selectNone") : t("samples.ask.selectAll")}</button>
          <button class="btn sm danger" disabled={!pick.size || busy} onclick={() => (confirming = [...pick])}>{t("samples.ask.delete")}</button>
        </div>
        {#if confirming}
          <div class="confirm" role="alertdialog" aria-label={t("samples.ask.confirmTitle", { n: confirming.length })}>
            <b>{t("samples.ask.confirmTitle", { n: confirming.length })}</b>
            <p class="small">{t("samples.ask.confirmText")}</p>
            {#if confirmUsed}<p class="small warn">{t("samples.ask.confirmUsed", { n: confirmUsed })}</p>{/if}
            <ul class="small faint names">{#each confirming.slice(0, 8) as p (p)}<li class="ell">{p.slice(p.lastIndexOf("/") + 1)}</li>{/each}{#if confirming.length > 8}<li>…</li>{/if}</ul>
            <div class="row2">
              <button class="btn sm danger" disabled={busy} onclick={trash}>{t("samples.ask.confirmGo")}</button>
              <button class="btn sm" onclick={() => (confirming = null)}>{t("samples.ask.cancel")}</button>
            </div>
          </div>
        {/if}
        <div class="frows">
          {#each folder as r (r.path)}
            <div class="fr" class:on={playingRow(r)} class:sel={pick.has(r.path)}>
              <input type="checkbox" checked={pick.has(r.path)} onchange={() => togglePick(r.path)} aria-label={r.name} />
              <button class="wv" onclick={() => player.play(r.path, null, r.name, 0, r.bpm)} tabindex="-1" aria-hidden="true"><SampleWave path={r.path} progress={playingRow(r) && player.len ? player.t / player.len : null} /></button>
              <button class="nm" onclick={() => player.play(r.path, null, r.name, 0, r.bpm)} title={r.path}><span class="ell">{r.name}</span></button>
              <span class="small" class:faint={!r.family}>{rowTag(r)}</span>
              <span class="small" class:faint={!r.used} class:used={r.used}>{usedText(r)}</span>
              <button class="icon del" title={t("samples.ask.deleteOne")} aria-label={t("samples.ask.deleteOne")} disabled={busy} onclick={() => (confirming = [r.path])}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3" /></svg>
              </button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {:else if loading}
    <p class="small muted pad">{t("samples.quick.loading")}</p>
  {:else}
    <p class="pad">{done ? t("samples.quick.finished", { n: fmtNum(done) }) : t("samples.quick.empty")}</p>
  {/if}
</section>

<style>
  .ask { display: flex; flex-direction: column; gap: 14px; padding: 14px 16px 16px; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .card { border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg); padding: 14px; }
  .q { display: flex; flex-direction: column; gap: 12px; }
  .qh { display: flex; align-items: flex-start; gap: 12px; }
  .who { flex: 1; min-width: 0; }
  .name { font-weight: 600; font-size: 15px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .path { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .answer { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .answer .n { margin-left: auto; color: var(--muted); }
  .guess { border: 1px solid var(--accent); background: var(--accent-bg); color: var(--ink); border-radius: 8px; padding: 7px 14px; cursor: pointer; font: inherit; font-weight: 600; white-space: nowrap; transition: background-color .15s; }
  .guess:hover:not(:disabled) { background: var(--accent); color: var(--on-accent); }
  .fams { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 999px; padding: 4px 12px; cursor: pointer; font: inherit; transition: background-color .15s, border-color .15s; }
  .chip:hover:not(:disabled) { border-color: var(--accent); }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .chip.plain { border-style: dashed; }
  .chip:disabled, .guess:disabled { opacity: .5; cursor: default; }
  kbd { font: 11px/1 ui-monospace, monospace; padding: 2px 4px; border: 1px solid var(--line); border-radius: 4px; color: var(--muted); }
  .lists { display: flex; flex-direction: column; max-height: 340px; overflow: auto; border-top: 1px solid var(--line); }
  h4 { margin: 10px 0 4px; font-weight: 600; position: sticky; top: 0; background: var(--bg); padding: 2px 0; z-index: 1; }
  .it, .fr { display: grid; grid-template-columns: 18px 56px minmax(0, 1fr) auto 7.5em; gap: 10px; align-items: center; padding: 3px 6px; border-radius: 6px; }
  .fr { grid-template-columns: 18px 56px minmax(0, 1fr) auto 7.5em 24px; }
  .it.on, .fr.on { background: var(--accent-bg); }
  .it.out { opacity: .45; }
  .fr.sel { background: color-mix(in srgb, var(--bad) 9%, transparent); }
  .wv, .nm { border: 0; background: none; padding: 0; cursor: pointer; color: var(--ink); font: inherit; text-align: left; }
  .nm { display: flex; flex-direction: column; min-width: 0; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .used { color: var(--ok); text-align: right; }
  .it > :last-child, .fr > span:nth-last-child(2) { text-align: right; }
  .keys { margin: 0; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); cursor: pointer; font: inherit; text-decoration: underline; }
  .folder { display: flex; flex-direction: column; gap: 8px; }
  .fh { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .grow { flex: 1; }
  .frows { max-height: 320px; overflow: auto; }
  .icon { border: 0; background: none; color: var(--muted); cursor: pointer; display: grid; place-items: center; padding: 3px; border-radius: 5px; opacity: 0; transition: opacity .15s, color .15s; }
  .fr:hover .icon, .icon:focus-visible { opacity: 1; }
  .del:hover { color: var(--bad); }
  .danger { color: var(--bad); border-color: color-mix(in srgb, var(--bad) 45%, var(--line)); }
  .danger:hover:not(:disabled) { background: var(--bad); color: #fff; }
  .confirm { border: 1px solid color-mix(in srgb, var(--bad) 55%, var(--line)); background: color-mix(in srgb, var(--bad) 8%, var(--bg)); border-radius: 8px; padding: 12px 14px; display: flex; flex-direction: column; gap: 6px; }
  .confirm p { margin: 0; }
  .warn { color: var(--bad); font-weight: 600; }
  .names { margin: 0; padding-left: 18px; }
  .row2 { display: flex; gap: 8px; margin-top: 4px; }
  .pad { padding: 8px 0; margin: 0; }
  .gauge { display: flex; align-items: center; gap: 10px; }
  .gauge .track { flex: 1; height: 8px; border-radius: 99px; background: var(--line); overflow: hidden; }
  .gauge i { display: block; height: 100%; border-radius: 99px; background: linear-gradient(90deg, var(--accent), var(--ok)); transition: width .5s cubic-bezier(0.2, 0.8, 0.2, 1); }
  @media (prefers-reduced-motion: reduce) { .gauge i { transition: none; } }
  .gauge > span { white-space: nowrap; }
</style>

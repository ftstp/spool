<script lang="ts">
  // The quick sort: the unsorted samples one at a time, or a whole folder at a time (a few of
  // its samples to listen to, then all of them sorted at once), played as they come, with what
  // Spool guesses. One key confirms the guess (Enter), names the family (1 to 8) or skips (S);
  // a click on a category settles it more precisely. Backspace undoes the last one.
  import { onDestroy, onMount } from "svelte";
  import * as api from "../api";
  import type { SampleRow } from "../api";
  import type { FolderToSort } from "../api/bindings/FolderToSort";
  import { app } from "../state.svelte";
  import { player } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { FAMILIES, catsOf, onlyCat } from "../sampleFamilies";

  let { close }: { close: () => void } = $props();

  /** One thing to sort: a sample, or a folder's unsorted samples. */
  type Item = {
    key: string;
    title: string;
    sub: string;
    paths: string[];
    examples: SampleRow[];
    guess: string | null;
    guessCat: string | null;
    /** "72 %" for a sample, "12 of 40" for a folder */
    guessHow: string;
  };
  const fromRow = (r: SampleRow): Item => ({
    key: r.path, title: r.name, sub: r.folder, paths: [r.path], examples: [r],
    guess: r.guess, guessCat: r.guess_cat, guessHow: `${Math.round((r.guess_conf ?? 0) * 100)} %`,
  });
  const fromFolder = (f: FolderToSort): Item => ({
    key: f.dir, title: f.dir.split("/").pop() ?? f.dir, sub: f.dir.split("/").slice(-3, -1).join(" / "), paths: f.paths, examples: f.examples,
    guess: f.guess, guessCat: null, guessHow: t("samples.quick.ofN", { n: fmtNum(f.guess_n), all: fmtNum(f.paths.length) }),
  });

  let byFolder = $state(false);
  let queue = $state<Item[]>([]);
  let left = $state(0);
  let done = $state(0);
  let loading = $state(true);
  let busy = $state(false);
  // the family picked, waiting for its category (or Enter: the family only)
  let fam = $state("");
  const skipped = new Set<string>();
  let last: Item | null = $state(null);
  const cur = $derived(queue[0] ?? null);
  // the example playing (folder: one of a few)
  let ex = $state(0);

  async function fill() {
    loading = true;
    try {
      if (byFolder) {
        const f = await api.unsortedFolders([...skipped]);
        queue = f.map(fromFolder);
        left = f.length;
      } else {
        const p = await api.samples({ q: "", family: "none", cat: null, kind: null, key: null, bpm: null, used: null, offset: skipped.size, limit: 100 });
        queue = p.rows.filter((r) => !skipped.has(r.path)).map(fromRow);
        left = p.total;
      }
    } catch (e) {
      app.say(String(e), { error: true });
    }
    loading = false;
  }
  onMount(fill);
  onDestroy(() => player.playing && player.toggle());
  function mode(f: boolean) {
    if (byFolder === f) return;
    byFolder = f;
    skipped.clear();
    last = null;
    queue = [];
    fill();
  }

  function play(i: number) {
    const r = cur?.examples[i];
    if (!r) return;
    ex = i;
    player.play(r.path, null, r.name, 0, r.bpm);
  }
  const playingEx = (r: SampleRow) => player.playing && player.src === api.mediaUrl(r.path);
  // each one plays as it comes
  let played = "";
  $effect(() => {
    if (cur && cur.key !== played) {
      played = cur.key;
      fam = "";
      play(0);
    }
  });

  async function next() {
    queue = queue.slice(1);
    fam = "";
    if (!queue.length) await fill();
  }
  async function sort(f: string, c: string | null) {
    if (!cur || busy) return;
    busy = true;
    try {
      await api.sortSamples(cur.paths, f, c ?? onlyCat(f));
      last = cur;
      done += cur.paths.length;
      left = Math.max(0, left - 1);
      await next();
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
  async function skip() {
    if (!cur) return;
    skipped.add(cur.key);
    await next();
  }
  async function undo() {
    if (!last || busy) return;
    busy = true;
    const r = last;
    last = null;
    try {
      await api.sortSamples(r.paths, null, null);
      done = Math.max(0, done - r.paths.length);
      left++;
      played = "";
      queue = [r, ...queue];
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
  function keys(e: KeyboardEvent) {
    if (e.target instanceof HTMLElement && e.target.matches("input, select, textarea")) return;
    const k = e.key;
    let used = true;
    if (k === "Escape") close();
    else if (k === "Enter") {
      if (fam) sort(fam, null);
      else if (cur?.guess) sort(cur.guess, cur.guessCat);
    } else if (k === "s" || k === "S" || k === "ArrowRight") skip();
    else if (k === "Backspace") undo();
    else if (k === " ") player.toggle();
    else if (k === "Tab" && cur && cur.examples.length > 1) play((ex + 1) % cur.examples.length);
    else if (/^[1-8]$/.test(k)) {
      const f = FAMILIES[+k - 1][0];
      if (catsOf(f).length) fam = f;
      else sort(f, null);
    } else used = false;
    if (used) e.preventDefault();
  }
  const guessText = $derived(
    cur?.guess ? t(cur.guessCat && cur.guessCat !== cur.guess ? `samples.cat.${cur.guessCat}` : `samples.fam.${cur.guess}`) : "",
  );
</script>

<svelte:window onkeydown={keys} />

<section class="quick" aria-label={t("samples.quick.title")}>
  <header>
    <div>
      <b>{t("samples.quick.title")}</b>
      <div class="small muted">{byFolder
        ? t("samples.quick.progressFolders", { done: fmtNum(done) })
        : t("samples.quick.progress", { done: fmtNum(done), left: fmtNum(left) })}</div>
    </div>
    <div class="seg" role="radiogroup" aria-label={t("samples.quick.mode")}>
      <button role="radio" aria-checked={!byFolder} class:on={!byFolder} onclick={() => mode(false)}>{t("samples.quick.oneByOne")}</button>
      <button role="radio" aria-checked={byFolder} class:on={byFolder} onclick={() => mode(true)} title={t("samples.quick.byFolderTip")}>{t("samples.quick.byFolder")}</button>
    </div>
    <button class="btn sm" onclick={close}>{t("samples.quick.stop")}</button>
  </header>

  {#if cur}
    <div class="card">
      <button class="pp" onclick={() => player.toggle()} aria-label={player.playing ? t("samples.pause") : t("samples.play")}>
        {#if player.playing}<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
        {:else}<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
      </button>
      <div class="who">
        <div class="name">{cur.title}{#if byFolder} <span class="count small muted">{t("samples.quick.nSamples", { n: fmtNum(cur.paths.length) })}</span>{/if}</div>
        <div class="small faint folder">{cur.sub}</div>
      </div>
      {#if cur.guess}
        <button class="guess" disabled={busy} onclick={() => cur?.guess && sort(cur.guess, cur.guessCat)} title={t("samples.quick.enter")}>
          {t("samples.quick.guess", { what: guessText, p: cur.guessHow })} <kbd>{t("samples.quick.enterKey")}</kbd>
        </button>
      {:else}
        <span class="small muted">{t("samples.quick.noGuess")}</span>
      {/if}
    </div>

    {#if byFolder && cur.examples.length > 1}
      <div class="examples">
        <span class="small muted">{t("samples.quick.listen")}</span>
        {#each cur.examples as r, i (r.path)}
          <button class="ex" class:on={playingEx(r)} onclick={() => play(i)} title={r.name}>{r.name}</button>
        {/each}
        <kbd>Tab</kbd>
      </div>
    {/if}

    <div class="fams">
      {#each FAMILIES as [f], i}
        <button class="chip" class:on={fam === f} disabled={busy} onclick={() => (catsOf(f).length ? (fam = f) : sort(f, null))}><kbd>{i + 1}</kbd> {t(`samples.fam.${f}`)}</button>
      {/each}
      <button class="chip plain" disabled={busy} onclick={skip}><kbd>S</kbd> {t("samples.quick.skip")}</button>
    </div>
    {#if fam}
      <div class="fams">
        {#each catsOf(fam) as c}
          <button class="chip" disabled={busy} onclick={() => sort(fam, c)}>{t(`samples.cat.${c}`)}</button>
        {/each}
        <button class="chip plain" disabled={busy} onclick={() => sort(fam, null)}><kbd>{t("samples.quick.enterKey")}</kbd> {t("samples.sort.familyOnly")}</button>
      </div>
    {/if}
    <p class="small faint keys">{t(byFolder ? "samples.quick.keysFolder" : "samples.quick.keys")}{#if last} · <button class="link" onclick={undo}>{t("samples.quick.undo", { name: last.title })}</button>{/if}</p>
  {:else if loading}
    <p class="small muted pad">{t("samples.quick.loading")}</p>
  {:else}
    <p class="pad">{done ? t("samples.quick.finished", { n: fmtNum(done) }) : t("samples.quick.empty")}</p>
  {/if}
</section>

<style>
  .quick { display: flex; flex-direction: column; gap: 14px; padding: 14px 16px 16px; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  header > div:first-child { margin-right: auto; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: var(--panel); color: var(--muted); padding: 5px 12px; cursor: pointer; font: inherit; transition: background-color .15s, color .15s; }
  .seg button.on { background: var(--accent-bg); color: var(--ink); }
  .card { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 14px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg); }
  .pp { width: 40px; height: 40px; border-radius: 50%; border: 1px solid var(--ink); background: var(--panel); color: var(--ink); display: grid; place-items: center; cursor: pointer; }
  .who { min-width: 0; }
  .name { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .count { font-weight: 400; margin-left: 6px; }
  .folder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .guess { border: 1px solid var(--accent); background: var(--accent-bg); color: var(--ink); border-radius: 8px; padding: 6px 12px; cursor: pointer; font: inherit; white-space: nowrap; transition: background-color .15s; }
  .guess:hover:not(:disabled) { background: var(--accent); color: var(--on-accent); }
  .examples { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .ex { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 3px 10px; cursor: pointer; font: inherit; font-size: 13px; max-width: 260px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: border-color .15s; }
  .ex:hover, .ex.on { border-color: var(--accent); }
  .ex.on { background: var(--accent-bg); }
  .fams { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 999px; padding: 4px 12px; cursor: pointer; font: inherit; transition: background-color .15s, border-color .15s; }
  .chip:hover:not(:disabled) { border-color: var(--accent); }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .chip.plain { border-style: dashed; }
  .chip:disabled, .guess:disabled { opacity: .5; cursor: progress; }
  kbd { font: 11px/1 ui-monospace, monospace; padding: 2px 4px; border: 1px solid var(--line); border-radius: 4px; color: var(--muted); }
  .keys { margin: 0; }
  .link { border: 0; background: none; padding: 0; color: var(--accent); cursor: pointer; font: inherit; text-decoration: underline; }
  .pad { padding: 8px 0; margin: 0; }
</style>

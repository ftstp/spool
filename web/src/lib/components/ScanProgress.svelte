<script lang="ts">
  // What the analysis is really doing, in the order it does it: the projects first, then what
  // the app learns from them, the MIDI files last. Each step done keeps what it found; the
  // step in progress shows how far, how long is left, and can be stopped (what was read is
  // kept, the rest is read at the next analysis).
  // `full`: the setup's and settings' large view, every step listed; otherwise the sidebar's.
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import frApp from "../i18n/locales/fr/app.json";
  import enApp from "../i18n/locales/en/app.json";

  let { full = false }: { full?: boolean } = $props();
  // The steps, in order. The core names the step in progress in its own words, in the
  // language it was last told: it is matched against both languages' names.
  const STEPS = ["find", "read", "samples", "plugins", "group", "temoins", "midiFind", "midiRead"] as const;
  type Step = (typeof STEPS)[number];
  const stepOf = (stage: string | undefined): Step | undefined =>
    stage ? STEPS.find((k) => frApp.scan.steps[k] === stage || enApp.scan.steps[k] === stage) : undefined;
  // what a step found, in words
  const FOUND: Partial<Record<Step, (n: number) => string>> = {
    find: (n) => t("app.scan.found.find", { n, num: fmtNum(n) }),
    read: (n) => t("app.scan.found.read", { n, num: fmtNum(n) }),
    midiFind: (n) => t("app.scan.found.midiFind", { n, num: fmtNum(n) }),
    midiRead: (n) => t("app.scan.found.midiRead", { n, num: fmtNum(n) }),
  };
  const s = $derived(app.scan);
  const pct = $derived(s && s.total ? Math.min(100, Math.round((s.done / s.total) * 100)) : null);
  const step = $derived(stepOf(s?.stage));
  const at = $derived(step ? STEPS.indexOf(step) : -1);
  const count = $derived(
    !s || !s.running ? "" : s.total ? `${fmtNum(s.done)} / ${fmtNum(s.total)}` : s.done ? ((step && FOUND[step]?.(s.done)) ?? fmtNum(s.done)) : "",
  );
  // the last part of what is being read: the end of a long path is what tells where it is
  const detail = $derived(s?.detail ? s.detail.replace(/\\/g, "/").split("/").slice(-3).join(" / ") : "");

  // What each step found, kept when the next one starts (cleared at a new analysis).
  // (keyed by step; the words are made when the step ends, in the language of that moment)
  let found = $state<Partial<Record<Step, string>>>({});
  let lastStage = "";
  let lastDone = 0;
  // the pace of the step in progress, for the time left
  let stageStart = 0;
  let stageDone0 = 0;
  $effect(() => {
    const st = s?.stage ?? "";
    const done = s?.done ?? 0;
    const lk = stepOf(lastStage);
    if (!s?.running) {
      if (lk && FOUND[lk]) found[lk] = FOUND[lk](lastDone);
      lastStage = "";
      return;
    }
    if (st !== lastStage) {
      if (!lastStage) found = {};
      else if (lk && FOUND[lk]) found[lk] = FOUND[lk](lastDone);
      lastStage = st;
      stageStart = Date.now();
      stageDone0 = done;
    }
    lastDone = done;
  });

  // The clock: the scan state object is replaced ~6 times a second while it runs, so the
  // timer hangs on a plain boolean (an effect reading `s` itself was torn down and set up
  // again before its first second: the clock stayed at 0 s).
  const running = $derived(!!s?.running);
  let started = $state(0);
  let now = $state(Date.now());
  $effect(() => {
    if (!running) {
      started = 0;
      return;
    }
    started = Date.now();
    now = started;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const elapsed = $derived(started ? Math.max(0, Math.round((now - started) / 1000)) : 0);
  const clock = (n: number) => (n < 60 ? `${n} s` : n < 3600 ? `${Math.floor(n / 60)} min ${String(n % 60).padStart(2, "0")}` : `${Math.floor(n / 3600)} h ${String(Math.floor((n % 3600) / 60)).padStart(2, "0")}`);
  // time left in the step: from its pace so far, once it has run a few seconds
  const left = $derived.by(() => {
    if (!s?.running || !s.total || s.done <= stageDone0) return "";
    const secs = (now - stageStart) / 1000;
    if (secs < 3) return "";
    const rate = (s.done - stageDone0) / secs;
    const rest = Math.round((s.total - s.done) / rate);
    return rest > 0 ? t("app.scan.left", { time: clock(rest) }) : "";
  });
  let stopping = $state(false);
  $effect(() => {
    if (!running) stopping = false;
  });
  async function stop() {
    stopping = true;
    await api.cancelScan();
  }
</script>

{#if full}
  <div class="full">
    <ol class="steps">
      {#each STEPS as k, i}
        {@const done = s?.running ? at > i : (s?.last ?? 0) > 0 && !!found[k]}
        <li class:done class:on={s?.running && i === at}>
          <span class="dot" aria-hidden="true"></span>
          <span class="name">{t(`app.scan.steps.${k}`)}</span>
          {#if s?.running && i === at && count}<span class="cnt">{count}</span>{:else if found[k] && (done || !s?.running)}<span class="cnt">{found[k]}</span>{/if}
        </li>
      {/each}
    </ol>
    <div class="bar" class:indet={s?.running && pct === null} aria-hidden="true"><i style:width={pct === null ? "" : pct + "%"}></i></div>
    <div class="foot small">
      <span class="muted ell" title={s?.detail}>{s?.running ? detail || "..." : ""}</span>
      {#if s?.running}<span class="faint">{left ? `${left} · ` : ""}{clock(elapsed)}</span>{/if}
    </div>
    {#if s?.running}
      <div class="stop"><button class="btn sm" disabled={stopping} onclick={stop}>{stopping ? t("app.scan.stopping") : t("app.scan.cancel")}</button><span class="small faint">{t("app.scan.keptNote")}</span></div>
    {:else if s?.cancelled}
      <p class="small warn">{t("app.scan.stopped")}</p>
    {/if}
  </div>
{:else if s?.running}
  <div class="mini small">
    <div class="row"><span class="ell">{step ? t(`app.scan.steps.${step}`) : scanStageText(s.stage) || t("app.scan.analysis")}</span>{#if count}<span class="faint">{count}</span>{/if}</div>
    <div class="bar thin" class:indet={pct === null} aria-hidden="true"><i style:width={pct === null ? "" : pct + "%"}></i></div>
    <div class="row"><span class="faint ell" title={s.detail}>{left || detail}</span><button class="link" disabled={stopping} onclick={stop} title={t("app.scan.stopTitle")}>{stopping ? t("app.scan.stoppingShort") : t("app.scan.cancelShort")}</button></div>
  </div>
{/if}

<style>
  .steps { list-style: none; margin: 0 0 16px; padding: 0; display: flex; flex-direction: column; gap: 9px; }
  .steps li { display: flex; align-items: center; gap: 10px; color: var(--faint); }
  .steps li.on { color: var(--ink); font-weight: 600; }
  .steps li.done { color: var(--muted); }
  .dot { width: 10px; height: 10px; border-radius: 50%; border: 2px solid currentColor; flex: none; }
  .done .dot { background: var(--ok); border-color: var(--ok); }
  .on .dot { border-color: var(--accent); background: var(--accent); animation: pulse 1.2s ease-in-out infinite; }
  .cnt { margin-left: auto; font-variant-numeric: tabular-nums; color: var(--muted); font-weight: 400; }
  .bar { height: 6px; border-radius: 3px; background: var(--line); overflow: hidden; position: relative; }
  .bar.thin { height: 3px; margin: 4px 0 3px; }
  .bar i { display: block; height: 100%; background: var(--accent); border-radius: inherit; transition: width .3s ease-out; }
  .bar.indet i { width: 30%; position: absolute; animation: slide 1.4s ease-in-out infinite; }
  .foot { display: flex; justify-content: space-between; gap: 12px; margin-top: 8px; min-height: 18px; }
  .foot span:last-child { font-variant-numeric: tabular-nums; flex: none; }
  .stop { display: flex; align-items: center; gap: 12px; margin-top: 12px; flex-wrap: wrap; }
  .warn { margin: 12px 0 0; color: var(--s-draft); }
  /* the sidebar's version: never wider than the sidebar, whatever the step or the file read */
  .mini { min-width: 0; max-width: 100%; overflow: hidden; }
  .row { display: flex; justify-content: space-between; gap: 8px; min-width: 0; }
  .row span:last-child { font-variant-numeric: tabular-nums; flex: none; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; flex: none; font-size: inherit; }
  .link:disabled { color: var(--faint); cursor: default; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  @keyframes slide { from { left: -30%; } to { left: 100%; } }
  @keyframes pulse { 50% { opacity: .45; } }
  @media (prefers-reduced-motion: reduce) { .bar.indet i, .on .dot { animation: none; } .bar.indet i { left: 0; width: 100%; opacity: .4; } }
</style>

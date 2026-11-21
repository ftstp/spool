<script lang="ts">
  // The audio analysis in detail: what it listens to now (the reference renders, the samples
  // the ear sorts, the others for "sounds alike"), how far, how long is left at the speed
  // measured, how many copies of the model work, or why it waits; what it has done so far.
  // Shown from the sidebar's status (a click) and in Settings > Spool.
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { aboutTime } from "../format";

  const a = $derived(app.scan?.audio);
  const pct = $derived(a && a.total ? Math.min(100, Math.round((Number(a.done) / Number(a.total)) * 100)) : 0);
  const state = $derived(
    !a ? "" : !a.enabled ? "off" : a.paused ? "paused" : a.busy_wait ? "busy" : a.running ? "running" : Number(a.samples_todo) + Number(a.samples_rest) > 0 ? "waiting" : "done",
  );
  const file = $derived(a?.detail ? a.detail.replace(/\\/g, "/").split("/").slice(-2).join(" / ") : "");
</script>

{#if a}
  <div class="ad">
    <div class="top">
      <b>{t("app.audioDetails.title")}</b>
      <span class="chip {state}">{t(`app.audioDetails.state.${state || "off"}`)}</span>
    </div>
    {#if a.running}
      <div class="small">{t(`app.audioDetails.phase.${a.phase || "renders"}`)}</div>
      <div class="bar" aria-hidden="true"><i style:width="{pct}%"></i></div>
      <div class="row small">
        <span class="num">{fmtNum(Number(a.done))} / {fmtNum(Number(a.total))}</span>
        <span class="muted">{a.eta_s ? t("app.audioDetails.left", { time: aboutTime(Number(a.eta_s)) }) : t("app.audioDetails.measuring")}</span>
      </div>
      {#if file}<div class="small faint ell" title={a.detail}>{file}</div>{/if}
      <div class="small muted">
        {Number(a.copies) > 1 ? t("app.audioDetails.copies", { n: Number(a.copies) }) : t("app.audioDetails.oneCopy")}{a.ms_per_file ? " · " + t("app.audioDetails.speed", { ms: Number(a.ms_per_file) }) : ""}
      </div>
    {:else if state === "busy"}
      <div class="small muted">{t("app.audioDetails.busyWhy")}</div>
    {/if}
    <ul class="small facts">
      <li>{t("app.audioDetails.renders", { n: fmtNum(Number(a.analysed)), total: fmtNum(Number(a.renders)) })}</li>
      <li>{t("app.audioDetails.unsorted", { n: fmtNum(Number(a.samples_todo)) })}{Number(a.samples_done) ? " · " + t("app.audioDetails.sortedByEar", { n: fmtNum(Number(a.samples_done)) }) : ""}</li>
      <li>{t("app.audioDetails.heard", { n: fmtNum(Number(a.samples_heard)), rest: fmtNum(Number(a.samples_rest)) })}</li>
      {#if !a.running && Number(a.samples_todo) + Number(a.samples_rest) > 0 && a.ms_per_file}
        <li class="muted">{t("app.audioDetails.estimate", { time: aboutTime(((Number(a.samples_todo) + Number(a.samples_rest)) * Number(a.ms_per_file)) / 1000) })}</li>
      {/if}
    </ul>
    <div class="acts">
      {#if a.enabled && a.ready}
        <button class="btn sm" onclick={() => api.audioPause(!a.paused)}>{a.paused ? t("app.audioDetails.resume") : t("app.audioDetails.pause")}</button>
      {/if}
      <a class="btn sm" href="#/samples">{t("app.audioDetails.toSamples")}</a>
    </div>
  </div>
{/if}

<style>
  .ad { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  /* room on the right for the popover's close button */
  .top { display: flex; justify-content: space-between; align-items: center; gap: 10px; padding-right: 26px; }
  .chip { font-size: 11.5px; border-radius: 6px; padding: 1px 8px; border: 1px solid var(--line); color: var(--muted); white-space: nowrap; }
  .chip.running { color: var(--ok); border-color: currentColor; }
  .chip.busy, .chip.paused { color: var(--lost); border-color: currentColor; }
  .bar { height: 4px; border-radius: 2px; background: var(--line); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--ink); border-radius: 2px; transition: width .4s; }
  .row { display: flex; justify-content: space-between; gap: 10px; }
  .num { font-variant-numeric: tabular-nums; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .facts { margin: 0; padding-left: 16px; display: grid; gap: 2px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; }
  @media (prefers-reduced-motion: reduce) { .bar i { transition: none; } }
</style>

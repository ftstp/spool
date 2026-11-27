<script lang="ts">
  // The sidebar's status, in two lines that never hide each other: the long work going on in
  // the background (a full analysis, the listening to the sounds, however many files are
  // left, even while it waits for the computer or for Live), and the quick checks that keep
  // what the app shows up to date. A click anywhere opens the details.
  import { app } from "../state.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { ago, aboutTime } from "../format";
  import ScanProgress from "./ScanProgress.svelte";

  const s = $derived(app.scan);
  const a = $derived(s?.audio);
  const longScan = $derived(!!s?.running && !!s.full);
  const quick = $derived(!!s?.running && !s.full);
  // what the listening still has to do (renders, then samples), whether it runs right now or not
  const left = $derived(a && a.enabled && a.ready ? Math.max(0, Number(a.renders) - Number(a.analysed)) + Number(a.samples_todo) + Number(a.samples_rest) : 0);
  const audioState = $derived(!a || !left ? "" : a.paused ? "paused" : a.running ? (a.live ? "live" : "running") : a.busy_wait ? "busy" : "waiting");
  const audioPct = $derived(a && a.running && a.total ? Math.min(100, Math.round((Number(a.done) / Number(a.total)) * 100)) : null);
  const quickText = $derived(
    quick ? t("app.activity.checking") : app.busy ? t("app.status.busy") : s?.error ? t("app.status.error", { error: s.error }) : s?.last ? t("app.status.upToDate", { ago: ago(s.last) }) : "",
  );
  const open = () => (app.audioDetails = !app.audioDetails);
</script>

<div class="act" role="button" tabindex="0" aria-expanded={app.audioDetails} title={t("app.activity.open")}
  onclick={(e) => { if (!(e.target as HTMLElement).closest("button")) open(); }}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open())}>
  {#if longScan}
    <div class="kind small">{t("app.activity.long")}</div>
    <ScanProgress />
  {/if}
  {#if audioState}
    <div class="line small two" class:dim={audioState !== "running" && audioState !== "live"}>
      <i class="dot {audioState}"></i>
      <span class="wrap">{t(`app.activity.audio.${audioState}`)}</span>
    </div>
    {#if audioPct !== null}<div class="bar"><i style:width={audioPct + "%"}></i></div>{/if}
    <div class="faint small sub num">{t("app.activity.left", { n: fmtNum(left) })}{a && a.running && a.eta_s ? ` · ${aboutTime(Number(a.eta_s))}` : ""}</div>
  {/if}
  {#if quickText}<div class="line small quick" class:err={!!s?.error}>
    {#if quick || app.busy}<span class="spin"></span>{:else}<i class="dot ok"></i>{/if}
    <span class="ell">{quickText}</span>
    {#if s?.pending}<span class="faint">{t("app.activity.pending")}</span>{/if}
  </div>{/if}
</div>

<style>
  .act { display: grid; gap: 4px; min-width: 0; cursor: pointer; border-radius: 8px; }
  .act:focus-visible { outline: 2px solid var(--ink); outline-offset: 2px; }
  .kind { font-weight: 600; color: var(--ink); }
  .line { display: flex; align-items: center; gap: 7px; min-width: 0; color: var(--muted); }
  .line.dim { color: var(--faint); }
  .line.err { color: var(--bad); }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; flex: 1; }
  .line.two { align-items: flex-start; }
  .line.two .dot { margin-top: 4px; }
  .wrap { min-width: 0; flex: 1; line-height: 1.35; }
  .num { flex: none; font-variant-numeric: tabular-nums; }
  .sub { padding-left: 15px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--faint); }
  .dot.ok { background: var(--ok); }
  .dot.running, .dot.live { background: var(--accent); animation: pulse 1.2s ease-in-out infinite; }
  .dot.busy, .dot.waiting { background: var(--s-draft, #c98500); }
  .bar { height: 3px; border-radius: 2px; background: var(--line); overflow: hidden; margin-left: 15px; }
  .bar i { display: block; height: 100%; background: var(--accent); }
  .spin { width: 10px; height: 10px; border-radius: 50%; border: 2px solid var(--line); border-top-color: var(--accent); animation: spin .8s linear infinite; flex: none; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pulse { 50% { opacity: .45; } }
  @media (prefers-reduced-motion: reduce) { .dot, .spin { animation: none; } }
</style>

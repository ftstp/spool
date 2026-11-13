<script lang="ts">
  // Lines over time on one axis (never two): 2px lines, a hairline grid, the last value
  // labelled at the end of each line, a crosshair and every series' value on hover. A
  // legend for two series or more.
  import { show, hide } from "./Tip.svelte";
  import { t } from "../i18n/index.svelte";

  type Series = { label: string; color: string; values: number[] };
  let { xs, series, height = 180, unit = "" }: { xs: string[]; series: Series[]; height?: number; unit?: string | ((n: number) => string) } = $props();
  const W = 600;
  const val = (n: number) => (typeof unit === "function" ? unit(n) : `${n}${unit}`);
  const H = $derived(height);
  const PAD = { l: 34, r: 40, t: 10, b: 20 };
  const max = $derived(Math.max(1, ...series.flatMap((s) => s.values)));
  // clean ticks: 0 and three round steps
  const step = $derived((() => {
    const raw = max / 3;
    const p = 10 ** Math.floor(Math.log10(raw));
    return [1, 2, 2.5, 5, 10].map((m) => m * p).find((s) => s >= raw) ?? raw;
  })());
  const top = $derived(Math.ceil(max / step) * step);
  const ticks = $derived(Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step));
  const X = (i: number) => PAD.l + (xs.length < 2 ? 0 : (i / (xs.length - 1)) * (W - PAD.l - PAD.r));
  const Y = (v: number) => H - PAD.b - (v / top) * (H - PAD.t - PAD.b);
  const path = (v: number[]) => v.map((y, i) => `${i ? "L" : "M"}${X(i).toFixed(1)},${Y(y).toFixed(1)}`).join("");
  let at = $state<number | null>(null);
  let svg = $state<SVGSVGElement>();
  function move(e: MouseEvent) {
    if (!svg) return;
    const r = svg.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * W;
    const i = Math.round(((x - PAD.l) / (W - PAD.l - PAD.r)) * (xs.length - 1));
    at = Math.max(0, Math.min(xs.length - 1, i));
    show(e, xs[at], series.map((s) => t("stats.chart.series", { label: s.label, value: val(s.values[at!]) })).join(" · "));
  }
  const fmt = (n: number) => (n >= 10000 ? Math.round(n / 1000) + "K" : String(Math.round(n)));
</script>

{#if xs.length}
  <svg bind:this={svg} viewBox="0 0 {W} {H}" class="lines" role="img" aria-label={series.map((s) => t("stats.chart.series", { label: s.label, value: s.values.at(-1) })).join(", ")}
    onmousemove={move} onmouseleave={() => { at = null; hide(); }}>
    {#each ticks as tk}
      <line x1={PAD.l} x2={W - PAD.r} y1={Y(tk)} y2={Y(tk)} class="grid" />
      <text x={PAD.l - 6} y={Y(tk) + 3.5} class="tick" text-anchor="end">{fmt(tk)}</text>
    {/each}
    <text x={PAD.l} y={H - 4} class="tick">{xs[0]}</text>
    {#if xs.length > 1}<text x={W - PAD.r} y={H - 4} class="tick" text-anchor="end">{xs.at(-1)}</text>{/if}
    {#if at !== null}<line x1={X(at)} x2={X(at)} y1={PAD.t} y2={H - PAD.b} class="cross" />{/if}
    {#each series as s (s.label)}
      <path d={path(s.values)} stroke={s.color} class="ln" />
      <circle cx={X(s.values.length - 1)} cy={Y(s.values.at(-1) ?? 0)} r="4" fill={s.color} class="dot" />
      <text x={X(s.values.length - 1) + 8} y={Y(s.values.at(-1) ?? 0) + 4} class="end">{s.values.at(-1)}</text>
      {#if at !== null}<circle cx={X(at)} cy={Y(s.values[at])} r="4" fill={s.color} class="dot" />{/if}
    {/each}
  </svg>
  {#if series.length > 1}
    <div class="legend small">{#each series as s (s.label)}<span><i style:background={s.color}></i>{s.label}</span>{/each}</div>
  {/if}
{:else}
  <p class="small faint">{t("stats.chart.noHistory")}</p>
{/if}

<style>
  .lines { width: 100%; height: auto; display: block; overflow: visible; }
  .grid { stroke: var(--line); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .cross { stroke: var(--faint); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .tick { fill: var(--faint); font-size: 10.5px; font-variant-numeric: tabular-nums; }
  .end { fill: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .ln { fill: none; stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; vector-effect: non-scaling-stroke; }
  .dot { stroke: var(--panel); stroke-width: 2; }
  .legend { display: flex; flex-wrap: wrap; gap: 4px 14px; margin-top: 6px; color: var(--muted); }
  .legend span { display: inline-flex; align-items: center; gap: 6px; }
  .legend i { width: 10px; height: 3px; border-radius: 2px; }
</style>

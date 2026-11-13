<script lang="ts">
  // A ranked list as bars: label, a bar of one hue (magnitude), the value. Rows can link
  // somewhere (filter the songs, open a plugin). Bars are capped thin; the value is text.
  import type { Snippet } from "svelte";
  import { show, hide } from "./Tip.svelte";
  import { t } from "../i18n/index.svelte";

  type Row = { label: string; n: number; href?: string; note?: string };
  let { rows, unit = "", max, badge, pick }:
    { rows: Row[]; unit?: string | ((n: number) => string); max?: number; badge?: Snippet<[Row]>; pick?: (label: string) => void } = $props();
  const top = $derived(max ?? Math.max(1, ...rows.map((r) => r.n)));
  const val = (n: number) => (typeof unit === "function" ? unit(n) : `${n}${unit}`);
</script>

<div class="list">
  {#each rows as r (r.label)}
    <a class="row" href={r.href} class:link={!!r.href || !!pick} onclick={() => pick?.(r.label)} onmousemove={(e) => show(e, r.label, `${val(r.n)}${r.note ? " · " + r.note : ""}`)} onmouseleave={hide}>
      <span class="lab">{r.label}{#if badge}{@render badge(r)}{/if}</span>
      <span class="track"><i style:width="{Math.max(2, (r.n / top) * 100)}%"></i></span>
      <span class="val">{r.n}</span>
    </a>
  {/each}
  {#if !rows.length}<p class="small faint">{t("stats.chart.empty")}</p>{/if}
</div>

<style>
  .list { display: flex; flex-direction: column; gap: 1px; }
  .row { display: grid; grid-template-columns: minmax(0, 40%) 1fr 3.2em; align-items: center; gap: 10px; padding: 3px 4px; border-radius: 6px; color: var(--ink); }
  .row.link:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
  .lab { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .track { height: 8px; display: block; }
  .track i { display: block; height: 100%; background: var(--accent); border-radius: 0 4px 4px 0; opacity: .85; }
  .val { text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 12px; }
</style>

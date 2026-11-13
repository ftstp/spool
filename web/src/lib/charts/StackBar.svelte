<script lang="ts">
  // A horizontal stacked bar: parts of one whole (songs per stage, projects per state).
  // Segments are separated by a 2px gap of the surface, rounded only at the two ends.
  // Each segment names itself on hover; `legend` adds the legend under it (identity never
  // rests on colour alone).
  import { show, hide } from "./Tip.svelte";
  import { t } from "../i18n/index.svelte";

  type Part = { key: string; label: string; n: number; color: string; href?: string };
  let { parts, legend = false, height = 10, unit = "", pick }:
    { parts: Part[]; legend?: boolean; height?: number; unit?: string | ((n: number) => string); pick?: (key: string) => void } = $props();
  const shown = $derived(parts.filter((p) => p.n > 0));
  const total = $derived(shown.reduce((a, p) => a + p.n, 0));
  const pct = (n: number) => (total ? Math.round((n / total) * 100) : 0);
  const val = (n: number) => (typeof unit === "function" ? unit(n) : `${n}${unit}`);
</script>

<div class="bar" style:height="{height}px" role="img" aria-label={shown.map((p) => `${p.label} ${p.n}`).join(", ")}>
  {#each shown as p (p.key)}
    <a class="seg" href={p.href} onclick={() => pick?.(p.key)} style:flex-grow={p.n} style:background={p.color} aria-label="{p.label} {p.n}"
      onmousemove={(e) => show(e, p.label, `${val(p.n)} · ${t("stats.pct", { n: pct(p.n) })}`)} onmouseleave={hide}></a>
  {/each}
  {#if !total}<span class="empty"></span>{/if}
</div>
{#if legend && shown.length}
  <div class="legend small">
    {#each shown as p (p.key)}
      <a class="li" href={p.href} onclick={() => pick?.(p.key)}><i style:background={p.color}></i>{p.label} <span class="faint">{p.n}</span></a>
    {/each}
  </div>
{/if}

<style>
  .bar { display: flex; gap: 2px; width: 100%; }
  .seg { min-width: 3px; height: 100%; display: block; transition: filter .15s; }
  .seg:first-child { border-radius: 4px 0 0 4px; }
  .seg:last-child { border-radius: 0 4px 4px 0; }
  .seg:only-child { border-radius: 4px; }
  .seg:hover { filter: brightness(1.15); }
  .empty { flex: 1; background: var(--line); border-radius: 4px; }
  .legend { display: flex; flex-wrap: wrap; gap: 4px 14px; margin-top: 8px; }
  .li { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); }
  a.li[href]:hover { color: var(--ink); }
  .li i { width: 8px; height: 8px; border-radius: 2px; flex: none; }
  .li .faint { font-variant-numeric: tabular-nums; }
</style>

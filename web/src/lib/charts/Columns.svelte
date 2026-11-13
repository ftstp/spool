<script lang="ts">
  // Vertical columns over an ordered axis (tempos, years, weeks): one hue, 4px rounded tops,
  // a 2px gap between touching columns, the value on hover. `ticks`: how many x labels to
  // show (the first, the last and evenly between), so they never collide.
  import { show, hide } from "./Tip.svelte";

  type Col = { label: string; n: number; title?: string };
  let { cols, height = 90, ticks = 6, unit = "", color = "var(--accent)", mini = false }:
    { cols: Col[]; height?: number; ticks?: number; unit?: string | ((n: number) => string); color?: string; mini?: boolean } = $props();
  const top = $derived(Math.max(1, ...cols.map((c) => c.n)));
  const every = $derived(Math.max(1, Math.ceil(cols.length / Math.max(1, ticks))));
  const val = (n: number) => (typeof unit === "function" ? unit(n) : `${n}${unit}`);
</script>

<div class="cols" class:mini style:height="{height}px" role="img" aria-label={cols.map((c) => `${c.title ?? c.label} ${c.n}`).join(", ")}>
  {#each cols as c, i (c.label + i)}
    <div class="slot" onmousemove={(e) => show(e, c.title ?? c.label, val(c.n))} onmouseleave={hide} role="presentation">
      <i style:height="{c.n ? Math.max(3, (c.n / top) * 100) : 0}%" style:background={color}></i>
    </div>
  {/each}
</div>
{#if !mini}
  <div class="axis">
    {#each cols as c, i (c.label + i)}<span>{i % every === 0 || i === cols.length - 1 ? c.label : ""}</span>{/each}
  </div>
{/if}

<style>
  .cols { display: flex; align-items: flex-end; gap: 2px; border-bottom: 1px solid var(--line); }
  .mini { border-bottom: 0; }
  .slot { flex: 1; height: 100%; display: flex; align-items: flex-end; justify-content: center; min-width: 0; cursor: default; }
  .slot i { display: block; width: 100%; max-width: 24px; border-radius: 4px 4px 0 0; transition: filter .15s; }
  .mini .slot i { border-radius: 2px 2px 0 0; }
  .slot:hover i { filter: brightness(1.2); }
  .axis { display: flex; gap: 2px; margin-top: 4px; }
  .axis span { flex: 1; min-width: 0; text-align: center; font-size: 10.5px; color: var(--faint); white-space: nowrap; overflow: visible; font-variant-numeric: tabular-nums; }
</style>

<script lang="ts">
  // A count that shows its change: when it goes down (samples found during a search), the
  // number rolls down to its new value and a small "-12" floats off it, in the good colour.
  // Going up, or the first value, it just shows. Reduced motion: the new value at once, the
  // "-12" still shown a moment.
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { fmtNum } from "../i18n/index.svelte";

  let { n }: { n: number } = $props();
  const reduce = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  const shown = new Tween(0, { duration: reduce ? 0 : 900, easing: cubicOut });
  let prev: number | null = null;
  let drops = $state<{ id: number; d: number }[]>([]);
  let id = 0;
  $effect(() => {
    const v = n;
    if (prev === null || v >= prev) {
      shown.set(v, { duration: 0 });
    } else {
      const d = prev - v;
      const k = ++id;
      drops = [...drops, { id: k, d }];
      setTimeout(() => (drops = drops.filter((x) => x.id !== k)), 1800);
      shown.set(v);
    }
    prev = v;
  });
  const falling = $derived(drops.length > 0);
</script>

<span class="cd" class:falling>{fmtNum(Math.round(shown.current))}{#each drops as x (x.id)}<span class="drop" aria-hidden="true">-{fmtNum(x.d)}</span>{/each}</span>

<style>
  .cd { position: relative; display: inline-block; font-variant-numeric: tabular-nums; transition: color .4s; }
  .cd.falling { color: var(--ok); }
  .drop { position: absolute; left: 100%; top: -2px; margin-left: 6px; font-size: 12px; font-weight: 700; color: var(--ok); white-space: nowrap; pointer-events: none; animation: rise 1.8s ease-out forwards; }
  @keyframes rise {
    0% { opacity: 0; transform: translateY(6px); }
    15% { opacity: 1; transform: translateY(0); }
    70% { opacity: 1; }
    100% { opacity: 0; transform: translateY(-14px); }
  }
  @media (prefers-reduced-motion: reduce) {
    .drop { animation: fade 1.8s linear forwards; }
    @keyframes fade { 0%, 70% { opacity: 1; } 100% { opacity: 0; } }
  }
</style>

<script lang="ts">
  // A sample's shape in a few pixels, on its row: a hit is a spike and its tail, a pad a
  // block, a riser a ramp, a drum loop a comb. The part played lit while it plays.
  import { waveOf } from "../sampleWaves.svelte";

  let { path, progress = null }: { path: string; progress?: number | null } = $props();
  const w = $derived(waveOf(path));
  const H = 20;
  const bars = $derived(
    (w ?? []).map((v, i) => {
      const h = Math.max(0.6, (v / 255) * H);
      return { x: i, y: (H - h) / 2, h };
    }),
  );
  const n = $derived(w?.length || 48);
</script>

<svg class="sw" class:wait={w === undefined} viewBox="0 0 {n} {H}" preserveAspectRatio="none" aria-hidden="true">
  {#if w}
    {#each bars as b (b.x)}
      <rect x={b.x + 0.15} y={b.y} width="0.7" height={b.h} class:lit={progress !== null && b.x / n <= progress} />
    {/each}
  {:else}
    <rect x="0" y={H / 2 - 0.5} width={n} height="1" class="flat" />
  {/if}
</svg>

<style>
  .sw { width: 56px; height: 20px; display: block; flex: none; }
  rect { fill: var(--muted); opacity: 0.75; }
  rect.lit { fill: var(--accent); opacity: 1; }
  .flat { fill: var(--line); }
  .wait .flat { animation: pulse 1.2s ease-in-out 3; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  @media (prefers-reduced-motion: reduce) { .wait .flat { animation: none; } }
</style>

<script lang="ts">
  // Notes drawn as a piano roll: time across, pitch up, a row a semitone (the black keys'
  // rows darker, each C named), the playhead on them while they play. `notes`: [pitch, start
  // s, length s, velocity]; `at`: where the playhead is (seconds), null when not playing.
  import { noteLabel } from "../theory";
  import { i18n } from "../i18n/index.svelte";

  let { notes, dur, at = null }: { notes: [number, number, number, number][]; dur: number; at?: number | null } = $props();
  const lo = $derived(Math.min(...notes.map((n) => n[0])) - 2);
  const hi = $derived(Math.max(...notes.map((n) => n[0])) + 2);
  const rows = $derived(Math.max(12, hi - lo + 1));
  const span = $derived(Math.max(0.5, dur));
  const BLACK = [1, 3, 6, 8, 10];
  const H = 6;
  const W = 600;
  const y = (p: number) => (hi - p) * H;
  const x = (s: number) => (s / span) * W;
  // a line a second (a beat at 120 is half a second: too dense)
  const secs = $derived(Array.from({ length: Math.floor(span) }, (_, i) => i + 1));
</script>

<div class="pr" role="img" aria-label="piano roll">
  <div class="keys" style:height="{rows * H}px">
    {#each Array.from({ length: rows }, (_, i) => hi - i) as p (p)}
      {#if p % 12 === 0}<span class="c" style:top="{y(p)}px">{noteLabel(p, i18n.lang === "fr")}</span>{/if}
    {/each}
  </div>
  <svg viewBox="0 0 {W} {rows * H}" preserveAspectRatio="none" style:height="{rows * H}px">
    {#each Array.from({ length: rows }, (_, i) => hi - i) as p (p)}
      {#if BLACK.includes(((p % 12) + 12) % 12)}<rect class="blk" x="0" y={y(p)} width={W} height={H} />{/if}
      {#if p % 12 === 0}<line class="oct" x1="0" x2={W} y1={y(p) + H} y2={y(p) + H} />{/if}
    {/each}
    {#each secs as s (s)}<line class="sec" x1={x(s)} x2={x(s)} y1="0" y2={rows * H} />{/each}
    {#each notes as [p, s, l, v], i (i)}
      <rect class="n" x={x(s)} y={y(p) + 0.5} width={Math.max(1.5, x(l))} height={H - 1} rx="1" style:opacity={0.45 + 0.55 * (v / 127)} />
    {/each}
    {#if at !== null}<line class="head" x1={x(at)} x2={x(at)} y1="0" y2={rows * H} />{/if}
  </svg>
</div>

<style>
  .pr { display: grid; grid-template-columns: 26px minmax(0, 1fr); margin-top: 8px; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--bg); max-height: 220px; overflow-y: auto; }
  .keys { position: relative; border-right: 1px solid var(--line); background: var(--panel2); }
  .c { position: absolute; left: 2px; font-size: 8.5px; line-height: 6px; color: var(--muted); }
  svg { width: 100%; display: block; }
  .blk { fill: color-mix(in srgb, var(--ink) 5%, transparent); }
  .oct { stroke: var(--line); stroke-width: 0.5; }
  .sec { stroke: var(--line); stroke-width: 0.5; stroke-dasharray: 2 2; }
  .n { fill: var(--accent); }
  .head { stroke: var(--ink); stroke-width: 1.2; vector-effect: non-scaling-stroke; }
</style>

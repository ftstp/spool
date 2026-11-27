<script lang="ts">
  // The level of what plays, as a thin bar (green, then yellow from -12 dBFS, red from -3)
  // with the loudest moment of the last second marked; drawn only while something plays.
  // `level`: the peak now, 0..1 (linear), asked for at each frame. `lufs`: the short-term
  // loudness, written under the bar: a loud master keeps its peaks near 0 dBFS, the bar stays
  // full, its loudness still says how loud it is.
  import { onDestroy } from "svelte";
  import { t, fmtNum } from "../i18n/index.svelte";

  let { level, lufs, playing }: { level: () => number; lufs?: () => number | null; playing: boolean } = $props();
  let loud = $state<number | null>(null);
  let loudAt = 0;
  const FLOOR = -48;
  const pos = (lin: number) => (lin <= 0 ? 0 : Math.max(0, Math.min(1, 1 - (20 * Math.log10(lin)) / FLOOR)));
  let now = $state(0);
  let hold = $state(0);
  let heldAt = 0;
  let raf = 0;
  function tick() {
    const v = pos(level());
    // up at once, down smoothly (a meter's ballistics)
    now = v > now ? v : now * 0.88 + v * 0.12;
    const ms = performance.now();
    if (v >= hold || ms - heldAt > 1000) {
      hold = v;
      heldAt = ms;
    }
    // the number changes 4 times a second, readable
    if (lufs && ms - loudAt > 250) {
      loudAt = ms;
      const l = lufs();
      loud = l == null ? null : Math.round(l * 10) / 10;
    }
    raf = requestAnimationFrame(tick);
  }
  $effect(() => {
    cancelAnimationFrame(raf);
    if (playing) raf = requestAnimationFrame(tick);
    else {
      now = 0;
      hold = 0;
      loud = null;
    }
  });
  onDestroy(() => cancelAnimationFrame(raf));
  const db = $derived(hold > 0 ? Math.round(FLOOR * (1 - hold)) : null);
</script>

<span class="vu" role="meter" aria-label={t("song.player.vu")} aria-valuemin={0} aria-valuemax={1} aria-valuenow={now}
  title={db === null ? t("song.player.vu") : t("song.player.vuTip", { n: db })}>
  <i class="fill" style:clip-path="inset(0 {((1 - now) * 100).toFixed(1)}% 0 0)"></i>
  {#if hold > 0}<i class="pk" class:hot={hold > 1 - 3 / -FLOOR} style:left="{hold * 100}%"></i>{/if}
</span>
{#if lufs}<span class="lufs" title={t("song.player.lufsTip")}>{loud == null ? "" : t("song.player.lufs", { n: loud <= -70 ? "-∞" : fmtNum(loud) })}</span>{/if}

<style>
  .vu { position: relative; display: block; width: 100%; height: 4px; border-radius: 2px; background: var(--line); overflow: hidden; }
  /* the colours stay where their levels are: the bar is uncovered, not stretched */
  .fill { position: absolute; inset: 0; background: linear-gradient(to right, var(--ok) 0 75%, #e0b43a 75% 93.75%, var(--bad) 93.75% 100%); }
  .pk { position: absolute; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: var(--ink); opacity: .7; }
  .pk.hot { background: var(--bad); opacity: 1; }
  .lufs { display: block; height: 11px; font-size: 9.5px; line-height: 11px; color: var(--faint); font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
</style>

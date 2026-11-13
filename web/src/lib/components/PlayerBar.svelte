<script lang="ts">
  // The bar at the bottom of the window while something plays (or is paused).
  import { player, mmss, infoFor, loudness } from "../player.svelte";
  import Wave from "./Wave.svelte";
  import type { AudioInfo } from "../api";
  import { t } from "../i18n/index.svelte";
  let peaks = $state<number[] | null>(null);
  let info = $state<AudioInfo | null>(null);
  // a derived value: the numbers follow a change of language
  const ld = $derived(loudness(info));
  $effect(() => {
    const rel = player.rel;
    peaks = null;
    info = null;
    infoFor(rel).then((i) => {
      if (player.rel !== rel) return;
      peaks = i?.peaks ?? null;
      info = i;
    });
  });
</script>

{#if player.src || player.error}
  <div class="bar" role="region" aria-label={t("song.player.region")}>
    <button class="pp" onclick={() => player.toggle()} disabled={!player.src} aria-label={player.playing ? t("song.player.pause") : t("song.player.play")}>
      {#if player.playing}<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
      {:else}<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" /></svg>{/if}
    </button>
    <div class="who">
      {#if player.sid}<a href="#/song/{player.sid}" class="t">{player.title}</a>{:else}<span class="t">{player.title}</span>{/if}
      <div class="small faint n" title={[player.name, ld].filter(Boolean).join("\n")}>{player.error || player.name}</div>
    </div>
    <div class="w"><Wave {peaks} height={32} progress={player.len ? player.t / player.len : 0} seek={(f) => player.seek(f)} /></div>
    <span class="small faint time">{mmss(player.t)} / {mmss(player.len)}</span>
    <button class="x" onclick={() => player.stop()} aria-label={t("song.player.closePlayer")} title={t("common.close")}>×</button>
  </div>
{/if}

<style>
  .bar { flex: none; height: 58px; display: flex; align-items: center; gap: 14px; padding: 0 18px; border-top: 1px solid var(--line); background: var(--panel2); }
  .pp { width: 36px; height: 36px; flex: none; border-radius: 50%; border: 0; background: var(--accent); color: var(--on-accent); cursor: pointer; display: grid; place-items: center; }
  /* only the disc grows; the glyph stays sharp (see PlayBtn) */
  .pp { position: relative; isolation: isolate; background: none !important; }
  .pp::before { content: ""; position: absolute; inset: 0; z-index: -1; border-radius: 50%; background: var(--accent); transition: transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1); will-change: transform; backface-visibility: hidden; }
  .pp:disabled::before { background: var(--line); }
  .pp:hover:not(:disabled)::before { transform: scale(1.1); }
  .pp:active:not(:disabled)::before { transform: scale(0.96); transition-duration: 90ms; }
  @media (prefers-reduced-motion: reduce) { .pp::before { transition: none; } .pp:hover:not(:disabled)::before { transform: none; } }
  .pp:disabled { background: var(--line); color: var(--faint); }
  .who { width: 230px; flex: none; min-width: 0; }
  .t { display: block; color: var(--ink); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t:hover { color: var(--accent); }
  .n { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .w { flex: 1; min-width: 0; }
  .time { font-variant-numeric: tabular-nums; white-space: nowrap; }
  .x { border: 0; background: none; color: var(--muted); font-size: 20px; cursor: pointer; padding: 0 4px; }
  .x:hover { color: var(--ink); }
</style>

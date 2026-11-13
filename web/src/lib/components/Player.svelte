<script lang="ts">
  // The reference render on a song sheet: its waveform, drives the app's single player.
  import { mediaUrl } from "../api";
  import { player, mmss, infoFor, loudness } from "../player.svelte";
  import type { AudioInfo } from "../api";
  import Wave from "./Wave.svelte";
  import { t } from "../i18n/index.svelte";
  let { rel, sid, title }: { rel: string; sid: number; title: string } = $props();
  const name = $derived(rel.replace(/\\/g, "/").split("/").pop() ?? rel);
  const mine = $derived(player.is(rel));
  let info = $state<AudioInfo | null>(null);
  const peaks = $derived(info?.peaks ?? null);
  let loading = $state(false);
  $effect(() => {
    const want = rel;
    info = null;
    loading = true;
    infoFor(want).then((i) => {
      if (rel === want) info = i;
      loading = false;
    });
  });
  const click = () => (mine ? player.toggle() : player.play(rel, sid, title));
</script>

<div class="player">
  <button class="pp" onclick={click} disabled={!mediaUrl(rel)} aria-label={mine && player.playing ? t("song.player.pause") : t("song.player.play")}>
    {#if mine && player.playing}<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
    {:else}<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" /></svg>{/if}
  </button>
  <div class="mid">
    <div class="top"><span class="name" title={name}>{name}</span>
      <span class="small faint">{!mediaUrl(rel) ? t("song.player.inApp") : mine ? `${mmss(player.t)} / ${mmss(player.len)}` : loading ? t("song.player.loadingWave") : ""}</span></div>
    {#if loudness(info)}<div class="ld small faint" title={t("song.player.loudnessTip")}>{loudness(info)}</div>{/if}
    <Wave {peaks} height={40} progress={mine && player.len ? player.t / player.len : 0}
      seek={(f) => (mine ? player.seek(f) : player.play(rel, sid, title))} />
  </div>
</div>

<style>
  .player { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-radius: 10px; background: var(--panel2); border: 1px solid var(--line); }
  .pp { width: 40px; height: 40px; flex: none; border-radius: 50%; border: 0; background: var(--accent); color: var(--on-accent); cursor: pointer; display: grid; place-items: center; }
  /* only the disc grows; the glyph stays sharp (see PlayBtn) */
  .pp { position: relative; isolation: isolate; background: none !important; }
  .pp::before { content: ""; position: absolute; inset: 0; z-index: -1; border-radius: 50%; background: var(--accent); transition: transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1); will-change: transform; backface-visibility: hidden; }
  .pp:disabled::before { background: var(--line); }
  .pp:hover:not(:disabled)::before { transform: scale(1.1); }
  .pp:active:not(:disabled)::before { transform: scale(0.96); transition-duration: 90ms; }
  @media (prefers-reduced-motion: reduce) { .pp::before { transition: none; } .pp:hover:not(:disabled)::before { transform: none; } }
  .pp:disabled { background: var(--line); color: var(--faint); cursor: default; }
  .mid { flex: 1; min-width: 0; }
  .top { display: flex; justify-content: space-between; gap: 10px; font-size: 12.5px; margin-bottom: 2px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

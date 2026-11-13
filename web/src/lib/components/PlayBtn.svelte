<script lang="ts">
  // Plays a song's reference render from a card or a row (shown only for songs that have one).
  // On hover only the disc grows (a pseudo-element, GPU-composited); the glyph never scales,
  // so it stays sharp and still: scaling the whole button re-rasterised the icon every frame.
  import { player } from "../player.svelte";
  import { t } from "../i18n/index.svelte";
  let { sid, title, size = 28 }: { sid: number; title: string; size?: number } = $props();
  const on = $derived(player.sid === sid && player.playing);
  // whole pixels: an icon at a fractional size lands between pixels and blurs
  const glyph = $derived(Math.round(size * 0.46));
</script>

<button class="play" class:on style:width="{size}px" style:height="{size}px" aria-label={on ? t("song.player.pause") : t("song.player.listenTemoin")} title={on ? t("song.player.pause") : t("song.player.listenTemoin")}
  onclick={(e) => { e.preventDefault(); e.stopPropagation(); player.playSong(sid, title); }}>
  {#if on}<svg viewBox="0 0 24 24" width={glyph} height={glyph} fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
  {:else}<svg class="tri" viewBox="0 0 24 24" width={glyph} height={glyph} fill="currentColor" aria-hidden="true"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" /></svg>{/if}
</button>

<style>
  .play { position: relative; flex: none; border: 0; background: none; color: var(--on-accent); cursor: pointer; display: grid; place-items: center; padding: 0; border-radius: 50%; isolation: isolate; }
  .play::before {
    content: ""; position: absolute; inset: 0; z-index: -1; border-radius: 50%; background: var(--accent);
    transition: transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 220ms ease-out;
    will-change: transform; backface-visibility: hidden;
  }
  .play:hover::before { transform: scale(1.12); box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22); }
  .play:active::before { transform: scale(0.96); transition-duration: 90ms; }
  .play:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  /* a triangle's weight sits left of its box: nudged right to look centred */
  .tri { transform: translateX(6%); }
  @media (prefers-reduced-motion: reduce) { .play::before { transition: none; } .play:hover::before { transform: none; } }
</style>

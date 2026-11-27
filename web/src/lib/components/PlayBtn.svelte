<script lang="ts" module>
  // a list played through reaches a song with no render: its project, the messages (samples
  // missing) said like any other
  import { queueHooks } from "../player.svelte";
  import { startProjectRel } from "../projectPlay";
  import { app as app0 } from "../state.svelte";
  queueHooks.project = (rel, title) => startProjectRel(rel, title, (m) => app0.say(m));
</script>

<script lang="ts">
  // Plays a song's reference render from a card or a row; a song with none plays its newest
  // project instead (`project`: its file), with the app's synth and its samples, as its sheet
  // does under "Project".
  // On hover only the disc grows (a pseudo-element, GPU-composited); the glyph never scales,
  // so it stays sharp and still: scaling the whole button re-rasterised the icon every frame.
  import { player } from "../player.svelte";
  import { t } from "../i18n/index.svelte";
  import { app } from "../state.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { playProjectRel, sessionKey } from "../projectPlay";
  // `play`: what a click does instead of playing the song alone (a release plays on from it).
  // `queue`: the list the button sits in; its songs play one after the other from this one
  // (each its render, or its project when it has none), with previous / next in the player
  // bar.
  import type { SongQueue } from "../player.svelte";
  let { sid, title, size = 28, play, project = "", queue }: { sid: number; title: string; size?: number; play?: () => void; project?: string; queue?: SongQueue } = $props();
  const projOn = $derived(!!project && midi.rel === "notes:" + sessionKey(project) && (midi.playing || midi.loading));
  const on = $derived(project ? projOn : player.sid === sid && player.playing);
  const label = $derived(on ? t("song.player.pause") : project ? t("song.player.listenProject") : t("song.player.listenTemoin"));
  function click(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    // this song heard now: paused (its render) or stopped (its project)
    if (on) return project ? midi.stop() : player.toggle();
    // its render loaded and paused: on from where it was
    if (!project && player.sid === sid && player.src) return player.toggle();
    if (play) return play();
    if (queue) {
      const l = queue.songs.filter((s) => !!s.temoin || !!s.rel);
      const i = l.findIndex((s) => s.id === sid);
      if (i >= 0 && l.length > 1) return void player.playQueue(l, i, queue.name);
    }
    if (project) {
      player.endQueue();
      playProjectRel(project, title, (m) => app.say(m)).catch((err) => app.say(String(err), { error: true }));
      return;
    }
    player.playSong(sid, title);
  }
  // whole pixels: an icon at a fractional size lands between pixels and blurs
  const glyph = $derived(Math.round(size * 0.46));
</script>

<button class="play" class:on class:proj={!!project} style:width="{size}px" style:height="{size}px" aria-label={label} title={label} onclick={click}>
  {#if on}<svg viewBox="0 0 24 24" width={glyph} height={glyph} fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
  {:else}<svg class="tri" viewBox="0 0 24 24" width={glyph} height={glyph} fill="currentColor" aria-hidden="true"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
</button>

<style>
  .play { position: relative; flex: none; border: 0; background: none; color: var(--on-accent); cursor: pointer; display: grid; place-items: center; padding: 0; border-radius: 50%; isolation: isolate; }
  .play::before {
    content: ""; position: absolute; inset: 0; z-index: -1; border-radius: 50%; background: var(--accent);
    transition: transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 220ms ease-out;
    will-change: transform; backface-visibility: hidden;
  }
  /* the project, not a render: the same button, outlined (the sound is the app's synth) */
  .play.proj { color: var(--accent); }
  .play.proj::before { background: var(--panel); box-shadow: inset 0 0 0 1.5px var(--accent); }
  .play.proj:hover::before { box-shadow: inset 0 0 0 1.5px var(--accent), 0 4px 14px rgba(0, 0, 0, 0.22); }
  .play:hover::before { transform: scale(1.12); box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22); }
  .play:active::before { transform: scale(0.96); transition-duration: 90ms; }
  .play:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  /* the song heard: a ring breathing around its button (lists keep that button shown) */
  .play.on::after { content: ""; position: absolute; inset: -4px; border-radius: 50%; box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent); animation: breathe 1.6s ease-in-out infinite; pointer-events: none; }
  @keyframes breathe { 0%, 100% { opacity: .35; transform: scale(.94); } 50% { opacity: 1; transform: scale(1.04); } }
  /* the triangle path itself sits half-way between its box centre and its centroid (PlayIcon) */

  @media (prefers-reduced-motion: reduce) { .play::before { transition: none; } .play:hover::before { transform: none; } .play.on::after { animation: none; opacity: 1; } }
</style>

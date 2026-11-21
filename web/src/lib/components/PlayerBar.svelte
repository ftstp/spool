<script lang="ts">
  import VolumeKnob from "./VolumeKnob.svelte";
  // The bar at the bottom of the window while something plays (or is paused).
  import { player, mmss, infoFor, loudness, browse } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import Wave from "./Wave.svelte";
  import TempoControl from "./TempoControl.svelte";
  import KeyControl from "./KeyControl.svelte";
  import ChordNow from "./ChordNow.svelte";
  import KeyChord from "./KeyChord.svelte";
  import type { AudioInfo } from "../api";
  import { sheetTransport } from "../sheetTransport.svelte";
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
  // a MIDI file heard from the MIDI page or a mood board shows here too, while it plays (or
  // after, when nothing else was playing)
  // a list listened through (Samples, MIDI pages): previous / next while its file plays
  const midiList = $derived(!!browse.cur && browse.cur === midi.rel && browse.n > 1);
  const audioList = $derived(!!browse.cur && browse.cur === player.rel && browse.n > 1 && !player.queue);
  // a list of songs played through (each its render or its project): previous / next while
  // its song is the one heard, or the next one is being read
  const q = $derived(player.queue && player.queue.items.length > 1 ? player.queue : null);
  const qOn = $derived(!!q && (q.wait || q.cur === midi.rel || q.cur === player.src));
  // a song's render or its project playing from its sheet: the same switch as the sheet's
  // bar, the same state (one goes on from where the other was)
  const sheetSrc = $derived(Object.values(sheetTransport.by).find((x) => !!x.temoin && (player.is(x.temoin) || midi.rel === x.project)) ?? null);
  const mode = $derived(
    midi.playing ? "midi" : player.playing ? "audio" : midi.shown && !player.src ? "midi" : player.src || player.error ? "audio" : midi.shown ? "midi" : null,
  );
</script>

{#snippet srcSwitch()}
  {#if sheetSrc}
    <div class="srcs" role="radiogroup" aria-label={t("song.session.srcLabel")}>
      <button role="radio" aria-checked={sheetSrc.src === "temoin"} class:on={sheetSrc.src === "temoin"} title={t("song.session.playRenderTip")} onclick={() => sheetSrc.setSrc("temoin")}>{t("song.session.playRender")}</button>
      <button role="radio" aria-checked={sheetSrc.src === "project"} class:on={sheetSrc.src === "project"} title={t("song.session.playMidiTip")} onclick={() => sheetSrc.setSrc("project")}>{t("song.session.playMidi")}</button>
    </div>
  {/if}
{/snippet}

<!-- one bar for both players (the MIDI one and the audio one): switching from one to the
     other (a song's Render / Project) changes what it says, never rebuilds it, so nothing
     under the pointer moves or blinks -->
{#if mode}
  {@const isMidi = mode === "midi"}
  {@const list = isMidi ? midiList : audioList}
  {@const queue = !list && qOn}
  <div class="bar" role="region" aria-label={t("song.player.region")}>
    {#if list}<button class="skip" onclick={() => browse.step(-1)} disabled={browse.i <= 0} aria-label={t("song.player.prevFile")} title={t("song.player.prevFile")}><svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="5" y="5" width="2.5" height="14" rx="1" /><path d="M19 6.2v11.6a1 1 0 0 1-1.5.86L9 13.4a1.6 1.6 0 0 1 0-2.8l8.5-5.26A1 1 0 0 1 19 6.2z" /></svg></button>
    {:else if queue}<button class="skip" onclick={() => player.prev()} aria-label={t("song.player.prev")} title={t("song.player.prev")}><svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="5" y="5" width="2.5" height="14" rx="1" /><path d="M19 6.2v11.6a1 1 0 0 1-1.5.86L9 13.4a1.6 1.6 0 0 1 0-2.8l8.5-5.26A1 1 0 0 1 19 6.2z" /></svg></button>{/if}
    <button class="pp" onclick={() => (sheetSrc ? sheetSrc.playPause() : isMidi ? midi.toggle() : player.toggle())} disabled={!isMidi && !player.src}
      aria-label={isMidi ? (midi.playing ? t("midi.player.stop") : t("midi.player.play")) : player.playing ? t("song.player.pause") : t("song.player.play")}>
      {#if isMidi && midi.playing}<svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5" /></svg>
      {:else if !isMidi && player.playing}<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
      {:else}<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
    </button>
    {#if list}<button class="skip" onclick={() => browse.step(1)} disabled={browse.i >= browse.n - 1} aria-label={t("song.player.nextFile")} title={t("song.player.nextFile")}><svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="16.5" y="5" width="2.5" height="14" rx="1" /><path d="M5 6.2v11.6a1 1 0 0 0 1.5.86l8.5-5.26a1.6 1.6 0 0 0 0-2.8L6.5 5.34A1 1 0 0 0 5 6.2z" /></svg></button>
    {:else if queue && q}<button class="skip" onclick={() => player.next()} disabled={q.i >= q.items.length - 1} aria-label={t("song.player.next")} title={t("song.player.next")}><svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><rect x="16.5" y="5" width="2.5" height="14" rx="1" /><path d="M5 6.2v11.6a1 1 0 0 0 1.5.86l8.5-5.26a1.6 1.6 0 0 0 0-2.8L6.5 5.34A1 1 0 0 0 5 6.2z" /></svg></button>{/if}
    <div class="who">
      {#if isMidi}<span class="t">{midi.name}</span>
        <div class="small faint n">{midi.error || (queue && q ? t("song.player.inQueue", { i: q.i + 1, n: q.items.length, name: q.name }) : t("midi.player.inBar", { sound: t(`midi.player.${midi.sound}`) }))}</div>
      {:else}
        {#if player.sid}<a href="#/song/{player.sid}" class="t">{player.title}</a>{:else}<span class="t">{player.title}</span>{/if}
        <div class="small faint n" title={[player.name, ld].filter(Boolean).join("\n")}>{player.error || (queue && q ? t("song.player.inQueue", { i: q.i + 1, n: q.items.length, name: q.name }) : player.name)}</div>
      {/if}
    </div>
    {@render srcSwitch()}
    <div class="w">
      <Wave peaks={isMidi ? midi.peaks : peaks} height={32} progress={isMidi ? (midi.len ? midi.t / midi.len : 0) : player.len ? player.t / player.len : 0} seek={(f) => (isMidi ? midi.seek(f) : player.seek(f))} />
    </div>
    <span class="small faint time">{isMidi ? `${mmss(midi.t / midi.eff)} / ${mmss(midi.len / midi.eff)}` : `${mmss(player.t / player.eff)} / ${mmss(player.len / player.eff)}`}</span>
    <ChordNow />
    <KeyChord />
    {#if isMidi}<TempoControl p={midi} /><KeyControl p={midi} />{:else}<TempoControl p={player} /><KeyControl p={player} />{/if}
    <button class="loop" class:on={isMidi ? midi.loop : player.loop} aria-pressed={isMidi ? midi.loop : player.loop} title={t("common.tempo.loopTip")} onclick={() => (isMidi ? (midi.loop = !midi.loop) : player.setLoop(!player.loop))}>
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 2l4 4-4 4" /><path d="M3 11v-1a4 4 0 0 1 4-4h14" /><path d="M7 22l-4-4 4-4" /><path d="M21 13v1a4 4 0 0 1-4 4H3" /></svg>
    </button>
    <VolumeKnob />
    <button class="x" onclick={() => (isMidi ? midi.close() : player.stop())} aria-label={t("song.player.closePlayer")} title={t("common.close")}>×</button>
  </div>
{/if}

<style>
  .bar { flex: none; height: 58px; display: flex; align-items: center; gap: 14px; padding: 0 18px; border-top: 1px solid var(--line); background: var(--panel2); }
  .skip { width: 30px; height: 30px; flex: none; border-radius: 50%; border: 0; background: none; color: var(--ink); cursor: pointer; display: grid; place-items: center; }
  .skip:hover:not(:disabled) { background: var(--panel2); }
  .skip:disabled { opacity: .35; cursor: default; }
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
  /* fixed widths: what the time or the chord says never moves the waveform */
  .time { flex: none; width: 84px; text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .loop { border: 0; background: none; color: var(--faint); cursor: pointer; padding: 4px; border-radius: 6px; display: grid; place-items: center; }
  .loop:hover { color: var(--ink); }
  .loop.on { color: var(--ink); background: var(--accent-bg); }
  .x { border: 0; background: none; color: var(--muted); font-size: 20px; cursor: pointer; padding: 0 4px; }
  .srcs { display: inline-flex; flex: none; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .srcs button { border: 0; background: none; color: var(--muted); padding: 4px 10px; cursor: pointer; font: inherit; font-size: 12px; }
  .srcs button + button { border-left: 1px solid var(--line); }
  .srcs button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .x:hover { color: var(--ink); }
</style>

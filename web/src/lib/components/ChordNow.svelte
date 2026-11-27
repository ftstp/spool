<script lang="ts">
  // In the player bar: the chord heard and the next one: a song's reference render wherever it
  // was started from (the chords come from the song's project), or its project from its sheet. Each change comes in with a short fade timed on the
  // tempo (a sixteenth of a bar of four), transposed with the player's key.
  import { player } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { chordSource, songChords, type ChordRun, type ChordSource } from "../chordNow.svelte";
  import { spelling } from "../theory";
  import { i18n, t } from "../i18n/index.svelte";

  let beat = $state<number | null>(null);
  // the song the audio player plays, its chords read from its project
  let auto = $state<ChordSource | null>(null);
  $effect(() => {
    const sid = player.sid;
    const on = player.playing && !!player.src;
    auto = null;
    if (sid == null || !on) return;
    let gone = false;
    songChords(sid)
      .then((s) => !gone && (auto = s))
      .catch(() => {});
    return () => (gone = true);
  });
  // which player plays these chords now (null: neither)
  function whichOf(s: ChordSource | null) {
    if (!s) return null;
    if (s.temoin && player.is(s.temoin) && player.playing) return "audio";
    if (midi.playing && midi.rel === s.midiRel) return "midi";
    return null;
  }
  // the open arrangement's first (it is what the chart shows), else the song's
  const src = $derived(whichOf(chordSource.v) ? chordSource.v : auto);
  const which = $derived(whichOf(src));
  $effect(() => {
    const w = which;
    const s = src;
    if (!w || !s) {
      beat = null;
      return;
    }
    let raf = 0;
    const tick = () => {
      // the render starts `offset` beats into the arrangement (renderOffset)
      beat = w === "audio" ? (player.pos() * s.bpm) / 60 + (s.offset ?? 0) : (midi.t * s.bpm) / 60;
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(raf);
  });
  const semis = $derived(which === "midi" ? midi.semis : player.semis);
  const eff = $derived(which === "midi" ? midi.eff : player.eff);
  const idx = $derived.by(() => {
    if (beat === null || !src) return -1;
    return src.runs.findIndex((r) => beat! >= r.from && beat! < r.to);
  });
  const nextIdx = $derived.by(() => {
    if (beat === null || !src) return -1;
    // the next chord that is another one (the same coming back after a gap is not "next")
    const c = idx >= 0 ? src.runs[idx] : null;
    return src.runs.findIndex((r) => r.from > beat! + 1e-6 && !(c && r.root === c.root && r.q === c.q && r.bass === c.bass));
  });

  // the names: as the chart writes them (Latin when it is set so, else in the app's language)
  const latin = $derived.by(() => {
    try {
      const m = localStorage.getItem("spool.chart.mode");
      return m === "latin" || (m !== "us" && i18n.lang === "fr");
    } catch {
      return i18n.lang === "fr";
    }
  });
  const LAT: Record<string, string> = { C: "Do", D: "Ré", E: "Mi", F: "Fa", G: "Sol", A: "La", B: "Si" };
  const accs = (n: string) => n.replace("#", "♯").replace(/(.)b$/, "$1♭");
  function name(r: ChordRun) {
    const names = spelling((src!.tonic + semis + 12) % 12, src!.minor);
    const note = (pc: number) => {
      const n = names[(((pc + semis) % 12) + 12) % 12];
      return accs(latin ? LAT[n[0]] + n.slice(1) : n);
    };
    const q = r.q;
    const head = note(r.root) + (latin && q && !/^\d/.test(q) ? " " : "") + q;
    return head + (r.bass !== null ? "/" + note(r.bass) : "");
  }
  // a quarter of a beat at the speed heard
  const fade = $derived(src ? Math.round(60000 / (src.bpm * (eff || 1)) / 4) : 120);
</script>

<!-- shown while the song plays, the same width whatever it says: the waveform beside it stays put -->
{#if src && which}
  <span class="cn" title={t("song.player.chordsTip")} style:--fade="{fade}ms">
    {#key idx}<b class="cur">{idx >= 0 ? name(src.runs[idx]) : "-"}</b>{/key}
    <span class="arrow">›</span>
    {#key nextIdx}<span class="nx">{nextIdx >= 0 ? name(src.runs[nextIdx]) : ""}</span>{/key}
  </span>
{/if}

<style>
  .cn { flex: none; width: 168px; display: inline-flex; align-items: baseline; gap: 6px; white-space: nowrap; overflow: hidden; font-variant-numeric: tabular-nums; }
  .cur { flex: none; max-width: 100px; overflow: hidden; text-overflow: ellipsis; font-size: 15px; font-weight: 700; color: var(--ink); animation: in var(--fade) ease-out; }
  .nx { min-width: 0; overflow: hidden; text-overflow: ellipsis; font-size: 12.5px; color: var(--muted); animation: in var(--fade) ease-out; }
  .arrow { flex: none; }
  .arrow { color: var(--faint); font-size: 12px; }
  @keyframes in { from { opacity: 0; transform: translateY(3px); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) { .cur, .nx { animation: none; } }
</style>

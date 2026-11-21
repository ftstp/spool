<script lang="ts">
  // Ideas a whole progression at a time: the song's own loop, variations of it, and the
  // classics of its mode written in its key, the closest to the song first. Each one heard
  // at the song's tempo (a bar a chord), and dragged into Live as MIDI.
  import * as api from "../api";
  import type { Harmony } from "../api/bindings/Harmony";
  import { app } from "../state.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { t } from "../i18n/index.svelte";
  import { chordName, chordNotes, degree, spelling, type Chord } from "../theory";
  import { classics, sameLoop, shared, songLoop, variations, type Prog } from "../progressions";
  import PlayIcon from "./PlayIcon.svelte";

  let { h, bpm, bar, title }: { h: Harmony; bpm: number; bar: number; title: string } = $props();
  const tonic = $derived(h.tonic);
  const names = $derived(spelling(tonic ?? 0, h.minor));
  const loop = $derived(songLoop(h.bars));
  const vars = $derived(loop && tonic !== null ? variations(loop, tonic, h.minor) : []);
  const list = $derived.by(() => {
    if (tonic === null) return [];
    const all = classics(tonic, h.minor).map((p) => ({ ...p, mine: !!loop && sameLoop(p.chords, loop), near: loop ? shared(p.chords, loop) : 0 }));
    return all.sort((a, b) => Number(b.mine) - Number(a.mine) || b.near / b.chords.length - a.near / a.chords.length);
  });
  let style = $state("");
  const styles = $derived([...new Set(list.flatMap((p) => p.styles))].sort());
  const shown = $derived(style ? list.filter((p) => p.styles.includes(style)) : list);

  const nm = (c: Chord) => chordName(c, names);
  function deg(c: Chord) {
    if (tonic === null) return "";
    let q = c.q;
    if (q.startsWith("dim")) q = "°" + q.slice(3);
    else if (q === "m7b5") q = "ø7";
    else if (q.startsWith("m") && !q.startsWith("maj")) q = q.slice(1);
    return degree(c, tonic) + q;
  }
  const notesOf = (cs: Chord[]): [number, number, number][] => cs.flatMap((c, i) => chordNotes(c).map((p) => [p, i * bar, bar * 0.95] as [number, number, number]));
  const keyOf = (cs: Chord[]) => "prog|" + cs.map((c) => c.root + c.q).join(",");
  const on = (cs: Chord[]) => midi.is("notes:" + keyOf(cs));
  function hear(cs: Chord[]) {
    if (on(cs)) return midi.stop();
    midi.playNotes(keyOf(cs), cs.map(nm).join(" "), notesOf(cs), bar * cs.length, bpm, false);
  }
  function drag(cs: Chord[]) {
    api.writeMidi(`${title} - ${cs.map(nm).join(" ")}`, notesOf(cs), bpm).then((p) => api.dragOut(p)).catch((e) => app.say(String(e), { error: true }));
  }
</script>

{#snippet row(p: Prog & { mine?: boolean }, why: string)}
  <!-- a click on the row plays it; held and moved, it goes into Live as MIDI -->
  <li draggable="true" class:on={on(p.chords)} title={t("song.prog.dragTip")} ondragstart={(e) => { e.preventDefault(); drag(p.chords); }} onclick={() => hear(p.chords)}>
    <button class="pl" aria-label={t("song.harmony.hear")} onclick={(e) => { e.stopPropagation(); hear(p.chords); }}><PlayIcon on={on(p.chords)} size={11} /></button>
    <span class="cs">
      {#each p.chords as c, i (i)}<span class="c" class:changed={p.from !== undefined && p.from !== "" && +p.from === i}><b>{nm(c)}</b><i>{deg(c)}</i></span>{/each}
    </span>
    <span class="why">{why}</span>
    {#if p.mine}<span class="tag small">{t("song.prog.yours")}</span>{/if}
  </li>
{/snippet}

<div class="ideas">
  {#if tonic === null}
    <p class="small faint">{t("song.harmony.noKey")}</p>
  {:else}
    {#if loop}
      <h3>{t("song.prog.loop")}</h3>
      <ul>{@render row({ id: "loop", chords: loop, styles: [] }, t("song.prog.loopWhy"))}</ul>
      {#if vars.length}
        <h3>{t("song.prog.variations")}</h3>
        <ul>{#each vars as v, i (i)}{@render row(v, t(`song.prog.var.${v.id}`))}{/each}</ul>
      {/if}
    {/if}
    <div class="classhead">
      <h3>{t("song.prog.classics", { key: (names[tonic] ?? "") + (h.minor ? "m" : "") })}</h3>
      <select bind:value={style} aria-label={t("song.harmony.style")}>
        <option value="">{t("song.harmony.anyStyle")}</option>
        {#each styles as s (s)}<option value={s}>{t(`song.prog.styles.${s}`)}</option>{/each}
      </select>
    </div>
    <ul>{#each shown as p (p.id)}{@render row(p, t(`song.prog.why.${p.id}`))}{/each}</ul>
  {/if}
</div>

<style>
  .ideas { margin-top: 10px; }
  h3 { font-size: 12px; font-weight: 600; color: var(--muted); margin: 12px 0 4px; }
  .classhead { display: flex; align-items: center; gap: 10px; }
  .classhead h3 { flex: 1; }
  select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 2px 6px; font: inherit; font-size: 12px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  li { display: grid; grid-template-columns: 24px auto minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 4px 6px; border-radius: 7px; cursor: grab; }
  li:hover { background: var(--accent-bg); }
  li.on { background: color-mix(in srgb, var(--rec) 12%, transparent); }
  .pl { width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--line); background: var(--panel); color: var(--ink); display: grid; place-items: center; cursor: pointer; padding: 0; }
  .cs { display: inline-flex; gap: 12px; }
  .c { display: inline-flex; flex-direction: column; line-height: 1.15; min-width: 34px; }
  .c b { font-size: 13px; }
  .c i { font-style: normal; font-size: 10.5px; color: var(--faint); }
  .c.changed b { color: var(--accent); }
  .why { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { border: 1px solid var(--accent); color: var(--accent); border-radius: 4px; padding: 0 5px; }
</style>

<script lang="ts">
  // Under the arrangement: the song's key on the circle of fifths, and what could come after
  // the chord chosen (in the chord row, or on the circle), by level and by style, each with
  // why in one line. A suggestion is heard after the chosen chord, and dragged into Live as
  // those two bars of MIDI.
  import * as api from "../api";
  import type { Harmony } from "../api/bindings/Harmony";
  import { app } from "../state.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { t } from "../i18n/index.svelte";
  import CircleOfFifths from "./CircleOfFifths.svelte";
  import ProgressionIdeas from "./ProgressionIdeas.svelte";
  import { chordName, chordNotes, degree, diatonic, inKey, scaleOf, spelling, suggest, type Chord, type Level, type Style } from "../theory";
  // the aid shown beside the suggestions, chosen (and kept) by the user: the circle of fifths,
  // the key's seven chords and their functions, a keyboard, or none
  type View = "scale" | "circle" | "keys" | "none";
  const KEEP = "spool.harmony.view";
  let view = $state<View>((() => { try { return (localStorage.getItem(KEEP) as View) || "scale"; } catch { return "scale"; } })());
  function setView(v: View) {
    view = v;
    try { localStorage.setItem(KEEP, v); } catch { /* not kept */ }
  }
  // whole progressions (default), or what could follow one chord
  const TAB = "spool.harmony.tab";
  let tab = $state<"progs" | "chords">((() => { try { return localStorage.getItem(TAB) === "chords" ? "chords" : "progs"; } catch { return "progs"; } })());
  function setTab(v: "progs" | "chords") {
    tab = v;
    try { localStorage.setItem(TAB, v); } catch { /* not kept */ }
  }
  const isMinorQ = (q: string) => (q.startsWith("m") && !q.startsWith("maj")) || q.startsWith("dim");

  let { h, cur = $bindable(), bpm, bar, title }: { h: Harmony; cur: Chord | null; bpm: number; bar: number; title: string } = $props();
  const tonic = $derived(h.tonic ?? null);
  const names = $derived(spelling(tonic ?? 0, h.minor));
  const used = $derived.by(() => {
    const m = new Map<string, number>();
    for (const b of h.bars) {
      const minorish = (b.quality.startsWith("m") && !b.quality.startsWith("maj")) || b.quality.startsWith("dim");
      const k = b.root + (minorish ? "m" : "");
      m.set(k, (m.get(k) ?? 0) + 1);
    }
    return m;
  });
  // the progression as it comes back: distinct chords in order of appearance, with their bars
  const prog = $derived.by(() => {
    const seen = new Map<string, { c: Chord; n: number }>();
    for (const b of h.bars) {
      const e = seen.get(b.name);
      if (e) e.n++;
      else seen.set(b.name, { c: { root: b.root, q: b.quality }, n: 1 });
    }
    return [...seen.entries()].slice(0, 12);
  });
  let level = $state<Level | "">("");
  let style = $state<Style | "">("");
  const list = $derived(
    tonic === null || !cur ? [] : suggest(tonic, h.minor, cur).filter((s) => (!level || s.level === level) && (!style || s.styles.includes(style))),
  );
  const pair = (a: Chord, b: Chord): [number, number, number][] => [
    ...chordNotes(a).map((p) => [p, 0, bar * 0.95] as [number, number, number]),
    ...chordNotes(b).map((p) => [p, bar, bar * 0.95] as [number, number, number]),
  ];
  function hear(c: Chord, after = true) {
    const notes = after && cur ? pair(cur, c) : chordNotes(c).map((p) => [p, 0, bar * 0.95] as [number, number, number]);
    midi.playNotes(`harm|${c.root}|${c.q}|${after}`, after && cur ? `${chordName(cur, names)} → ${chordName(c, names)}` : chordName(c, names), notes, after && cur ? bar * 2 : bar, bpm, false);
  }
  function choose(c: Chord) {
    cur = c;
    hear(c, false);
  }
  function drag(c: Chord) {
    if (!cur) return;
    const name = `${title} - ${chordName(cur, names)} ${chordName(c, names)}`;
    api.writeMidi(name, pair(cur, c), bpm).then((p) => api.dragOut(p)).catch((e) => app.say(String(e), { error: true }));
  }
  const STYLES: Style[] = ["jazz", "soul", "reggae", "fanfare", "orchestral"];
</script>

<div class="tabs" role="tablist">
  <button role="tab" aria-selected={tab === "progs"} class:on={tab === "progs"} onclick={() => setTab("progs")}>{t("song.prog.tab")}</button>
  <button role="tab" aria-selected={tab === "chords"} class:on={tab === "chords"} onclick={() => setTab("chords")}>{t("song.prog.tabChords")}</button>
</div>
{#if tab === "progs"}
  <ProgressionIdeas {h} {bpm} {bar} {title} />
{:else}
<div class="views" role="radiogroup" aria-label={t("song.harmony.viewLabel")}>
  {#each [["scale", t("song.harmony.view.scale")], ["circle", t("song.harmony.view.circle")], ["keys", t("song.harmony.view.keys")], ["none", t("song.harmony.view.none")]] as [v, l] (v)}
    <button role="radio" aria-checked={view === v} class:on={view === v} title={t(`song.harmony.viewTip.${v}`)} onclick={() => setView(v as View)}>{l}</button>
  {/each}
</div>
<div class="harm" class:solo={view === "none"}>
  {#if view !== "none"}
  <div class="left">
    {#if view === "circle"}
      <CircleOfFifths {tonic} minor={h.minor} {used} {cur} pick={choose} />
    {:else if view === "scale" && tonic !== null}
      <div class="scale">
        {#each diatonic(tonic, h.minor) as c (c.root)}
          {@const n = used.get(c.root + (isMinorQ(c.q) ? "m" : "")) ?? 0}
          <button class="dc" data-fn={c.fn} class:on={cur && cur.root === c.root} class:used={n > 0} title={t(`song.harmony.fn.${c.fn}`)} onclick={() => choose(c)}>
            <span class="deg">{degree(c, tonic)}</span>
            <b>{chordName(c, names)}</b>
            <span class="fn">{t(`song.harmony.fnShort.${c.fn}`)}</span>
            {#if n}<span class="n">×{n}</span>{/if}
          </button>
        {/each}
      </div>
      <p class="small faint legend">{t("song.harmony.fnLegend")}</p>
    {:else if view === "keys" && tonic !== null}
      {@const sc = scaleOf(tonic, h.minor)}
      {@const on = cur ? chordNotes(cur).slice(1).map((p) => p % 12) : []}
      <svg viewBox="0 0 280 90" class="kb" role="img" aria-label={t("song.harmony.view.keys")}>
        {#each Array.from({ length: 14 }, (_, i) => i) as w (w)}
          {@const pc = [0, 2, 4, 5, 7, 9, 11][w % 7]}
          <rect x={w * 20} y="0" width="19.5" height="88" rx="2" class="w" class:in={sc.includes(pc)} class:on={on.includes(pc)} class:root={cur && cur.root === pc} />
          {#if sc.includes(pc) && !on.includes(pc)}<circle cx={w * 20 + 10} cy="76" r="2.5" class="dot" />{/if}
          <text x={w * 20 + 10} y="66" class="kl">{names[pc]}</text>
        {/each}
        {#each Array.from({ length: 14 }, (_, i) => i).filter((w) => ![2, 6].includes(w % 7)) as w (w)}
          {@const pc = [1, 3, -1, 6, 8, 10][w % 7]}
          <rect x={w * 20 + 13} y="0" width="13" height="54" rx="2" class="b" class:in={sc.includes(pc)} class:on={on.includes(pc)} class:root={cur && cur.root === pc} />
        {/each}
      </svg>
      <p class="small faint legend">{t("song.harmony.keysLegend")}</p>
    {/if}
    {#if prog.length}
      <div class="prog small">
        <span class="faint">{t("song.harmony.used")}</span>
        {#each prog as [name, e] (name)}
          <button class="chip" class:out={tonic !== null && !inKey(e.c, tonic, h.minor)} class:on={cur && cur.root === e.c.root && cur.q === e.c.q}
            title={tonic !== null ? degree(e.c, tonic) + (inKey(e.c, tonic, h.minor) ? "" : " · " + t("song.harmony.outOfKey")) : ""}
            onclick={() => choose(e.c)}>{name}<span class="faint"> ×{e.n}</span></button>
        {/each}
      </div>
    {/if}
  </div>
  {/if}
  <div class="right">
    {#if tonic === null}
      <p class="small faint">{t("song.harmony.noKey")}</p>
    {:else if !cur}
      <p class="small faint">{t("song.harmony.pick")}</p>
    {:else}
      <div class="top">
        <h3>{t("song.harmony.after", { chord: chordName(cur, names) })} <span class="faint small">{degree(cur, tonic)}</span></h3>
        <div class="filters">
          <div class="seg">
            {#each [["", t("song.harmony.level.all")], ["base", t("song.harmony.level.base")], ["color", t("song.harmony.level.color")], ["advanced", t("song.harmony.level.advanced")]] as [v, l] (v)}
              <button class:on={level === v} onclick={() => (level = v as Level | "")}>{l}</button>
            {/each}
          </div>
          <select bind:value={style} aria-label={t("song.harmony.style")}>
            <option value="">{t("song.harmony.anyStyle")}</option>
            {#each STYLES as s (s)}<option value={s}>{t(`song.harmony.styles.${s}`)}</option>{/each}
          </select>
        </div>
      </div>
      <ul>
        {#each list as s (s.root + s.q + s.why)}
          <li draggable="true" title={t("song.harmony.dragTip")} ondragstart={(e) => { e.preventDefault(); drag(s); }}>
            <button class="pl" aria-label={t("song.harmony.hear")} title={t("song.harmony.hearTip", { a: chordName(cur, names), b: chordName(s, names) })} onclick={() => hear(s)}>
              <svg viewBox="0 0 24 24" width="11" height="11" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>
            </button>
            <b class="nm">{chordName(s, names)}</b>
            <span class="deg faint">{degree(s, tonic)}</span>
            <span class="why">{t(`song.harmony.why.${s.why}`, { to: s.to ?? "", key: (names[tonic] ?? "") + (h.minor ? "m" : "") })}</span>
            <span class="lv small" data-l={s.level}>{t(`song.harmony.level.${s.level}`)}</span>
          </li>
        {:else}
          <li class="faint small">{t("song.harmony.none")}</li>
        {/each}
      </ul>
    {/if}
  </div>
</div>
{/if}

<style>
  .tabs { display: flex; gap: 16px; margin-top: 14px; border-bottom: 1px solid var(--line); }
  .tabs button { border: 0; background: none; color: var(--muted); padding: 4px 0 6px; cursor: pointer; font: inherit; font-size: 13px; font-weight: 600; border-bottom: 2px solid transparent; margin-bottom: -1px; }
  .tabs button.on { color: var(--ink); border-bottom-color: var(--accent); }
  .harm { display: grid; grid-template-columns: minmax(200px, 250px) minmax(0, 1fr); gap: 18px; margin-top: 8px; align-items: start; }
  .harm.solo { grid-template-columns: 1fr; }
  .views { display: inline-flex; margin-top: 14px; border: 1px solid var(--line); border-radius: 7px; overflow: hidden; }
  .views button { border: 0; background: none; color: var(--muted); padding: 3px 10px; cursor: pointer; font: inherit; font-size: 12px; }
  .views button + button { border-left: 1px solid var(--line); }
  .views button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .scale { display: grid; grid-template-columns: repeat(auto-fill, minmax(70px, 1fr)); gap: 4px; }
  .dc { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: 1px; padding: 6px 8px; border-radius: 8px; border: 1px solid var(--line); background: var(--panel); color: var(--ink); cursor: pointer; font: inherit; text-align: left; border-top-width: 3px; }
  .dc[data-fn="T"] { border-top-color: #3fb38a; }
  .dc[data-fn="SD"] { border-top-color: #4f8fe0; }
  .dc[data-fn="D"] { border-top-color: #e0604f; }
  .dc.used { background: var(--accent-bg); }
  .dc.on { box-shadow: 0 0 0 2px var(--rec); }
  .dc .deg, .dc .fn { font-size: 11px; color: var(--muted); }
  .dc .n { position: absolute; top: 4px; right: 6px; font-size: 10.5px; color: var(--muted); }
  .legend { margin: 6px 0 0; }
  .kb { width: 100%; max-width: 280px; height: auto; display: block; }
  .kb .w { fill: var(--panel); stroke: var(--line); }
  .kb .w.in { fill: color-mix(in srgb, var(--accent) 10%, var(--panel)); }
  .kb .b { fill: var(--ink); }
  .kb .b.in { fill: color-mix(in srgb, var(--accent) 45%, var(--ink)); }
  .kb .on { fill: color-mix(in srgb, var(--rec) 55%, var(--panel)); }
  .kb .b.on { fill: var(--rec); }
  .kb .root { stroke: var(--rec); stroke-width: 2; }
  .kb .dot { fill: var(--accent); }
  .kb .kl { font-size: 8px; text-anchor: middle; fill: var(--faint); pointer-events: none; }
  @container (max-width: 720px) { .harm { grid-template-columns: 1fr; } }
  .prog { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; margin-top: 8px; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 6px; padding: 1px 7px; cursor: pointer; font: inherit; }
  .chip.out { border-style: dashed; }
  .chip.on { border-color: var(--rec); }
  .top { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; justify-content: space-between; }
  .top h3 { margin: 0; font-size: 15px; }
  .filters { display: flex; gap: 6px; align-items: center; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 7px; overflow: hidden; }
  .seg button { border: 0; background: none; color: var(--muted); padding: 3px 8px; cursor: pointer; font: inherit; font-size: 12px; }
  .seg button + button { border-left: 1px solid var(--line); }
  .seg button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 3px 6px; font: inherit; font-size: 12px; }
  ul { list-style: none; margin: 8px 0 0; padding: 0; display: flex; flex-direction: column; gap: 2px; max-height: 300px; overflow-y: auto; }
  li { display: grid; grid-template-columns: 24px 70px 44px minmax(0, 1fr) auto; gap: 8px; align-items: center; padding: 4px 6px; border-radius: 7px; cursor: pointer; font-size: 13px; }
  li:hover { background: var(--panel2); }
  .pl { width: 24px; height: 24px; border-radius: 50%; border: 0; background: var(--accent-bg); color: var(--ink); cursor: pointer; display: grid; place-items: center; }
  .pl:hover { background: var(--accent); color: var(--on-accent); }
  .nm { font-size: 14px; }
  .deg { font-variant-numeric: tabular-nums; font-size: 12px; }
  .why { color: var(--muted); overflow: hidden; text-overflow: ellipsis; }
  .lv { border-radius: 5px; padding: 0 6px; background: var(--panel2); color: var(--muted); white-space: nowrap; }
  .lv[data-l="advanced"] { color: var(--ink); background: color-mix(in srgb, var(--rec) 15%, var(--panel)); }
</style>

<script lang="ts">
  // The circle of fifths: major keys outside, their relative minors inside, each a fifth from
  // its neighbours. The song's key is filled, the chords of its scale tinted, the chords the
  // song uses marked (the more bars, the stronger), the chosen one ringed. A click picks a chord.
  import { spelling, type Chord } from "../theory";
  import { t } from "../i18n/index.svelte";

  let { tonic, minor, used, cur, pick }: {
    tonic: number | null;
    minor: boolean;
    /** Bars per chord root and mode: "0" (C major), "9m" (A minor). */
    used: Map<string, number>;
    cur: Chord | null;
    pick: (c: Chord) => void;
  } = $props();

  const ORDER = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];
  const names = $derived(spelling(tonic ?? 0, minor));
  const max = $derived(Math.max(1, ...used.values()));
  // the position (0..11) of the key's major (the relative major for a minor key)
  const home = $derived(tonic === null ? -1 : ORDER.indexOf(minor ? (tonic + 3) % 12 : tonic));
  const near = (i: number) => home >= 0 && [0, 1, 11].includes((i - home + 12) % 12);
  const C = 150;
  function seg(i: number, r0: number, r1: number) {
    const a0 = ((i * 30 - 15 - 90) * Math.PI) / 180;
    const a1 = ((i * 30 + 15 - 90) * Math.PI) / 180;
    const p = (r: number, a: number) => `${C + r * Math.cos(a)},${C + r * Math.sin(a)}`;
    return `M${p(r0, a0)} L${p(r1, a0)} A${r1},${r1} 0 0 1 ${p(r1, a1)} L${p(r0, a1)} A${r0},${r0} 0 0 0 ${p(r0, a0)} Z`;
  }
  const at = (i: number, r: number) => {
    const a = ((i * 30 - 90) * Math.PI) / 180;
    return { x: C + r * Math.cos(a), y: C + r * Math.sin(a) };
  };
  const isCur = (root: number, m: boolean) => !!cur && cur.root % 12 === root && (cur.q.startsWith("m") && !cur.q.startsWith("maj") || cur.q.startsWith("dim")) === m;
</script>

<svg viewBox="0 0 300 300" class="cof" role="group" aria-label={t("song.harmony.circle")}>
  {#each ORDER as maj, i (maj)}
    {@const mn = (maj + 9) % 12}
    {@const uMaj = used.get(String(maj)) ?? 0}
    {@const uMin = used.get(mn + "m") ?? 0}
    <path d={seg(i, 92, 146)} class="s" class:near={near(i)} class:key={!minor && i === home} class:cur={isCur(maj, false)}
      style:--u={uMaj / max} onclick={() => pick({ root: maj, q: "" })} role="button" tabindex="-1" aria-label={names[maj]} data-tip={names[maj]}></path>
    <path d={seg(i, 50, 92)} class="s in" class:near={near(i)} class:key={minor && i === home} class:cur={isCur(mn, true)}
      style:--u={uMin / max} onclick={() => pick({ root: mn, q: "m" })} role="button" tabindex="-1" aria-label={names[mn] + "m"} data-tip={names[mn] + "m"}></path>
    {@const o = at(i, 119)}
    {@const n = at(i, 71)}
    <text x={o.x} y={o.y} class="l" class:on={!minor && i === home}>{names[maj]}</text>
    <text x={n.x} y={n.y} class="l sm" class:on={minor && i === home}>{names[mn]}m</text>
    {#if uMaj}<circle cx={at(i, 136).x} cy={at(i, 136).y} r={2 + 3 * (uMaj / max)} class="dot" />{/if}
    {#if uMin}<circle cx={at(i, 84).x} cy={at(i, 84).y} r={2 + 3 * (uMin / max)} class="dot" />{/if}
  {/each}
  <text x={C} y={C - 4} class="c">{tonic === null ? "?" : names[tonic] + (minor ? "m" : "")}</text>
  <text x={C} y={C + 12} class="c sub">{t("song.harmony.keyWord")}</text>
</svg>

<style>
  /* a click focuses the slice: no box drawn around it (it is not reached by the keyboard) */
  path:focus { outline: none; }
  .cof { width: 100%; max-width: 230px; height: auto; display: block; }
  .s { fill: color-mix(in srgb, var(--accent) calc(var(--u) * 26%), var(--panel)); stroke: var(--line); stroke-width: 1; cursor: pointer; transition: fill .15s; }
  .s.in { fill: color-mix(in srgb, var(--accent) calc(var(--u) * 26%), var(--panel2)); }
  .s.near { stroke: color-mix(in srgb, var(--accent) 45%, var(--line)); }
  .s.key { fill: var(--accent); }
  .s:hover { fill: color-mix(in srgb, var(--accent) 35%, var(--panel)); }
  .s.cur { stroke: var(--rec); stroke-width: 2.5; }
  .l { font-size: 12px; font-weight: 600; text-anchor: middle; dominant-baseline: middle; fill: var(--ink); pointer-events: none; }
  .l.sm { font-size: 10px; font-weight: 500; fill: var(--muted); }
  .l.on { fill: var(--on-accent); }
  .dot { fill: var(--rec); pointer-events: none; }
  .c { font-size: 18px; font-weight: 700; text-anchor: middle; fill: var(--ink); }
  .c.sub { font-size: 10px; font-weight: 400; fill: var(--faint); }
</style>

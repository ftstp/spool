<script lang="ts">
  // The song's chords at a glance, written as a chart is: "F#m/A | G | C G/B | %", one line
  // per section (the locators), a run of bars that comes back as it was written once with
  // "x n". Every chord of a bar (two halves, or a chord a beat) and its bass when it is not
  // the root. In American names (Am7), Latin ones (La m7) or degrees (vi7), the choice kept.
  // A chord clicked plays it and chooses it for the ideas below.
  import type { Harmony } from "../api/bindings/Harmony";
  import type { BarChord } from "../api/bindings/BarChord";
  import { t, i18n } from "../i18n/index.svelte";
  import { spelling, type Chord } from "../theory";
  import PlayIcon from "./PlayIcon.svelte";

  let {
    h,
    bars,
    bpb,
    sections,
    head = null,
    cur = null,
    pick,
    playAll,
    playingAll = false,
    melody = [],
    bpm = 0,
    jump,
  }: {
    h: Harmony;
    bars: number;
    bpb: number;
    sections: { name: string; from: number; to: number }[];
    head?: number | null;
    cur?: Chord | null;
    pick: (c: Chord, name: string) => void;
    playAll?: () => void;
    playingAll?: boolean;
    /** The song's tune (pitch, beat, length in beats), for the staff: one line of notes. */
    melody?: [number, number, number][];
    /** The song's tempo, written at the head of the staff. */
    bpm?: number;
    /** A bar clicked: the playback goes there (in beats from the song's start). */
    jump?: (beat: number) => void;
  } = $props();

  type Mode = "us" | "latin" | "degrees" | "staff";
  const KEEP = "spool.chart.mode";
  let mode = $state<Mode>((() => {
    try {
      const m = localStorage.getItem(KEEP);
      return m === "latin" || m === "degrees" || m === "staff" ? m : "us";
    } catch {
      return "us";
    }
  })());
  function setMode(m: Mode) {
    mode = m;
    try { localStorage.setItem(KEEP, m); } catch { /* not kept */ }
  }

  // ---- the names
  const acc = (s: string) => s.replace(/#/g, "♯").replace(/([A-G]|Do|Ré|Mi|Fa|Sol|La|Si)b/g, "$1♭");
  const LATIN: Record<string, string> = { C: "Do", D: "Ré", E: "Mi", F: "Fa", G: "Sol", A: "La", B: "Si" };
  const latinNote = (n: string) => {
    const m = /^([A-G])([#b]?)$/.exec(n);
    return m ? acc(LATIN[m[1]] + m[2]) : n;
  };
  function latin(name: string) {
    const m = /^([A-G][#b]?)(.*)$/.exec(name);
    if (!m) return name;
    // "La m7", "Do m", "Ré sus2", "Fa maj7": the quality apart from the note ("Dom" read as
    // one word), only a number stays on it ("Sol7")
    const q = m[2];
    const apart = q && !/^\d/.test(q);
    return latinNote(m[1]) + (apart ? " " : "") + q;
  }
  function degreeOf(b: BarChord) {
    if (!b.degree) return acc(b.name);
    let q = b.quality;
    if (q.startsWith("dim")) q = "°" + q.slice(3);
    else if (q.startsWith("m") && !q.startsWith("maj")) q = q.slice(1);
    return acc(b.degree) + q;
  }
  function label(b: BarChord) {
    const bass = b.bass ? "/" + (mode === "latin" ? latinNote(b.bass) : acc(b.bass)) : "";
    if (mode === "latin") return latin(b.name) + bass;
    if (mode === "degrees" && h.tonic !== null) return degreeOf(b) + bass;
    return acc(b.name) + bass;
  }
  const full = (b: BarChord) => b.name + (b.bass ? "/" + b.bass : "");

  // ---- the bars: each one's chords; per section, the runs of bars that repeat
  const byBar = $derived.by(() => {
    const m = new Map<number, BarChord[]>();
    for (const b of h.bars) {
      const l = m.get(b.bar);
      if (l) l.push(b);
      else m.set(b.bar, [b]);
    }
    return m;
  });
  const barKey = (b: number) => (byBar.get(b) ?? []).map((c) => full(c) + "@" + c.at).join(" ");
  type Run = { from: number; len: number; times: number };
  // a section's bars cut into runs: the block (1, 2, 4 or 8 bars) that repeats from here
  // over the most bars, else the next four bars
  function runs(from: number, to: number): Run[] {
    const out: Run[] = [];
    let b = from;
    while (b < to) {
      let best: Run | null = null;
      for (const len of [1, 2, 4, 8]) {
        if (b + len * 2 > to) break;
        let times = 1;
        while (b + (times + 1) * len <= to && [...Array(len).keys()].every((k) => barKey(b + k) === barKey(b + times * len + k))) times++;
        if (times > 1 && (!best || times * len > best.times * best.len)) best = { from: b, len, times };
      }
      // a held chord over a few bars is written bar by bar ("%"), not as a repeat
      if (best && best.len === 1 && best.times < 4) best = null;
      const r = best ?? { from: b, len: Math.min(4, to - b), times: 1 };
      out.push(r);
      b += r.len * r.times;
    }
    return out;
  }
  const parts = $derived.by(() => {
    const cuts = sections.map((s) => ({ name: s.name, from: Math.floor(s.from / bpb), to: Math.ceil(s.to / bpb) }));
    if (!cuts.length || cuts[0].from > 0) cuts.unshift({ name: "", from: 0, to: cuts[0]?.from ?? bars });
    const last = Math.max(0, ...h.bars.map((b) => b.bar + 1));
    const out: { name: string; runs: Run[] }[] = [];
    for (const s of cuts) {
      const to = Math.min(s.to, bars, last);
      let from = s.from;
      // the empty bars a section opens with say nothing
      while (from < to && !byBar.has(from)) from++;
      if (to <= from) continue;
      out.push({ name: s.name, runs: runs(from, to) });
    }
    return out;
  });
  const nowBar = $derived(head === null ? -1 : Math.floor(head / bpb));
  // While it plays, the line holding the bar heard is kept in sight: when it moves to a line
  // below the window (or above it), the page scrolls to it. Only while the user follows: a line
  // left on screen by a scroll of their own elsewhere is not taken back.
  let chartEl = $state<HTMLElement | null>(null);
  let lastLine: Element | null = null;
  const inView = (el: Element) => {
    const r = el.getBoundingClientRect();
    return r.bottom > 0 && r.top < innerHeight;
  };
  $effect(() => {
    const b = nowBar;
    if (b < 0 || !chartEl) return void (lastLine = null);
    queueMicrotask(() => {
      const now = chartEl?.querySelector(".now");
      const line = now?.closest(".sline, .row") ?? now;
      if (!line || line === lastLine) return;
      const following = !lastLine || inView(lastLine);
      lastLine = line;
      if (!following) return;
      const r = line.getBoundingClientRect();
      if (r.bottom > innerHeight - 24 || r.top < 120) line.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    });
  });
  const nowAt = $derived(head === null ? -1 : head - nowBar * bpb);
  const barOn = (r: Run, k: number) => nowBar >= r.from && nowBar < r.from + r.len * r.times && (nowBar - r.from) % r.len === k;
  const chordOn = (r: Run, k: number, c: BarChord) => barOn(r, k) && nowAt >= c.at && nowAt < c.at + c.len;
  const isCur = (c: BarChord) => !!cur && cur.root === c.root && cur.q === c.quality;
  // a bar played as the one before it: "%"
  const repeats = (b: number) => b > 0 && byBar.has(b) && barKey(b) === barKey(b - 1) && (byBar.get(b)?.length ?? 0) === 1;

  // ---- the staff: a lead sheet, the chords above a treble staff and the tune on it, bar by
  // bar (four a line, every bar written: the tune changes where the chords repeat). The notes
  // sit where they fall in the bar; a long one (two beats or more) is hollow.
  const STEP: Record<string, number> = { C: 0, D: 1, E: 2, F: 3, G: 4, A: 5, B: 6 };
  const names = $derived(spelling(h.tonic ?? 0, h.minor));
  // the key signature of the key's major: sharps (F C G D A E B) or flats (B E A D G C F)
  const sig = $derived.by(() => {
    if (h.tonic === null) return { acc: "", letters: [] as string[] };
    const major = h.minor ? (h.tonic + 3) % 12 : h.tonic;
    const sharps: Record<number, number> = { 7: 1, 2: 2, 9: 3, 4: 4, 11: 5, 6: 6 };
    const flats: Record<number, number> = { 5: 1, 10: 2, 3: 3, 8: 4, 1: 5 };
    if (sharps[major]) return { acc: "#", letters: ["F", "C", "G", "D", "A", "E", "B"].slice(0, sharps[major]) };
    if (flats[major]) return { acc: "b", letters: ["B", "E", "A", "D", "G", "C", "F"].slice(0, flats[major]) };
    return { acc: "", letters: [] as string[] };
  });
  // where each sign of the signature sits on a treble staff (E4 = 30)
  const SIG_STEP: Record<string, Record<string, number>> = {
    "#": { F: 38, C: 35, G: 39, D: 36, A: 33, E: 37, B: 34 },
    b: { B: 34, E: 37, A: 33, D: 36, G: 32, C: 35, F: 31 },
  };
  const GAP = 5; // half a staff space
  const TOP = 50; // the top line's y (room above for the chord names and high ledger lines)
  const yOf = (step: number) => TOP + (38 - step) * GAP;
  const SIG_W = $derived(34 + sig.letters.length * 8);
  // the first line opens with the time signature too
  const TS_W = 20;
  const MW = 200; // a bar's width
  const PAD = 12;
  const xIn = (x0: number, beat: number) => x0 + PAD + (beat / bpb) * (MW - 2 * PAD);
  // the tune moved by octaves to sit on the staff (its middle near B4)
  const shift = $derived.by(() => {
    if (!melody.length) return 0;
    const ps = melody.map((m) => m[0]).sort((x, y) => x - y);
    return 12 * Math.round((71 - ps[Math.floor(ps.length / 2)]) / 12);
  });
  // the tune cut at the bar lines (a note held over one goes on, tied, in the next), in
  // sixteenths: a bar of four beats holds sixteen, never more
  const UNITS = $derived(Math.max(1, Math.round(bpb * 4)));
  const notesByBar = $derived.by(() => {
    const m = new Map<number, { p: number; from: number; to: number; tied: boolean }[]>();
    for (const [p, s0, d] of melody) {
      let from = Math.round(s0 * 4);
      const end = Math.max(from + 1, Math.round((s0 + d) * 4));
      let tied = false;
      while (from < end) {
        const b = Math.floor(from / UNITS);
        const to = Math.min(end, (b + 1) * UNITS);
        const l = m.get(b) ?? [];
        l.push({ p: p + shift, from: from - b * UNITS, to: to - b * UNITS, tied });
        m.set(b, l);
        tied = true;
        from = to;
      }
    }
    // one note at a time: each ends where the next one starts
    for (const l of m.values()) {
      l.sort((x, y) => x.from - y.from);
      for (let i = 0; i + 1 < l.length; i++) l[i].to = Math.min(l[i].to, l[i + 1].from);
    }
    return m;
  });
  // a length in sixteenths cut into written values (whole 16, dotted half 12, half 8, dotted
  // quarter 6, quarter 4, dotted eighth 3, eighth 2, sixteenth 1): a quarter or longer starts
  // on a beat, nothing runs over the bar
  const NOTE_V = [16, 12, 8, 6, 4, 3, 2, 1];
  const REST_V = [16, 8, 4, 2, 1];
  function cut(pos: number, len: number, values: number[]) {
    const out: { pos: number; v: number }[] = [];
    while (len > 0) {
      const v = values.find((x) => x <= len && pos + x <= UNITS && (x < 4 || pos % 4 === 0) && (x !== 16 || UNITS === 16)) ?? 1;
      out.push({ pos, v });
      pos += v;
      len -= v;
    }
    return out;
  }
  // a bar whose notes climb far over the staff (or sink far under it) is written an octave
  // lower (higher) under an "8va" ("8vb") sign: every note stays readable, none is cut
  function barOct(b: number): number {
    const ps = (notesByBar.get(b) ?? []).map((n) => noteOf(n.p).step);
    if (!ps.length) return 0;
    const hi = Math.max(...ps);
    const lo = Math.min(...ps);
    if (hi > 45 && lo - 7 >= 24) return -12;
    if (lo < 23 && hi + 7 <= 44) return 12;
    return 0;
  }
  type Ev = { pos: number; v: number; p: number | null; tieIn: boolean };
  // a bar written: its notes and the rests between them (a silent bar: one whole rest)
  function barEvents(b: number): Ev[] {
    const ns = notesByBar.get(b) ?? [];
    const out: Ev[] = [];
    let pos = 0;
    for (const n of ns) {
      if (n.to <= n.from) continue;
      if (n.from > pos) for (const r of cut(pos, n.from - pos, REST_V)) out.push({ ...r, p: null, tieIn: false });
      cut(n.from, n.to - n.from, NOTE_V).forEach((x, i) => out.push({ ...x, p: n.p, tieIn: i > 0 || n.tied }));
      pos = n.to;
    }
    if (pos < UNITS) {
      if (pos === 0) out.push({ pos: 0, v: UNITS, p: null, tieIn: false });
      else for (const r of cut(pos, UNITS - pos, REST_V)) out.push({ ...r, p: null, tieIn: false });
    }
    return out;
  }
  function noteOf(p: number) {
    const nm = names[((p % 12) + 12) % 12];
    const letter = nm[0];
    const a = nm.slice(1);
    const step = STEP[letter] + (Math.floor(p / 12) - 1) * 7;
    const inSig = sig.letters.includes(letter) ? sig.acc : "";
    const show = a === inSig ? "" : a === "#" ? "♯" : a === "b" ? "♭" : "♮";
    return { step, show };
  }
  const ledgers = (step: number) => {
    const out: number[] = [];
    for (let s = 28; s >= step; s -= 2) out.push(s);
    for (let s = 40; s <= step; s += 2) out.push(s);
    return out;
  };
  // per section, its bars four a line (the empty bars it opens with left out)
  const staffParts = $derived.by(() => {
    const cuts = sections.map((s) => ({ name: s.name, from: Math.floor(s.from / bpb), to: Math.ceil(s.to / bpb) }));
    if (!cuts.length || cuts[0].from > 0) cuts.unshift({ name: "", from: 0, to: cuts[0]?.from ?? bars });
    const last = Math.max(0, ...h.bars.map((b) => b.bar + 1), ...[...notesByBar.keys()].map((b) => b + 1));
    const out: { name: string; lines: number[][] }[] = [];
    for (const s of cuts) {
      const to = Math.min(s.to, bars, last);
      let from = s.from;
      while (from < to && !byBar.has(from) && !notesByBar.has(from)) from++;
      if (to <= from) continue;
      const lines: number[][] = [];
      for (let b = from; b < to; b += perLine) lines.push([...Array(Math.min(perLine, to - b)).keys()].map((k) => b + k));
      out.push({ name: s.name, lines });
    }
    return out;
  });
  // four bars a line, two when the chart is narrow (the notes stay readable)
  let chartW = $state(800);
  const perLine = $derived(chartW < 680 ? 2 : 4);
  // a bar's chord names, each one lowered when it would run into the one before it
  function chordRow(b: number, x0: number) {
    let end = -1e9;
    let low = false;
    return (byBar.get(b) ?? []).map((c) => {
      const x = xIn(x0, c.at) - 4;
      const text = chordText(c);
      low = x < end ? !low : false;
      end = x + text.length * 7.4;
      return { c, x, y: low ? 29 : 15, text };
    });
  }
  const chordText = (c: BarChord) => (mode === "staff" ? (i18n.lang === "fr" ? latin(c.name) : acc(c.name)) + (c.bass ? "/" + (i18n.lang === "fr" ? latinNote(c.bass) : acc(c.bass)) : "") : label(c));
</script>

<div class="chart" bind:clientWidth={chartW} bind:this={chartEl}>
  <div class="top">
    {#if playAll}<button class="playall" class:on={playingAll} title={playingAll ? t("song.chart.stopAll") : t("song.chart.playAll")} aria-label={playingAll ? t("song.chart.stopAll") : t("song.chart.playAll")} onclick={playAll}><PlayIcon on={playingAll} size={11} /></button>{/if}
    <span class="title">{t("song.chart.title")}</span>
    {#if h.key}<span class="small faint">{t("song.chart.key", { key: mode === "latin" ? latin(h.key) : acc(h.key) })}</span>{/if}
    <span class="grow"></span>
    <div class="modes" role="radiogroup" aria-label={t("song.chart.notation")}>
      {#each ["us", "latin", "degrees", "staff"] as m (m)}
        <button role="radio" aria-checked={mode === m} class:on={mode === m} disabled={m === "degrees" && h.tonic === null} title={t(`song.chart.modeTip.${m}`)} onclick={() => setMode(m as Mode)}>{t(`song.chart.mode.${m}`)}</button>
      {/each}
    </div>
  </div>
  {#if mode === "staff"}
    {#if !melody.length}<p class="small faint">{t("song.chart.noTune")}</p>{/if}
    <div class="staves">
      {#each staffParts as p, pi (pi)}
        {#if p.name}<div class="sname staffname">{p.name}</div>{/if}
        {#each p.lines as line, li (li)}
          {@const first = pi === 0 && li === 0}
          {@const head0 = SIG_W + (first ? TS_W : 0)}
          {@const W = head0 + MW * line.length}
          <div class="sline">
          <svg class="staff" viewBox="0 0 {W} 136" style:max-width="{W * 1.15}px" role="img" aria-label={line.map((b) => (byBar.get(b) ?? []).map((c) => c.name).join(" ") || "-").join(" | ")}>
            {#if first && bpm > 0}<text class="tempo" x="2" y="11">♩ = {Math.round(bpm)}</text>{/if}
            {#each [0, 1, 2, 3, 4] as k (k)}<line x1="0" x2={W} y1={TOP + k * GAP * 2} y2={TOP + k * GAP * 2} />{/each}
            <text class="clef" x="2" y={TOP + 33}>𝄞</text>
            {#each sig.letters as s, i (i)}<text class="acc" x={30 + i * 8} y={yOf(SIG_STEP[sig.acc][s]) + 4}>{sig.acc === "#" ? "♯" : "♭"}</text>{/each}
            {#if first}
              <text class="ts" x={SIG_W + 1} y={TOP + GAP * 4 - 1}>{bpb}</text>
              <text class="ts" x={SIG_W + 1} y={TOP + GAP * 8 - 1}>4</text>
            {/if}
            {#each line as b, k (b)}
              {@const x0 = head0 + k * MW}
              {@const evs = barEvents(b)}
              {@const oct = barOct(b)}
              <line x1={x0 + MW} x2={x0 + MW} y1={TOP} y2={TOP + GAP * 8} />
              <g class="ms" class:now={b === nowBar}>
                <rect class:go={!!jump} x={x0 + 1} y="0" width={MW - 2} height="136" role="button" tabindex="-1" data-tip={jump ? t("song.chart.jumpTip", { n: b + 1 }) : undefined} onclick={() => jump?.(b * bpb)} onkeydown={() => {}}></rect>
                {#each chordRow(b, x0) as k, ci (ci)}
                  <text class="sym" class:cur={isCur(k.c)} role="button" tabindex="-1" x={k.x} y={k.y} onclick={() => pick({ root: k.c.root, q: k.c.quality }, full(k.c))} onkeydown={() => {}}>{k.text}</text>
                {/each}
                {#if oct}<text class="ottava" x={x0 + 6} y={oct < 0 ? 9 : 134}>{oct < 0 ? "8va" : "8vb"}</text><line class="ottline" x1={x0 + 30} x2={x0 + MW - 6} y1={oct < 0 ? 6 : 131} y2={oct < 0 ? 6 : 131} />{/if}
                {#each evs as e, ei (ei)}
                  {@const cx = xIn(x0, e.pos / 4) + 4}
                  {#if e.p === null}
                    {#if e.v >= 16 || e.v === UNITS}<rect class="rest" x={x0 + MW / 2 - 6} y={yOf(36)} width="12" height={GAP} />
                    {:else if e.v === 8}<rect class="rest" x={cx - 5} y={yOf(34) - GAP} width="12" height={GAP} />
                    {:else}<text class="rst" x={cx - 4} y={yOf(34) + 7}>{e.v === 4 ? "𝄽" : e.v === 2 ? "𝄾" : "𝄿"}</text>{/if}
                  {:else}
                    {@const q = noteOf(e.p + oct)}
                    {@const cy = yOf(q.step)}
                    {@const up = q.step < 34}
                    {#each ledgers(q.step) as st (st)}<line class="ledger" x1={cx - 8} x2={cx + 8} y1={yOf(st)} y2={yOf(st)} />{/each}
                    {#if q.show && !e.tieIn}<text class="acc" x={cx - 15} y={cy + 4}>{q.show}</text>{/if}
                    {#if e.tieIn}
                      {@const px = ei > 0 && evs[ei - 1].p !== null ? xIn(x0, evs[ei - 1].pos / 4) + 4 : x0 - 6}
                      <path class="tie" d="M{px + 5},{cy + (up ? 6 : -6)} Q{(px + cx) / 2},{cy + (up ? 12 : -12)} {cx - 5},{cy + (up ? 6 : -6)}" />
                    {/if}
                    <ellipse class="head" class:hollow={e.v >= 8} {cx} {cy} rx="5" ry="3.6" transform="rotate(-20 {cx} {cy})" />
                    {#if e.v === 12 || e.v === 6 || e.v === 3}<circle class="dot" cx={cx + 9} cy={q.step % 2 === 0 ? cy - GAP / 2 : cy} r="1.6" />{/if}
                    {#if e.v < 16}
                      {@const sx = up ? cx + 4.6 : cx - 4.6}
                      {@const ey = up ? cy - 30 : cy + 30}
                      <line class="stem" x1={sx} x2={sx} y1={up ? cy - 1 : cy + 1} y2={ey} />
                      {#each Array(e.v === 1 ? 2 : e.v === 2 || e.v === 3 ? 1 : 0) as _, fi (fi)}
                        <path class="flag" d={up ? `M${sx},${ey + fi * 6} c1,6 8,8 6,15` : `M${sx},${ey - fi * 6} c1,-6 8,-8 6,-15`} />
                      {/each}
                    {/if}
                  {/if}
                {/each}
              </g>
            {/each}
          </svg>
          </div>
        {/each}
      {/each}
    </div>
  {:else}
  {#if !parts.length}<p class="small faint">{t("song.chart.empty")}</p>{/if}
  <div class="sheet">
    {#each parts as p, pi (pi)}
      <div class="sec">
        <span class="sname" title={p.name}>{p.name || "·"}</span>
        <span class="flow">
          {#each p.runs as r, ri (ri)}
            <span class="run" class:rep={r.times > 1}>
              {#each Array(r.len) as _, k (k)}
                {@const b = r.from + k}
                {@const cs = byBar.get(b) ?? []}
                <span class="bar" class:now={barOn(r, k)} class:go={!!jump} role="presentation" title={jump ? t("song.chart.jumpTip", { n: b + 1 }) : undefined}
                  onclick={(e) => { if (!(e.target as HTMLElement).closest("button")) jump?.(b * bpb); }}>
                  {#if !cs.length}<span class="faint">-</span>
                  {:else if k > 0 && repeats(b)}<span class="pct" title={full(cs[0])}>%</span>
                  {:else}
                    {#each cs as c, ci (ci)}
                      <button class="ch" class:cur={isCur(c)} class:on={chordOn(r, k, c)} title={`${full(c)}${c.degree ? ` (${c.degree})` : ""}`} onclick={() => pick({ root: c.root, q: c.quality }, full(c))}>{label(c)}</button>
                    {/each}
                  {/if}
                </span>{#if k < r.len - 1}<span class="sep">|</span>{/if}
              {/each}
              {#if r.times > 1}<span class="times" title={t("song.chart.timesTip", { n: r.times })}>×{r.times}</span>{/if}
            </span>
            {#if ri < p.runs.length - 1}<span class="sep">|</span>{/if}
          {/each}
        </span>
      </div>
    {/each}
  </div>
  {/if}
</div>

<style>
  .chart { margin-top: 12px; }
  .top { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 6px; }
  .title { font-weight: 600; font-size: 13px; }
  .playall { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line); background: var(--panel); color: var(--ink); display: grid; place-items: center; cursor: pointer; padding: 0; }
  .playall.on { border-color: var(--rec); color: var(--rec); }
  .grow { flex: 1; }
  .modes { display: inline-flex; border: 1px solid var(--line); border-radius: 7px; overflow: hidden; }
  .modes button { border: 0; background: none; color: var(--muted); padding: 2px 9px; cursor: pointer; font: inherit; font-size: 12px; }
  .modes button + button { border-left: 1px solid var(--line); }
  .modes button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .modes button:disabled { opacity: .4; cursor: default; }
  .sheet { display: grid; gap: 3px; }
  .sec { display: grid; grid-template-columns: 76px minmax(0, 1fr); gap: 10px; align-items: baseline; }
  .sname { font-size: 11px; font-weight: 600; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .flow { display: flex; flex-wrap: wrap; align-items: baseline; row-gap: 2px; font-size: 13.5px; line-height: 1.6; }
  .run { display: inline-flex; align-items: baseline; flex-wrap: wrap; }
  .run.rep { background: color-mix(in srgb, var(--accent) 8%, transparent); border-radius: 5px; padding: 0 5px; }
  .bar { display: inline-flex; gap: 6px; padding: 0 2px; border-radius: 4px; }
  .bar.now { background: color-mix(in srgb, var(--rec) 16%, transparent); }
  .bar.go { cursor: pointer; }
  .bar.go:hover:not(.now) { background: color-mix(in srgb, var(--accent) 9%, transparent); }
  .sep { color: var(--faint); padding: 0 6px; }
  .pct { color: var(--faint); }
  .times { font-size: 11.5px; font-weight: 700; color: var(--muted); padding-left: 6px; }
  .ch { border: 0; background: none; padding: 0; color: var(--ink); font: inherit; font-weight: 600; cursor: pointer; white-space: nowrap; }
  .ch:hover { color: var(--accent); }
  .ch.cur { text-decoration: underline; text-decoration-color: var(--accent); text-underline-offset: 3px; }
  .ch.on { color: var(--rec); }
  .staves { display: flex; flex-direction: column; gap: 2px; }
  .staffname { margin-top: 8px; }
  /* a line drawn only when it comes near the screen (a long song is hundreds of notes) */
  .sline { padding-block: 10px; content-visibility: auto; contain-intrinsic-size: auto 170px; }
  .staff { display: block; width: 100%; height: auto; color: var(--ink); overflow: visible; }
  .staff .ottava { font-size: 11px; font-style: italic; font-weight: 700; }
  .staff line.ottline { stroke-dasharray: 3 3; stroke: var(--ink); }
  .staff line { stroke: color-mix(in srgb, var(--ink) 50%, transparent); stroke-width: 1; }
  .staff line.ledger, .staff line.stem { stroke: var(--ink); stroke-width: 1.2; }
  .staff text { fill: var(--ink); }
  .staff .clef { font-size: 46px; font-family: "Segoe UI Symbol", "Noto Music", "Bravura", serif; }
  .staff .acc { font-size: 13px; }
  .staff .sym { font-size: 13px; font-weight: 700; cursor: pointer; }
  .staff .sym:hover, .staff .sym.cur { fill: var(--accent); }
  .staff .head { fill: var(--ink); stroke: var(--ink); stroke-width: 1.2; }
  .staff .head.hollow { fill: none; stroke-width: 2; }
  .ms > rect:first-child { fill: transparent; outline: none; }
  .ms > rect.go { cursor: pointer; }
  .ms > rect.go:hover { fill: color-mix(in srgb, var(--accent) 7%, transparent); }
  .ms.now > rect:first-child { fill: color-mix(in srgb, var(--rec) 12%, transparent); }
  .staff .rest { fill: var(--ink); }
  .staff .rst { font-size: 26px; font-family: "Segoe UI Symbol", "Noto Music", "Bravura", serif; }
  .staff .ts { font-size: 21px; font-weight: 800; font-family: Georgia, "Times New Roman", serif; }
  .staff .tempo { font-size: 11.5px; font-weight: 600; }
  .staff .tie, .staff .flag { fill: none; stroke: var(--ink); stroke-width: 1.3; }
  .staff .dot { fill: var(--ink); }
</style>

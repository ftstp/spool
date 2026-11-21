<script lang="ts">
  // The references of a song, an artist or a release: released songs (or any audio) to hold
  // one's work against. Each is listened to, and compared with `against` (a song's reference
  // render): loudness, peak, range, format, and the tonal balance of the two, band by band.
  // A file is only read; "remove" only detaches it.
  import * as api from "../api";
  import type { RefItem } from "../api/bindings/RefItem";
  import type { AudioInfo } from "../api/bindings/AudioInfo";
  import { app } from "../state.svelte";
  import { player, infoFor, playingFile, playOrPause } from "../player.svelte";
  import PlayIcon from "./PlayIcon.svelte";
  import { spectrum, rangeDiff, type Spectrum } from "../spectrum";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { onMount } from "svelte";

  let { kind, key, against = null, againstLabel = "", sid = null, title = "" }: {
    kind: "song" | "artist" | "release"; key: string; against?: string | null; againstLabel?: string; sid?: number | null; title?: string;
  } = $props();

  let list = $state<RefItem[]>([]);
  // audio files dropped from Explorer anywhere on the card holding the list (its section of a
  // song's sheet, an artist's or a release's card) become references; the sheet's own drop
  // (onto the mood board) leaves that card to it (data-refdrop)
  const AUDIO = /\.(wav|mp3|flac|aif|aiff|m4a|ogg)$/i;
  let box = $state<HTMLElement>();
  const area = () => box?.closest<HTMLElement>(".card") ?? box ?? null;
  const inside = (x: number, y: number) => {
    const el = area();
    const r = el?.getBoundingClientRect();
    return !!el?.offsetParent && !!r && r.width > 0 && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  };
  $effect(() => {
    const el = area();
    if (!el) return;
    el.dataset.refdrop = "1";
    return () => {
      delete el.dataset.refdrop;
      el.classList.remove("refdrop-on");
    };
  });
  onMount(() => {
    let off: (() => void) | undefined;
    let gone = false;
    api.onFileDrop(async (k, paths, x, y) => {
      const el = area();
      const on = k !== "leave" && inside(x, y);
      el?.classList.toggle("refdrop-on", k === "over" && on);
      if (k !== "drop" || !on) return;
      const files = paths.filter((p) => AUDIO.test(p));
      if (!files.length) return app.say(t("refs.dropNoAudio"), { error: true });
      try {
        for (const p of files) list = await api.refMark(p, kind, key, true);
        app.say(t("refs.dropped", { n: files.length }));
      } catch (e) {
        app.say(String(e), { error: true });
      }
    }).then((f) => (gone ? f() : (off = f)));
    return () => {
      gone = true;
      off?.();
    };
  });
  $effect(() => {
    void app.version;
    const [k, x] = [kind, key];
    api.refs(k, x).then((r) => k === kind && x === key && (list = r)).catch(() => (list = []));
  });
  async function add() {
    try {
      const p = await api.pickFile(t("refs.pick"), ["wav", "mp3", "flac", "aif", "aiff", "m4a", "ogg"]);
      if (p) list = await api.refMark(p, kind, key, true);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function detach(r: RefItem) {
    try {
      list = await api.refMark(r.path, kind, key, false);
      if (open === r.path) open = null;
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }

  // one comparison open at a time
  let open = $state<string | null>(null);
  let ia = $state<AudioInfo | null>(null), ib = $state<AudioInfo | null>(null);
  let sa = $state<Spectrum | null>(null), sb = $state<Spectrum | null>(null);
  let busy = $state(false);
  $effect(() => {
    const [r, a] = [open, against];
    ia = ib = null;
    sa = sb = null;
    if (!r || !a) return;
    busy = true;
    infoFor(a).then((i) => open === r && (ia = i));
    infoFor(r).then((i) => open === r && (ib = i));
    Promise.all([spectrum(a), spectrum(r)])
      .then(([x, y]) => open === r && ((sa = x), (sb = y)))
      .catch((e) => app.say(t("refs.spectrumFailed", { error: String(e) }), { error: true }))
      .finally(() => (busy = false));
  });
  const diff = $derived(sa && sb ? rangeDiff(sa, sb) : []);

  // the curves: frequency on a log scale, 0 dB at the top, -60 dB at the bottom
  const W = 560, H = 150, FLOOR = -60;
  const x = (f: number) => (Math.log10(f / 30) / Math.log10(18000 / 30)) * W;
  const y = (db: number) => (Math.max(FLOOR, db) / FLOOR) * H;
  const path = (s: Spectrum) => s.f.map((f, i) => `${i ? "L" : "M"}${x(f).toFixed(1)},${y(s.db[i]).toFixed(1)}`).join("");
  const TICKS = [50, 100, 200, 500, 1000, 2000, 5000, 10000];
  const hz = (f: number) => (f >= 1000 ? `${f / 1000}k` : String(f));
  const viaLabel = (r: RefItem) => (r.via === "artist" ? t("refs.viaArtist", { name: r.from }) : r.via === "release" && r.from ? t("refs.viaRelease", { name: r.from }) : "");
  const fmt = (i: AudioInfo | null) => (i ? `${fmtNum(i.sample_rate / 1000)} kHz · ${i.channels === 1 ? t("refs.mono") : t("refs.stereo")}` : "-");
  // rounded, and never "-0"
  const fmtDb = (n: number | null | undefined, d = 1) => (n == null ? "-" : fmtNum(Math.round(n * 10 ** d) / 10 ** d + 0));
</script>

<section class="refs" bind:this={box}>
  <div class="head">
    <h3>{t("refs.title")} {#if list.length}<span class="faint">{list.length}</span>{/if}</h3>
    <button class="btn sm" title={t("refs.addTip")} onclick={add}>{t("refs.add")}</button>
  </div>
  {#if !list.length}<p class="small faint">{t(`refs.none.${kind}`)}</p>{/if}
  {#each list as r (r.path)}
    <div class="row">
      <button class="btn sm pl" disabled={!r.exists} title={t("refs.listen")} aria-label={t("refs.listen")} onclick={() => playOrPause(r.path, () => player.play(r.path, sid, title || r.name))}>
        <PlayIcon on={playingFile(r.path)} /></button>
      <span class="grow ell" title={r.path}>{r.name}{#if viaLabel(r)} <span class="small faint">{viaLabel(r)}</span>{/if}{#if !r.exists} <span class="small bad">{t("refs.missing")}</span>{/if}</span>
      {#if against && r.exists}<button class="btn sm" class:on={open === r.path} onclick={() => (open = open === r.path ? null : r.path)}>{t("refs.compare")}</button>{/if}
      {#if r.via === kind}<button class="x" title={t("refs.detach")} aria-label={t("refs.detach")} onclick={() => detach(r)}>×</button>{/if}
    </div>
    {#if open === r.path && against}
      <div class="cmp">
        <table class="small">
          <thead><tr><th></th><th>{againstLabel || t("refs.yours")}</th><th>{t("refs.theRef")}</th></tr></thead>
          <tbody>
            <tr><td class="muted">{t("refs.lufs")}</td><td>{fmtDb(ia?.lufs)}</td><td>{fmtDb(ib?.lufs)}</td></tr>
            <tr><td class="muted">{t("refs.peak")}</td><td>{fmtDb(ia?.peak_db)}</td><td>{fmtDb(ib?.peak_db)}</td></tr>
            <tr><td class="muted">{t("refs.rms")}</td><td>{fmtDb(ia?.rms_db)}</td><td>{fmtDb(ib?.rms_db)}</td></tr>
            <tr><td class="muted">{t("refs.lra")}</td><td>{fmtDb(ia?.lra)}</td><td>{fmtDb(ib?.lra)}</td></tr>
            <tr><td class="muted">{t("refs.format")}</td><td>{fmt(ia)}</td><td>{fmt(ib)}</td></tr>
          </tbody>
        </table>
        {#if sa && sb}
          <svg class="spec" viewBox="0 -6 {W} {H + 24}" role="img" aria-label={t("refs.spectrum")}>
            {#each [0, -12, -24, -36, -48] as d (d)}<line x1="0" x2={W} y1={y(d)} y2={y(d)} class="grid" />{/each}
            {#each TICKS as f (f)}<line x1={x(f)} x2={x(f)} y1="0" y2={H} class="grid" /><text x={x(f)} y={H + 14} text-anchor="middle">{hz(f)}</text>{/each}
            <path d={path(sb)} class="ref" />
            <path d={path(sa)} class="mine" />
          </svg>
          <div class="legend small"><span class="k mine"></span>{againstLabel || t("refs.yours")} <span class="k ref"></span>{t("refs.theRef")}</div>
          <div class="ranges small">
            {#each diff as d (d.range)}
              <span class="rg" class:up={d.db >= 1.5} class:down={d.db <= -1.5}><b>{t(`refs.range.${d.range}`)}</b> {d.db > 0 ? "+" : ""}{fmtNum(d.db)} dB</span>
            {/each}
          </div>
          <p class="small faint">{t("refs.diffHint")}</p>
        {:else if busy}<p class="small faint">{t("refs.reading")}</p>{/if}
      </div>
    {/if}
  {/each}
</section>

<style>
  .refs { margin-top: 14px; }
  .head { display: flex; justify-content: space-between; align-items: baseline; }
  h3 { margin: 0 0 6px; font-size: 13px; }
  .row { display: flex; align-items: center; gap: 8px; padding: 4px 0; }
  .grow { flex: 1; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pl { padding: 2px 7px; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; }
  .x:hover { color: var(--bad); }
  .cmp { margin: 4px 0 10px; padding: 10px; border-radius: 8px; background: var(--panel2); }
  table { border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; }
  th, td { text-align: left; padding: 3px 8px 3px 0; }
  th { font-weight: 600; color: var(--muted); }
  .spec { width: 100%; height: auto; margin-top: 10px; display: block; }
  .spec .grid { stroke: var(--line); stroke-width: 1; }
  .spec text { font-size: 10px; fill: var(--faint); }
  .spec path { fill: none; stroke-width: 2; }
  .spec .mine { stroke: var(--accent); }
  .spec .ref { stroke: var(--muted); stroke-dasharray: 5 3; }
  .legend { display: flex; align-items: center; gap: 6px; color: var(--muted); margin-top: 4px; }
  .k { width: 16px; height: 0; border-top: 2px solid var(--accent); display: inline-block; margin-left: 8px; }
  .k.ref { border-top: 2px dashed var(--muted); }
  .ranges { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  .rg { padding: 2px 8px; border-radius: 6px; border: 1px solid var(--line); font-variant-numeric: tabular-nums; }
  .rg.up { border-color: var(--fix); }
  .rg.down { border-color: var(--lost); }
</style>

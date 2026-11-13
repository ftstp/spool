<script lang="ts">
  // Statistics: the whole catalogue in charts (they used to sit at the bottom of Home).
  // Every chart that counts songs opens the songs list, filtered.
  import { app } from "../lib/state.svelte";
  import StackBar from "../lib/charts/StackBar.svelte";
  import BarList from "../lib/charts/BarList.svelte";
  import Columns from "../lib/charts/Columns.svelte";
  import Lines from "../lib/charts/Lines.svelte";
  import Library from "../lib/components/Library.svelte";
  import { stageLabel, STAGES } from "../lib/format";
  import { t, fmtDate } from "../lib/i18n/index.svelte";
  import { f } from "../lib/filters.svelte";
  import * as api from "../lib/api";
  import type { HomeStats, Progress } from "../lib/api";

  const o = $derived(app.overview);
  let h = $state<HomeStats | null>(null);
  let p = $state<Progress | null>(null);
  let seq = 0;
  $effect(() => {
    app.version; // again after every change
    const n = ++seq; // the newest answer only
    api.homeStats().then((x) => n === seq && (h = x)).catch(() => n === seq && (h = null));
    api.progress().then((x) => n === seq && (p = x)).catch(() => n === seq && (p = null));
  });

  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "2-digit" });
  const hist = $derived(p?.history ?? []);
  const byDay = $derived((() => {
    const seen = new Map<string, (typeof hist)[number]>();
    for (const x of hist) seen.set(day(x.at), x);
    return [...seen.entries()].reverse().slice(0, 8);
  })());
  const months = $derived(
    (p?.done_by_month ?? []).map((m) => {
      const [y, mo] = m.label.split("-");
      return { label: fmtDate(new Date(+y, +mo - 1, 1), { month: "short", year: "2-digit" }), n: m.n };
    }),
  );
  const proj = $derived(p ? [p.complete_now, ...p.projection.map((n) => p!.complete_now + n)] : []);
  const projX = $derived(p ? [t("stats.today"), ...p.projection_names.map((n, i) => `${i + 1}. ${n}`)] : []);

  const ORDER = [...STAGES, "none"];
  const stageParts = (c: Record<string, number>) => ORDER.map((k) => ({ key: k, label: stageLabel(k), n: c[k] ?? 0, color: `var(--s-${k})`, href: "#/songs" }));
  const SEV = ["ok", "fix", "lost", "bad"];
  const nSongs = (n: number) => t("songs.nSongs", { n });
  const nProjects = (n: number) => t("stats.nProjects", { n });
  const keySrc = (k: string) => (k === "estimé" ? "estimated" : k === "Live" ? "live" : "noted");
  const hover = $derived(t("stats.hover").split("{link}"));
  const SEVC = ["var(--ok)", "var(--fix)", "var(--lost)", "var(--bad)"];

  function toStage(k: string) {
    f.stages = k === "none" ? [] : [k];
    f.artist = "";
    location.hash = "#/songs";
  }
  function toArtist(a: string) {
    f.artist = a === "Sans artiste" || a === t("songs.noArtist") ? "" : a;
    f.stages = [];
    location.hash = "#/songs";
  }
  function toStyle(s: string) {
    f.style = s;
    location.hash = "#/songs";
  }
  function toKey(k: string) {
    f.key = k;
    location.hash = "#/songs";
  }
</script>

<div class="page">
  <header class="head">
    <div>
      <h1>{t("stats.title")}</h1>
      {#if o && h}<p class="sub muted">{nSongs(o.songs)} · {nProjects(h.files)}</p>{/if}
    </div>
  </header>

  {#if !o || !h}<p class="muted">{t("common.loading")}</p>
  {:else}
    <div class="sgrid">
      <section class="card">
        <h2>{t("stats.progress")}</h2>
        <StackBar parts={stageParts(o.stages)} legend height={12} unit={nSongs} pick={toStage} />
        <h3>{t("stats.byArtist")}</h3>
        <div class="artists">
          {#each h.by_artist.slice(0, 12) as a (a.artist)}
            <a class="arow" href="#/songs" onclick={() => toArtist(a.artist)}>
              <span class="an">{a.artist}</span>
              <span class="nn">{Object.values(a.stages).reduce((x, y) => x + y, 0)}</span>
              <StackBar parts={stageParts(a.stages)} height={8} unit={nSongs} />
            </a>
          {/each}
        </div>
      </section>
      <section class="card">
        <h2>{t("stats.opens")} <span class="faint">{h.files}</span></h2>
        <StackBar parts={h.sev.map((n, i) => ({ key: String(i), label: t(`songs.sev.${SEV[i]}`), n, color: SEVC[i], href: "#/repair" }))} legend height={12} unit={nProjects} />
        <h3>{t("stats.blocking")}</h3>
        <BarList rows={h.blocking.map((b) => ({ label: b.label, n: b.n, href: "#/repair" }))} unit={nSongs} />
      </section>
      <section class="card">
        <h2>{t("stats.tempos")} <span class="faint">{t("stats.of", { n: h.with_tempo, total: h.total })}</span></h2>
        <Columns cols={h.tempos.map((x) => ({ label: String(+x.label), n: x.n, title: t("stats.tempoRange", { a: +x.label, b: +x.label + 4 }) }))} unit={nSongs} />
        <h3 title={Object.entries(h.key_src).map(([k, n]) => t(`stats.keySrc.${keySrc(k)}`, { n })).join(", ")}>{t("stats.keys")} <span class="faint">{t("stats.of", { n: h.with_key, total: h.total })}</span></h3>
        <BarList rows={h.keys.map((k) => ({ label: k.label, n: k.n, href: "#/songs" }))} unit={nSongs} pick={toKey} />
      </section>
      <section class="card">
        <h2>{t("stats.styles")} <span class="faint">{t("stats.of", { n: h.with_style, total: h.total })}</span></h2>
        <BarList rows={h.styles.map((k) => ({ label: k.label, n: k.n, href: "#/songs" }))} unit={nSongs} pick={toStyle} />
        <h3 title={t("stats.yearsTitle")}>{t("stats.years")}</h3>
        <Columns cols={h.years.map((y) => ({ label: y.label, n: y.n }))} height={70} unit={nSongs} />
      </section>
      <section class="card">
        <h2>{t("stats.instr")}</h2>
        <BarList rows={h.instr.map((d) => ({ label: d.name, n: d.n, note: d.native ? t("stats.nativeInstr") : t("stats.plugin") }))} unit={nProjects}>
          {#snippet badge(r)}{#if h?.instr.find((d) => d.name === r.label)?.native}<span class="live">Live</span>{/if}{/snippet}
        </BarList>
      </section>
      <section class="card">
        <h2>{t("stats.fx")}</h2>
        <BarList rows={h.fx.map((d) => ({ label: d.name, n: d.n, note: d.native ? t("stats.nativeFx") : t("stats.plugin") }))} unit={nProjects}>
          {#snippet badge(r)}{#if h?.fx.find((d) => d.name === r.label)?.native}<span class="live">Live</span>{/if}{/snippet}
        </BarList>
        <h3>{t("stats.lives")}</h3>
        <BarList rows={[...h.lives].sort((a, b) => (parseInt(b.label) || 0) - (parseInt(a.label) || 0)).map((l) => ({ label: l.label === "?" ? t("stats.unknown") : t("stats.liveVersion", { v: l.label }), n: l.n }))} unit={nSongs} />
      </section>
      <div class="libwrap"><Library /></div>
      {#if p}
        <section class="card">
          <h2>{t("stats.recovered")} <span class="faint">{t("stats.scans", { n: hist.length })}</span></h2>
          <Lines xs={hist.map((x) => day(x.at))} unit={nProjects} height={150} series={[
            { label: t("stats.seriesOpen"), color: "var(--ok)", values: hist.map((x) => x.sev[0] + x.sev[1]) },
            { label: t("stats.seriesComplete"), color: "var(--accent)", values: hist.map((x) => x.complete) },
          ]} />
        </section>
        <section class="card">
          <h2>{t("stats.ifInstall")} <span class="faint">{p.complete_now} → {proj.at(-1)}</span></h2>
          {#if p.projection.length}
            <Lines xs={projX} unit={(n) => t("stats.completeUnit", { n })} height={150} series={[{ label: t("stats.completeSeries"), color: "var(--fix)", values: proj }]} />
            <p class="small faint">{hover[0]}<a href="#/repair">{t("stats.repairLink")}</a>{hover[1] ?? ""}</p>
          {:else}<p class="small faint">{t("stats.nothingToInstall")}</p>{/if}
        </section>
        {#if byDay.length > 1}
          <section class="card">
            <h2>{t("stats.stagesOverDays")}</h2>
            <div class="days">
              {#each byDay as [d, x] (d)}
                <div class="drow"><span class="small faint">{d}</span><StackBar height={8} unit={nSongs} parts={stageParts(x.stages)} /></div>
              {/each}
            </div>
          </section>
        {/if}
        {#if months.length}
          <section class="card">
            <h2>{t("stats.doneByMonth")}</h2>
            <Columns cols={months} height={80} unit={(n) => t("stats.doneUnit", { n })} />
          </section>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 20px; margin-bottom: 16px; }
  .sub { margin: 0; }
  h3 { margin: 16px 0 6px; font-size: 12.5px; font-weight: 600; color: var(--muted); }
  .sgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(360px, 100%), 1fr)); gap: 14px; align-items: start; }
  .sgrid .card h2 { display: flex; justify-content: space-between; align-items: baseline; }
  .sgrid .card h2 .faint { font-weight: 400; font-size: 12px; }
  .artists { display: flex; flex-direction: column; gap: 4px; }
  .arow { display: grid; grid-template-columns: minmax(0, 38%) 2.4em 1fr; align-items: center; gap: 8px; color: var(--ink); padding: 3px 4px; border-radius: 6px; }
  .arow:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
  .an { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .nn { text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 12px; }
  .live { margin-left: 6px; font-size: 10px; padding: 0 5px; border-radius: 4px; border: 1px solid var(--line); color: var(--muted); }
  .libwrap { grid-column: 1 / -1; }
  .days { display: flex; flex-direction: column; gap: 7px; }
  .drow { display: grid; grid-template-columns: 6.5em 1fr; align-items: center; gap: 10px; }
</style>

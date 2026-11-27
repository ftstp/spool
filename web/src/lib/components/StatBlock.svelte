<script module lang="ts">
  /** The Statistics page's blocks, in its order ("progress" also on Home, short). */
  export const STAT_BLOCKS = ["progress", "opens", "tempos", "styles", "instr", "fx", "recovered", "ifInstall", "stagesDays", "doneMonth"] as const;
  export type StatId = (typeof STAT_BLOCKS)[number];
</script>

<script lang="ts">
  // One block of the statistics (a card with its charts). Every chart that counts songs
  // opens the songs list, filtered.
  import StackBar from "../charts/StackBar.svelte";
  import BarList from "../charts/BarList.svelte";
  import Columns from "../charts/Columns.svelte";
  import Lines from "../charts/Lines.svelte";
  import { stageLabel, STAGES } from "../format";
  import { t, fmtDate } from "../i18n/index.svelte";
  import { f } from "../filters.svelte";
  import type { HomeStats, Overview, Progress } from "../api";

  // `short`: the figures without their detail (Home: "Progress" without its list by artist)
  let { id, o, h, p, short = false }: { id: StatId; o: Overview; h: HomeStats; p: Progress | null; short?: boolean } = $props();

  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "2-digit" });
  const hist = $derived(p?.history ?? []);
  const byDay = $derived.by(() => {
    const seen = new Map<string, (typeof hist)[number]>();
    for (const x of hist) seen.set(day(x.at), x);
    return [...seen.entries()].reverse().slice(0, 8);
  });
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
  const keySrc = (k: string) => (k === "estimé" ? "estimated" : k === "Live" ? "live" : k === "audio" ? "audio" : "noted");
  const hover = $derived(t("stats.hover").split("{link}"));
  const SEVC = ["var(--ok)", "var(--fix)", "var(--lost)", "var(--bad)"];

  function toStage(k: string) {
    f.stages = k === "none" ? [] : [k];
    f.artist = [];
    location.hash = "#/songs";
  }
  function toArtist(a: string) {
    f.artist = a === "Sans artiste" || a === t("songs.noArtist") ? [] : [a];
    f.stages = [];
    location.hash = "#/songs";
  }
  function toStyle(s: string) {
    f.style = [s];
    location.hash = "#/songs";
  }
  function toKey(k: string) {
    f.key = [k];
    location.hash = "#/songs";
  }
  // every key used, the first 12 at once; and how many songs per mode ("F dorien": dorien)
  let allKeys = $state(false);
  // the mode as keys are written ("A mineur harmonique") -> its label in the locale
  const MODES: Record<string, string> = { major: "major", minor: "minor", dorien: "dorien", phrygien: "phrygien", lydien: "lydien", mixolydien: "mixolydien", locrien: "locrien", "mineur harmonique": "harmonic", "mineur mélodique": "melodic" };
  const modeOf = (k: string) => (k.includes(" ") ? k.slice(k.indexOf(" ") + 1) : /m$/.test(k) ? "minor" : "major");
  const modes = $derived.by(() => {
    const m = new Map<string, number>();
    for (const k of h.keys) m.set(modeOf(k.label), (m.get(modeOf(k.label)) ?? 0) + k.n);
    return [...m].sort((a, b) => b[1] - a[1]);
  });
  const modeName = (m: string) => (MODES[m] ? t(`stats.mode.${MODES[m]}`) : m);
  function toMode(m: string) {
    f.key = h.keys.filter((k) => modeOf(k.label) === m).map((k) => k.label);
    location.hash = "#/songs";
  }
</script>

{#if id === "progress"}
  <section class="card sb">
    <h2>{t("stats.progress")}</h2>
    <StackBar parts={stageParts(o.stages)} legend height={12} unit={nSongs} pick={toStage} />
    {#if short}<a class="small more" href="#/stats">{t("home.allStats")}</a>{:else}
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
    {/if}
  </section>
{:else if id === "opens"}
  <section class="card sb">
    <h2>{t("stats.opens")} <span class="faint">{h.files}</span></h2>
    <StackBar parts={h.sev.map((n, i) => ({ key: String(i), label: t(`songs.sev.${SEV[i]}`), n, color: SEVC[i], href: "#/repair" }))} legend height={12} unit={nProjects} />
    <h3>{t("stats.blocking")}</h3>
    <BarList rows={h.blocking.map((b) => ({ label: b.label, n: b.n, href: "#/repair" }))} unit={nSongs} />
  </section>
{:else if id === "tempos"}
  <section class="card sb">
    <h2>{t("stats.tempos")} <span class="faint">{t("stats.of", { n: h.with_tempo, total: h.total })}</span></h2>
    <Columns cols={h.tempos.map((x) => ({ label: String(+x.label), n: x.n, title: t("stats.tempoRange", { a: +x.label, b: +x.label + 4 }) }))} unit={nSongs} />
    <h3 title={Object.entries(h.key_src).map(([k, n]) => t(`stats.keySrc.${keySrc(k)}`, { n })).join(", ")}>{t("stats.keys")} <span class="faint">{t("stats.of", { n: h.with_key, total: h.total })}</span></h3>
    {#if modes.length > 1}
      <p class="small muted modes">{t("stats.byMode")} {#each modes as [m, n], i (m)}{#if i}<span class="faint">, </span>{/if}<button class="link" title={t("stats.modeTip", { mode: modeName(m) })} onclick={() => toMode(m)}>{modeName(m)}</button> <span class="faint">{n}</span>{/each}</p>
    {/if}
    <BarList rows={(allKeys ? h.keys : h.keys.slice(0, 12)).map((k) => ({ label: k.label, n: k.n, href: "#/songs" }))} unit={nSongs} pick={toKey} />
    {#if h.keys.length > 12}<button class="link small" onclick={() => (allKeys = !allKeys)}>{allKeys ? t("stats.fewerKeys") : t("stats.allKeys", { n: h.keys.length })}</button>{/if}
  </section>
{:else if id === "styles"}
  <section class="card sb">
    <h2>{t("stats.styles")} <span class="faint">{t("stats.of", { n: h.with_style, total: h.total })}</span></h2>
    <BarList rows={h.styles.map((k) => ({ label: k.label, n: k.n, href: "#/songs" }))} unit={nSongs} pick={toStyle} />
    <h3 title={t("stats.yearsTitle")}>{t("stats.years")}</h3>
    <Columns cols={h.years.map((y) => ({ label: y.label, n: y.n }))} height={70} unit={nSongs} />
  </section>
{:else if id === "instr"}
  <section class="card sb">
    <h2>{t("stats.instr")}</h2>
    <BarList rows={h.instr.map((d) => ({ label: d.name, n: d.n, note: d.native ? t("stats.nativeInstr") : t("stats.plugin") }))} unit={nProjects}>
      {#snippet badge(r)}{#if h.instr.find((d) => d.name === r.label)?.native}<span class="live">Live</span>{/if}{/snippet}
    </BarList>
  </section>
{:else if id === "fx"}
  <section class="card sb">
    <h2>{t("stats.fx")}</h2>
    <BarList rows={h.fx.map((d) => ({ label: d.name, n: d.n, note: d.native ? t("stats.nativeFx") : t("stats.plugin") }))} unit={nProjects}>
      {#snippet badge(r)}{#if h.fx.find((d) => d.name === r.label)?.native}<span class="live">Live</span>{/if}{/snippet}
    </BarList>
    <h3>{t("stats.lives")}</h3>
    <BarList rows={[...h.lives].sort((a, b) => (parseInt(b.label) || 0) - (parseInt(a.label) || 0)).map((l) => ({ label: l.label === "?" ? t("stats.unknown") : t("stats.liveVersion", { v: l.label }), n: l.n }))} unit={nSongs} />
  </section>
{:else if id === "recovered" && p}
  <section class="card sb">
    <h2>{t("stats.recovered")} <span class="faint">{t("stats.scans", { n: hist.length })}</span></h2>
    <Lines xs={hist.map((x) => day(x.at))} unit={nProjects} height={150} series={[
      { label: t("stats.seriesOpen"), color: "var(--ok)", values: hist.map((x) => x.sev[0] + x.sev[1]) },
      { label: t("stats.seriesComplete"), color: "var(--accent)", values: hist.map((x) => x.complete) },
    ]} />
  </section>
{:else if id === "ifInstall" && p}
  <section class="card sb">
    <h2>{t("stats.ifInstall")} <span class="faint">{p.complete_now} → {proj.at(-1)}</span></h2>
    {#if p.projection.length}
      <Lines xs={projX} unit={(n) => t("stats.completeUnit", { n })} height={150} series={[{ label: t("stats.completeSeries"), color: "var(--fix)", values: proj }]} />
      <p class="small faint">{hover[0]}<a href="#/repair">{t("stats.repairLink")}</a>{hover[1] ?? ""}</p>
    {:else}<p class="small faint">{t("stats.nothingToInstall")}</p>{/if}
  </section>
{:else if id === "stagesDays" && p && byDay.length > 1}
  <section class="card sb">
    <h2>{t("stats.stagesOverDays")}</h2>
    <div class="days">
      {#each byDay as [d, x] (d)}
        <div class="drow"><span class="small faint">{d}</span><StackBar height={8} unit={nSongs} parts={stageParts(x.stages)} /></div>
      {/each}
    </div>
  </section>
{:else if id === "doneMonth" && p && months.length}
  <section class="card sb">
    <h2>{t("stats.doneByMonth")}</h2>
    <Columns cols={months} height={80} unit={(n) => t("stats.doneUnit", { n })} />
  </section>
{/if}

<style>
  h3 { margin: 16px 0 6px; font-size: 12.5px; font-weight: 600; color: var(--muted); }
  .link { border: 0; background: none; padding: 0; font: inherit; color: var(--ink); cursor: pointer; text-decoration: underline dotted var(--faint); text-underline-offset: 3px; }
  .link:hover { text-decoration-color: var(--ink); }
  .modes { margin: 0 0 8px; }
  .sb > .link { margin-top: 6px; color: var(--muted); }
  .sb h2 { display: flex; justify-content: space-between; align-items: baseline; }
  .sb h2 .faint { font-weight: 400; font-size: 12px; }
  .artists { display: flex; flex-direction: column; gap: 4px; }
  .arow { display: grid; grid-template-columns: minmax(0, 38%) 2.4em 1fr; align-items: center; gap: 8px; color: var(--ink); padding: 3px 4px; border-radius: 6px; }
  .arow:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
  .an { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
  .nn { text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 12px; }
  .live { margin-left: 6px; font-size: 10px; padding: 0 5px; border-radius: 4px; border: 1px solid var(--line); color: var(--muted); }
  .more { display: inline-block; margin-top: 12px; }
  .days { display: flex; flex-direction: column; gap: 7px; }
  .drow { display: grid; grid-template-columns: 6.5em 1fr; align-items: center; gap: 10px; }
</style>

<script lang="ts">
  // Home: where things stand at a glance (four tiles with a small chart each), what to
  // pick up, what to validate. The statistics have their own page.
  import { app } from "../lib/state.svelte";
  import SongCard from "../lib/components/SongCard.svelte";
  import Stage from "../lib/components/Stage.svelte";
  import StackBar from "../lib/charts/StackBar.svelte";
  import Columns from "../lib/charts/Columns.svelte";
  import { stageLabel, STAGES } from "../lib/format";
  import { t, fmtDate } from "../lib/i18n/index.svelte";
  import { f } from "../lib/filters.svelte";
  import * as api from "../lib/api";
  import type { HomeStats, Progress } from "../lib/api";

  const o = $derived(app.overview);
  const inprog = $derived(o ? (o.stages["mix"] ?? 0) + (o.stages["master"] ?? 0) : 0);

  let h = $state<HomeStats | null>(null);
  let p = $state<Progress | null>(null);
  let seq = 0;
  $effect(() => {
    app.version; // reload after every change
    const n = ++seq; // the newest answer only
    api.homeStats().then((x) => n === seq && (h = x)).catch(() => n === seq && (h = null));
    api.progress().then((x) => n === seq && (p = x)).catch(() => n === seq && (p = null));
  });

  // "Reprendre": whole rows only, at least ~10 cards, whatever the width
  let cardsW = $state(0);
  const cols = $derived(Math.max(1, Math.floor((cardsW + 12) / (230 + 12))));
  const nRecent = $derived(cols * Math.max(2, Math.ceil(10 / cols)));

  const ORDER = [...STAGES, "none"];
  const stageParts = (c: Record<string, number>) =>
    ORDER.map((k) => ({ key: k, label: stageLabel(k), n: c[k] ?? 0, color: `var(--s-${k})`, href: "#/songs" }));
  const SEV = ["ok", "fix", "lost", "bad"];
  const nSongs = (n: number) => t("songs.nSongs", { n });
  const nProjects = (n: number) => t("stats.nProjects", { n });
  const SEVC = ["var(--ok)", "var(--fix)", "var(--lost)", "var(--bad)"];
  const opens = $derived(h && h.files ? Math.round((100 * (h.sev[0] + h.sev[1])) / h.files) : 0);
  const recentWeeks = $derived(h ? h.weeks.slice(-4).reduce((a, b) => a + b, 0) : 0);
  // a song worked on in several of the last 4 weeks counts in each: say "songs-weeks" plainly
  const weekCols = $derived(
    (h?.weeks ?? []).map((n, i, a) => {
      const d = new Date(Date.now() - (a.length - 1 - i) * 7 * 864e5);
      const lab = fmtDate(d, { day: "numeric", month: "short" });
      return { label: lab, n, title: t("home.tile.week", { date: lab }) };
    }),
  );

  const sub = $derived(
    o
      ? [
          nSongs(o.songs),
          h && nProjects(h.files),
          inprog && t("home.inprog", { n: inprog }),
          h && t("home.opensPct", { n: opens }),
        ]
          .filter(Boolean)
          .join(" · ")
      : "",
  );

  // charts that open the songs list, filtered
  function toStage(k: string) {
    f.stages = k === "none" ? [] : [k];
    f.artist = "";
    location.hash = "#/songs";
  }
</script>

{#if o}
  <div class="page">
    <h1>{t("home.title")}</h1>
    <p class="sub muted">
      {sub}
      · <a href="#/stats">{t("home.allStats")}</a>
    </p>

    {#if h}
      <div class="tiles">
        <a class="tile card" href="#/songs" onclick={() => (f.stages = [])}>
          <div class="tt"><span class="small muted">{t("home.tile.songs")}</span><b>{o.songs}</b></div>
          <StackBar parts={stageParts(o.stages)} height={8} pick={toStage} />
          <div class="small faint">{t("home.tile.songsFoot", { done: o.stages["done"] ?? 0, inprog })}</div>
        </a>
        <a class="tile card" href="#/repair">
          <div class="tt"><span class="small muted">{t("home.tile.opens")}</span><b>{t("stats.pct", { n: opens })}</b></div>
          <StackBar parts={h.sev.map((n, i) => ({ key: String(i), label: t(`songs.sev.${SEV[i]}`), n, color: SEVC[i] }))} height={8} unit={nProjects} />
          <div class="small faint">{t("home.tile.opensFoot", { fix: h.sev[1], bad: h.sev[3] })}</div>
        </a>
        <div class="tile card">
          <div class="tt"><span class="small muted">{t("home.tile.activity")}</span><b>{recentWeeks}</b></div>
          <Columns cols={weekCols} height={34} mini unit={(n) => t("home.tile.savedSongs", { n })} />
          <div class="small faint">{t("home.tile.activityFoot")}</div>
        </div>
        <a class="tile card" href="#/songs">
          <div class="tt"><span class="small muted">{t("home.tile.audio")}</span><b>{h.with_audio}<span class="of">/{h.total}</span></b></div>
          <StackBar parts={[{ key: "a", label: t("songs.filter.audioYes"), n: h.with_audio, color: "var(--accent)" }, { key: "b", label: t("songs.filter.audioNo"), n: h.total - h.with_audio, color: "var(--line)" }]} height={8} unit={nSongs} />
          <div class="small faint">{t("home.tile.audioFoot", { n: h.total ? Math.round((100 * h.with_audio) / h.total) : 0 })}</div>
        </a>
      </div>
    {/if}

    <div class="grid">
      <div class="main">
        <section>
          <div class="head"><h2>{t("home.resume")}</h2><a href="#/songs" class="small">{t("home.allSongs")}</a></div>
          <div class="cards" bind:clientWidth={cardsW}>{#each o.recent.slice(0, nRecent) as s (s.id)}<SongCard song={s} />{/each}</div>
        </section>
        <section class="card">
          <h2>{t("home.almost")}</h2>
          {#each o.close.slice(0, 6) as s (s.id)}
            <div class="row">
              <div><a class="lk" href="#/song/{s.id}"><b>{s.title}</b></a><div class="small muted">{[s.artist, s.style].filter(Boolean).join(" · ")}</div></div>
              <div class="r small"><Stage stage={s.stage} /><div class="faint">{s.check}/{s.checks}</div></div>
            </div>
          {/each}
        </section>
      </div>
      <section class="card side">
        <div class="head"><h2>{t("home.validate")} <span class="faint">{o.queue.length}</span></h2>
          {#if o.queue.length > 1}<button class="btn sm" title={t("home.acceptAllTitle")}
            onclick={() => app.act(api.acceptAll(o.queue.map((q) => q.id)))}>{t("home.acceptAll")}</button>{/if}</div>
        {#each o.queue.slice(0, 6) as q (q.id)}
          <div class="row">
            <div><a class="lk" href="#/song/{q.song_id}"><b>{q.title}</b></a>
              <div class="small" title={q.reasons.join("\n")}>{#if q.stage}<Stage stage={q.stage} /> →{/if} <Stage stage={q.value} /></div>
            </div>
            <div class="yn">
              <button class="ok" title={t("home.accept")} aria-label={t("home.accept")} onclick={() => app.act(api.suggestionAct(q.id, true))}>✓</button>
              <button title={t("home.refuse")} aria-label={t("home.refuse")} onclick={() => app.act(api.suggestionAct(q.id, false))}>✕</button>
            </div>
          </div>
        {/each}
        {#if !o.queue.length}<p class="small faint">{t("home.nothing")}</p>{/if}
      </section>
    </div>

  </div>
{/if}

<style>
  .page { padding: 24px 28px 60px; }
  .sub { margin: 0 0 18px; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr)); gap: 12px; margin-bottom: 20px; }
  .tile { display: flex; flex-direction: column; gap: 9px; color: inherit; transition: border-color .15s; }
  a.tile:hover { border-color: var(--faint); }
  .tt { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .tt b { font-size: 24px; font-variant-numeric: tabular-nums; letter-spacing: -0.01em; }
  .of { font-size: 14px; color: var(--faint); font-weight: 500; }
  .grid { display: grid; grid-template-columns: minmax(0, 2.4fr) minmax(320px, 1fr); gap: 18px; align-items: start; }
  @media (max-width: 1150px) { .grid { grid-template-columns: minmax(0, 1fr); } }
  .main { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 10px; }
  .head h2 { margin: 0; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .row:last-child { border-bottom: 0; }
  .r { text-align: right; }
  .lk { color: inherit; }
  .lk:hover { color: var(--accent); }
  h3 { margin: 16px 0 6px; font-size: 12.5px; font-weight: 600; color: var(--muted); }
  .chips { display: flex; gap: 5px; flex-wrap: wrap; justify-content: flex-end; }
  .yn { display: flex; gap: 4px; flex: none; }
  .yn button { width: 26px; height: 26px; border-radius: 7px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); cursor: pointer; }
  .yn button:hover { color: var(--bad); border-color: var(--bad); }
  .yn .ok:hover { color: var(--ok); border-color: var(--ok); }
  .chip { cursor: pointer; background: var(--panel); color: inherit; font-size: 11.5px; padding: 2px 8px; border-radius: 99px; border: 1px solid var(--line); white-space: nowrap; }

</style>

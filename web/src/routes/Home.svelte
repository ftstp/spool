<script lang="ts">
  import { widthOf } from "../lib/size";
  // Home: what to do now, laid out by the user (which sections, in which order; kept in this
  // window's storage). Its own sections only: understanding the catalogue is the Statistics
  // page's job, so its charts are not offered here; "Progress" alone is, short (where things
  // stand), its per-artist list left to Statistics.
  import { app } from "../lib/state.svelte";
  import { lastStats } from "../lib/homeStats.svelte";
  import SongCard from "../lib/components/SongCard.svelte";
  import PlayBtn from "../lib/components/PlayBtn.svelte";
  import FavStar from "../lib/components/FavStar.svelte";
  import { favs, favsOf, loadFavs, openFav } from "../lib/favorites.svelte";
  import { player } from "../lib/player.svelte";
  import { midi } from "../lib/midiPlayer.svelte";
  const FAV_KINDS = ["song", "artist", "release", "project", "board", "sample", "midi", "plugin"];
  void loadFavs();
  import Stage from "../lib/components/Stage.svelte";
  import StackBar from "../lib/charts/StackBar.svelte";
  import Columns from "../lib/charts/Columns.svelte";
  import StatBlock from "../lib/components/StatBlock.svelte";
  import { stageLabel, STAGES } from "../lib/format";
  import { t, fmtDate } from "../lib/i18n/index.svelte";
  import { f } from "../lib/filters.svelte";
  import * as api from "../lib/api";
  import { openMenu, type MenuItem } from "../lib/menu.svelte";
  import { dragSource, dropTarget, pdrag, type DragItem } from "../lib/pdrag.svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import type { HomeStats, Progress } from "../lib/api";

  const o = $derived(app.overview);
  const inprog = $derived(o ? (o.stages["mix"] ?? 0) + (o.stages["master"] ?? 0) : 0);

  let h = $state<HomeStats | null>(lastStats.h);
  let p = $state<Progress | null>(lastStats.p);
  let seq = 0;
  $effect(() => {
    app.version; // reload after every change
    const n = ++seq; // the newest answer only
    api.homeStats().then((x) => n === seq && (h = lastStats.h = x)).catch(() => n === seq && (h = null));
    api.progress().then((x) => n === seq && (p = lastStats.p = x)).catch(() => n === seq && (p = null));
  });

  // "Reprendre": whole rows only, three by default; how many is the user's (its menu, or
  // the button beside its title), up to the 24 songs the base gives
  let cardsW = $state(0);
  const cols = $derived(Math.max(1, Math.floor((cardsW + 12) / (230 + 12))));
  const ROWS = "spool.home.resumeRows";
  let rows = $state((() => { try { return Math.max(1, Math.min(6, Number(localStorage.getItem(ROWS)) || 3)); } catch { return 3; } })());
  const maxRows = $derived(Math.max(1, Math.floor((o?.recent.length ?? 0) / cols)));
  const nRecent = $derived(cols * Math.min(rows, maxRows));
  function setRows(n: number) {
    rows = n;
    try { localStorage.setItem(ROWS, String(n)); } catch { /* not kept */ }
  }
  const rowItems = (): MenuItem[] =>
    [1, 2, 3, 4, 5, 6].filter((n) => n <= Math.max(maxRows, 3)).map((n) => ({ label: t("home.rows", { n }), checked: rows === n, run: () => setRows(n) }));
  function rowsMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu({ preventDefault() {}, stopPropagation() {}, clientX: r.left, clientY: r.bottom + 4 } as MouseEvent, rowItems(), t("home.rowsTitle"));
  }

  const ORDER = [...STAGES, "none"];
  const stageParts = (c: Record<string, number>) =>
    ORDER.map((k) => ({ key: k, label: stageLabel(k), n: c[k] ?? 0, color: `var(--s-${k})`, href: "#/songs" }));
  const SEV = ["ok", "fix", "lost", "bad"];
  const nSongs = (n: number) => t("songs.nSongs", { n });
  const nProjects = (n: number) => t("stats.nProjects", { n });
  const SEVC = ["var(--ok)", "var(--fix)", "var(--lost)", "var(--bad)"];
  // ---- the figures: about finishing songs (what is done, what is close, what fell asleep)
  // and working regularly. Each opens the songs it counts.
  const year = new Date().getFullYear();
  const doneYear = $derived((p?.done_by_month ?? []).filter((m) => m.label.startsWith(`${year}-`)).reduce((a, m) => a + m.n, 0));
  const doneLast = $derived((p?.done_by_month ?? []).filter((m) => m.label.startsWith(`${year - 1}-`)).reduce((a, m) => a + m.n, 0));
  const doneCols = $derived(
    Array.from({ length: new Date().getMonth() + 1 }, (_, i) => {
      const key = `${year}-${String(i + 1).padStart(2, "0")}`;
      const lab = fmtDate(new Date(year, i, 1), { month: "short" });
      return { label: lab, n: (p?.done_by_month ?? []).find((m) => m.label === key)?.n ?? 0, title: lab };
    }),
  );
  // the songs of "To validate", for their play buttons (one list, played through)
  const songOf = (id: number) => app.songs.find((x) => x.id === id);
  const validateSongs = $derived(o ? o.queue.map((q) => songOf(q.song_id)).filter((x): x is NonNullable<typeof x> => !!x) : []);
  const DAY = 86400;
  const now = Date.now() / 1000;
  const live = $derived(app.songs.filter((x) => !x.archived));
  const closeSongs = $derived(live.filter((x) => x.stage === "mix" || x.stage === "master"));
  const closeNoAudio = $derived(closeSongs.filter((x) => !x.temoin).length);
  // asleep: arranged or further on, and not saved for three months
  const ASLEEP_STAGES = ["arrangement", "mix", "master"];
  const asleep = $derived(live.filter((x) => ASLEEP_STAGES.includes(x.stage ?? "") && now - x.mtime > 90 * DAY));
  const countBy = (l: { stage: string | null }[]) => l.reduce<Record<string, number>>((m, x) => ((m[x.stage ?? "none"] = (m[x.stage ?? "none"] ?? 0) + 1), m), {});
  // weeks with something saved, of the last four (and of the 26 shown)
  const activeWeeks = $derived(h ? h.weeks.slice(-4).filter((n) => n > 0).length : 0);
  const activeAll = $derived(h ? h.weeks.filter((n) => n > 0).length : 0);
  // the songs list, with only what a figure counts
  function goSongs(stages: string[], activity: "" | "asleep" = "") {
    f.stages = stages;
    f.artist = [];
    f.style = [];
    f.key = [];
    f.audio = "";
    f.plugins = [];
    f.rating = 0;
    f.kind = [];
    f.activity = activity;
    f.view = "list";
    location.hash = "#/songs";
  }
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
          inprog && t("home.inprog", { n: inprog }),
        ]
          .filter(Boolean)
          .join(" · ")
      : "",
  );

  // ---- the layout: one list, in the user's order, of the figures (small tiles) and the
  // blocks. Every one of them can be moved anywhere, removed, and added back.
  const TILES = ["done", "close", "asleep", "activity"] as const;
  // the figures before 0.1.39 (about repairing projects): replaced by these, where they were
  const OLD_TILES = ["songs", "opens", "audio"];
  const BLOCKS = ["resume", "favs", "almost", "validate", "progress"] as const;
  type Tile = (typeof TILES)[number];
  type Block = (typeof BLOCKS)[number];
  type Item = Tile | Block;
  const ALL: readonly Item[] = [...TILES, ...BLOCKS];
  const isTile = (x: Item): x is Tile => (TILES as readonly string[]).includes(x);
  const DEFAULT: Item[] = [...TILES, "resume", "favs", "almost", "validate"];
  const LAYOUT = "spool.home.layout";
  const FAVS_SEEN = "spool.home.favsSeen";
  function loadLayout(): Item[] {
    try {
      const v = JSON.parse(localStorage.getItem(LAYOUT) || "null");
      // before 0.1.37: the figures and the blocks in two lists, the figures first
      const l: unknown[] | null = Array.isArray(v) ? v : v && Array.isArray(v.tiles) && Array.isArray(v.blocks) ? [...v.tiles, ...v.blocks] : null;
      if (l) {
        const at = l.findIndex((x) => OLD_TILES.includes(x as string));
        if (at >= 0) l.splice(at, 0, ...TILES.filter((x) => !l.includes(x)));
        // the favourites (0.1.66): shown once, after "Pick up", then where the user leaves them
        if (!l.includes("favs") && !localStorage.getItem(FAVS_SEEN)) {
          const r = l.indexOf("resume");
          l.splice(r >= 0 ? r + 1 : l.length, 0, "favs");
          localStorage.setItem(LAYOUT, JSON.stringify(l));
        }
        localStorage.setItem(FAVS_SEEN, "1");
        return [...new Set(l.filter((x): x is Item => ALL.includes(x as Item)))];
      }
    } catch {
      /* the default */
    }
    return [...DEFAULT];
  }
  let layout = $state<Item[]>(loadLayout());
  function keepLayout() {
    try {
      localStorage.setItem(LAYOUT, JSON.stringify(layout));
    } catch {
      /* not kept */
    }
  }
  let queueAll = $state(false);
  function removeItem(id: Item) {
    layout = layout.filter((x) => x !== id);
    keepLayout();
  }
  // an item taken to a given place in the list (added when it was not shown)
  function place(id: Item, to: number) {
    const l = layout.filter((x) => x !== id);
    l.splice(Math.max(0, Math.min(to, l.length)), 0, id);
    if (l.join() === layout.join()) return;
    layout = l;
    keepLayout();
  }
  const hidden = $derived(ALL.filter((x) => !layout.includes(x)));
  const labelOf = (x: Item) => (isTile(x) ? t(`home.tile.${x}`) : t(`home.blocks.${x}`));

  // Rearranging: switched on from any section's right-click menu. Each section can then be
  // dragged; the others make room as it passes. Escape or "Done" ends it.
  // Where it goes is worked out from where the sections sit, not from what is under the
  // pointer: the sections slide while they make room, and one sliding under the pointer
  // used to send the dragged one back, so it seemed to hold on before moving.
  let reorg = $state(false);
  let grid = $state<HTMLElement | null>(null);

  // Every row is full, and the sections of a row are the same height: no holes. On a grid of
  // 12 columns, the figures go up to four a row, the blocks two a row, "Pick up" alone; a row
  // left with fewer shares the whole width (three figures at a third each, a block alone
  // across). Fewer a row when the window is narrow.
  let gridW = $state(0);
  const shown = $derived(layout.filter((x) => !isStat(x) || h));
  const spans = $derived.by(() => {
    const perTile = gridW >= 1000 ? 4 : gridW >= 520 ? 2 : 1;
    const perBlock = gridW >= 860 ? 2 : 1;
    const out: Record<string, number> = {};
    for (let i = 0; i < shown.length; ) {
      const x = shown[i];
      if (x === "resume") {
        out[x] = 12;
        i++;
        continue;
      }
      // a run of figures, or of blocks, cut into rows
      let j = i;
      while (j < shown.length && shown[j] !== "resume" && isTile(shown[j]) === isTile(x)) j++;
      const run = shown.slice(i, j);
      const per = isTile(x) ? perTile : perBlock;
      for (let k = 0; k < run.length; k += per) {
        const row = run.slice(k, k + per);
        for (const y of row) out[y] = 12 / row.length;
      }
      i = j;
    }
    return out;
  });
  const itemOf = (x: Item): DragItem => ({ kind: "home-item", data: x, label: labelOf(x) });
  const dragging = $derived(reorg ? (pdrag.item?.data as string | undefined) : undefined);
  const gridTarget = {
    accept: (it: DragItem) => it.kind === "home-item",
    over: (it: DragItem, e: PointerEvent) => {
      if (!grid) return;
      const id = it.data as Item;
      const g = grid.getBoundingClientRect();
      const x = e.clientX - g.left, y = e.clientY - g.top;
      // the cells where they are laid out (offsets ignore the slide under way)
      const cells = [...grid.querySelectorAll<HTMLElement>(":scope > .cell")].map((el) => ({
        id: el.dataset.id as Item, l: el.offsetLeft, t: el.offsetTop, r: el.offsetLeft + el.offsetWidth, b: el.offsetTop + el.offsetHeight,
      }));
      const others = cells.filter((c) => c.id !== id);
      if (!others.length) return;
      // the row under the pointer (or the nearest), then the place in it: before the first
      // cell whose middle is right of the pointer
      let best = others[0], bd = Infinity;
      for (const c of others) {
        const dy = y < c.t ? c.t - y : y > c.b ? y - c.b : 0;
        const dx = x < c.l ? c.l - x : x > c.r ? x - c.r : 0;
        const d = dy * 4 + dx;
        if (d < bd) (bd = d), (best = c);
      }
      const row = others.filter((c) => c.t < best.b && c.b > best.t);
      const after = row.filter((c) => x >= (c.l + c.r) / 2);
      const anchor = after.length ? after[after.length - 1] : null;
      const order = others.map((c) => c.id);
      const to = anchor ? order.indexOf(anchor.id) + 1 : order.indexOf(row[0].id);
      place(id, to);
    },
    drop: () => {},
  };

  // what can be added, at `at` in the list: the blocks, then the figures
  function addItems(at: number): MenuItem[] {
    const blocks = hidden.filter((x) => !isTile(x));
    const tiles = hidden.filter(isTile);
    const items: MenuItem[] = blocks.map((x) => ({ label: labelOf(x), run: () => place(x, at) }));
    if (tiles.length) {
      if (items.length) items.push({ sep: true });
      items.push({ label: t("home.layout.tiles"), sub: tiles.map((x) => ({ label: labelOf(x), run: () => place(x, at) })) });
    }
    return items;
  }
  function addMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu({ preventDefault() {}, stopPropagation() {}, clientX: r.left, clientY: r.bottom + 4 } as MouseEvent, addItems(layout.length), t("home.layout.addSection"));
  }

  // right-click on a section: rearrange, remove, add a section right after it. A song inside
  // a block keeps its own menu.
  function itemMenu(e: MouseEvent, id: Item) {
    if (!reorg && (e.target as HTMLElement).closest("[data-song], a[href^='#/song/'], input, textarea")) return;
    const add = addItems(layout.indexOf(id) + 1);
    const items: MenuItem[] = [
      { label: reorg ? t("home.layout.reorgEnd") : t("home.layout.reorg"), checked: reorg, run: () => (reorg = !reorg) },
      { sep: true },
      ...(id === "resume" ? [{ label: t("home.rowsTitle"), sub: rowItems() }] : []),
      add.length ? { label: t("home.layout.add"), sub: add } : { label: t("home.layout.nothingToAdd"), disabled: true },
      { label: t("home.layout.remove"), danger: true, run: () => removeItem(id) },
      { sep: true },
      { label: t("home.layout.reset"), run: resetLayout },
    ];
    openMenu(e, items, labelOf(id));
  }

  function resetLayout() {
    layout = [...DEFAULT];
    keepLayout();
  }
  const isStat = (b: Item): b is "progress" => b === "progress";

  // charts that open the songs list, filtered
  function toStage(k: string) {
    f.stages = k === "none" ? [] : [k];
    f.artist = [];
    location.hash = "#/songs";
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !pdrag.item && (reorg = false)} />

{#if o}
  <div class="page">
    <h1>{t("home.title")}</h1>
    <div class="top">
      <p class="sub muted">
        {sub}
        · <a href="#/stats">{t("home.allStats")}</a>
      </p>
    </div>
    {#if reorg}
      <div class="reorgbar" role="status"><span>{t("home.layout.reorgHint")}</span><button class="btn sm primary" onclick={() => (reorg = false)}>{t("home.layout.done")}</button></div>
    {/if}

    <!-- the figures are there at once, at their size: what takes a moment fills them in
         place, so nothing below moves when it arrives -->
    <div class="blocks" class:reorg class:wait={!h} bind:this={grid} use:widthOf={(x) => (gridW = x)} use:dropTarget={gridTarget}>
      {#each shown as b (b)}
        <div class="cell" class:tilec={isTile(b)} class:wide={b === "resume"} class:moving={dragging === b} data-id={b} style:grid-column="span {spans[b] ?? 12}"
          animate:flip={{ duration: 160, easing: cubicOut }} oncontextmenu={(e) => itemMenu(e, b)} role="presentation"
          onclickcapture={(e) => { if (reorg && !(e.target as Element).closest(".rm")) { e.preventDefault(); e.stopPropagation(); } }}
          use:dragSource={reorg ? itemOf(b) : null}>
        {#if reorg}<button class="rm" title={t("home.layout.remove")} aria-label={`${t("home.layout.remove")}: ${labelOf(b)}`} onclick={() => removeItem(b)}>×</button>{/if}
        {#if b === "done"}
            <a class="tile card" href="#/songs" onclick={(e) => { e.preventDefault(); goSongs(["done"]); }}>
              <div class="tt"><span class="small muted">{t("home.tile.done", { year })}</span><b>{p ? doneYear : "…"}</b></div>
              <div class="viz">{#if p}<Columns cols={doneCols} height={34} mini unit={(n) => t("home.tile.doneUnit", { n })} />{:else}<i class="ph cols"></i>{/if}</div>
              <div class="small faint foot">{p ? (doneYear || doneLast ? t("home.tile.doneFoot", { n: doneLast, year: year - 1 }) : t("home.tile.doneNone")) : " "}</div>
            </a>
        {:else if b === "close"}
            <a class="tile card" href="#/songs" onclick={(e) => { e.preventDefault(); goSongs(["mix", "master"]); }}>
              <div class="tt"><span class="small muted">{t("home.tile.close")}</span><b>{closeSongs.length}</b></div>
              <div class="viz"><StackBar parts={stageParts(countBy(closeSongs)).filter((x) => x.n)} height={8} unit={nSongs} /></div>
              <div class="small faint foot">{t("home.tile.closeFoot", { mix: o.stages["mix"] ?? 0, master: o.stages["master"] ?? 0 })}{#if closeNoAudio} · <span class="warn">{t("home.tile.noAudio", { n: closeNoAudio })}</span>{/if}</div>
            </a>
        {:else if b === "asleep"}
            <a class="tile card" href="#/songs" onclick={(e) => { e.preventDefault(); goSongs(ASLEEP_STAGES, "asleep"); }}>
              <div class="tt"><span class="small muted">{t("home.tile.asleep")}</span><b>{asleep.length}</b></div>
              <div class="viz"><StackBar parts={stageParts(countBy(asleep)).filter((x) => x.n)} height={8} unit={nSongs} /></div>
              <div class="small faint foot">{t("home.tile.asleepFoot")}</div>
            </a>
        {:else if b === "activity"}
            <div class="tile card">
              <div class="tt"><span class="small muted">{t("home.tile.activity")}</span><b>{#if h}{activeWeeks}<span class="of">/4</span>{:else}…{/if}</b></div>
              <div class="viz">{#if h}<Columns cols={weekCols} height={34} mini unit={(n) => t("home.tile.savedSongs", { n })} />{:else}<i class="ph cols"></i>{/if}</div>
              <div class="small faint foot">{h ? t("home.tile.activityFoot", { n: activeAll, total: h.weeks.length }) : " "}</div>
            </div>
        {:else if b === "resume"}
          <section class="wide">
            <div class="head"><h2>{t("home.resume")}</h2><span class="hr"><button class="link small rowsbtn" title={t("home.rowsTitle")} onclick={rowsMenu}>{t("home.rows", { n: rows })} ▾</button><a href="#/songs" class="small">{t("home.allSongs")}</a></span></div>
            <div class="cards" use:widthOf={(x) => (cardsW = x)}>{#each o.recent.slice(0, nRecent) as s (s.id)}<SongCard song={s} queue={{ songs: o.recent.slice(0, nRecent), name: t("home.resume") }} />{/each}</div>
          </section>
        {:else if b === "favs"}
          <section class="card favs">
            <h2>{t("common.fav.title")} <span class="faint">{favs.list.length || ""}</span></h2>
            {#if !favs.list.length}<p class="small muted">{t("common.fav.none")}</p>{/if}
            {#each FAV_KINDS.filter((k) => favsOf(k).length) as k (k)}
              <h3 class="small muted">{t(`common.fav.kind.${k}`)}</h3>
              {#each favsOf(k).slice(0, 8) as x (x.id)}
                <div class="row fr">
                  <FavStar kind={x.kind} id={x.id} label={x.label} sub={x.sub} song={x.song} size={14} />
                  {#if k === "sample"}<button class="rpl" title={t("samples.play")} aria-label={t("samples.play")} onclick={() => player.play(x.id, null, x.label, 0, null)}>▶</button>{/if}
                  {#if k === "midi"}<button class="rpl" title={t("midi.player.play")} aria-label={t("midi.player.play")} onclick={() => midi.play(x.id, "", x.label)}>▶</button>{/if}
                  <button class="grow lk fl" onclick={() => openFav(x)}><b>{x.label}</b>{#if x.sub}<span class="small muted">{" · "}{x.sub}</span>{/if}</button>
                </div>
              {/each}
              {#if favsOf(k).length > 8}<p class="small faint">+{favsOf(k).length - 8}</p>{/if}
            {/each}
          </section>
        {:else if b === "almost"}
          <section class="card">
            <h2>{t("home.almost")}</h2>
            {#each o.close.slice(0, 6) as s (s.id)}
              <div class="row">
                <span class="rp">{#if s.temoin || s.rel}<PlayBtn sid={s.id} title={s.title ?? ""} size={26} queue={{ songs: o.close.slice(0, 6), name: t("home.almost") }} project={s.temoin ? "" : s.rel} />{/if}</span>
                <div class="grow"><a class="lk" href="#/song/{s.id}"><b>{s.title}</b></a><div class="small muted">{[s.artist, s.style].filter(Boolean).join(" · ")}</div></div>
                <div class="r small"><Stage stage={s.stage} /><div class="faint">{s.check}/{s.checks}</div></div>
              </div>
            {/each}
          </section>
        {:else if b === "validate"}
          <section class="card">
            <div class="head"><h2>{t("home.validate")} <span class="faint">{o.queue.length}</span></h2>
              {#if o.queue.length > 1}<button class="btn sm" title={t("home.acceptAllTitle", { n: o.queue.length })}
                onclick={() => app.act(api.acceptAll(o.queue.map((q) => q.id)))}>{t("home.acceptAll", { n: o.queue.length })}</button>{/if}</div>
            <!-- the first ones, and the rest on demand: "Accept all" says what it is about to accept -->
            <div class="queue" class:all={queueAll}>
            {#each queueAll ? o.queue : o.queue.slice(0, 6) as q (q.id)}
              {@const qs = songOf(q.song_id)}
              <div class="row">
                <span class="rp">{#if qs && (qs.temoin || qs.rel)}<PlayBtn sid={qs.id} title={qs.title ?? ""} size={26} queue={{ songs: validateSongs, name: t("home.validate") }} project={qs.temoin ? "" : qs.rel} />{/if}</span>
                <div class="grow"><a class="lk" href="#/song/{q.song_id}"><b>{q.title}</b></a>
                  <div class="small" title={q.reasons.join("\n")}>{#if q.stage}<Stage stage={q.stage} /> →{/if} <Stage stage={q.value} /></div>
                </div>
                <div class="yn">
                  <button class="ok" title={t("home.accept")} aria-label={t("home.accept")} onclick={() => app.act(api.suggestionAct(q.id, true))}>✓</button>
                  <button title={t("home.refuse")} aria-label={t("home.refuse")} onclick={() => app.act(api.suggestionAct(q.id, false))}>✕</button>
                </div>
              </div>
            {/each}
            </div>
            {#if o.queue.length > 6}
              <button class="link small more" aria-expanded={queueAll} onclick={() => (queueAll = !queueAll)}>
                {queueAll ? `▴ ${t("home.queueLess")}` : `▾ ${t("home.queueMore", { n: o.queue.length - 6 })}`}
              </button>
            {/if}
            {#if !o.queue.length}<p class="small faint">{t("home.nothing")}</p>{/if}
          </section>
        {:else if isStat(b) && h}
          <div class="sw"><StatBlock id={b} {o} {h} {p} short /></div>
        {/if}
        </div>
      {/each}
      <!-- adding a section: a quiet button that shows when the pointer is on the page -->
      {#if hidden.length}
        <button class="addsec" class:show={reorg || !layout.length} onclick={addMenu}>+ {t("home.layout.addSection")}</button>
      {/if}
    </div>
    {#if !layout.length}<p class="faint">{t("home.layout.empty")}</p>{/if}

  </div>
{/if}

<style>
  .page { padding: 24px 28px 60px; }
  .top { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; margin: 0 0 18px; }
  .sub { margin: 0; }
  /* the place a chart takes while its figures come (a bar, or the weeks' columns) */
  .ph { display: block; border-radius: 4px; background: var(--panel2); }
  .ph.bar { height: 8px; }
  .ph.cols { height: 34px; }
  .blocks.wait .tt b { color: var(--faint); }
  .tile { display: flex; flex-direction: column; gap: 9px; color: inherit; transition: border-color .15s; }
  a.tile:hover { border-color: var(--faint); }
  .tt { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .tt b { font-size: 24px; font-variant-numeric: tabular-nums; letter-spacing: -0.01em; }
  .warn { color: var(--fix); }
  .of { font-size: 14px; color: var(--faint); font-weight: 500; }
  /* the blocks in the user's order: two or three a row, "Pick up" across the whole width */
  /* one list, wrapped: the figures small (about four a row), the blocks two or three a row,
     "Pick up" across the whole width */
  .blocks { position: relative; display: grid; grid-template-columns: repeat(12, minmax(0, 1fr)); gap: 16px; align-items: stretch; }
  .blocks .wide { min-width: 0; }
  /* a section fills its cell: the cards of a row end on the same line */
  .blocks .cell > section, .sw, .sw > :global(section) { height: 100%; box-sizing: border-box; }
  .sw { display: flex; flex-direction: column; }
  /* the figures: the same parts at the same heights (number, chart slot, a two-line note) */
  .viz { height: 34px; display: flex; flex-direction: column; justify-content: center; }
  .viz > :global(*) { width: 100%; }
  .foot { margin-top: auto; min-height: 2.6em; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .head { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 10px; }
  .head h2 { margin: 0; }
  .hr { display: flex; align-items: baseline; gap: 14px; }
  .rowsbtn { border: 0; background: none; padding: 0; font: inherit; cursor: pointer; color: var(--faint); }
  .rowsbtn:hover { color: var(--ink); }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .row:last-child { border-bottom: 0; }
  .rp { flex: none; width: 26px; display: grid; place-items: center; }
  .row .grow { flex: 1; min-width: 0; }
  .r { text-align: right; }
  .lk { color: inherit; }
  .lk:hover { color: var(--accent); }
  .queue.all { max-height: 420px; overflow-y: auto; margin-right: -6px; padding-right: 6px; }
  .more { margin-top: 8px; }
  .yn { display: flex; gap: 4px; flex: none; }
  .yn button { width: 26px; height: 26px; border-radius: 7px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); cursor: pointer; }
  .yn button:hover { color: var(--bad); border-color: var(--bad); }
  .yn .ok:hover { color: var(--ok); border-color: var(--ok); }
  /* rearranging: every section outlined and grabbed whole (nothing inside it reacts) */
  .reorgbar { position: sticky; top: 8px; z-index: 20; display: flex; align-items: center; justify-content: space-between; gap: 12px; margin: 0 0 14px; padding: 8px 10px 8px 14px; border-radius: var(--radius); background: var(--accent-bg); border: 1px solid var(--accent); font-size: 13px; }
  .cell { min-width: 0; }
  .cell { position: relative; }
  .blocks .cell > :global(.tile) { height: 100%; box-sizing: border-box; }
  .reorg .cell { cursor: grab; border-radius: var(--radius); outline: 1.5px dashed color-mix(in srgb, var(--accent) 55%, transparent); outline-offset: 3px; }
  .reorg .cell > :global(*:not(.rm)) { pointer-events: none; }
  /* removing a section while rearranging */
  .rm { position: absolute; top: -9px; right: -9px; z-index: 2; width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--line); background: var(--panel); color: var(--muted); font-size: 14px; line-height: 1; cursor: pointer; display: grid; place-items: center; padding: 0; box-shadow: 0 2px 8px rgba(0, 0, 0, .18); }
  .rm:hover { color: var(--bad); border-color: var(--bad); }
  .reorg .cell.moving { opacity: .35; }
  /* adding a section: shown when the pointer is on the page (or there is nothing else) */
  .addsec { grid-column: 1 / -1; justify-self: start; padding: 7px 14px; border-radius: var(--radius); border: 1px dashed var(--line); background: none; color: var(--muted); font-size: 12.5px; cursor: pointer; opacity: 0; transition: opacity .15s, border-color .15s, color .15s; }
  .page:hover .addsec, .addsec:focus-visible, .addsec.show { opacity: 1; }
  .addsec:hover { border-color: var(--accent); color: var(--ink); }
  .sw { min-width: 0; }
  .favs h3 { margin: 10px 0 2px; font-weight: 600; }
  .fr { align-items: center; gap: 6px; }
  .fl { border: 0; background: none; padding: 0; text-align: left; cursor: pointer; color: var(--ink); font: inherit; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fl:hover b { color: var(--accent); }
  .rpl { border: 1px solid var(--line); background: var(--bg); color: var(--ink); border-radius: 50%; width: 22px; height: 22px; font-size: 9px; cursor: pointer; flex: none; }
</style>

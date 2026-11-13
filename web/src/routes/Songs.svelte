<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Stage from "../lib/components/Stage.svelte";
  import { STAGES, ago, bpm, dur } from "../lib/format";
  import type { SongItem } from "../lib/api";
  import { f, setView } from "../lib/filters.svelte";
  import Grid from "../lib/views/Grid.svelte";
  import Board from "../lib/views/Board.svelte";
  import Calendar from "../lib/views/Calendar.svelte";
  import Folders from "../lib/views/Folders.svelte";
  import PlayBtn from "../lib/components/PlayBtn.svelte";
  import Stars from "../lib/components/Stars.svelte";
  import Song from "./Song.svelte";
  import Art from "../lib/components/Art.svelte";
  import ListOverview from "../lib/components/ListOverview.svelte";
  import { tabs, openSong, closeSong, closeAll, prune, ordered, editGroup } from "../lib/tabs.svelte";
  import { t, compare } from "../lib/i18n/index.svelte";

  const VIEWS = ["list", "folders", "board", "cal", "grid"] as const;

  const all = $derived(app.songs);
  const noArtist = $derived(t("songs.noArtist"));
  const artists = $derived([...new Set(all.map((s) => s.artist || noArtist))].sort());
  const styles = $derived([...new Set(all.flatMap((s) => (s.style || "").toLowerCase().split(/\s*,\s*/).filter(Boolean)))].sort());
  const kinds = $derived([...new Set(all.flatMap((s) => s.kinds))].sort(compare));
  const keys = $derived([...new Set(all.map((s) => s.key).filter(Boolean) as string[])].sort());
  const counts = $derived(
    Object.fromEntries(["none", ...STAGES].map((k) => [k, all.filter((s) => (s.stage || "none") === k && !s.archived).length])),
  );

  const sorts: Record<string, (a: SongItem, b: SongItem) => number> = {
    recent: (a, b) => b.mtime - a.mtime,
    created: (a, b) => b.created - a.created,
    title: (a, b) => compare(a.title || "", b.title || ""),
    tempo: (a, b) => (a.tempo || 999) - (b.tempo || 999),
    stage: (a, b) => STAGES.indexOf(b.stage as never) - STAGES.indexOf(a.stage as never) || b.mtime - a.mtime,
    key: (a, b) => compare(a.key || "~", b.key || "~") || b.mtime - a.mtime,
    style: (a, b) => compare((a.style || "~").toLowerCase(), (b.style || "~").toLowerCase()) || b.mtime - a.mtime,
    audio: (a, b) => Number(b.temoin) - Number(a.temoin) || b.mtime - a.mtime,
    rating: (a, b) => (b.rating ?? 0) - (a.rating ?? 0) || b.mtime - a.mtime,
  };
  const SEVK = ["ok", "fix", "lost", "bad"];
  const now = Date.now() / 1000;
  const DAY = 86400;

  const list = $derived.by(() => {
    const q = app.query.toLowerCase();
    const inR = (x: number) => (f.bmin == null || x >= f.bmin) && (f.bmax == null || x <= f.bmax);
    return all
      .filter(
        (s) =>
          (f.onlyArchived ? s.archived : f.archived || !s.archived) &&
          (!f.plugins || SEVK[Math.min(3, s.sev)] === f.plugins) &&
          (!f.activity || (f.activity === "recent" ? now - s.mtime < 183 * DAY : now - s.mtime > 365 * DAY)) &&
          (!f.audio || (f.audio === "yes") === s.temoin) &&
          (!f.rating || (s.rating ?? 0) >= f.rating) &&
          (!f.kind || s.kinds.includes(f.kind)) &&
          (!q || [s.title, s.artist, s.style, s.dir, ...s.instr].some((v) => (v || "").toLowerCase().includes(q))) &&
          (!f.stages.length || f.stages.includes(s.stage || "none")) &&
          (!f.artist || (s.artist || noArtist) === f.artist) &&
          (!f.style || (s.style || "").toLowerCase().split(/\s*,\s*/).includes(f.style)) &&
          (!f.key || s.key === f.key) &&
          ((f.bmin == null && f.bmax == null) || (!!s.tempo && (inR(s.tempo) || (f.half && (inR(s.tempo * 2) || inR(s.tempo / 2)))))),
      )
      .sort(sorts[f.sort]);
  });
  // tabs of songs that no longer exist go away
  $effect(() => {
    const have = new Set(app.songs.map((s) => s.id));
    if (app.loaded) prune((id) => have.has(id));
  });
  const titleOf = (id: number) => all.find((s) => s.id === id)?.title ?? t("songs.song");
  const anyFilter = $derived(
    !!(f.stages.length || f.artist || f.style || f.key || f.bmin != null || f.bmax != null || f.plugins || f.activity || f.audio || f.onlyArchived || f.rating || f.kind),
  );
  function reset() {
    Object.assign(f, { stages: [], artist: "", style: "", key: "", bmin: null, bmax: null, half: false, plugins: "", activity: "", audio: "", onlyArchived: false, rating: 0, kind: "" });
  }
  const toggle = (k: string) => (f.stages = f.stages.includes(k) ? f.stages.filter((x) => x !== k) : [...f.stages, k]);
  const meta = (s: SongItem) =>
    [s.artist, s.style || (s.ssugg && "~" + s.ssugg.style), bpm(s.tempo), s.key && (s.key_src === "estimé" ? "~" + s.key : s.key), dur(Number(s.len))]
      .filter(Boolean)
      .join(" · ");
</script>

<div class="songs">
  <header class="bar">
    <div class="title"><h1>{t("songs.title")}</h1><span class="small muted">{t("songs.count", { n: list.length, total: all.filter((s) => !s.archived).length })}</span>
      <div class="seg" role="tablist" aria-label={t("songs.views.label")}>
        {#each VIEWS as k}<button role="tab" aria-selected={f.view === k} class:on={f.view === k} onclick={() => setView(k)}>{t(`songs.views.${k}`)}</button>{/each}
      </div>
    </div>
    <div class="filters">
      <div class="chips">
        {#each ["none", ...STAGES] as k}
          {#if counts[k] || f.stages.includes(k)}
            <button class="chip" class:on={f.stages.includes(k)} onclick={() => toggle(k)}>
              <Stage stage={k === "none" ? null : k} /> <span class="faint">{counts[k]}</span>
            </button>
          {/if}
        {/each}
      </div>
      <select bind:value={f.artist}><option value="">{t("songs.filter.artistAll")}</option>{#each artists as a}<option>{a}</option>{/each}</select>
      <select bind:value={f.style}><option value="">{t("songs.filter.styleAll")}</option>{#each styles as s}<option>{s}</option>{/each}</select>
      <select bind:value={f.key}><option value="">{t("songs.filter.keyAll")}</option>{#each keys as k}<option>{k}</option>{/each}</select>
      <span class="range">
        <input type="number" placeholder={t("songs.filter.bpm")} bind:value={f.bmin} /> {t("songs.filter.to")} <input type="number" placeholder={t("songs.filter.bpm")} bind:value={f.bmax} />
        <label title={t("songs.filter.half")}><input type="checkbox" bind:checked={f.half} /> ½×2</label>
      </span>
      <select bind:value={f.plugins} aria-label={t("songs.filter.plugins")}>
        <option value="">{t("songs.filter.pluginsAll")}</option><option value="ok">{t("songs.sev.ok")}</option><option value="fix">{t("songs.sev.fix")}</option>
        <option value="lost">{t("songs.sev.lost")}</option><option value="bad">{t("songs.sev.bad")}</option>
      </select>
      <select bind:value={f.activity} aria-label={t("songs.filter.activity")}>
        <option value="">{t("songs.filter.activityAll")}</option><option value="recent">{t("songs.filter.recent")}</option><option value="idle">{t("songs.filter.idle")}</option>
      </select>
      <select bind:value={f.audio} aria-label={t("songs.filter.audio")}>
        <option value="">{t("songs.filter.audioAll")}</option><option value="yes">{t("songs.filter.audioYes")}</option><option value="no">{t("songs.filter.audioNo")}</option>
      </select>
      <select bind:value={f.rating} aria-label={t("songs.filter.rating")}>
        <option value={0}>{t("songs.filter.ratingAll")}</option>{#each [5, 4, 3, 2, 1] as n}<option value={n}>{"★".repeat(n)}{n < 5 ? t("songs.filter.andUp") : ""}</option>{/each}
      </select>
      {#if kinds.length}<select bind:value={f.kind} aria-label={t("songs.filter.kind")}><option value="">{t("songs.filter.kindAll")}</option>{#each kinds as k}<option>{k}</option>{/each}</select>{/if}
      <select bind:value={f.sort} aria-label={t("songs.sort.label")}>
        {#each ["recent", "created", "stage", "audio", "rating", "key", "style", "title", "tempo"] as k}<option value={k}>{t(`songs.sort.${k}`)}</option>{/each}
      </select>
      <label class="small"><input type="checkbox" bind:checked={f.archived} disabled={f.onlyArchived} /> {t("songs.filter.archived")}</label>
      <label class="small"><input type="checkbox" bind:checked={f.onlyArchived} /> {t("songs.filter.onlyArchived")}</label>
      {#if anyFilter}<button class="link small" onclick={reset}>{t("songs.filter.reset")}</button>{/if}
    </div>
  </header>
  <!-- the list always keeps its narrow column: beside it the song open, else an overview -->
  <div class="split" class:open={tabs.active !== null || f.view === "list"} class:narrow={f.view === "list"}>
  <div class="viewcol">
  {#if f.view === "grid"}<Grid {list} />
  {:else if f.view === "board"}<Board {list} />
  {:else if f.view === "cal"}<Calendar {list} />
  {:else if f.view === "folders"}<Folders {list} />
  {:else}
  <div class="list">
    {#each list as s (s.id)}
      <a class="item" class:sel={tabs.active === s.id} href="#/song/{s.id}">
        <div class="pic"><Art title={s.title} cover={s.cover} size={40} radius={7} />{#if s.temoin}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={26} /></span>{/if}</div>
        <div class="t">{s.title}{#if s.archived} <span class="faint small">{t("songs.item.archived")}</span>{/if}</div>
        <div class="r"><Stars value={s.rating} small /><Stage stage={s.stage} /></div>
        <div class="m">{meta(s)}</div>
        <div class="r small">
          <span class="faint" title={t("songs.checks")}>{s.check}/{s.checks}</span>
          {#if s.sev >= 2}<span class="pill p{s.sev}">{s.sev === 3 ? t("songs.item.blocked") : t("songs.item.settings")}</span>{/if}
          <span class="faint">{ago(s.mtime)}</span>
        </div>
      </a>
    {:else}
      <p class="empty muted">{t("songs.empty")}</p>
    {/each}
  </div>
  {/if}
  </div>
  {#if tabs.active === null && f.view === "list"}
    <section class="pane"><div class="sheet"><ListOverview {list} /></div></section>
  {:else if tabs.active !== null}
    <section class="pane" aria-label={t("songs.tabs.label")}>
      <div class="tabbar" role="tablist">
        {#each ordered() as part, pi (part.group ? "g" + part.group.id : "n" + pi)}
          {#if part.group}
            {@const g = part.group}
            <button class="gchip" style:--gc={g.color} data-group={g.id} title={t(g.folded ? "songs.tabs.groupUnfold" : "songs.tabs.groupFold", { name: g.name })}
              onclick={() => editGroup(g.id, { folded: !g.folded })}>{g.name}{#if g.folded} <span class="gn">{g.ids.length}</span>{/if}</button>
          {/if}
          {#if !part.group?.folded}
            {#each part.ids as id (id)}
              <div class="tab" class:on={tabs.active === id} class:grouped={!!part.group} style:--gc={part.group?.color} role="tab" aria-selected={tabs.active === id} tabindex="0" data-tab={id}
                onclick={() => openSong(id)} onkeydown={(e) => e.key === "Enter" && openSong(id)}
                onauxclick={(e) => { if (e.button === 1) { e.preventDefault(); closeSong(id); } }} title={t("songs.tabs.tab", { title: titleOf(id) })}>
                <span class="tt">{titleOf(id)}</span>
                <button class="tx" aria-label={t("songs.tabs.close", { title: titleOf(id) })} onclick={(e) => { e.stopPropagation(); closeSong(id); }}>✕</button>
              </div>
            {/each}
          {/if}
        {/each}
        <button class="closeall small" title={t("songs.tabs.closeAllTitle")} onclick={closeAll}>{t("songs.tabs.closeAll")}</button>
      </div>
      <div class="sheet">{#key tabs.active}<Song id={tabs.active} embedded />{/key}</div>
    </section>
  {/if}
  </div>
</div>

<style>
  .songs { display: flex; flex-direction: column; height: 100%; }
  .bar { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 8px 28px; padding: 14px 22px 12px; border-bottom: 1px solid var(--line); }
  .title { min-width: 170px; display: flex; flex-direction: column; gap: 4px; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); margin-top: 6px; width: fit-content; }
  .seg button { border: 0; background: none; padding: 3px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  .seg button:hover { color: var(--ink); }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; box-shadow: 0 1px 2px rgba(0, 0, 0, .25); }
  .filters { display: flex; flex-wrap: wrap; gap: 8px 10px; align-items: center; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; flex-basis: 100%; }
  .chip { display: inline-flex; gap: 6px; align-items: center; padding: 3px 10px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .range { display: inline-flex; gap: 6px; align-items: center; font-size: 12px; color: var(--muted); }
  .range input[type="number"] { width: 72px; }
  .split { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr); }
  .split.open { grid-template-columns: minmax(380px, 1fr) minmax(0, 1.25fr); }
  .split.open.narrow { grid-template-columns: minmax(320px, 400px) minmax(0, 1fr); }
  .viewcol { min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden; container-type: inline-size; }
  .pane { min-width: 0; min-height: 0; display: flex; flex-direction: column; border-left: 1px solid var(--line); background: var(--bg); }
  .tabbar { display: flex; align-items: flex-end; gap: 2px; padding: 6px 8px 0; border-bottom: 1px solid var(--line); background: var(--panel2); overflow-x: auto; flex: none; }
  .tab { display: flex; align-items: center; gap: 6px; max-width: 200px; min-width: 90px; padding: 6px 6px 6px 12px; border-radius: 8px 8px 0 0; color: var(--muted); cursor: pointer; font-size: 12.5px; border: 1px solid transparent; border-bottom: 0; }
  .tab:hover { color: var(--ink); background: color-mix(in srgb, var(--line) 40%, transparent); }
  .tab.on { background: var(--bg); color: var(--ink); border-color: var(--line); font-weight: 600; margin-bottom: -1px; }
  .tt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .tx { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 10px; padding: 2px 5px; border-radius: 5px; }
  .tx:hover { color: var(--ink); background: var(--line); }
  .tab.grouped { box-shadow: inset 0 2px 0 var(--gc); }
  .gchip { align-self: center; margin: 0 2px 4px 6px; border: 0; border-radius: 6px; padding: 2px 8px; font-size: 11.5px; font-weight: 600; cursor: pointer; white-space: nowrap;
    color: #fff; background: var(--gc); }
  .gchip:hover { filter: brightness(1.1); }
  .gn { opacity: .8; font-weight: 500; }
  .closeall { margin-left: auto; border: 0; background: none; color: var(--muted); cursor: pointer; padding: 6px 8px; white-space: nowrap; }
  .closeall:hover { color: var(--ink); }
  .sheet { flex: 1; min-height: 0; overflow: auto; }
  .item.sel { background: var(--accent-bg); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .list { overflow: auto; flex: 1; min-height: 0; padding: 6px 12px 40px; }
  .item { color: inherit; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 2px 12px; padding: 8px 12px; border-bottom: 1px solid var(--line); border-radius: 8px; }
  .item:hover { background: color-mix(in srgb, var(--line) 40%, transparent); }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .item:hover .pb, .pb:focus-within { opacity: 1; }
  .pic { grid-row: span 2; position: relative; }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r { text-align: right; display: flex; gap: 8px; justify-content: flex-end; align-items: center; }
  .pill { font-size: 11px; padding: 1px 7px; border-radius: 99px; }
  .p2 { background: color-mix(in srgb, var(--lost) 18%, transparent); color: var(--lost); }
  .p3 { background: color-mix(in srgb, var(--bad) 18%, transparent); color: var(--bad); }
  .empty { padding: 40px; text-align: center; }
</style>

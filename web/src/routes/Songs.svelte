<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Splitter from "../lib/components/Splitter.svelte";
  // the list's width beside a song (list view), the view's width beside it (board, calendar, folders)
  let listW = $state(400);
  // a tab being moved by hand, and where it would land
  let dragTab = $state<number | null>(null);
  let dropAt = $state<{ id: number; after: boolean } | null>(null);
  let viewW = $state(560);
  import Stage from "../lib/components/Stage.svelte";
  import { STAGES, ago, bpm, dur, stageLabel } from "../lib/format";
  import type { SongItem } from "../lib/api";
  import { f, setView, presets, savePreset, applyPreset, removePreset, setDefaultPreset, currentPreset } from "../lib/filters.svelte";
  import { parse, matches, releasesBySong } from "../lib/songQuery";
  import { songDrag } from "../lib/releaseSuggest.svelte";
  import { dragSource, dropTarget } from "../lib/pdrag.svelte";
  import Board from "../lib/views/Board.svelte";
  import Calendar from "../lib/views/Calendar.svelte";
  import Folders from "../lib/views/Folders.svelte";
  import PlayBtn from "../lib/components/PlayBtn.svelte";
  import Stars from "../lib/components/Stars.svelte";
  import Song from "./Song.svelte";
  import Art from "../lib/components/Art.svelte";
  import ListOverview from "../lib/components/ListOverview.svelte";
  import { tabs, openSong, closeSong, closeAll, prune, ordered, editGroup, moveTab } from "../lib/tabs.svelte";
  import { t, compare } from "../lib/i18n/index.svelte";

  const VIEWS = ["list", "folders", "board", "cal"] as const;

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

  // the list drawn in a window: the rows' height is fixed (one line of title, one of details)
  let listEl = $state<HTMLDivElement>();
  let listTop = $state(0);
  let listH = $state(900);
  const rowH = 57;
  const from = $derived(Math.max(0, Math.floor(listTop / rowH) - 12));
  const to = $derived(Math.min(list.length, Math.ceil((listTop + listH) / rowH) + 12));
  // a new filter shows the top of the list
  $effect(() => {
    void list;
    if (listEl && listEl.scrollTop > list.length * rowH) listEl.scrollTop = 0;
  });

  const list = $derived.by(() => {
    const words = parse(app.query);
    const onRel = words.length ? releasesBySong(app.releases) : null;
    const inR = (x: number) => (f.bmin == null || x >= f.bmin) && (f.bmax == null || x <= f.bmax);
    return all
      .filter(
        (s) =>
          (f.onlyArchived ? s.archived : f.archived || !s.archived) &&
          (!f.plugins || SEVK[Math.min(3, s.sev)] === f.plugins) &&
          (!f.activity || (f.activity === "recent" ? now - s.mtime < 183 * DAY : now - s.mtime > (f.activity === "asleep" ? 90 : 365) * DAY)) &&
          (!f.audio || (f.audio === "yes") === s.temoin) &&
          (!f.rating || (s.rating ?? 0) >= f.rating) &&
          (!f.kind || s.kinds.includes(f.kind)) &&
          matches(s, words, onRel) &&
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
  // in a small folder split into several songs: the ones that are not the newest, and the
  // title of the newest (where the latest work is)
  const newerOf = $derived.by(() => {
    const by = new Map<string, SongItem[]>();
    for (const s of all) if (s.dir && !s.archived) by.set(s.dir, [...(by.get(s.dir) ?? []), s]);
    const m = new Map<number, string>();
    for (const g of by.values()) {
      if (g.length < 2 || g.length > 6) continue;
      const newest = g.reduce((a, b) => (b.mtime > a.mtime ? b : a));
      for (const s of g) if (s !== newest) m.set(s.id, newest.title ?? "");
    }
    return m;
  });
  // the filters set, one label each (a click removes it)
  let panel = $state(false);
  const active = $derived.by(() => {
    const out: { key: string; label: string; clear: () => void }[] = [];
    for (const k of f.stages) out.push({ key: "st" + k, label: stageLabel(k === "none" ? null : k), clear: () => toggle(k) });
    if (f.artist) out.push({ key: "a", label: f.artist, clear: () => (f.artist = "") });
    if (f.style) out.push({ key: "s", label: f.style, clear: () => (f.style = "") });
    if (f.key) out.push({ key: "k", label: f.key, clear: () => (f.key = "") });
    if (f.bmin != null || f.bmax != null)
      out.push({ key: "b", label: `${f.bmin ?? "…"}-${f.bmax ?? "…"} bpm${f.half ? " ½×2" : ""}`, clear: () => ((f.bmin = null), (f.bmax = null)) });
    if (f.rating) out.push({ key: "r", label: "★".repeat(f.rating) + (f.rating < 5 ? "+" : ""), clear: () => (f.rating = 0) });
    if (f.audio) out.push({ key: "au", label: t(f.audio === "yes" ? "songs.filter.audioYes" : "songs.filter.audioNo"), clear: () => (f.audio = "") });
    if (f.activity) out.push({ key: "ac", label: t(`songs.filter.${f.activity}`), clear: () => (f.activity = "") });
    if (f.kind) out.push({ key: "kd", label: f.kind, clear: () => (f.kind = "") });
    if (f.plugins) out.push({ key: "p", label: t(`songs.sev.${f.plugins}`), clear: () => (f.plugins = "") });
    if (f.onlyArchived) out.push({ key: "oa", label: t("songs.filter.onlyArchived"), clear: () => (f.onlyArchived = false) });
    else if (f.archived) out.push({ key: "ar", label: t("songs.filter.archived"), clear: () => (f.archived = false) });
    return out;
  });
  // the panel closes on a click outside it or Escape
  function outside(e: MouseEvent) {
    if (panel && !(e.target as HTMLElement).closest(".panel, .fbtn")) panel = false;
  }
  // presets: the filters, sort and view saved under a name (the default one at launch)
  let presetName = $state("");
  const cur = $derived.by(() => {
    void [f.view, f.stages, f.artist, f.style, f.key, f.bmin, f.bmax, f.half, f.sort, f.archived, f.plugins, f.activity, f.audio, f.rating, f.kind, f.onlyArchived];
    return currentPreset();
  });
  function pickPreset(name: string) {
    const p = presets.list.find((x) => x.name === name);
    if (p) applyPreset(p);
  }
  function keepPreset() {
    savePreset(presetName);
    presetName = "";
  }
  const toggle = (k: string) => (f.stages = f.stages.includes(k) ? f.stages.filter((x) => x !== k) : [...f.stages, k]);
  const meta = (s: SongItem) =>
    [s.artist, s.style, bpm(s.tempo), s.key, dur(Number(s.len))]
      .filter(Boolean)
      .join(" · ");
</script>

<svelte:window onclick={outside} onkeydown={(e) => e.key === "Escape" && (panel = false)} />

<div class="songs">
  <!-- one line: the filters behind one button (shown as labels once set), the sort, the
       views. The search field at the top filters this list too. -->
  <header class="bar" class:slim={tabs.active !== null}>
    <div class="row1">
      <h1>{t("songs.title")}</h1>
      <span class="small muted">{t("songs.count", { n: list.length, total: all.filter((s) => !s.archived).length })}</span>
      <button class="btn sm fbtn" class:on={panel} aria-expanded={panel} onclick={() => (panel = !panel)}>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 5h18l-7 8v6l-4 2v-8z" /></svg>
        {t("songs.filter.button")}{#if active.length}<span class="n">{active.length}</span>{/if}
      </button>
      {#if presets.list.length}
        <select class="presel" value={cur?.name ?? ""} aria-label={t("songs.presets.pick")} onchange={(e) => pickPreset(e.currentTarget.value)}>
          <option value="" disabled>{t("songs.presets.pick")}</option>
          {#each presets.list as p (p.name)}<option value={p.name}>{p.def ? "★ " : ""}{p.name}</option>{/each}
        </select>
      {/if}
      <span class="grow"></span>
      <label class="small muted sort">
        <select bind:value={f.sort} aria-label={t("songs.sort.label")}>
          {#each ["recent", "created", "stage", "audio", "rating", "key", "style", "title", "tempo"] as k}<option value={k}>{t(`songs.sort.${k}`)}</option>{/each}
        </select>
      </label>
      <div class="seg" role="tablist" aria-label={t("songs.views.label")}>
        {#each VIEWS as k}<button role="tab" aria-selected={f.view === k} class:on={f.view === k} onclick={() => setView(k)}>{t(`songs.views.${k}`)}</button>{/each}
      </div>
    </div>
    {#if active.length || app.query}
      <div class="active">
        {#if app.query}<button class="tag" title={t("songs.filter.removeTip")} onclick={() => (app.query = "")}>« {app.query} » <span class="x">×</span></button>{/if}
        {#each active as a (a.key)}<button class="tag" title={t("songs.filter.removeTip")} onclick={a.clear}>{a.label} <span class="x">×</span></button>{/each}
        <button class="link small" onclick={() => { reset(); app.query = ""; }}>{t("songs.filter.reset")}</button>
      </div>
    {/if}
    {#if panel}
      <div class="panel card" role="dialog" aria-label={t("songs.filter.button")}>
        <section>
          <h3>{t("songs.filter.group.music")}</h3>
          <label><span>{t("songs.filter.artist")}</span><select bind:value={f.artist}><option value="">{t("songs.filter.artistAll")}</option>{#each artists as a}<option>{a}</option>{/each}</select></label>
          <label><span>{t("songs.filter.style")}</span><select bind:value={f.style}><option value="">{t("songs.filter.styleAll")}</option>{#each styles as st}<option>{st}</option>{/each}</select></label>
          <label><span>{t("songs.filter.key")}</span><select bind:value={f.key}><option value="">{t("songs.filter.keyAll")}</option>{#each keys as k}<option>{k}</option>{/each}</select></label>
          <div class="lbl"><span>{t("songs.filter.tempo")}</span>
            <span class="range"><input type="number" placeholder={t("songs.filter.bpm")} bind:value={f.bmin} /> {t("songs.filter.to")} <input type="number" placeholder={t("songs.filter.bpm")} bind:value={f.bmax} />
              <label class="inline" title={t("songs.filter.half")}><input type="checkbox" bind:checked={f.half} /> ½×2</label></span></div>
        </section>
        <section>
          <h3>{t("songs.filter.group.progress")}</h3>
          <div class="chips">
            {#each ["none", ...STAGES] as k}
              {#if counts[k] || f.stages.includes(k)}
                <button class="chip" class:on={f.stages.includes(k)} onclick={() => toggle(k)}><Stage stage={k === "none" ? null : k} /> <span class="faint">{counts[k]}</span></button>
              {/if}
            {/each}
          </div>
          <label><span>{t("songs.filter.rating")}</span><select bind:value={f.rating}><option value={0}>{t("songs.filter.ratingAll")}</option>{#each [5, 4, 3, 2, 1] as n}<option value={n}>{"★".repeat(n)}{n < 5 ? t("songs.filter.andUp") : ""}</option>{/each}</select></label>
          <label><span>{t("songs.filter.audio")}</span><select bind:value={f.audio}><option value="">{t("songs.filter.audioAll")}</option><option value="yes">{t("songs.filter.audioYes")}</option><option value="no">{t("songs.filter.audioNo")}</option></select></label>
          <label><span>{t("songs.filter.activity")}</span><select bind:value={f.activity}><option value="">{t("songs.filter.activityAll")}</option><option value="recent">{t("songs.filter.recent")}</option><option value="asleep">{t("songs.filter.asleep")}</option><option value="idle">{t("songs.filter.idle")}</option></select></label>
          {#if kinds.length}<label><span>{t("songs.filter.kind")}</span><select bind:value={f.kind}><option value="">{t("songs.filter.kindAll")}</option>{#each kinds as k}<option>{k}</option>{/each}</select></label>{/if}
        </section>
        <section>
          <h3>{t("songs.filter.group.tech")}</h3>
          <label><span>{t("songs.filter.plugins")}</span><select bind:value={f.plugins}>
            <option value="">{t("songs.filter.pluginsAll")}</option><option value="ok">{t("songs.sev.ok")}</option><option value="fix">{t("songs.sev.fix")}</option>
            <option value="lost">{t("songs.sev.lost")}</option><option value="bad">{t("songs.sev.bad")}</option></select></label>
          <label class="inline small"><input type="checkbox" bind:checked={f.archived} disabled={f.onlyArchived} /> {t("songs.filter.archived")}</label>
          <label class="inline small"><input type="checkbox" bind:checked={f.onlyArchived} /> {t("songs.filter.onlyArchived")}</label>
          <button class="btn sm close" onclick={() => (panel = false)}>{t("songs.filter.done")}</button>
        </section>
        <section class="presets">
          <h3>{t("songs.presets.title")}</h3>
          {#if presets.list.length}
            <div class="plist">
              {#each presets.list as p (p.name)}
                <div class="prow" class:on={cur?.name === p.name}>
                  <button class="pname" onclick={() => applyPreset(p)}>{p.name}</button>
                  <button class="icon star" class:def={p.def} aria-pressed={!!p.def} title={p.def ? t("songs.presets.undefault") : t("songs.presets.default")} onclick={() => setDefaultPreset(p.def ? null : p.name)}>★</button>
                  <button class="icon" title={t("songs.presets.remove", { name: p.name })} onclick={() => removePreset(p.name)}>×</button>
                </div>
              {/each}
            </div>
          {:else}<p class="small faint">{t("songs.presets.none")}</p>{/if}
          <form class="psave" onsubmit={(e) => { e.preventDefault(); keepPreset(); }}>
            <input bind:value={presetName} placeholder={t("songs.presets.name")} aria-label={t("songs.presets.name")} />
            <button class="btn sm" disabled={!presetName.trim()}>{t("songs.presets.save")}</button>
          </form>
          <p class="small faint">{t("songs.presets.hint")}</p>
        </section>
      </div>
    {/if}
  </header>
  <!-- the list always keeps its narrow column: beside it the song open, else an overview -->
  <div class="split" class:open={tabs.active !== null || f.view === "list"} style:--lw="{f.view === "list" ? listW : viewW}px">
  <div class="viewcol">
  {#if f.view === "board"}<Board {list} />
  {:else if f.view === "cal"}<Calendar {list} />
  {:else if f.view === "folders"}<Folders {list} />
  {:else}
  <div class="list" bind:this={listEl} bind:clientHeight={listH} onscroll={() => (listTop = listEl?.scrollTop ?? 0)}>
    <!-- only the rows in sight exist (and a few around): 400 rows drawn in full made every
         scroll of the sheet beside them repaint thousands of elements -->
    <div style:height="{from * rowH}px" aria-hidden="true"></div>
    {#each list.slice(from, to) as s (s.id)}
      <a class="item" class:sel={tabs.active === s.id} href="#/song/{s.id}" use:dragSource={songDrag(s.id, s.title)} title={t("songs.item.dragTip")}>
        <div class="pic"><Art title={s.title} cover={s.cover} size={40} radius={7} />{#if s.temoin || s.rel}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={26} queue={{ songs: list, name: t("songs.title") }} project={s.temoin ? "" : s.rel} /></span>{/if}</div>
        <div class="t">{s.title}{#if s.archived} <span class="faint small">{t("songs.item.archived")}</span>{/if}</div>
        <div class="r"><Stars value={s.rating} small /><Stage stage={s.stage} /></div>
        <div class="m">{meta(s)}</div>
        <div class="r small">
          <span class="faint" title={t("songs.checks")}>{s.check}/{s.checks}</span>
          {#if newerOf.get(s.id)}<span class="pill nw" title={t("songs.item.newerTip", { title: newerOf.get(s.id) ?? "" })}>{t("songs.item.newer")}</span>{/if}
          {#if s.copies?.length}<span class="pill cp" title={t("songs.item.copyTip", { n: s.copies.length })}>{t("songs.item.copy")}</span>{/if}
          {#if s.sev >= 2}<span class="pill p{s.sev}">{s.sev === 3 ? t("songs.item.blocked") : t("songs.item.settings")}</span>{/if}
          <span class="faint">{ago(s.mtime)}</span>
        </div>
      </a>
    {:else}
      <p class="empty muted">{t("songs.empty")}</p>
    {/each}
    <div style:height="{Math.max(0, list.length - to) * rowH}px" aria-hidden="true"></div>
  </div>
  {/if}
  </div>
  {#if tabs.active !== null || f.view === "list"}
    {#if f.view === "list"}<Splitter bind:width={listW} key="songs-list" min={280} max={760} label={t("songs.resize")} />
    {:else}<Splitter bind:width={viewW} key="songs-view" min={320} max={1100} label={t("songs.resize")} />{/if}
  {/if}
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
                class:dropl={dropAt?.id === id && !dropAt.after} class:dropr={dropAt?.id === id && dropAt.after}
                use:dragSource={{ kind: "tab", data: id, label: titleOf(id) }}
                use:dropTarget={{
                  accept: (it) => it.kind === "tab" && it.data !== id,
                  over: (_, e, el) => { const r = el.getBoundingClientRect(); dropAt = { id, after: e.clientX > r.left + r.width / 2 }; },
                  leave: () => { if (dropAt?.id === id) dropAt = null; },
                  drop: (it) => { if (dropAt) moveTab(it.data as number, dropAt.id, dropAt.after); dropAt = null; },
                }}
                onclick={() => openSong(id)} onkeydown={(e) => e.key === "Enter" && openSong(id)}
                onauxclick={(e) => { if (e.button === 1) { e.preventDefault(); closeSong(id); } }} title={t("songs.tabs.tab", { title: titleOf(id) })}>
                <span class="tt">{titleOf(id)}</span>
                <button class="tx" aria-label={t("songs.tabs.close", { title: titleOf(id) })} onclick={(e) => { e.stopPropagation(); closeSong(id); }}>✕</button>
              </div>
            {/each}
          {/if}
        {/each}
        <button class="closeall" title={t("songs.tabs.closeAllTitle")} aria-label={t("songs.tabs.closeAll")} onclick={closeAll}>
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 6l12 12M18 6L6 18" /></svg></button>
      </div>
      <div class="sheet">{#key tabs.active}<Song id={tabs.active} embedded />{/key}</div>
    </section>
  {/if}
  </div>
</div>

<style>
  .songs { display: flex; flex-direction: column; height: 100%; }
  /* the same margins as every other page's title (24px above, 28px aside) */
  .bar { position: relative; padding: 18px 28px 10px; border-bottom: 1px solid var(--line); display: flex; flex-direction: column; gap: 8px; }
  .bar.slim { padding: 10px 20px 8px; }
  .bar.slim h1 { font-size: 20px; }
  .row1 { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .row1 h1 { margin: 0; }
  .grow { flex: 1; }
  .fbtn { display: inline-flex; align-items: center; gap: 6px; }
  .fbtn.on { border-color: var(--accent); }
  .fbtn .n { background: var(--accent); color: var(--on-accent); border-radius: 99px; font-size: 11px; padding: 0 6px; font-weight: 700; }
  .sort { display: inline-flex; align-items: center; gap: 6px; }
  .active { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .tag { display: inline-flex; align-items: center; gap: 4px; padding: 2px 6px 2px 10px; border-radius: 99px; border: 1px solid var(--accent); background: var(--accent-bg); color: var(--ink); font: inherit; font-size: 12px; cursor: pointer; }
  .tag .x { color: var(--muted); padding: 0 3px; }
  .tag:hover .x { color: var(--bad); }
  .panel { position: absolute; top: calc(100% - 4px); left: 20px; z-index: 60; display: grid; grid-template-columns: repeat(3, minmax(220px, 1fr)); gap: 18px; max-width: 920px; box-shadow: 0 18px 40px rgba(0, 0, 0, .35); }
  .panel section { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .panel .presets { grid-column: 1 / -1; border-top: 1px solid var(--line); padding-top: 12px; }
  .plist { display: flex; flex-wrap: wrap; gap: 6px; }
  .prow { display: inline-flex; align-items: center; border: 1px solid var(--line); border-radius: 999px; padding: 1px 4px 1px 2px; background: var(--panel2); }
  .prow.on { border-color: var(--accent); background: var(--accent-bg); }
  .pname { border: 0; background: none; color: var(--ink); font: inherit; font-size: 12.5px; padding: 3px 8px; cursor: pointer; }
  .prow .icon { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 13px; padding: 2px 5px; border-radius: 999px; }
  .prow .icon:hover { color: var(--ink); }
  .prow .star.def { color: var(--accent); }
  .psave { display: flex; gap: 6px; max-width: 420px; }
  .psave input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 5px 9px; font: inherit; font-size: 13px; }
  .presel { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 4px 8px; font: inherit; font-size: 12.5px; max-width: 200px; }
  .panel h3 { margin: 0 0 2px; font-size: 12px; color: var(--muted); font-weight: 600; }
  .panel label:not(.inline), .panel .lbl { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--muted); }
  .panel select { width: 100%; }
  .panel .inline { display: inline-flex; align-items: center; gap: 6px; color: var(--ink); }
  .panel .close { align-self: flex-start; margin-top: auto; }
  @media (max-width: 900px) { .panel { grid-template-columns: 1fr; right: 20px; } }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); margin-top: 6px; width: fit-content; }
  .seg button { border: 0; background: none; padding: 3px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  .seg button:hover { color: var(--ink); }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; box-shadow: 0 1px 2px rgba(0, 0, 0, .25); }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; flex-basis: 100%; }
  .chip { display: inline-flex; gap: 6px; align-items: center; padding: 3px 10px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .range { display: inline-flex; gap: 6px; align-items: center; font-size: 12px; color: var(--muted); }
  .range input[type="number"] { width: 72px; }
  .split { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr); }
  /* the list (or the view) at the width set with the bar between them */
  .split.open { grid-template-columns: minmax(0, var(--lw)) auto minmax(0, 1fr); }
  .viewcol { min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden; container-type: inline-size; }
  .pane { min-width: 0; min-height: 0; display: flex; flex-direction: column; border-left: 1px solid var(--line); background: var(--bg); }
  .tabbar { display: flex; align-items: flex-end; gap: 2px; padding: 6px 8px 0; border-bottom: 1px solid var(--line); background: var(--panel2); overflow-x: auto; flex: none; }
  /* where a dragged tab will land */
  .tab.dropl { box-shadow: inset 2px 0 0 var(--ink); }
  .tab.dropr { box-shadow: inset -2px 0 0 var(--ink); }
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
  .item { height: 57px; box-sizing: border-box; color: inherit; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 2px 12px; padding: 8px 12px; border-bottom: 1px solid var(--line); border-radius: 8px; }
  .item:hover { background: color-mix(in srgb, var(--line) 40%, transparent); }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .item:hover .pb, .pb:focus-within, .pb:has(:global(.play.on)) { opacity: 1; }
  .pic { grid-row: span 2; position: relative; }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r { text-align: right; display: flex; gap: 8px; justify-content: flex-end; align-items: center; }
  .pill { font-size: 11px; padding: 1px 7px; border-radius: 99px; }
  .pill.nw { background: var(--panel2); color: var(--muted); border: 1px solid var(--line); }
  .pill.cp { background: color-mix(in srgb, var(--fix) 18%, transparent); color: var(--ink); }
  .p2 { background: color-mix(in srgb, var(--lost) 18%, transparent); color: var(--lost); }
  .p3 { background: color-mix(in srgb, var(--bad) 18%, transparent); color: var(--bad); }
  .empty { padding: 40px; text-align: center; }
</style>

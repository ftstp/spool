<script lang="ts">
  import { heightOf } from "../size";
  // Every project file (.als) as its own row, whatever the songs they are grouped into: a
  // catalogue read one song a folder still shows each project saved, old ideas included.
  // Sorted by date (newest or oldest first), name or tempo; the search box filters them too.
  import * as api from "../api";
  import type { ProjectFile } from "../api/bindings/ProjectFile";
  import { app } from "../state.svelte";
  import { t, compare, fmtNum } from "../i18n/index.svelte";
  import { ago, bpm, dur } from "../format";
  import { cached, keep } from "../pageCache";

  let rows = $state<ProjectFile[] | null>(null);
  let error = $state("");
  let sort = $state<"new" | "old" | "name" | "tempo">("new");
  let withOld = $state(true);
  $effect(() => {
    const v = app.version;
    const c = cached<ProjectFile[]>("projects");
    if (c) rows = c.value;
    if (c?.version === v) return;
    api.projectFiles().then((r) => {
      rows = r;
      keep("projects", v, r);
    }).catch((e) => (error = String(e)));
  });
  const name = (rel: string) => rel.split("/").pop()?.replace(/\.als$/i, "") ?? rel;
  const folder = (rel: string) => rel.split("/").slice(0, -1).join("/");
  const shown = $derived.by(() => {
    const words = app.query.toLowerCase().split(/\s+/).filter(Boolean);
    const l = (rows ?? []).filter((p) => (withOld || !p.old) && words.every((w) => (p.rel + " " + (p.title ?? "")).toLowerCase().includes(w)));
    const by: Record<typeof sort, (a: ProjectFile, b: ProjectFile) => number> = {
      new: (a, b) => b.mtime - a.mtime,
      old: (a, b) => a.mtime - b.mtime,
      name: (a, b) => compare(name(a.rel), name(b.rel)),
      tempo: (a, b) => (a.tempo ?? 999) - (b.tempo ?? 999),
    };
    return l.sort(by[sort]);
  });
  // drawn in a window: a catalogue holds thousands
  let listEl = $state<HTMLDivElement>();
  let top = $state(0);
  let h = $state(800);
  const ROW = 48;
  const from = $derived(Math.max(0, Math.floor(top / ROW) - 10));
  const to = $derived(Math.min(shown.length, Math.ceil((top + h) / ROW) + 10));
  const open = (p: ProjectFile) => api.openInLive(p.rel).catch((e) => app.say(String(e), { error: true }));
</script>

<div class="pv">
  <div class="bar small">
    <span class="muted">{rows ? t("songs.projects.count", { n: fmtNum(shown.length) }) : ""}</span>
    <label class="muted"><input type="checkbox" bind:checked={withOld} /> {t("songs.projects.withOld")}</label>
    <span class="grow"></span>
    <select bind:value={sort} aria-label={t("songs.sort.label")}>
      <option value="new">{t("songs.projects.sortNew")}</option>
      <option value="old">{t("songs.projects.sortOld")}</option>
      <option value="name">{t("songs.projects.sortName")}</option>
      <option value="tempo">{t("songs.projects.sortTempo")}</option>
    </select>
  </div>
  {#if error}<p class="small warn">{error}</p>{/if}
  {#if !rows}
    <p class="small muted pad">{t("songs.projects.loading")}</p>
  {:else}
    <div class="list" bind:this={listEl} use:heightOf={(x) => (h = x)} onscroll={(e) => (top = e.currentTarget.scrollTop)}>
      <div style:height="{shown.length * ROW}px" class="inner">
        {#each shown.slice(from, to) as p, i (p.rel)}
          <div class="row" class:old={p.old || p.archived} style:top="{(from + i) * ROW}px">
            <div class="who">
              <div class="n" title={p.rel}>{name(p.rel)}{#if p.old}<span class="tag">{t("songs.projects.old")}</span>{/if}</div>
              <div class="small faint f" title={folder(p.rel)}>{folder(p.rel)}</div>
            </div>
            {#if p.song}<a class="small song" href="#/song/{p.song}" title={t("songs.projects.songTip")}>{p.title ?? "?"}</a>{:else}<span></span>{/if}
            <span class="small num">{p.tempo ? bpm(p.tempo) : ""}</span>
            <span class="small num">{p.key ?? ""}</span>
            <span class="small num faint">{p.len ? dur(Math.round(p.len)) : ""}</span>
            <span class="small num faint" title={new Date(p.mtime * 1000).toLocaleString()}>{ago(p.mtime)}</span>
            <button class="btn sm" onclick={() => open(p)} title={t("songs.projects.liveTip")}>{t("songs.projects.live")}</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .pv { display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .bar { display: flex; align-items: center; gap: 14px; padding: 8px 28px; border-bottom: 1px solid var(--line); }
  .bar label { display: inline-flex; gap: 6px; align-items: center; cursor: pointer; }
  .bar select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 4px 8px; font: inherit; }
  .grow { flex: 1; }
  .list { flex: 1; overflow: auto; min-height: 0; }
  .inner { position: relative; }
  .row { position: absolute; left: 0; right: 0; height: 48px; box-sizing: border-box; display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) 70px 54px 54px 90px auto; gap: 12px; align-items: center; padding: 0 28px; border-bottom: 1px solid var(--line); }
  .row:hover { background: var(--panel2); }
  .row.old { opacity: .7; }
  .who { min-width: 0; }
  .n, .f { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .n { font-weight: 600; font-size: 13.5px; }
  .tag { margin-left: 8px; font-weight: 400; font-size: 11px; padding: 0 6px; border-radius: 99px; border: 1px solid var(--line); color: var(--muted); }
  .song { color: var(--ink); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .song:hover { color: var(--accent); }
  .num { text-align: right; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .pad { padding: 14px 28px; }
  .warn { color: var(--lost); padding: 8px 28px; }
</style>

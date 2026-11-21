<script lang="ts">
  // What changed between two saves of a song's project: by default the two newest. Read from
  // what the scan kept of each file; no project is opened.
  import * as api from "../api";
  import type { AlsDiff } from "../api/bindings/AlsDiff";
  import { t } from "../i18n/index.svelte";
  import { ago } from "../format";

  let { files }: { files: { rel: string; name: string; mtime: number }[] } = $props();
  const sorted = $derived([...files].sort((x, y) => y.mtime - x.mtime));
  let a = $state("");
  let b = $state("");
  $effect(() => {
    if (!sorted.some((f) => f.rel === b)) b = sorted[0]?.rel ?? "";
    if (!sorted.some((f) => f.rel === a)) a = sorted[1]?.rel ?? "";
  });
  let d = $state<AlsDiff | null>(null);
  let error = $state("");
  $effect(() => {
    const [x, y] = [a, b];
    d = null;
    error = "";
    if (!x || !y || x === y) return;
    // the older one first, whichever order they were picked in
    const fx = sorted.find((f) => f.rel === x)!;
    const fy = sorted.find((f) => f.rel === y)!;
    const [o, n] = fx.mtime <= fy.mtime ? [x, y] : [y, x];
    api.alsDiff(o, n).then((r) => x === a && y === b && (d = r)).catch((e) => (error = String(e)));
  });
  const what = (w: string) => t(`song.diff.what.${w}`);
  const list = (xs: string[], max = 12) => xs.slice(0, max).join(", ") + (xs.length > max ? ` ${t("song.diff.more", { n: xs.length - max })}` : "");
</script>

{#if sorted.length >= 2}
  <details class="diff">
    <summary class="small muted">{t("song.diff.title")}</summary>
    <div class="pick small">
      <select bind:value={a} aria-label={t("song.diff.from")}>{#each sorted as f (f.rel)}<option value={f.rel}>{f.name} · {ago(f.mtime)}</option>{/each}</select>
      <span class="faint">→</span>
      <select bind:value={b} aria-label={t("song.diff.to")}>{#each sorted as f (f.rel)}<option value={f.rel}>{f.name} · {ago(f.mtime)}</option>{/each}</select>
    </div>
    {#if error}<p class="small bad">{error}</p>
    {:else if a === b}<p class="small faint">{t("song.diff.pickTwo")}</p>
    {:else if !d}<p class="small faint">…</p>
    {:else if d.same}<p class="small faint">{t("song.diff.same")}</p>
    {:else}
      <ul class="small">
        {#each d.changes as c (c.what)}<li><b>{what(c.what)}</b> {c.from || "?"} → {c.to || "?"}</li>{/each}
        {#if d.tracks_added.length}<li class="add"><b>{t("song.diff.tracksAdded")}</b> {list(d.tracks_added)}</li>{/if}
        {#if d.tracks_removed.length}<li class="rem"><b>{t("song.diff.tracksRemoved")}</b> {list(d.tracks_removed)}</li>{/if}
        {#each d.tracks_changed.slice(0, 20) as c (c.name + (c.was ?? ""))}
          <li><b>{c.name}</b>{#if c.was} <span class="faint">({t("song.diff.was", { name: c.was })})</span>{/if}
            {#if c.added.length}<span class="add"> + {c.added.join(", ")}</span>{/if}
            {#if c.removed.length}<span class="rem"> − {c.removed.join(", ")}</span>{/if}
            {#if c.notes}<span class="faint"> · {t("song.diff.notes", { from: c.notes[0], to: c.notes[1] })}</span>{/if}
            {#if c.clips}<span class="faint"> · {t("song.diff.clips", { from: c.clips[0], to: c.clips[1] })}</span>{/if}
          </li>
        {/each}
        {#if d.tracks_changed.length > 20}<li class="faint">{t("song.diff.more", { n: d.tracks_changed.length - 20 })}</li>{/if}
        {#if d.plugins_added.length}<li class="add"><b>{t("song.diff.pluginsAdded")}</b> {list(d.plugins_added)}</li>{/if}
        {#if d.plugins_removed.length}<li class="rem"><b>{t("song.diff.pluginsRemoved")}</b> {list(d.plugins_removed)}</li>{/if}
        {#if d.samples_added.length}<li class="add"><b>{t("song.diff.samplesAdded")}</b> {list(d.samples_added)}</li>{/if}
        {#if d.samples_removed.length}<li class="rem"><b>{t("song.diff.samplesRemoved")}</b> {list(d.samples_removed)}</li>{/if}
        {#if d.locators_added.length}<li class="add"><b>{t("song.diff.locatorsAdded")}</b> {list(d.locators_added)}</li>{/if}
        {#if d.locators_removed.length}<li class="rem"><b>{t("song.diff.locatorsRemoved")}</b> {list(d.locators_removed)}</li>{/if}
      </ul>
    {/if}
  </details>
{/if}

<style>
  .diff { margin-top: 8px; }
  .pick { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin: 6px 0; }
  select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 3px 6px; font: inherit; max-width: 46%; }
  ul { margin: 0; padding-left: 16px; display: grid; gap: 3px; color: var(--muted); }
  b { color: var(--ink); font-weight: 600; }
  .add { color: var(--ok); }
  .rem { color: var(--lost); }
  li.add, li.rem { color: var(--muted); }
  li.add b { color: var(--ok); }
  li.rem b { color: var(--lost); }
</style>

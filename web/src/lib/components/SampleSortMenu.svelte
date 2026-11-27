<script lang="ts">
  // Sorting one sample by hand (or its folder's unsorted ones with it): a family, then a
  // category if wanted. What the user chooses wins over the name and the audio, stays across
  // scans, and teaches the library's own model.
  import * as api from "../api";
  import type { SampleRow } from "../api";
  import { app } from "../state.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { FAMILIES, catsOf, onlyCat, dirOf } from "../sampleFamilies";

  let { row, close, done }: { row: SampleRow; close: () => void; done: () => void } = $props();
  let fam = $state(row.cat_src === "user" ? (row.family ?? "") : (row.guess ?? ""));
  let folder = $state(false);
  let busy = $state(false);
  const unsorted = $derived(!row.family || row.family === "long" || row.family === "unreadable");

  async function sort(f: string | null, c: string | null) {
    busy = true;
    try {
      const n = folder && f
        ? (await api.sortSamples([row.path], f, c)) + (await api.sortSampleFolder(dirOf(row.path), f, c))
        : await api.sortSamples([row.path], f, c);
      app.say(f ? t("samples.sort.done", { n: fmtNum(n), what: t(c ? `samples.cat.${c}` : `samples.fam.${f}`) }) : t("samples.sort.reset"));
      done();
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
</script>

<div class="sortmenu small" role="group" aria-label={t("samples.sort.title")}>
  <div class="line">
    <span class="muted">{t("samples.sort.as")}</span>
    {#each FAMILIES as [f]}
      <button class="chip" class:on={fam === f} disabled={busy} onclick={() => (catsOf(f).length ? (fam = f) : sort(f, onlyCat(f)))} title={t(`samples.famTip.${f}`)}>{t(`samples.fam.${f}`)}</button>
    {/each}
    <button class="icon x" onclick={close} aria-label={t("samples.sort.close")}>×</button>
  </div>
  {#if fam && catsOf(fam).length}
    <div class="line">
      <span class="muted">{t("samples.sort.which")}</span>
      {#each catsOf(fam) as c}
        <button class="chip" class:on={row.cat_src === "user" && row.cat === c} disabled={busy} onclick={() => sort(fam, c)}>{t(`samples.cat.${c}`)}</button>
      {/each}
      <button class="chip plain" disabled={busy} onclick={() => sort(fam, null)}>{t("samples.sort.familyOnly")}</button>
    </div>
  {/if}
  <div class="line">
    {#if unsorted}
      <label><input type="checkbox" bind:checked={folder} /> {t("samples.sort.folder")}</label>
    {/if}
    {#if row.cat_src === "user"}
      <button class="chip plain" disabled={busy} onclick={() => sort(null, null)}>{t("samples.sort.auto")}</button>
    {/if}
  </div>
</div>

<style>
  .sortmenu { display: flex; flex-direction: column; gap: 6px; padding: 8px 12px 10px 56px; background: var(--panel2); border-top: 1px solid var(--line); }
  .line { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .line:empty { display: none; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 999px; padding: 2px 10px; cursor: pointer; font: inherit; transition: background-color .15s, border-color .15s; }
  .chip:hover:not(:disabled) { border-color: var(--accent); }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .chip.plain { border-style: dashed; }
  .chip:disabled { opacity: .5; cursor: progress; }
  .x { margin-left: auto; border: 0; background: none; color: var(--muted); font-size: 16px; line-height: 1; padding: 2px 6px; cursor: pointer; border-radius: 6px; }
  .x:hover { color: var(--ink); background: var(--panel); }
  label { display: inline-flex; gap: 6px; align-items: center; color: var(--muted); cursor: pointer; }
</style>

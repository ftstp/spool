<script lang="ts">
  // The samples kept twice or more (the same name, size and first bytes): the space they take,
  // and where each copy is. Spool never deletes anything: each copy can be shown in Explorer.
  import * as api from "../api";
  import type { Dupes } from "../api/bindings/Dupes";
  import { app } from "../state.svelte";
  import { player, playingFile, playOrPause } from "../player.svelte";
  import PlayIcon from "./PlayIcon.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";

  let { close }: { close: () => void } = $props();
  let groups = $state<Dupes[] | null>(null);
  let error = $state("");
  $effect(() => {
    api.sampleDupes().then((g) => (groups = g)).catch((e) => (error = String(e)));
  });
  const mb = (b: number) => fmtNum(Math.round((b / 1e6) * 10) / 10);
  const wasted = $derived((groups ?? []).reduce((a, g) => a + g.size * (g.paths.length - 1), 0));
  const reveal = (p: string) => api.reveal(p).catch((e) => app.say(String(e), { error: true }));
</script>

<div class="dupes">
  <div class="head">
    <div>
      <b>{t("samples.dupes.title")}</b>
      <div class="small muted">
        {#if groups}{groups.length ? t(groups.length >= 500 ? "samples.dupes.summaryMore" : "samples.dupes.summary", { n: fmtNum(groups.length), mb: mb(wasted) }) : t("samples.dupes.none")}{:else if !error}{t("samples.dupes.looking")}{/if}
      </div>
    </div>
    <button class="btn sm" onclick={close}>{t("samples.back")}</button>
  </div>
  {#if error}<p class="small warn">{error}</p>{/if}
  {#if groups?.length}<p class="small faint note">{t("samples.dupes.note")}</p>{/if}
  {#each groups ?? [] as g (g.paths[0])}
    <div class="g">
      <div class="gh"><b>{g.name}</b> <span class="small faint">{t("samples.dupes.copies", { n: g.paths.length, mb: mb(g.size) })}</span>
        <button class="icon" aria-label={t("samples.play")} title={t("samples.play")} onclick={() => playOrPause(g.paths[0], () => player.play(g.paths[0], null, g.name))}>
          <PlayIcon on={playingFile(g.paths[0])} /></button>
      </div>
      {#each g.paths as p (p)}
        <div class="p small"><span class="ell" title={p}>{p}</span>
          <button class="link" onclick={() => reveal(p)}>{t("samples.reveal")}</button></div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .dupes { display: flex; flex-direction: column; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 10px 12px; border-bottom: 1px solid var(--line); }
  .note { margin: 8px 12px 0; }
  .g { padding: 8px 12px; border-top: 1px solid var(--line); }
  .g:first-of-type { border-top: 0; }
  .gh { display: flex; align-items: center; gap: 8px; }
  .p { display: flex; justify-content: space-between; gap: 10px; color: var(--muted); padding: 1px 0 1px 12px; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; flex: none; font: inherit; }
  .icon { border: 0; background: none; color: var(--muted); padding: 3px; border-radius: 6px; cursor: pointer; }
  .icon:hover { color: var(--ink); background: var(--panel2); }
  .warn { color: var(--lost); padding: 10px 12px; }
</style>

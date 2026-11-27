<script lang="ts">
  import Skeleton from "../lib/components/Skeleton.svelte";
  // Statistics: the whole catalogue in charts. Each block (StatBlock) can also be put on Home.
  import { app } from "../lib/state.svelte";
  import { lastStats } from "../lib/homeStats.svelte";
  import Library from "../lib/components/Library.svelte";
  import StatBlock, { STAT_BLOCKS } from "../lib/components/StatBlock.svelte";
  import { t } from "../lib/i18n/index.svelte";
  import * as api from "../lib/api";
  import type { HomeStats, Progress } from "../lib/api";

  const o = $derived(app.overview);
  let h = $state<HomeStats | null>(lastStats.h);
  let p = $state<Progress | null>(lastStats.p);
  let seq = 0;
  $effect(() => {
    app.version; // again after every change
    const n = ++seq; // the newest answer only
    api.homeStats().then((x) => n === seq && (h = lastStats.h = x)).catch(() => n === seq && (h = null));
    api.progress().then((x) => n === seq && (p = lastStats.p = x)).catch(() => n === seq && (p = null));
  });
  const nSongs = (n: number) => t("songs.nSongs", { n });
  const nProjects = (n: number) => t("stats.nProjects", { n });
  // the catalogue's figures first, the sample library, then the history
  const FIRST = STAT_BLOCKS.slice(0, 6);
  const AFTER = STAT_BLOCKS.slice(6);
</script>

<div class="page">
  <header class="head">
    <div>
      <h1>{t("stats.title")}</h1>
      {#if o && h}<p class="sub muted">{nSongs(o.songs)} · {nProjects(h.files)}</p>{/if}
    </div>
  </header>

  {#if !o || !h}<Skeleton kind="page" />
  {:else}
    <div class="sgrid">
      {#each FIRST as id (id)}<StatBlock {id} {o} {h} {p} />{/each}
      <div class="libwrap"><Library /></div>
      {#each AFTER as id (id)}<StatBlock {id} {o} {h} {p} />{/each}
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 20px; margin-bottom: 16px; }
  .sub { margin: 0; }
  .sgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(360px, 100%), 1fr)); gap: 14px; align-items: start; }
  .libwrap { grid-column: 1 / -1; }
</style>

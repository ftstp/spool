<script lang="ts">
  // Songs that are really one: another song (often a variant in the same folder) attached to
  // this one, its projects and future saves included. "Split" undoes it.
  import * as api from "../api";
  import type { SongDetail } from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { sid, d }: { sid: number; d: SongDetail } = $props();
  let picking = $state(false);
  let allSibs = $state(false);
  /** A folder holding hundreds of projects lists them all: a few, the rest on demand. */
  const SIBS = 12;
  const sibs = $derived(allSibs ? d.siblings : d.siblings.slice(0, SIBS));
  let q = $state("");
  const others = $derived(
    app.songs
      .filter((s) => s.id !== sid && !s.archived && (!q || (s.title || "").toLowerCase().includes(q.toLowerCase())))
      .slice(0, 8),
  );
  async function run<T>(p: Promise<T>, ok?: (r: T) => string) {
    try {
      const r = await p;
      if (ok) app.say(ok(r));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const attach = (id: number) => {
    picking = false;
    q = "";
    run(api.mergeSong(id, sid), (r) => r);
  };
</script>

<div class="merges small">
  {#if d.siblings.length}
    <div class="row"><span class="faint">{d.siblings.length > SIBS ? t("song.merges.sameFolderCount", { n: d.siblings.length }) : t("song.merges.sameFolder")}</span>
      {#each sibs as o (o.id)}
        <span class="sib"><a href="#/song/{o.id}">{o.title}</a>
          <button class="link" title={t("song.merges.attachTip", { title: o.title })} onclick={() => attach(o.id)}>{t("song.merges.attachHere")}</button></span>
      {/each}
      {#if d.siblings.length > SIBS}
        <button class="link more" onclick={() => (allSibs = !allSibs)}>{allSibs ? t("song.merges.less") : t("song.merges.seeOthers", { n: d.siblings.length - SIBS })}</button>
      {/if}
    </div>
  {/if}
  {#each d.merged as m (m.key)}
    <div class="row"><span class="faint">{t("song.merges.attached")}</span> {m.name}
      <button class="link" onclick={() => run(api.splitSong(sid, m.key), () => t("song.merges.splitDone", { name: m.name }))}>{t("song.merges.split")}</button></div>
  {/each}
  {#if picking}
    <div class="pick">
      <input type="search" placeholder={t("song.merges.pickPh")} bind:value={q} use:focus onkeydown={(e) => { if (e.key === "Escape") picking = false; }} />
      {#each others as o (o.id)}<button class="opt" onclick={() => attach(o.id)}>{o.title}<span class="faint dir">{o.dir}</span></button>{/each}
    </div>
  {:else}
    <button class="link" onclick={() => (picking = true)}>{t("song.merges.attachOther")}</button>
  {/if}
</div>

<script module lang="ts">
  /** `autofocus` does nothing on an element created later. */
  export function focus(n: HTMLElement) {
    n.focus();
  }
</script>

<style>
  .merges { margin-top: 10px; display: flex; flex-direction: column; gap: 4px; color: var(--muted); }
  .row { display: flex; flex-wrap: wrap; gap: 4px 12px; align-items: baseline; }
  .sib { display: inline-flex; gap: 6px; align-items: baseline; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; font-size: 12px; }
  .more { font-weight: 600; }
  .sib .link { opacity: 0; transition: opacity .15s; }
  .sib:hover .link, .sib .link:focus-visible { opacity: 1; }

  .pick { display: flex; flex-direction: column; gap: 2px; margin-top: 4px; }
  .pick input { width: 100%; margin-bottom: 4px; }
  .opt { text-align: left; border: 0; background: none; color: var(--ink); padding: 4px 6px; border-radius: 6px; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dir { margin-left: 8px; font-size: 11.5px; }
  .opt:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
</style>

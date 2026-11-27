<script lang="ts">
  // The title bar of an open section of a song's sheet: its name, what it holds, and a button
  // that closes it (the sections bar above the sheet opens it again).
  import type { Snippet } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { setSectionOpen } from "../tabs.svelte";
  let { sid, key, title, children, actions }: { sid: number; key: string; title: string; children?: Snippet; actions?: Snippet } = $props();
  // it stays under the sheet's bar while its section scrolls by; its height is handed to the
  // section (--fh-h) for what sticks under it (the arrangement's timeline)
  let el = $state<HTMLElement>();
  $effect(() => {
    const e = el;
    const sec = e?.parentElement;
    if (!e || !sec) return;
    const ro = new ResizeObserver(() => sec.style.setProperty("--fh-h", `${e.offsetHeight}px`));
    ro.observe(e);
    return () => ro.disconnect();
  });
</script>

<div class="head fh" bind:this={el}>
  <h2>{title}</h2>
  {@render children?.()}
  <span class="grow"></span>
  {@render actions?.()}
  <button class="x" title={t("song.sections.close")} aria-label={t("song.sections.closeName", { name: title })} onclick={() => setSectionOpen(sid, key, false)}>
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M6 6l12 12M18 6L6 18" /></svg>
  </button>
</div>

<style>
  .fh { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin: -6px -16px 8px; padding: 6px 16px;
    position: sticky; top: var(--sheet-top, 0px); z-index: 5; background: var(--panel); border-radius: var(--radius) var(--radius) 0 0; }
  .fh h2 { margin: 0; }
  .grow { flex: 1; }
  .x { width: 24px; height: 24px; flex: none; display: grid; place-items: center; border: 0; border-radius: 6px; background: none; color: var(--faint); cursor: pointer; padding: 0; }
  .x:hover { color: var(--ink); background: var(--panel2); }
  .x:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
</style>

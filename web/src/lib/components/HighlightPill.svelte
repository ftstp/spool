<script lang="ts">
  // The pill at the top of the window: one useful thing at a time (lib/highlights), the next
  // one every 25 seconds (not while the pointer rests on it), a click goes where it is dealt
  // with, the arrow shows the next one at once.
  import { app } from "../state.svelte";
  import { highlights } from "../highlights.svelte";
  import { t } from "../i18n/index.svelte";

  let { fix = 0 }: { fix?: number } = $props();
  const list = $derived(highlights(app.songs, app.releases));
  let i = $state(0);
  let hold = $state(false);
  const cur = $derived(list.length ? list[i % list.length] : null);
  $effect(() => {
    if (hold || list.length < 2) return;
    const id = setInterval(() => (i = (i + 1) % list.length), 25000);
    return () => clearInterval(id);
  });
</script>

{#if cur}
  <span class="hl" class:warn={cur.tone === "warn"} role="group" onpointerenter={() => (hold = true)} onpointerleave={() => (hold = false)}>
    {#key cur.key + cur.text}<a class="tx" href={cur.href}><i></i><span>{cur.text}</span></a>{/key}
    {#if list.length > 1}<button class="nx" title={t("app.hl.next", { i: (i % list.length) + 1, n: list.length })} aria-label={t("app.hl.nextLabel")} onclick={() => (i = (i + 1) % list.length)}>
      <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.4"><path d="M9 6l6 6-6 6" /></svg></button>{/if}
  </span>
{:else if fix}
  <a class="hl" href="#/repair"><span class="tx"><i class="w"></i><span>{t("app.top.fixable", { n: fix })}</span></span></a>
{/if}

<style>
  .hl { display: inline-flex; align-items: center; min-width: 0; max-width: 46ch; border-radius: 999px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); font-size: 12px; text-decoration: none; }
  .tx { display: inline-flex; align-items: center; gap: 7px; min-width: 0; padding: 4px 4px 4px 11px; color: inherit; text-decoration: none; animation: hlin .35s ease-out; }
  .tx span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tx:hover { color: var(--ink); }
  .tx i { width: 7px; height: 7px; flex: none; border-radius: 50%; background: var(--accent); }
  .warn .tx i, .tx i.w { background: var(--lost); }
  .nx { border: 0; background: none; color: var(--faint); padding: 4px 9px 4px 4px; cursor: pointer; display: grid; place-items: center; }
  .nx:hover { color: var(--ink); }
  @keyframes hlin { from { opacity: 0; transform: translateY(3px); } }
  @media (prefers-reduced-motion: reduce) { .tx { animation: none; } }
</style>

<script lang="ts">
  // A rating, 1 to 5 stars: a click sets it, a click on the current one clears it. `small`:
  // shown only (lists).
  let { value, set, small = false }: { value: number | null | undefined; set?: (n: number | null) => void; small?: boolean } = $props();
  import { t } from "../i18n/index.svelte";
  let hover = $state(0);
  const n = $derived(hover || value || 0);
</script>

{#if set}
  <span class="stars" role="group" aria-label={t("song.stars.label")} onmouseleave={() => (hover = 0)}>
    {#each [1, 2, 3, 4, 5] as i}
      <button aria-pressed={value === i} aria-label={t("song.stars.star", { n: i })} title={value === i ? t("song.stars.clear") : t("song.stars.outOf5", { n: i })}
        class:on={i <= n} onmouseenter={() => (hover = i)} onclick={() => set(value === i ? null : i)}>★</button>
    {/each}
  </span>
{:else if value}
  <span class="stars ro" class:small title={t("song.stars.outOf5", { n: value })} aria-label={t("song.stars.outOf5", { n: value })}>{"★".repeat(value)}<span class="off">{"★".repeat(5 - value)}</span></span>
{/if}

<style>
  .stars { display: inline-flex; gap: 1px; line-height: 1; }
  button { border: 0; background: none; padding: 0 1px; cursor: pointer; font-size: 16px; color: var(--line); transition: color .1s, transform .1s; }
  button.on { color: #e8b84a; }
  button:hover { transform: scale(1.12); }
  .ro { color: #e8b84a; font-size: 12px; letter-spacing: 1px; }
  .ro.small { font-size: 10.5px; }
  .off { color: var(--line); }
</style>

<script lang="ts">
  // A star: the thing starred as a favourite, or no longer. `sub`: what tells it apart in the
  // Home list (a folder, an artist); `song`: the song to open it from (a board item, a project).
  import { isFav, toggleFav } from "../favorites.svelte";
  import { t } from "../i18n/index.svelte";

  let { kind, id, label, sub = "", song = null, size = 15 }: { kind: string; id: string; label: string; sub?: string; song?: number | null; size?: number } = $props();
  const on = $derived(isFav(kind, id));
</script>

<button class="fav" class:on title={on ? t("common.fav.off") : t("common.fav.on")} aria-label={on ? t("common.fav.off") : t("common.fav.on")} aria-pressed={on}
  onclick={(e) => { e.preventDefault(); e.stopPropagation(); void toggleFav({ kind, id, label, sub, song }); }}>
  <svg viewBox="0 0 24 24" width={size} height={size} fill={on ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" /></svg>
</button>

<style>
  .fav { border: 0; background: none; padding: 3px; border-radius: 5px; color: var(--faint); cursor: pointer; display: inline-grid; place-items: center; transition: color .15s, transform .15s; flex: none; }
  .fav:hover { color: var(--ink); }
  .fav.on { color: #e0a526; }
  .fav:active { transform: scale(0.9); }
</style>

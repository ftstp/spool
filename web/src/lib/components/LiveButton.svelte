<script lang="ts">
  // "Open in Live": a project opened in Ableton Live, said at a glance (the word Live and a
  // glyph of a mixer's faders), never a bare square icon. `label` shows the full words.
  import { liveOpen } from "../liveOpen.svelte";
  import { t } from "../i18n/index.svelte";

  let { rel, label = false, size = "sm" }: { rel: string; label?: boolean; size?: "sm" | "md" } = $props();
  const open = (e: MouseEvent) => {
    e.stopPropagation();
    // asks first when Spool holds changes the set does not have yet (liveOpen)
    void liveOpen.open(rel);
  };
</script>

<button class="live {size}" title={t("song.openInLiveTip")} aria-label={t("song.openInLive")} onclick={open}>
  <svg viewBox="0 0 16 16" width={size === "md" ? 14 : 12} height={size === "md" ? 14 : 12} aria-hidden="true">
    <rect x="1.5" y="3" width="2" height="10" rx="0.6" />
    <rect x="5.5" y="3" width="2" height="10" rx="0.6" />
    <rect x="9.5" y="3" width="2" height="10" rx="0.6" />
    <rect x="13" y="3" width="2" height="10" rx="0.6" />
    <rect x="0.5" y="7" width="4" height="2" rx="0.6" class="k" />
    <rect x="8.5" y="5" width="4" height="2" rx="0.6" class="k" />
  </svg>
  <span>{label ? t("song.openInLive") : "Live"}</span>
</button>

<style>
  .live { display: inline-flex; align-items: center; gap: 5px; border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; cursor: pointer; font: inherit; font-weight: 600; white-space: nowrap; transition: background-color .15s, border-color .15s; }
  .live.sm { padding: 2px 7px; font-size: 11.5px; }
  .live.md { padding: 6px 12px; font-size: 13px; }
  .live svg rect { fill: currentColor; }
  .live svg .k { fill: var(--accent); }
  .live:hover { border-color: var(--accent); background: var(--accent-bg); }
</style>

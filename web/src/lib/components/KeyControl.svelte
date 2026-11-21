<script lang="ts">
  // Higher or lower by semitones, the speed kept, for whatever plays (a reference render, a
  // project, a MIDI file): - and +, the value (the mouse wheel over it; a click puts it back).
  import type { Pitch } from "../pitch";
  import { t } from "../i18n/index.svelte";

  let { p }: { p: Pitch } = $props();
  const shown = $derived(p.semis === 0 ? t("common.pitch.none") : t("common.pitch.semis", { n: (p.semis > 0 ? "+" : "") + p.semis }));
  function wheel(node: HTMLElement) {
    const on = (e: WheelEvent) => {
      e.preventDefault();
      p.setSemis(p.semis + (e.deltaY < 0 ? 1 : -1));
    };
    node.addEventListener("wheel", on, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", on) };
  }
</script>

<span class="key" title={t("common.pitch.tip")} use:wheel>
  <button class="st" aria-label={t("common.pitch.down")} onclick={() => p.setSemis(p.semis - 1)}>−</button>
  <button class="v" class:changed={p.semis !== 0} aria-label={t("common.pitch.reset")} onclick={() => p.setSemis(0)}>{shown}</button>
  <button class="st" aria-label={t("common.pitch.up")} onclick={() => p.setSemis(p.semis + 1)}>+</button>
</span>

<style>
  .key { display: inline-flex; align-items: center; gap: 0; white-space: nowrap; font-size: 12px; }
  button { border: 1px solid transparent; background: none; color: var(--muted); font: inherit; cursor: pointer; border-radius: 6px; padding: 2px 6px; transition: background-color .15s, color .15s; }
  button:hover { color: var(--ink); background: var(--panel); }
  .st { width: 18px; padding: 2px 0; font-size: 14px; line-height: 1; }
  .v { width: 50px; padding: 2px 0; font-variant-numeric: tabular-nums; text-align: center; }
  .v.changed { color: var(--ink); font-weight: 600; }
</style>

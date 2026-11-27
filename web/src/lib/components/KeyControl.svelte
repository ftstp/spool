<script lang="ts">
  // Higher or lower by semitones, the speed kept, for whatever plays (a reference render, a
  // project, a MIDI file). Shown as the key heard when the song's key is known ("Fm", then
  // "Gm" two semitones up, the shift under it), else as the shift alone. - and +, the mouse
  // wheel over it; a click on the value puts it back.
  import { transposeKey, type Pitch } from "../pitch";
  import { t } from "../i18n/index.svelte";

  let { p, base = null }: { p: Pitch; base?: string | null } = $props();
  const now = $derived(base ? transposeKey(base, p.semis) : null);
  const shift = $derived((p.semis > 0 ? "+" : "") + p.semis);
  function wheel(node: HTMLElement) {
    const on = (e: WheelEvent) => {
      e.preventDefault();
      p.setSemis(p.semis + (e.deltaY < 0 ? 1 : -1));
    };
    node.addEventListener("wheel", on, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", on) };
  }
</script>

<span class="ctl" title={t("common.pitch.tip")} use:wheel>
  <button class="st" aria-label={t("common.pitch.down")} onclick={() => p.setSemis(p.semis - 1)}>−</button>
  <button class="v" class:changed={p.semis !== 0} aria-label={t("common.pitch.reset")} onclick={() => p.setSemis(0)}>
    <span class="lb">{t("common.pitch.label")}</span>
    <b>{now ?? (p.semis ? shift : t("common.pitch.orig"))}</b>
    {#if now && p.semis}<span class="was">{t("common.pitch.from", { key: base ?? "", n: shift })}</span>{/if}
  </button>
  <button class="st" aria-label={t("common.pitch.up")} onclick={() => p.setSemis(p.semis + 1)}>+</button>
</span>

<style>
  .ctl { display: inline-flex; align-items: center; white-space: nowrap; }
  button { border: 1px solid transparent; background: none; color: var(--muted); font: inherit; cursor: pointer; border-radius: 6px; transition: background-color .15s, color .15s; }
  button:hover { color: var(--ink); background: var(--panel); }
  .st { width: 18px; padding: 2px 0; font-size: 14px; line-height: 1; }
  .v { width: 58px; padding: 1px 0; display: flex; flex-direction: column; align-items: center; line-height: 1.15; }
  .lb { font-size: 9.5px; color: var(--faint); letter-spacing: .02em; }
  b { font-size: 13px; font-weight: 600; color: var(--muted); font-variant-numeric: tabular-nums; }
  .was { font-size: 9.5px; color: var(--faint); }
  .v.changed b { color: var(--accent); }
</style>

<script lang="ts">
  // Faster / slower without changing the key, for whatever plays (a sample, a reference
  // render, a MIDI file): the tempo heard in big ("128"), what it was under it once changed,
  // - and +, the mouse wheel over it, dragged up / right to go faster; a click without moving
  // puts it back as recorded. "Lock to" a tempo when one is known. A file whose tempo is
  // unknown (none in its name or its data) shows a percentage instead.
  import { clampRate, stepRate, type Tempo } from "../tempo";
  import { t, fmtNum } from "../i18n/index.svelte";

  let { p, compact = false }: { p: Tempo; compact?: boolean } = $props();
  const big = $derived(p.base ? fmtNum(Math.round(p.base * p.eff * 10) / 10) : t("common.tempo.pct", { n: Math.round(p.eff * 100) }));
  const changed = $derived(Math.abs(p.eff - 1) > 0.001);

  // the wheel: one step a notch (Svelte adds wheel listeners as passive, which could not stop
  // the page from scrolling, hence the action)
  function wheel(node: HTMLElement) {
    const on = (e: WheelEvent) => {
      e.preventDefault();
      p.setRate(stepRate(p.eff, p.base, e.deltaY < 0 ? 1 : -1));
    };
    node.addEventListener("wheel", on, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", on) };
  }

  // dragging the value: 4 px a step (1 BPM, or 1 % when the tempo is unknown)
  let drag: { x: number; y: number; from: number; moved: boolean } | null = null;
  let justDragged = false;
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, from: p.base ? p.base * p.eff : p.eff, moved: false };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const steps = Math.round((e.clientX - drag.x - (e.clientY - drag.y)) / 4);
    if (!steps && !drag.moved) return;
    drag.moved = true;
    p.setRate(p.base ? clampRate((Math.round(drag.from) + steps) / p.base) : clampRate(Math.round(drag.from * 100 + steps) / 100));
  }
  function up() {
    justDragged = !!drag?.moved;
    drag = null;
  }
  function click() {
    if (justDragged) return void (justDragged = false);
    p.setRate(1);
  }
  const tip = $derived(p.base ? t("common.tempo.tip") : t("common.tempo.tipUnknown"));
</script>

<span class="ctl" class:compact title={tip} use:wheel>
  <button class="st" aria-label={t("common.tempo.slower")} onclick={() => p.setRate(stepRate(p.eff, p.base, -1))}>−</button>
  <button class="v" class:changed aria-label={t("common.tempo.reset")} title={tip}
    onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} onclick={click}>
    <span class="lb">{p.base ? t("common.tempo.label") : t("common.tempo.speed")}</span>
    <b>{big}</b>
    {#if changed && p.base}<span class="was">{t("common.tempo.from", { n: fmtNum(Math.round(p.base * 10) / 10) })}</span>{/if}
  </button>
  <button class="st" aria-label={t("common.tempo.faster")} onclick={() => p.setRate(stepRate(p.eff, p.base, 1))}>+</button>
  {#if p.target}
    <button class="lock" class:on={p.lock} aria-pressed={p.lock} title={t("common.tempo.lockTip", { n: fmtNum(p.target) })} onclick={() => p.setLock(!p.lock)}>
      {t("common.tempo.lock", { n: fmtNum(p.target) })}
    </button>
  {/if}
</span>

<style>
  .ctl { display: inline-flex; align-items: center; white-space: nowrap; font-size: 12px; }
  button { border: 1px solid transparent; background: none; color: var(--muted); font: inherit; cursor: pointer; border-radius: 6px; padding: 2px 6px; transition: background-color .15s, color .15s; }
  button:hover { color: var(--ink); background: var(--panel); }
  .st { width: 18px; padding: 2px 0; font-size: 14px; line-height: 1; }
  .v { width: 58px; padding: 1px 0; display: flex; flex-direction: column; align-items: center; line-height: 1.15; cursor: ew-resize; touch-action: none; user-select: none; }
  .lb { font-size: 9.5px; color: var(--faint); letter-spacing: .02em; }
  b { font-size: 13px; font-weight: 600; color: var(--muted); font-variant-numeric: tabular-nums; }
  .was { font-size: 9.5px; color: var(--faint); }
  .v.changed b { color: var(--accent); }
  .compact .v { width: 50px; }
  .lock { margin-left: 4px; border-color: var(--line); }
  .lock.on { border-color: var(--accent); color: var(--ink); background: var(--accent-bg); }
</style>

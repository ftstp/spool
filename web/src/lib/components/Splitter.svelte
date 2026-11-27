<script lang="ts">
  // A bar between two side-by-side areas: dragged (or moved with the arrow keys), it sets the
  // width of one of them, kept across launches under `key`. `side`: which area the width is
  // for ("left": the one before the bar, "right": the one after).
  let {
    width = $bindable(),
    min = 220,
    max = 900,
    side = "left",
    key,
    label,
  }: { width: number; min?: number; max?: number; side?: "left" | "right"; key: string; label: string } = $props();

  // the width kept from the last time
  try {
    const kept = Number(localStorage.getItem("pf:split:" + key));
    if (kept >= min && kept <= max) width = kept;
  } catch {
    /* not kept */
  }
  const keep = () => {
    try {
      localStorage.setItem("pf:split:" + key, String(Math.round(width)));
    } catch {
      /* not kept */
    }
  };
  const clamp = (w: number) => Math.max(min, Math.min(max, w));
  let dragging = $state(false);
  function down(e: PointerEvent) {
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    dragging = true;
    const x0 = e.clientX, w0 = width;
    const move = (m: PointerEvent) => (width = clamp(w0 + (m.clientX - x0) * (side === "left" ? 1 : -1)));
    const up = () => {
      dragging = false;
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      keep();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }
  function keys(e: KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const dir = (e.key === "ArrowRight" ? 1 : -1) * (side === "left" ? 1 : -1);
      width = clamp(width + dir * step);
      keep();
    }
  }
</script>

<div class="splitter" class:dragging role="separator" aria-orientation="vertical" aria-label={label} aria-valuenow={Math.round(width)} aria-valuemin={min} aria-valuemax={max}
  tabindex="0" onpointerdown={down} onkeydown={keys} ondblclick={() => { width = clamp((min + max) / 2); keep(); }}></div>

<style>
  /* a thin line, a wider grip: easy to catch, quiet to look at */
  .splitter { position: relative; width: 7px; margin: 0 -3px; cursor: col-resize; z-index: 5; touch-action: none; }
  .splitter::after { content: ""; position: absolute; top: 0; bottom: 0; left: 3px; width: 1px; background: var(--line); transition: background-color .15s, width .15s, left .15s; }
  .splitter:hover::after, .splitter.dragging::after, .splitter:focus-visible::after { left: 2px; width: 3px; background: var(--muted); }
  .splitter:focus-visible { outline: none; }
</style>

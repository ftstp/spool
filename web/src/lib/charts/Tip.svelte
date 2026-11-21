<script lang="ts" module>
  // One hover tooltip for every chart: a chart calls show() with the pointer position and
  // two lines (what, how many), hide() when the pointer leaves.
  export const tip = $state({ on: false, x: 0, y: 0, title: "", value: "" });
  export function show(e: MouseEvent, title: string, value = "") {
    tip.on = true;
    tip.x = e.clientX;
    tip.y = e.clientY;
    tip.title = title;
    tip.value = value;
  }
  export function hide() {
    tip.on = false;
  }
</script>

{#if tip.on}
  <div class="tip" role="tooltip" style:left="{Math.min(tip.x, innerWidth - 230)}px" style:top="{tip.y - 12}px">
    <div class="t">{tip.title}</div>
    {#if tip.value}<div class="v">{tip.value}</div>{/if}
  </div>
{/if}

<style>
  .tip { position: fixed; transform: translate(-50%, -100%); pointer-events: none; z-index: 400; background: var(--ink); color: var(--bg); border-radius: 8px;
    padding: 6px 10px; font-size: 12px; line-height: 1.35; max-width: 260px; box-shadow: 0 8px 24px rgba(0, 0, 0, .3); white-space: nowrap; }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; }
  .v { opacity: .75; font-variant-numeric: tabular-nums; }
</style>

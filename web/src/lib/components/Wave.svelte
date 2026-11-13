<script lang="ts">
  // A waveform drawn from peaks, the played part in the accent colour. Click to seek.
  let { peaks, progress = 0, seek, height = 44 }: { peaks: number[] | null; progress?: number; seek?: (frac: number) => void; height?: number } = $props();
  let canvas = $state<HTMLCanvasElement>();
  let w = $state(0);

  $effect(() => {
    const c = canvas;
    if (!c || !w) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = w * dpr;
    c.height = height * dpr;
    const g = c.getContext("2d")!;
    const css = getComputedStyle(c);
    const played = css.getPropertyValue("--accent").trim() || "#7ba6dc";
    const rest = css.getPropertyValue("--faint").trim() || "#686870";
    g.clearRect(0, 0, c.width, c.height);
    const p = peaks;
    if (!p?.length) {
      g.fillStyle = rest;
      g.fillRect(0, c.height / 2 - dpr, c.width * 1, 2 * dpr);
      g.fillStyle = played;
      g.fillRect(0, c.height / 2 - dpr, c.width * progress, 2 * dpr);
      return;
    }
    const bw = c.width / p.length;
    const max = Math.max(...p, 0.01);
    p.forEach((v, i) => {
      const h = Math.max(2 * dpr, (v / max) * (c.height - 4 * dpr));
      g.fillStyle = i / p.length < progress ? played : rest;
      g.fillRect(i * bw + bw * 0.15, (c.height - h) / 2, Math.max(1, bw * 0.7), h);
    });
  });
</script>

<div class="wave" bind:clientWidth={w} style:height="{height}px">
  <canvas bind:this={canvas} style:width="{w}px" style:height="{height}px"
    onclick={(e) => seek?.(e.offsetX / w)} class:seek={!!seek} aria-hidden="true"></canvas>
</div>

<style>
  .wave { width: 100%; min-width: 0; }
  canvas { display: block; }
  .seek { cursor: pointer; }
</style>

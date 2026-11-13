<script lang="ts">
  // A song's picture: its cover when it has one, else its initial on a colour of its own.
  import { mediaUrl } from "../api";
  import { hue } from "../format";
  let { title, cover, size = 52, radius = 9 }: { title: string | null; cover?: string | null; size?: number | string; radius?: number } = $props();
  const url = $derived(cover ? mediaUrl(cover) : "");
  let broken = $state(false);
</script>

<div class="art" style:width={typeof size === "number" ? size + "px" : size} style:border-radius="{radius}px" style="--h:{hue(title ?? '')}">
  {#if url && !broken}<img src={url} alt="" loading="lazy" onerror={() => (broken = true)} />
  {:else}<span style:font-size={typeof size === "number" ? Math.round(size * 0.38) + "px" : "clamp(20px, 3vw, 54px)"}>{(title || "?").slice(0, 1).toUpperCase()}</span>{/if}
</div>

<style>
  .art { aspect-ratio: 1; flex: none; overflow: hidden; display: grid; place-items: center; color: #fff; font-weight: 650;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
</style>

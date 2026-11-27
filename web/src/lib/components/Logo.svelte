<script lang="ts">
  // Spool's mark: two tape reels joined by the tape, and the red record light. The square
  // takes the ink colour and the reels the page's, so it reads in light and dark alike.
  let { size = 30 }: { size?: number } = $props();
</script>

<svg class="logo" width={size} height={size} viewBox="0 0 54 54" role="img" aria-label="Spool">
  <rect width="54" height="54" rx="12" class="sq" />
  <circle cx="18" cy="27" r="8" class="reel" />
  <circle cx="36" cy="27" r="8" class="reel" />
  <circle cx="18" cy="27" r="2" class="hub" />
  <circle cx="36" cy="27" r="2" class="hub" />
  <path d="M18 35h18" class="tape" />
  <circle cx="44" cy="11" r="4.5" class="rec" />
</svg>

<style>
  .logo { flex: none; display: block; }
  .sq { fill: var(--ink); }
  .reel { fill: none; stroke: var(--bg); stroke-width: 3; }
  .hub { fill: var(--bg); }
  .tape { stroke: var(--bg); stroke-width: 3; }
  .rec { fill: var(--rec); }
</style>

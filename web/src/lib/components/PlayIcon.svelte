<script lang="ts">
  // The triangle is drawn half-way between box-centred and centroid-centred, the usual optical
  // centre of a play glyph in a disc; every copy of it uses this same path.
  // The play triangle, or the pause bars while what the button plays is playing: every play
  // button of the app says the same thing.
  let { on = false, size = 12 }: { on?: boolean; size?: number } = $props();
</script>

{#if on}<svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
{:else}<svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden="true"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}

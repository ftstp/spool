<script lang="ts">
  import { stageLabel } from "../format";
  let { stage }: { stage?: string | null } = $props();
</script>

<span class="stage s-{stage || 'none'}"><i></i>{stageLabel(stage)}</span>

<style>
  .stage { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; white-space: nowrap; }
  i { width: 7px; height: 7px; border-radius: 50%; background: var(--s-none); display: inline-block; }
  .s-draft i { background: var(--s-draft); } .s-arrangement i { background: var(--s-arrangement); }
  .s-mix i { background: var(--s-mix); } .s-master i { background: var(--s-master); }
  .s-done i { background: var(--s-done); } .s-pause i { background: var(--s-pause); }
</style>

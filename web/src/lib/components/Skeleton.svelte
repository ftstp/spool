<script lang="ts">
  // What a page will look like, in grey, while its data is on its way: the page does not
  // jump when it comes, and the wait shows. `kind`: a song's sheet, a list, or a page of
  // blocks.
  import { t } from "../i18n/index.svelte";
  let { kind = "page", rows = 8 }: { kind?: "sheet" | "list" | "page"; rows?: number } = $props();
</script>

<div class="sk" role="status" aria-live="polite" aria-label={t("common.loading")}>
  {#if kind === "sheet"}
    <div class="head">
      <span class="b art"></span>
      <div class="lines"><span class="b h1"></span><span class="b l" style:width="40%"></span><span class="b l" style:width="55%"></span><span class="b chips"></span></div>
    </div>
    <span class="b card tall"></span>
    <span class="b card"></span>
    <div class="cols"><span class="b card tall"></span><span class="b card tall"></span><span class="b card tall"></span></div>
  {:else if kind === "list"}
    <span class="b h1 short"></span>
    {#each Array(rows) as _, i (i)}<div class="row"><span class="b dot"></span><span class="b l" style:width={`${45 + ((i * 17) % 35)}%`}></span><span class="b l tiny"></span></div>{/each}
  {:else}
    <span class="b h1 short"></span>
    <div class="cols"><span class="b card"></span><span class="b card"></span><span class="b card"></span></div>
    <span class="b card tall"></span>
    <span class="b card"></span>
  {/if}
  <span class="sr">{t("common.loading")}</span>
</div>

<style>
  .sk { padding: 20px 24px; display: grid; gap: 14px; }
  .b { display: block; border-radius: 8px; background: linear-gradient(90deg, var(--panel) 0%, var(--panel2) 40%, var(--panel) 80%); background-size: 200% 100%; animation: shine 1.4s ease-in-out infinite; }
  @keyframes shine { from { background-position: 100% 0; } to { background-position: -100% 0; } }
  @media (prefers-reduced-motion: reduce) { .b { animation: none; } }
  .head { display: flex; gap: 18px; align-items: center; }
  .art { width: 140px; height: 140px; border-radius: 12px; flex: none; }
  .lines { flex: 1; display: grid; gap: 10px; }
  .h1 { height: 30px; width: 60%; }
  .h1.short { width: 220px; }
  .l { height: 13px; }
  .chips { height: 28px; width: 70%; }
  .card { height: 64px; }
  .card.tall { height: 160px; }
  .cols { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }
  .row { display: flex; align-items: center; gap: 12px; height: 34px; }
  .dot { width: 34px; height: 34px; border-radius: 8px; flex: none; }
  .tiny { width: 60px; margin-left: auto; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
</style>

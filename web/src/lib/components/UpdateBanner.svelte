<script lang="ts">
  // A newer Spool, offered at the foot of the sidebar: install (Spool restarts) or later.
  import { updates } from "../update.svelte";
  import { t } from "../i18n/index.svelte";

  let { mini = false }: { mini?: boolean } = $props();
  const u = $derived(updates.info);
</script>

{#if u && !updates.later}
  <div class="up" class:mini title={u.notes ? t("app.update.whatsNew", { notes: u.notes }) : `Spool ${u.version}`}>
    {#if mini}
      <button class="dot" aria-label={t("app.update.install", { version: u.version })} disabled={updates.installing} onclick={() => updates.install()}>↑</button>
    {:else}
      <div class="t"><b>Spool {u.version}</b> {t("app.update.isAvailable")}</div>
      {#if u.notes}<div class="n small">{u.notes.split("\n")[0]}</div>{/if}
      <div class="a">
        <button class="btn sm primary" disabled={updates.installing} onclick={() => updates.install()}>{updates.installing ? t("app.update.downloading") : t("app.update.installRestart")}</button>
        {#if !updates.installing}<button class="link small" onclick={() => (updates.later = true)}>{t("app.update.later")}</button>{/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .up { margin: 6px 0; padding: 10px 11px; border-radius: 10px; border: 1px solid var(--line); background: var(--panel); display: flex; flex-direction: column; gap: 6px; font-size: 12.5px; }
  .up.mini { padding: 4px; align-items: center; }
  .t b { font-family: var(--mono); }
  .n { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .a { display: flex; align-items: center; gap: 10px; }
  .link { border: 0; background: none; color: var(--muted); cursor: pointer; padding: 0; text-decoration: underline; }
  .dot { width: 28px; height: 28px; border-radius: 50%; border: 0; background: var(--rec); color: #fff; cursor: pointer; font-weight: 700; }
  .btn:disabled, .dot:disabled { opacity: .6; cursor: default; }
</style>

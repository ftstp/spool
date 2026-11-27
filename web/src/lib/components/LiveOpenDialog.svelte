<script lang="ts">
  // The question before a project opens in Live, when Spool holds changes it does not have
  // yet (see liveOpen): what to carry into a copy, or the original as it is.
  import { liveOpen } from "../liveOpen.svelte";
  import { t } from "../i18n/index.svelte";

  const q = $derived(liveOpen.ask);
  const any = $derived(!!q?.choices.some((c) => c.on));
  let dlg = $state<HTMLDialogElement>();
  $effect(() => {
    if (!dlg) return;
    if (q && !dlg.open) dlg.showModal();
    else if (!q && dlg.open) dlg.close();
  });
</script>

<dialog bind:this={dlg} class="lo" aria-labelledby="lo-title" onclose={() => (liveOpen.ask = null)}>
  {#if q}
    <h2 id="lo-title">{t("liveOpen.title", { name: q.name })}</h2>
    <p class="small muted">{t("liveOpen.lead")}</p>
    <ul>
      {#each q.choices as c (c.code)}
        <li>
          <label>
            <input type="checkbox" bind:checked={c.on} />
            <span><b>{c.label}</b><span class="small muted">{c.detail}</span></span>
          </label>
        </li>
      {/each}
    </ul>
    <p class="small faint">{t("liveOpen.copyNote")}</p>
    <div class="acts">
      <button class="btn" onclick={() => liveOpen.original()} disabled={liveOpen.busy}>{t("liveOpen.original")}</button>
      <span class="grow"></span>
      <button class="btn" onclick={() => (liveOpen.ask = null)} disabled={liveOpen.busy}>{t("liveOpen.cancel")}</button>
      <button class="btn primary" onclick={() => liveOpen.withChanges()} disabled={liveOpen.busy || !any}>
        {liveOpen.busy ? t("liveOpen.writing") : t("liveOpen.withChanges")}</button>
    </div>
  {/if}
</dialog>

<style>
  .lo { width: min(520px, calc(100vw - 32px)); padding: 20px 22px; border: 1px solid var(--line); border-radius: 14px; background: var(--panel); color: var(--ink); box-shadow: 0 24px 60px rgba(0, 0, 0, .4); }
  .lo::backdrop { background: rgba(0, 0, 0, .45); }
  h2 { margin: 0 0 4px; font-size: 16px; overflow-wrap: anywhere; }
  p { margin: 0 0 10px; }
  ul { list-style: none; margin: 8px 0 12px; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  label { display: flex; gap: 10px; align-items: flex-start; padding: 8px 10px; border: 1px solid var(--line); border-radius: 9px; cursor: pointer; }
  label:hover { background: var(--panel2); }
  label input { margin-top: 3px; }
  label > span { display: flex; flex-direction: column; gap: 2px; }
  .acts { display: flex; gap: 8px; align-items: center; margin-top: 6px; }
  .grow { flex: 1; }
</style>

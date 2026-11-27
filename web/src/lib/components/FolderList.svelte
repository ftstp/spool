<script lang="ts">
  // A list of folders, added with the system's folder picker (or a typed path) and removed
  // one by one. The caller does the adding and removing: it may refuse, with a reason.
  import * as api from "../api";
  import { t } from "../i18n/index.svelte";

  let {
    items,
    add,
    remove,
    pick,
    empty,
    note,
    check,
  }: {
    items: { path: string; note?: string }[];
    add: (path: string) => Promise<unknown>;
    remove: (path: string) => Promise<unknown>;
    pick?: string;
    empty?: string;
    note?: (path: string) => string | undefined;
    /** A warning to confirm before adding (a huge folder), or null to add at once. */
    check?: (path: string) => Promise<string | null>;
  } = $props();
  // a folder waiting for the user's yes, with the reason
  let pending = $state<{ path: string; why: string } | null>(null);
  let typed = $state("");
  let typing = $state(false);
  let error = $state("");
  let busy = $state(false);

  async function act(p: Promise<unknown>) {
    busy = true;
    error = "";
    try {
      await p;
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      busy = false;
    }
  }
  async function browse() {
    let path: string | null = null;
    try {
      path = await api.pickFolder(pick ?? t("app.folders.pick"));
    } catch (e) {
      error = String(e);
      return;
    }
    if (path) await consider(path);
  }
  /** Adds at once, or first asks when `check` has something to say. */
  async function consider(path: string) {
    if (check) {
      busy = true;
      error = "";
      const why = await check(path).catch(() => null);
      busy = false;
      if (why) {
        pending = { path, why };
        return false;
      }
    }
    return act(add(path));
  }
  async function confirm() {
    const p = pending;
    pending = null;
    if (p && (await act(add(p.path)))) {
      typed = "";
      typing = false;
    }
  }
  async function addTyped() {
    const p = typed.trim().replace(/^"|"$/g, "");
    if (p && (await consider(p))) {
      typed = "";
      typing = false;
    }
  }
</script>

<div class="fl">
  {#if items.length}
    <ul>
      {#each items as it (it.path)}
        <li>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg>
          <div class="grow">
            <div class="path" title={it.path}>{it.path}</div>
            {#if it.note || note?.(it.path)}<div class="small faint">{it.note ?? note?.(it.path)}</div>{/if}
          </div>
          <button class="btn sm" disabled={busy} onclick={() => act(remove(it.path))}>{t("app.folders.remove")}</button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="small faint none">{empty ?? t("app.folders.empty")}</p>
  {/if}
  <div class="acts">
    <button class="btn" disabled={busy} onclick={browse}>
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
      {t("app.folders.add")}
    </button>
    {#if !typing}<button class="link small" onclick={() => (typing = true)}>{t("app.folders.paste")}</button>{/if}
  </div>
  {#if typing}
    <div class="type">
      <input type="text" bind:value={typed} placeholder={t("app.folders.pathPlaceholder")} onkeydown={(e) => { if (e.key === "Enter") addTyped(); if (e.key === "Escape") typing = false; }} />
      <button class="btn" disabled={busy || !typed.trim()} onclick={addTyped}>{t("app.folders.addTyped")}</button>
    </div>
  {/if}
  {#if pending}
    <div class="ask" role="alertdialog" aria-label={t("app.folders.big")}>
      <div class="path small" title={pending.path}>{pending.path}</div>
      <p class="small">{pending.why}</p>
      <div class="acts"><button class="btn primary" onclick={confirm}>{t("app.folders.addAnyway")}</button><button class="btn" onclick={() => (pending = null)}>{t("common.cancel")}</button></div>
    </div>
  {/if}
  {#if error}<p class="small err" role="alert">{error}</p>{/if}
</div>

<style>
  ul { list-style: none; margin: 0 0 10px; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--line); border-radius: 9px; background: var(--panel2); }
  li svg { color: var(--muted); flex: none; }
  .grow { flex: 1; min-width: 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .none { margin: 0 0 10px; }
  .acts { display: flex; align-items: center; gap: 12px; }
  .acts .btn { display: inline-flex; align-items: center; gap: 6px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .type { display: flex; gap: 8px; margin-top: 8px; }
  .type input { flex: 1; }
  .err { color: var(--bad); margin: 8px 0 0; }
  .ask { margin-top: 10px; padding: 10px 12px; border: 1px solid var(--s-draft); border-radius: 9px; background: var(--panel2); }
  .ask p { margin: 6px 0 10px; }
  .btn:disabled { opacity: .6; cursor: default; }
</style>

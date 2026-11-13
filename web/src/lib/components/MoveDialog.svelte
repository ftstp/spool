<script lang="ts">
  // Moving a folder: pick where (or create a folder there), see where it lands and whether a
  // project would lose a sample, then move. Undo with Ctrl+Z.
  import * as api from "../api";
  import type { MovePlan } from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { src, close }: { src: string; close: () => void } = $props();
  let folders = $state<string[]>([]);
  let q = $state("");
  let dest = $state<string | null>(null);
  let plan = $state<MovePlan | null>(null);
  let err = $state("");
  let busy = $state(false);
  let newName = $state("");
  api.fsFolders().then((x) => (folders = x)).catch((e) => (err = String(e)));
  const leaf = (p: string) => p.split("/").pop() ?? p;
  const list = $derived(
    folders
      .filter((d) => d !== src && !d.startsWith(src + "/") && (!q || d.toLowerCase().includes(q.toLowerCase())))
      .slice(0, 200),
  );
  async function choose(d: string) {
    dest = d;
    plan = null;
    err = "";
    try {
      plan = await api.fsPlan(src, d);
    } catch (e) {
      err = String(e);
    }
  }
  async function mkdir() {
    const base = dest ?? "";
    const rel = (base ? base + "/" : "") + newName.trim();
    if (!newName.trim()) return;
    const made = await app.fileAct(api.fsMkdir(rel));
    if (made) {
      newName = "";
      folders = await api.fsFolders();
      choose(made);
    }
  }
  async function go() {
    if (!dest) return;
    busy = true;
    const r = await app.fileAct(api.fsMove([[src, dest]]));
    busy = false;
    if (r?.skipped.length) err = r.skipped.map((s) => t("dialogs.move.skipped", { name: leaf(s.src), why: s.why })).join(t("dialogs.move.joiner"));
    else if (r) close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />
<div class="scrim" role="presentation" onclick={close}></div>
<div class="dlg card" role="dialog" aria-label={t("dialogs.move.label", { name: leaf(src) })}>
  <h2>{t("dialogs.move.title", { name: leaf(src) })}</h2>
  <p class="small muted">{t("dialogs.move.intro")}</p>
  <input type="search" placeholder={t("dialogs.move.search")} bind:value={q} />
  <div class="list">
    {#each list as d (d)}
      <button class="row" class:on={dest === d} onclick={() => choose(d)}><b>{leaf(d)}</b><span class="faint small">{d}</span></button>
    {:else}<p class="small faint">{t("dialogs.move.none")}</p>{/each}
  </div>
  <div class="mk small">
    <input type="text" placeholder={dest ? t("dialogs.move.newIn", { name: leaf(dest) }) : t("dialogs.move.newRoot")} bind:value={newName} onkeydown={(e) => e.key === "Enter" && mkdir()} />
    <button class="btn sm" disabled={!newName.trim()} onclick={mkdir}>{t("dialogs.move.create")}</button>
  </div>
  {#if plan}
    <p class="small">{t("dialogs.move.arrival")} <b>{plan.dst}</b>{plan.renamed ? t("dialogs.move.renamed") : ""}</p>
    {#if plan.risks.length}
      <p class="small bad">{t("dialogs.move.impossible", { n: plan.risks.length, list: plan.risks.slice(0, 3).map(([p, s]) => t("dialogs.move.sampleIn", { sample: s, project: p })).join(", ") + (plan.risks.length > 3 ? "..." : "") })}</p>
    {/if}
  {/if}
  {#if err}<p class="small bad">{err}</p>{/if}
  <div class="acts">
    <button class="btn" onclick={close}>{t("common.cancel")}</button>
    <button class="btn primary" disabled={!plan || !!plan.risks.length || busy} onclick={go}>{busy ? t("dialogs.move.moving") : t("dialogs.move.go")}</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .4); z-index: 80; }
  .dlg { position: fixed; z-index: 81; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(620px, calc(100vw - 32px)); max-height: calc(100vh - 60px);
    display: flex; flex-direction: column; gap: 10px; box-shadow: 0 30px 80px rgba(0, 0, 0, .45); padding: 18px 20px; }
  h2 { margin: 0; font-size: 16px; }
  p { margin: 0; }
  .list { overflow: auto; min-height: 120px; max-height: 340px; border: 1px solid var(--line); border-radius: 9px; padding: 4px; }
  .row { display: flex; flex-direction: column; align-items: flex-start; width: 100%; border: 0; background: none; padding: 6px 8px; border-radius: 7px; cursor: pointer; color: var(--ink); text-align: left; }
  .row:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .row.on { background: var(--accent-bg); }
  .row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .mk { display: flex; gap: 8px; }
  .mk input { flex: 1; }
  .acts { display: flex; justify-content: flex-end; gap: 8px; }
  .bad { color: var(--bad); }
  .btn:disabled { opacity: .5; cursor: default; }
</style>

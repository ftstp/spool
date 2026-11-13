<script lang="ts">
  // Reorganising the main projects folder by artist: the app proposes, you tick what to move,
  // nothing moves before "Move". One Ctrl+Z puts everything back.
  import * as api from "../api";
  import type { ReorgPlan } from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { close }: { close: () => void } = $props();
  const TEMPLATES = ["artist", "artist_style", "artist_album"] as const;
  let template = $state<string>("artist");
  let plan = $state<ReorgPlan | null>(null);
  let picked = $state<Set<string>>(new Set());
  let err = $state("");
  let busy = $state(false);
  $effect(() => {
    const t = template;
    plan = null;
    api.fsReorg(t).then((p) => {
      if (t !== template) return;
      plan = p;
      picked = new Set(p.moves.map((m) => m.src));
    }).catch((e) => (err = String(e)));
  });
  function toggle(s: string) {
    const n = new Set(picked);
    if (n.has(s)) n.delete(s);
    else n.add(s);
    picked = n;
  }
  async function go() {
    if (!plan) return;
    busy = true;
    const moves = plan.moves.filter((m) => picked.has(m.src)).map((m) => [m.src, m.dest] as [string, string]);
    const r = await app.fileAct(api.fsMove(moves));
    busy = false;
    if (r?.skipped.length) err = t("dialogs.reorg.partial", { done: r.done.length, skipped: r.skipped.length, list: r.skipped.slice(0, 4).map((s) => `${s.src.split("/").pop()} (${s.why})`).join(", ") });
    else if (r) close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />
<div class="scrim" role="presentation" onclick={close}></div>
<div class="dlg card" role="dialog" aria-label={t("dialogs.reorg.title")}>
  <h2>{t("dialogs.reorg.title")}</h2>
  <p class="small muted">{t("dialogs.reorg.intro")}</p>
  <div class="seg small" role="radiogroup" aria-label={t("dialogs.reorg.model")}>
    {#each TEMPLATES as k}<button role="radio" aria-checked={template === k} class:on={template === k} onclick={() => (template = k)}>{t(`dialogs.reorg.templates.${k}`)}</button>{/each}
  </div>
  {#if !plan}<p class="small faint">{t("dialogs.reorg.computing")}</p>
  {:else}
    <p class="small">{t("dialogs.reorg.toSort", { n: plan.moves.length })}{plan.skipped.length ? ` · ${t("dialogs.reorg.left", { n: plan.skipped.length })}` : ""}.</p>
    <div class="list">
      {#each plan.moves as m (m.src)}
        <label class="row">
          <input type="checkbox" checked={picked.has(m.src)} onchange={() => toggle(m.src)} />
          <span class="mv"><span class="ell" title={m.src}>{m.src}</span><span class="to ell" title={m.dst}>→ {m.dst}</span>
            <span class="faint small ell">{m.songs.join(", ")}</span></span>
        </label>
      {:else}<p class="small faint pad">{t("dialogs.reorg.empty")}</p>{/each}
    </div>
  {/if}
  {#if err}<p class="small bad">{err}</p>{/if}
  <div class="acts">
    <button class="btn" onclick={close}>{t("common.cancel")}</button>
    <button class="btn primary" disabled={!plan || !picked.size || busy} onclick={go}>{busy ? t("dialogs.reorg.moving") : t("dialogs.reorg.go", { n: picked.size })}</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .4); z-index: 80; }
  .dlg { position: fixed; z-index: 81; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(760px, calc(100vw - 32px)); max-height: calc(100vh - 60px);
    display: flex; flex-direction: column; gap: 10px; box-shadow: 0 30px 80px rgba(0, 0, 0, .45); padding: 18px 20px; }
  h2 { margin: 0; font-size: 16px; }
  p { margin: 0; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); width: fit-content; flex-wrap: wrap; }
  .seg button { border: 0; background: none; padding: 4px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .list { overflow: auto; max-height: 400px; border: 1px solid var(--line); border-radius: 9px; padding: 4px; }
  .row { display: flex; gap: 10px; align-items: flex-start; padding: 6px 8px; border-radius: 7px; cursor: pointer; }
  .row:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .row input { margin-top: 3px; }
  .mv { display: flex; flex-direction: column; min-width: 0; font-size: 12.5px; }
  .to { color: var(--accent); }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pad { padding: 10px; }
  .acts { display: flex; justify-content: flex-end; gap: 8px; }
  .bad { color: var(--bad); }
  .btn:disabled { opacity: .5; cursor: default; }
</style>

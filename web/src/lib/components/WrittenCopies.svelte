<script lang="ts">
  // Every copy Spool wrote in the user's folders (repairs, treatments passes), so they can be
  // found and tidied by hand. Nothing here deletes or moves: show, open, go to the song.
  import * as api from "../api";
  import type { WrittenCopy } from "../api";
  import { app } from "../state.svelte";
  import { t, fmtDate } from "../i18n/index.svelte";

  let all = $state<WrittenCopy[] | null>(null);
  let err = $state("");
  let show = $state<"all" | "untouched" | "retouched" | "gone">("all");
  let q = $state("");
  let seq = 0;
  $effect(() => {
    void app.version;
    const n = ++seq;
    api.writtenCopies().then((x) => n === seq && (all = x)).catch((e) => (err = String(e)));
  });

  const kept = $derived((all ?? []).filter((c) => c.exists));
  const list = $derived(
    (all ?? []).filter(
      (c) =>
        (show === "all" ? c.exists : show === "untouched" ? c.exists && !c.retouched : show === "retouched" ? c.exists && c.retouched : !c.exists) &&
        (!q || `${c.title ?? ""} ${c.fixed}`.toLowerCase().includes(q.toLowerCase())),
    ),
  );
  const bytes = $derived(kept.reduce((t, c) => t + (c.size ?? 0), 0));
  const mo = (b: number) => (b >= 1e9 ? t("repair.size.gb", { n: (b / 1e9).toFixed(1) }) : t("repair.size.mb", { n: b ? Math.max(1, Math.round(b / 1e6)) : 0 }));
  const name = (rel: string) => rel.split("/").pop() ?? rel;
  const dir = (rel: string) => rel.split("/").slice(0, -1).join("/");
  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "numeric" });
  const fail = (e: unknown) => app.say(String(e), { error: true });
</script>

{#if err}<p class="small bad">{err}</p>
{:else if !all}<p class="muted small">{t("common.loading")}</p>
{:else}
  <div class="bar2">
    <p class="small faint">{t("repair.copies.intro", { files: t("repair.copies.files", { n: kept.length }), size: mo(bytes) })}</p>
    <input type="search" placeholder={t("repair.copies.search")} bind:value={q} />
  </div>
  <div class="seg" role="radiogroup" aria-label={t("repair.copies.whichAria")}>
    {#each [["all", t("repair.copies.all"), kept.length], ["untouched", t("repair.copies.untouched"), kept.filter((c) => !c.retouched).length], ["retouched", t("repair.copies.retouched"), kept.filter((c) => c.retouched).length], ["gone", t("repair.copies.gone"), all.filter((c) => !c.exists).length]] as [k, l, n]}
      <button role="radio" aria-checked={show === k} class:on={show === k} onclick={() => (show = k as typeof show)}>{l} <span class="faint">{n}</span></button>
    {/each}
  </div>
  {#if !list.length}
    <p class="muted small pad">{t("repair.copies.none")}</p>
  {:else}
    <div class="table">
      <div class="tr th small faint"><span>{t("repair.copies.colCopy")}</span><span>{t("repair.copies.colSong")}</span><span>{t("repair.copies.colWritten")}</span><span>{t("repair.copies.colState")}</span><span></span></div>
      {#each list as c (c.fixed)}
        <div class="tr">
          <span class="p"><b class="ell" title={c.fixed}>{name(c.fixed)}</b><span class="small faint ell" title={dir(c.fixed)}>{dir(c.fixed) || t("repair.copies.mainFolder")}</span></span>
          <span class="ell small">{#if c.song}<a href="#/song/{c.song}">{c.title}</a>{:else}<span class="faint">{name(c.source)}</span>{/if}</span>
          <span class="small muted">{day(c.at)}{c.preset && c.preset !== "Réparation" ? ` · ${c.preset}` : ""}</span>
          <span class="small">{#if !c.exists}<span class="faint">{t("repair.copies.goneState")}</span>{:else if c.retouched}<span class="mine">{t("repair.copies.retouchedState")}</span>{:else}<span class="muted">{t("repair.copies.untouchedState")}</span>{/if}</span>
          <span class="acts">
            {#if c.exists}
              <button class="btn sm" onclick={() => api.openInLive(c.fixed).catch(fail)}>{t("repair.copies.openInLive")}</button>
              <button class="btn sm" onclick={() => api.reveal(c.fixed).catch(fail)}>{t("repair.copies.folder")}</button>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .bar2 { display: flex; align-items: flex-start; gap: 16px; margin-bottom: 10px; }
  .bar2 p { margin: 0; flex: 1; max-width: 820px; }
  .bar2 input { width: 240px; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); margin-bottom: 10px; }
  .seg button { border: 0; background: none; padding: 3px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12px; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .table { border: 1px solid var(--line); border-radius: var(--radius); overflow: hidden; background: var(--panel); }
  .tr { display: grid; grid-template-columns: minmax(220px, 2fr) minmax(0, 1fr) 170px 150px auto; gap: 12px; align-items: center; padding: 8px 14px; border-bottom: 1px solid var(--line); }
  .tr:last-child { border-bottom: 0; }
  .tr:hover:not(.th) { background: color-mix(in srgb, var(--line) 35%, transparent); }
  .th { background: var(--panel2); }
  .p { display: flex; flex-direction: column; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .acts { display: flex; gap: 6px; justify-content: flex-end; }
  .mine { color: var(--lost); }
  .pad { padding: 14px; }
  .bad { color: var(--bad); }
  a { color: var(--ink); }
  a:hover { color: var(--accent); }
</style>

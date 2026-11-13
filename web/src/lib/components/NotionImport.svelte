<script lang="ts">
  // Importing a Notion base exported as CSV: each column goes to a song field (guessed from
  // its name, corrected here), the Notion statuses become stages, rows are matched to songs
  // by name. Nothing is written before "Importer"; one Ctrl+Z takes it all back.
  import * as api from "../api";
  import type { NotionColumn, NotionPreview } from "../api";
  import { app } from "../state.svelte";
  import { STAGES, stageLabel } from "../format";
  import { t } from "../i18n/index.svelte";

  let { path, close }: { path: string; close: () => void } = $props();
  let pv = $state<NotionPreview | null>(null);
  let cols = $state<NotionColumn[] | null>(null);
  let stageMap = $state<Record<string, string>>({});
  let onlyEmpty = $state(true);
  let err = $state("");
  let busy = $state(false);
  let showRows = $state<"none" | "ok" | "no">("none");

  // the song fields a column can go to ("": ignored), worded at render time
  const TARGETS = ["", "title", "artist", "style", "key", "tempo", "stage", "notes", "rating", "kinds", "ft"];
  async function load(c?: NotionColumn[]) {
    err = "";
    try {
      const p = await api.notionPreview(path, c ?? null);
      pv = p;
      cols = p.columns;
      const m: Record<string, string> = {};
      for (const [v, s] of p.stages) m[v] = stageMap[v] ?? s;
      stageMap = m;
    } catch (e) {
      err = String(e);
    }
  }
  load();
  function setTarget(i: number, v: string) {
    if (!cols) return;
    const c = cols.map((x, j) => ({ ...x, target: j === i ? v : x.target }));
    load(c);
  }
  async function go() {
    if (!cols) return;
    busy = true;
    try {
      const [n, st] = await api.notionApply(path, cols, stageMap, onlyEmpty);
      app.undo = st;
      app.say(t("app.notion.done", { n }), { undo: true });
      close();
    } catch (e) {
      err = String(e);
    }
    busy = false;
  }
  const file = $derived(path.split(/[\\/]/).pop());
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />
<div class="scrim" role="presentation" onclick={close}></div>
<div class="dlg card" role="dialog" aria-label={t("app.notion.title")}>
  <h2>{t("app.notion.title")} <span class="faint small">{file}</span></h2>
  {#if err}<p class="small bad">{err}</p>{/if}
  {#if !pv || !cols}<p class="small faint">{t("app.notion.reading")}</p>
  {:else}
    <div class="grid">
      <section>
        <h3>{t("app.notion.columns")}</h3>
        <p class="small muted">{t("app.notion.columnsHint")}</p>
        <div class="cols">
          {#each cols as c, i (c.name)}
            <div class="col">
              <div class="cn"><b>{c.name}</b><span class="faint small ell">{c.sample.join(" · ") || t("app.notion.empty")}</span></div>
              <select value={c.target} onchange={(e) => setTarget(i, e.currentTarget.value)} aria-label={t("app.notion.fieldFor", { name: c.name })}>
                {#each TARGETS as k}<option value={k}>{t(`app.notion.target.${k || "ignore"}`)}</option>{/each}
              </select>
            </div>
          {/each}
        </div>
      </section>
      <section>
        {#if pv.stages.length}
          <h3>{t("app.notion.stages")}</h3>
          <p class="small muted">{t("app.notion.stagesHint")}</p>
          <div class="cols">
            {#each pv.stages as [v, , n] (v)}
              <div class="col">
                <div class="cn"><b>{v}</b><span class="faint small">{t("app.notion.rows", { n })}</span></div>
                <select bind:value={stageMap[v]} aria-label={t("app.notion.stageFor", { name: v })}>
                  <option value="">{t("app.notion.keep")}</option>{#each STAGES as s}<option value={s}>{stageLabel(s)}</option>{/each}
                </select>
              </div>
            {/each}
          </div>
        {/if}
        <h3>{t("app.notion.songs")}</h3>
        <p class="small"><b>{pv.matched}</b> {t("app.notion.matched", { n: pv.matched, total: pv.rows.length })}
          {#if pv.rows.length - pv.matched}<span class="muted">{t("app.notion.others", { n: pv.rows.length - pv.matched })}</span>{/if}</p>
        <div class="seg small">
          <button class:on={showRows === "ok"} onclick={() => (showRows = showRows === "ok" ? "none" : "ok")}>{t("app.notion.showMatched")}</button>
          <button class:on={showRows === "no"} onclick={() => (showRows = showRows === "no" ? "none" : "no")}>{t("app.notion.showOthers")}</button>
        </div>
        {#if showRows !== "none"}
          <div class="rows small">
            {#each pv.rows.filter((r) => (showRows === "ok") === (r.song !== null)) as r (r.row)}
              <div class="r"><span class="ell">{r.name || t("app.notion.noName")}</span>{#if r.song_title}<span class="faint ell">→ {r.how === "proche" ? t("app.notion.close", { title: r.song_title }) : r.song_title}</span>{/if}</div>
            {/each}
          </div>
        {/if}
        <label class="small opt"><input type="checkbox" bind:checked={onlyEmpty} /> {t("app.notion.onlyEmpty")}</label>
        <p class="small faint">{t("app.notion.noAudio")}</p>
      </section>
    </div>
  {/if}
  <div class="acts">
    <button class="btn" onclick={close}>{t("common.cancel")}</button>
    <button class="btn primary" disabled={!pv || !pv.matched || busy} onclick={go}>{busy ? t("app.notion.importing") : t("app.notion.import", { n: pv?.matched ?? 0 })}</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .45); z-index: 80; }
  .dlg { position: fixed; z-index: 81; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(1000px, calc(100vw - 32px)); max-height: calc(100vh - 40px);
    display: flex; flex-direction: column; gap: 10px; box-shadow: 0 30px 80px rgba(0, 0, 0, .45); padding: 18px 20px; overflow: auto; }
  h2 { margin: 0; font-size: 16px; }
  h3 { margin: 4px 0 4px; font-size: 13px; }
  p { margin: 0 0 6px; }
  .grid { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); gap: 20px; }
  @media (max-width: 800px) { .grid { grid-template-columns: 1fr; } }
  .cols { display: flex; flex-direction: column; gap: 4px; margin-bottom: 12px; }
  .col { display: grid; grid-template-columns: minmax(0, 1fr) 170px; gap: 10px; align-items: center; padding: 5px 8px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel2); }
  .cn { display: flex; flex-direction: column; min-width: 0; }
  .col select { font-size: 12.5px; padding: 3px 6px; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); margin: 4px 0 8px; }
  .seg button { border: 0; background: none; padding: 3px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .rows { max-height: 200px; overflow: auto; border: 1px solid var(--line); border-radius: 8px; padding: 4px 8px; margin-bottom: 10px; }
  .r { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 8px; padding: 2px 0; }
  .opt { display: flex; gap: 8px; align-items: center; margin: 6px 0; cursor: pointer; }
  .acts { display: flex; justify-content: flex-end; gap: 8px; }
  .bad { color: var(--bad); }
  .btn:disabled { opacity: .5; cursor: default; }
</style>

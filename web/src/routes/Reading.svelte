<script lang="ts">
  // How Spool reads the library's folders, said back with the library's own examples, and the
  // rules that can be changed: where a title comes from, which folders are one song. A change
  // is shown before it is made (how many songs, which folders, before and after), then made
  // in one Ctrl+Z.
  import * as api from "../lib/api";
  import type { ReadingReport } from "../lib/api/bindings/ReadingReport";
  import type { GroupPreview } from "../lib/api/bindings/GroupPreview";
  import { app } from "../lib/state.svelte";
  import { t, fmtNum } from "../lib/i18n/index.svelte";
  import Skeleton from "../lib/components/Skeleton.svelte";

  let r = $state<ReadingReport | null>(null);
  let error = $state("");
  // the rules being chosen (from those in force)
  let all = $state(false);
  let dirs = $state<string[]>([]);
  let titleFrom = $state("auto");
  $effect(() => {
    void app.version;
    api.readingReport()
      .then((x) => {
        r = x;
        all = x.group_mode === "folder";
        dirs = [...x.one_song_dirs];
        titleFrom = x.title_from;
      })
      .catch((e) => (error = String(e)));
  });
  const changed = $derived(!!r && (all !== (r.group_mode === "folder") || titleFrom !== r.title_from || [...dirs].sort().join("\n") !== [...r.one_song_dirs].sort().join("\n")));
  let preview = $state<GroupPreview | null>(null);
  let previewing = $state(false);
  let seq = 0;
  $effect(() => {
    const go = changed;
    const [a, d, tf] = [all, [...dirs], titleFrom];
    preview = null;
    if (!go) return;
    const n = ++seq;
    previewing = true;
    api.groupPreview(a, d, tf)
      .then((p) => n === seq && (preview = p))
      .catch((e) => n === seq && app.say(String(e), { error: true }))
      .finally(() => n === seq && (previewing = false));
  });
  // the songs are grouped again (a few seconds on a large catalogue): said on the button and
  // by the bar at the top of the window, the button kept from a second click
  let applying = $state(false);
  async function apply() {
    if (applying) return;
    applying = true;
    try {
      const st = await app.track(api.setGrouping(all, dirs, titleFrom));
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
      await app.track(app.reload());
    } catch (e) {
      app.say(String(e), { error: true });
    }
    applying = false;
  }
  function reset() {
    if (!r) return;
    all = r.group_mode === "folder";
    dirs = [...r.one_song_dirs];
    titleFrom = r.title_from;
  }
  const toggleDir = (d: string) => (dirs = dirs.includes(d) ? dirs.filter((x) => x !== d) : [...dirs, d]);
  const leaf = (p: string) => p.split("/").pop() || p;

</script>

<div class="page">
  <a class="small back" href="#/settings">← {t("app.nav.settings")}</a>
  <h1>{t("reading.title")}</h1>
  <p class="muted lede">{t("reading.lede")}</p>
  {#if error}<p class="bad">{error}</p>
  {:else if !r}<Skeleton kind="page" />
  {:else}
    <section class="card">
      <h2>{t("reading.understood", { files: fmtNum(r.files), songs: fmtNum(r.songs) })}</h2>
      <ul class="facts">
        {#if r.versions_n}
          <li><b>{t("reading.versions", { n: fmtNum(r.versions_n) })}</b>
            <div class="ex">{#each r.versions as e (e.rel)}<span><code>{leaf(e.rel)}</code> → {e.title}</span>{/each}</div></li>
        {/if}
        {#if r.version_dirs_n}
          <li><b>{t("reading.versionDirs", { n: fmtNum(r.version_dirs_n) })}</b>
            <div class="ex">{#each r.version_dirs as e (e.rel)}<span title={e.rel}><code>{e.rel.split("/").slice(-2).join("/")}</code> → {e.title}</span>{/each}</div></li>
        {/if}
        {#if r.project_dirs_n}<li><b>{t("reading.projectDirs", { n: fmtNum(r.project_dirs_n) })}</b></li>{/if}
        {#if r.from_folder_n}
          <li><b>{t("reading.fromFolder", { n: fmtNum(r.from_folder_n) })}</b>
            <div class="ex">{#each r.from_folder as e (e.rel)}<span title={e.rel}><code>{leaf(e.rel)}/</code> → {e.title}</span>{/each}</div></li>
        {/if}
        {#if r.numbered_n}<li><b>{t("reading.numbered", { n: fmtNum(r.numbered_n) })}</b> <span class="faint">{r.numbered.join(", ")}</span></li>{/if}
        {#if r.work_words.length}<li><b>{t("reading.words")}</b> <span class="faint">{r.work_words.join(", ")}</span> <a class="small" href="#/settings">{t("reading.edit")}</a></li>{/if}
      </ul>
    </section>

    <section class="card">
      <h2>{t("reading.rules")}</h2>
      <div class="rule">
        <div><b>{t("reading.titleRule.label")}</b><p class="small muted">{t("reading.titleRule.hint")}</p></div>
        <div class="seg" role="radiogroup" aria-label={t("reading.titleRule.label")}>
          {#each ["auto", "file", "folder"] as v (v)}<button role="radio" aria-checked={titleFrom === v} class:on={titleFrom === v} onclick={() => (titleFrom = v)}>{t(`reading.titleRule.${v}`)}</button>{/each}
        </div>
      </div>
      <div class="rule">
        <div><b>{t("reading.folder.label")}</b><p class="small muted">{t("reading.folder.hint")}</p></div>
        <div class="seg" role="radiogroup" aria-label={t("reading.folder.label")}>
          <button role="radio" aria-checked={!all} class:on={!all} onclick={() => (all = false)}>{t("reading.folder.names")}</button>
          <button role="radio" aria-checked={all} class:on={all} onclick={() => (all = true)}>{t("reading.folder.all")}</button>
        </div>
      </div>
      {#if !all}
        <div class="multi">
          <p class="small muted">{r.multi_n ? t("reading.multi", { n: fmtNum(r.multi_n) }) : t("reading.multiNone")}</p>
          {#each [...new Set([...dirs, ...r.multi.map((m) => m.dir)])] as d (d)}
            {@const m = r.multi.find((x) => x.dir === d)}
            <label class="fold">
              <input type="checkbox" checked={dirs.includes(d)} onchange={() => toggleDir(d)} />
              <span class="grow"><b>{leaf(d)}</b> <span class="faint small" title={d}>{d}</span>
                {#if m}<span class="small muted titles">{m.titles.join(" · ")}</span>{/if}</span>
              {#if dirs.includes(d)}<span class="small tag">{t("reading.oneSong")}</span>{/if}
            </label>
          {/each}
        </div>
      {/if}
      {#if changed}
        <div class="preview">
          {#if previewing || !preview}<p class="small muted">{t("reading.previewing")}</p>
          {:else}
            <p><b>{t("reading.preview", { now: fmtNum(preview.now), after: fmtNum(preview.after) })}</b> <span class="small faint">{t("reading.previewFolders", { n: preview.changed })}</span></p>
            <ul class="changes">
              {#each preview.changes as c (c.dir)}
                <li><span class="d" title={c.dir}>{leaf(c.dir) || "/"}</span>
                  <span class="b">{c.before.join(" · ") || "-"}</span><span class="arrow">→</span><span class="a">{c.after.join(" · ") || "-"}</span></li>
              {/each}
            </ul>
            <div class="row"><button class="btn primary" disabled={applying} onclick={apply}>{#if applying}<span class="spin" aria-hidden="true"></span>{t("reading.applying")}{:else}{t("reading.apply")}{/if}</button><button class="btn" disabled={applying} onclick={reset}>{t("common.cancel")}</button><span class="small faint">{t("reading.undoable")}</span></div>
          {/if}
        </div>
      {/if}
    </section>

  {/if}
</div>

<style>
  .page { padding: 20px 24px 40px; max-width: 980px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; }
  .page > :global(*) { min-width: 0; }
  .ex span { overflow-wrap: anywhere; }
  .back { color: var(--muted); }
  h1 { margin: 0; }
  .lede { margin: 0; max-width: 70ch; }
  h2 { margin: 0 0 10px; font-size: 15px; }
  .facts { margin: 0; padding-left: 18px; display: grid; gap: 8px; }
  .ex { display: flex; flex-wrap: wrap; gap: 4px 16px; font-size: 12.5px; color: var(--muted); margin-top: 2px; }
  code { font-size: 12px; background: var(--panel2); padding: 0 4px; border-radius: 4px; }
  .rule { display: flex; align-items: center; gap: 16px; justify-content: space-between; padding: 10px 0; border-top: 1px solid var(--line); flex-wrap: wrap; }
  .rule > div:first-child { flex: 1 1 280px; min-width: 0; }
  .rule p { margin: 2px 0 0; max-width: 60ch; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; flex: none; }
  .seg button { border: 0; background: none; color: var(--muted); padding: 5px 12px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .seg button + button { border-left: 1px solid var(--line); }
  .seg button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .multi { display: grid; gap: 2px; max-height: 360px; overflow-y: auto; border-top: 1px solid var(--line); padding-top: 8px; }
  .fold { display: flex; align-items: flex-start; gap: 10px; padding: 5px 6px; border-radius: 7px; cursor: pointer; }
  .fold:hover { background: var(--accent-bg); }
  .fold input { margin-top: 3px; }
  .grow { flex: 1; min-width: 0; }
  .titles { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { border: 1px solid var(--accent); color: var(--accent); border-radius: 4px; padding: 0 5px; flex: none; }
  .preview { margin-top: 12px; padding: 12px 14px; border-radius: 10px; background: var(--panel2); }
  .preview p { margin: 0 0 8px; }
  .changes { list-style: none; margin: 0 0 10px; padding: 0; display: grid; gap: 4px; font-size: 12.5px; max-height: 280px; overflow-y: auto; }
  .changes li { display: grid; grid-template-columns: 140px minmax(0, 1fr) 16px minmax(0, 1fr); gap: 8px; align-items: baseline; }
  .changes .d { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .changes .b { color: var(--faint); text-decoration: line-through; }
  .changes .arrow { color: var(--faint); }
  .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
</style>

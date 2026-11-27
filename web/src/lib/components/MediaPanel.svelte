<script lang="ts">
  // Loose media: pictures, videos, sounds and notes in folders with no project, outside every
  // song folder. Pictures first (what mood boards are mostly made of), sortable, shown a page
  // at a time as small thumbnails (made once by the app, the files only read). An item, or a
  // whole folder, goes on a song's mood board.
  import * as api from "../api";
  import type { MediaFolder } from "../api";
  import { ui } from "../contextMenus.svelte";
  import { t, fmtDate, fmtNum, compare } from "../i18n/index.svelte";

  let { close, wide = $bindable(false) }: { close: () => void; wide?: boolean } = $props();
  let folders = $state<MediaFolder[] | null>(null);
  let err = $state("");
  // read when the panel opens, and again on demand (a walk of the whole disk: not after
  // every change in the app)
  function load() {
    folders = null;
    err = "";
    api.fsMedia().then((x) => (folders = x)).catch((e) => (err = String(e)));
  }
  load();

  const KINDS = ["image", "video", "audio", "text", ""];
  const KIND_KEYS = ["image", "gif", "video", "audio", "text", "pdf"];
  const kindLabel = (k: string) => (KIND_KEYS.includes(k) ? t(`song.media.kind.${k}`) : k);
  let kind = $state("image");
  let sort = $state("recent");
  let q = $state("");
  const PAGE = 80;
  let shown = $state(PAGE);

  type It = MediaFolder["items"][number] & { dir: string };
  const all = $derived((folders ?? []).flatMap((m) => m.items.map((i) => ({ ...i, dir: m.dir }) as It)));
  const countOf = (k: string) => all.filter((i) => !k || (k === "image" ? i.kind === "image" || i.kind === "gif" : i.kind === k)).length;
  const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const list = $derived.by(() => {
    const n = norm(q.trim());
    const l = all.filter(
      (i) => (!kind || (kind === "image" ? i.kind === "image" || i.kind === "gif" : i.kind === kind)) && (!n || norm(i.name).includes(n) || norm(i.dir).includes(n)),
    );
    const by: Record<string, (a: It, b: It) => number> = {
      recent: (a, b) => b.mtime - a.mtime,
      old: (a, b) => a.mtime - b.mtime,
      big: (a, b) => b.size - a.size,
      name: (a, b) => compare(a.name, b.name),
      folder: (a, b) => compare(a.dir, b.dir) || compare(a.name, b.name),
    };
    return l.sort(by[sort]);
  });
  // a new filter starts from the first page again
  $effect(() => {
    void kind;
    void sort;
    void q;
    shown = PAGE;
  });
  const send = (items: string[]) => (ui.toBoard = items);
  const leaf = (d: string) => d.split("/").pop() || t("song.media.rootFolder");
  const mo = (b: number) =>
    b >= 1e6
      ? t("song.media.mb", { n: fmtNum(b / 1e6, { minimumFractionDigits: 1, maximumFractionDigits: 1 }) })
      : t("song.media.kb", { n: fmtNum(Math.max(1, Math.round(b / 1e3))) });
  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "numeric" });
</script>

<aside class="media" class:wide>
  <div class="mh">
    <h2>{t("song.media.title")}</h2>
    <div class="hb">
      <button class="x" onclick={load} title={t("song.media.reload")}>↻</button>
      <button class="x" onclick={() => (wide = !wide)} title={wide ? t("song.media.shrink") : t("song.media.widen")}>{wide ? "⇥" : "⇤"}</button>
      <button class="x" onclick={close} aria-label={t("common.close")}>✕</button>
    </div>
  </div>
  <p class="small muted">{t("song.media.intro")}</p>
  {#if err}<p class="small bad">{err}</p>{/if}
  {#if !folders}<p class="small faint">{t("song.media.searching")}</p>
  {:else}
    <div class="tools">
      <div class="seg">
        {#each KINDS as k}<button class:on={kind === k} onclick={() => (kind = k)}>{t(`song.media.kinds.${k || "all"}`)} <span class="faint">{countOf(k)}</span></button>{/each}
      </div>
      <div class="row">
        <input type="search" placeholder={t("song.media.search")} bind:value={q} />
        <select bind:value={sort} aria-label={t("song.media.sort")}>
          <option value="recent">{t("song.media.sortRecent")}</option><option value="old">{t("song.media.sortOld")}</option><option value="big">{t("song.media.sortBig")}</option>
          <option value="name">{t("song.media.sortName")}</option><option value="folder">{t("song.media.sortFolder")}</option>
        </select>
      </div>
    </div>
    {#if !list.length}<p class="small faint">{t("song.media.nothing")}</p>{/if}
    <div class="items">
      {#each list.slice(0, shown) as it, i (it.rel)}
        {#if sort === "folder" && (i === 0 || list[i - 1].dir !== it.dir)}
          <div class="fh" data-folder={it.dir}><b class="ell" title={it.dir}>{leaf(it.dir)}</b><button class="link small" onclick={() => send([it.dir])}>{t("song.media.wholeFolder")}</button></div>
        {/if}
        <button class="it" title={t("song.media.itemTip", { name: it.name, dir: it.dir, size: mo(it.size), date: day(it.mtime) })} data-media={it.rel} data-kind={it.kind} onclick={() => send([it.rel])}>
          {#if it.kind === "image" || it.kind === "gif"}<img src={api.thumbUrl(it.rel, 240)} alt={it.name} loading="lazy" decoding="async" />{:else}<span class="k">{kindLabel(it.kind)}</span>{/if}
          <span class="n ell">{it.name}</span>
        </button>
      {/each}
    </div>
    {#if list.length > shown}
      <button class="btn sm more" onclick={() => (shown += PAGE)}>{t("song.media.showMore", { k: Math.min(PAGE, list.length - shown), n: list.length - shown })}</button>
    {/if}
  {/if}
</aside>

<style>
  .media { border-left: 1px solid var(--line); padding: 14px 16px; overflow: auto; min-height: 0; display: flex; flex-direction: column; gap: 10px; background: var(--bg); }
  .mh { display: flex; justify-content: space-between; align-items: center; }
  .hb { display: flex; gap: 2px; }
  h2 { margin: 0; font-size: 15px; }
  p { margin: 0; }
  .x { border: 0; background: none; color: var(--muted); cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .x:hover { background: var(--panel); color: var(--ink); }
  .tools { display: flex; flex-direction: column; gap: 6px; position: sticky; top: -14px; background: var(--bg); padding: 4px 0 6px; z-index: 2; }
  .seg { display: flex; flex-wrap: wrap; gap: 2px; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .seg button { border: 0; background: none; padding: 3px 8px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12px; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; box-shadow: 0 1px 2px rgba(0, 0, 0, .15); }
  .row { display: flex; gap: 6px; }
  .row input { flex: 1; min-width: 0; }
  .items { display: grid; grid-template-columns: repeat(auto-fill, minmax(96px, 1fr)); gap: 6px; }
  .wide .items { grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); }
  .fh { grid-column: 1 / -1; display: flex; justify-content: space-between; align-items: baseline; gap: 8px; margin-top: 8px; font-size: 12.5px; }
  .it { border: 1px solid var(--line); border-radius: 8px; background: var(--panel2); padding: 0; cursor: pointer; overflow: hidden; display: flex; flex-direction: column; color: var(--ink); }
  .it:hover { border-color: var(--accent); }
  .it img { width: 100%; aspect-ratio: 1; object-fit: cover; display: block; background: var(--panel2); }
  .k { aspect-ratio: 1; display: grid; place-items: center; font-size: 11px; color: var(--muted); }
  .n { font-size: 10.5px; padding: 3px 5px; text-align: left; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .more { align-self: center; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; text-decoration: underline; }
  .bad { color: var(--bad); }
</style>

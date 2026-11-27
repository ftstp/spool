<script lang="ts" module>
  /** What a board does with its files: a song's (in its _moodboard folder) by default, an
   *  artist's (in Spool's own data folder) on the Artists page. */
  export type BoardOps = {
    note: (text: string, lyrics: boolean, name?: string) => Promise<unknown>;
    add: (f: File) => Promise<unknown>;
    remove: (name: string) => Promise<unknown>;
    meta: (patch: { palette?: string[] }) => Promise<unknown>;
    unlink?: (rel: string) => Promise<unknown>;
    /** Files dropped from Explorer, by their paths (a song's sheet takes them itself). */
    drop?: (paths: string[]) => Promise<import("../api/bindings/BoardDrop").BoardDrop>;
  };
</script>

<script lang="ts">
  import FavStar from "./FavStar.svelte";
  import { sectionOpen } from "../tabs.svelte";
  import FoldHead from "./FoldHead.svelte";
  // A mood board: pictures (drop files or pick them), notes and lyrics written in place, a
  // colour palette. A song's files live in its _moodboard folder; a removed one goes to
  // _moodboard/.corbeille, never deleted. An artist's live in Spool's data folder.
  import * as api from "../api";
  import type { Item } from "../api";
  import type { Board } from "../api/bindings/Board";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";
  import BoardFiles from "./BoardFiles.svelte";
  import { dropZone, dropMessage } from "../fileDrop";

  // `pick`: a picture chosen for something (an artist's photo), `picked` its name
  // `title`: the song's (written with each font found)
  // `children`: the song's notes (post-its), under the pictures
  let { sid = 0, board, ops = null, pick, picked = null, pickLabel = "", title = "", children }: { sid?: number; board: Board; ops?: BoardOps | null; pick?: (i: Item) => void; picked?: string | null; pickLabel?: string; title?: string; children?: import("svelte").Snippet } = $props();
  const o = $derived<BoardOps>(
    ops ?? {
      note: (text, lyrics, name) => api.boardNote(sid, text, lyrics, name),
      add: (f) => api.boardAdd(sid, f),
      remove: (name) => api.boardRemove(sid, name),
      meta: (patch) => api.boardMeta(sid, patch),
      unlink: (rel) => api.boardUnlink(sid, rel),
    },
  );
  const pictures = $derived([...board.items, ...board.found].filter((i) => i.kind === "image" || i.kind === "gif"));
  const notes = $derived(board.items.filter((i) => i.kind === "text"));
  // sounds and MIDI files: rows to listen to and drag into Live; the rest named
  const sounds = $derived(board.items.filter((i) => i.kind === "audio" || i.kind === "midi"));
  const others = $derived(board.items.filter((i) => !["image", "gif", "text", "audio", "midi", "font", "preset"].includes(i.kind)));
  // what lies in the song's folder beside the project (not Live's own folders): notes,
  // documents, videos, shown where they are (opened from Explorer, never moved)
  const loose = $derived(board.found.filter((i) => ["text", "doc", "video"].includes(i.kind)));
  // the fonts and presets of the board, and those kept in the song's folder (found by Spool)
  const fonts = $derived([...board.items, ...board.found].filter((i) => i.kind === "font"));
  const presets = $derived([...board.items, ...board.found].filter((i) => i.kind === "preset"));
  // each font loaded under a name of its own, to write the song's title with it
  let loaded = $state<Record<string, string>>({});
  $effect(() => {
    for (const f of fonts) {
      if (loaded[f.rel] !== undefined) continue;
      const fam = "board-font-" + Math.abs([...f.rel].reduce((a, c) => (a * 31 + c.charCodeAt(0)) | 0, 7));
      loaded[f.rel] = "";
      new FontFace(fam, `url("${api.mediaUrl(f.rel)}")`)
        .load()
        .then((ff) => {
          document.fonts.add(ff);
          loaded[f.rel] = fam;
        })
        .catch(() => {});
    }
  });
  const dragPreset = (i: Item) => api.dragOut(i.path).catch((e) => app.say(String(e), { error: true }));
  const hasLyrics = $derived(notes.some((n) => /^(paroles|lyrics|texte)/i.test(n.name)));

  let editing = $state<string | null>(null); // a note's name, or "+idea" / "+lyrics" for a new one
  let draft = $state("");
  let over = $state(false);
  let big = $state<Item | null>(null);
  let input = $state<HTMLInputElement>();

  const run = async (p: Promise<unknown>, ok?: string) => {
    try {
      await p;
      if (ok) app.say(ok);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  };
  function edit(n: Item | "+idea" | "+lyrics") {
    editing = typeof n === "string" ? n : n.name;
    draft = typeof n === "string" ? "" : (n.text ?? "");
  }
  async function save() {
    const name = editing;
    editing = null;
    if (!name) return;
    const old = notes.find((n) => n.name === name)?.text ?? "";
    if (draft.trim() === old.trim() || (name.startsWith("+") && !draft.trim())) return;
    // the version replaced is kept aside, and the toast offers Ctrl+Z
    await app.fileAct(o.note(draft, name === "+lyrics", name.startsWith("+") ? undefined : name));
  }
  async function add(files: FileList | File[] | null | undefined) {
    for (const f of files ?? []) await run(o.add(f), t("song.board.added", { name: f.name }));
  }
  const setAside = (i: Item) =>
    i.where === "lien" && o.unlink
      ? app.act(o.unlink(i.rel) as Promise<import("../api").UndoState>)
      : run(o.remove(i.name), app.physical || ops ? t("song.board.trashed", { name: i.name }) : t("song.board.unlinked", { name: i.name }));
  const palette = (p: string[]) => run(o.meta({ palette: p }));
  /** `autofocus` does nothing on an element created later: focus it once it is there. */
  const focus = (n: HTMLElement) => {
    n.focus();
  };
  const label = (n: Item) => n.name.replace(/\.(txt|md)$/i, "");
  // a song's board is one of the sheet's sections (open or not as the user left them)
  const foldable = $derived(!!sid && !ops);
  const shown = $derived(!foldable || sectionOpen(sid, "board"));
  // files dropped from Explorer on a board that takes them by path (an artist's)
  let card = $state<HTMLElement>();
  $effect(() => {
    const d = o.drop;
    if (!d) return;
    return dropZone(() => card, (on) => (over = on), async (paths) => {
      try {
        const m = dropMessage(await d(paths));
        app.say(m.text, { error: m.error });
      } catch (e) {
        app.say(String(e), { error: true });
      }
    });
  });
</script>

{#if shown}
<div class="card" bind:this={card} data-sec={foldable ? "board" : null} class:over role="group" aria-label={t("song.board.label")}
  ondragover={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); over = true; } }}
  ondragleave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) over = false; }}
  ondrop={(e) => { e.preventDefault(); over = false; add(e.dataTransfer?.files); }}>
  {#snippet boardActs()}
    <div class="acts">
      <button class="link small" onclick={() => input?.click()}>{t("song.board.addImage")}</button>
      <button class="link small" onclick={() => edit("+idea")}>{t("song.board.addIdea")}</button>
      {#if !hasLyrics}<button class="link small" onclick={() => edit("+lyrics")}>{t("song.board.addLyrics")}</button>{/if}
    </div>
  {/snippet}
  {#if foldable}
    <FoldHead {sid} key="board" title={t("song.board.title")} actions={boardActs} />
  {:else}
    <div class="head"><h2>{t("song.board.title")}</h2>{@render boardActs()}</div>
  {/if}
{#if shown}
  <input bind:this={input} type="file" accept="image/*,video/*,audio/*" multiple hidden onchange={(e) => { add(e.currentTarget.files); e.currentTarget.value = ""; }} />

  {#if pictures.length}
    <div class="pics">
      {#each pictures.slice(0, 12) as p (p.rel)}
        <div class="pic" data-media={p.rel} data-kind={p.kind} title={p.where === "dossier" ? t("song.board.inFolder", { name: p.name }) : p.name}>
          {#if api.mediaUrl(p.rel)}<button class="open" onclick={() => (big = p)} aria-label={t("song.board.enlarge", { name: p.name })}><img src={p.kind === "gif" ? api.mediaUrl(p.rel) : api.thumbUrl(p.rel, 512)} alt={p.name} loading="lazy" decoding="async"
            onerror={(e) => { const i = e.currentTarget as HTMLImageElement; if (!i.dataset.full) { i.dataset.full = "1"; i.src = api.mediaUrl(p.rel); } }} /></button>
          {:else}<span class="small faint">{p.name}</span>{/if}
          {#if pick && p.where === "board"}<button class="star" class:on={picked === p.name} title={pickLabel} aria-label={pickLabel} aria-pressed={picked === p.name} onclick={() => pick(p)}>★</button>{/if}
          <span class="fv" class:on={false}><FavStar kind="board" id={p.rel} label={p.name} sub={title} song={sid || null} size={14} /></span>
          {#if p.where === "board"}<button class="rm" title={t("song.board.setAside")} aria-label={t("song.board.setAsideName", { name: p.name })} onclick={() => setAside(p)}>×</button>{/if}
        </div>
      {/each}
    </div>
    {#if pictures.length > 12}<p class="small faint">{t("song.board.moreImages", { n: pictures.length - 12 })}</p>{/if}
  {/if}

  {@render children?.()}

  {#if editing?.startsWith("+")}
    <div class="note">
      <b>{editing === "+lyrics" ? t("song.board.lyrics") : t("song.board.newIdea")}</b>
      <textarea rows="5" bind:value={draft} use:focus placeholder={editing === "+lyrics" ? t("song.board.lyricsPh") : t("song.board.ideaPh")}
        onblur={save} onkeydown={(e) => { if (e.key === "Escape") editing = null; }}></textarea>
    </div>
  {/if}
  {#each notes as n (n.rel)}
    <div class="note">
      <div class="nh"><b>{label(n)}</b><button class="rm2" title={t("song.board.setAside")} aria-label={t("song.board.setAsideName", { name: label(n) })} onclick={() => setAside(n)}>×</button></div>
      {#if editing === n.name}
          <textarea rows="6" bind:value={draft} use:focus onblur={save} onkeydown={(e) => { if (e.key === "Escape") editing = null; }}></textarea>
      {:else}
        <button class="txt" onclick={() => edit(n)} title={t("song.board.edit")}>{n.text || t("song.board.empty")}</button>
      {/if}
    </div>
  {/each}
  {#if sounds.length}
    <BoardFiles files={sounds.map((i) => ({ path: i.rel, name: i.name }))} remove={(p) => { const i = sounds.find((x) => x.rel === p); if (i) setAside(i); }} />
  {/if}
  {#if fonts.length}
    <div class="fonts">
      {#each fonts as f (f.rel)}
        <div class="font" title={f.where === "dossier" ? t("song.board.inFolder", { name: f.name }) : f.name}>
          <span class="sample" style:font-family={loaded[f.rel] ? `"${loaded[f.rel]}"` : undefined}>{title || f.name.replace(/\.[^.]+$/, "")}</span>
          <span class="small faint">{f.name}</span>
        </div>
      {/each}
    </div>
  {/if}
  {#if presets.length}
    <div class="presets">
      <span class="small muted">{t("song.board.presets")}</span>
      {#each presets as pr (pr.rel)}
        <button class="preset" draggable="true" title={t("song.board.presetTip", { name: pr.name })}
          ondragstart={(e) => { e.preventDefault(); dragPreset(pr); }}>{pr.name.replace(/\.[^.]+$/, "")}</button>
      {/each}
    </div>
  {/if}
  {#if others.length}
    <p class="small faint">{t("song.board.also", { list: others.map((o) => o.name).join(", ") })}</p>
  {/if}
  {#if loose.length}
    <div class="loose">
      <span class="small muted">{t("song.board.inFolder")}</span>
      {#each loose as f (f.rel)}<button class="lf small" title={t("song.board.inFolderTip", { path: f.rel })} onclick={() => api.reveal(f.rel).catch((e) => app.say(String(e), { error: true }))}>{f.name}</button>{/each}
    </div>
  {/if}

  <div class="pal">
    {#each board.palette as c, i}
      <span class="sw" style:background={c} title={c}><button aria-label={t("song.board.removeColor", { c })} onclick={() => palette(board.palette.filter((_, j) => j !== i))}>×</button></span>
    {/each}
    <label class="addc" title={t("song.board.addColor")}><input type="color" onchange={(e) => palette([...board.palette, e.currentTarget.value])} />+</label>
  </div>
  {#if !pictures.length && !notes.length && !editing && !children}<p class="small faint">{t("song.board.hint")}</p>{/if}
{/if}
</div>
{/if}

{#if big}
  <div class="light" role="presentation" onclick={() => (big = null)} onkeydown={(e) => { if (e.key === "Escape") big = null; }}>
    <img src={api.mediaUrl(big.rel)} alt={big.name} data-media={big.rel} data-kind="image" />
    <div class="cap small">{big.name}{big.w ? ` · ${big.w}×${big.h}` : ""}</div>
  </div>
{/if}

<style>
  .card { transition: border-color .15s, background-color .15s; }
  .card.over { border-color: var(--accent); background: var(--accent-bg); }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; }
  .acts { display: flex; gap: 12px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .fonts { display: flex; flex-direction: column; gap: 6px; margin-top: 10px; }
  .font { display: flex; flex-direction: column; padding: 8px 10px; border-radius: 8px; background: var(--panel2); }
  .font .sample { font-size: 22px; line-height: 1.2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .presets { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin-top: 10px; }
  .preset { border: 1px solid var(--line); background: var(--panel); color: inherit; font: inherit; font-size: 12px; padding: 2px 9px; border-radius: 6px; cursor: grab; }
  .preset:hover { border-color: var(--accent); }
  .pics { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .pic { position: relative; aspect-ratio: 1; border-radius: 8px; overflow: hidden; background: var(--panel2); display: grid; place-items: center; text-align: center; padding: 2px; }
  .open { border: 0; padding: 0; width: 100%; height: 100%; cursor: zoom-in; background: none; }
  .open img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .rm, .rm2 { border: 0; cursor: pointer; display: grid; place-items: center; }
  .star { position: absolute; top: 4px; left: 4px; width: 22px; height: 22px; border-radius: 6px; border: 0; background: rgba(0, 0, 0, .6); color: #fff; opacity: 0; transition: opacity .15s; cursor: pointer; font-size: 12px; }
  .star.on { opacity: 1; color: #f5c84b; }
  .pic:hover .star, .star:focus-visible { opacity: 1; }
  .rm { position: absolute; top: 4px; right: 4px; width: 22px; height: 22px; border-radius: 6px; background: rgba(0, 0, 0, .6); color: #fff; opacity: 0; transition: opacity .15s; }
  .pic:hover .rm, .rm:focus-visible { opacity: 1; }
  .note { margin-top: 8px; padding: 8px 10px; border-radius: 8px; background: var(--panel2); }
  .nh { display: flex; justify-content: space-between; align-items: center; }
  .rm2 { background: none; color: var(--faint); font-size: 16px; opacity: 0; }
  .note:hover .rm2, .rm2:focus-visible { opacity: 1; }
  .rm2:hover { color: var(--bad); }
  .txt { display: block; width: 100%; text-align: left; border: 0; background: none; padding: 2px 0 0; cursor: text; color: var(--muted); white-space: pre-wrap; font-size: 12.5px; max-height: 180px; overflow: hidden; }
  textarea { width: 100%; resize: vertical; margin-top: 4px; background: var(--panel); border: 1px solid var(--accent); border-radius: 6px; padding: 6px 8px; color: inherit; font: inherit; font-size: 12.5px; outline: 0; }
  .pal { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 10px; align-items: center; }
  .sw { width: 24px; height: 24px; border-radius: 6px; position: relative; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .08); }
  .sw button { position: absolute; inset: 0; border: 0; background: rgba(0, 0, 0, .5); color: #fff; border-radius: 6px; cursor: pointer; opacity: 0; transition: opacity .15s; }
  .sw:hover button, .sw button:focus-visible { opacity: 1; }
  .addc { width: 24px; height: 24px; border-radius: 6px; border: 1px dashed var(--faint); color: var(--muted); display: grid; place-items: center; cursor: pointer; position: relative; font-size: 14px; }
  .addc input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
  .light { position: fixed; inset: 0; z-index: 60; background: rgba(0, 0, 0, .82); display: grid; place-items: center; align-content: center; gap: 10px; cursor: zoom-out; padding: 30px; }
  .light img { max-width: 92vw; max-height: 84vh; object-fit: contain; border-radius: 8px; }
  .cap { color: #ddd; text-align: center; }
  .loose { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 6px; margin: 6px 0; }
  .lf { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 6px; padding: 1px 8px; cursor: pointer; max-width: 28ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .lf:hover { border-color: var(--faint); }
  /* the star: on the picture at hover, always when starred */
  .fv { position: absolute; left: 4px; top: 4px; border-radius: 6px; background: rgba(0, 0, 0, .55); opacity: 0; transition: opacity .15s; }
  .fv :global(.fav) { color: #fff; }
  .fv :global(.fav.on) { color: #f2bc3d; }
  .pic:hover .fv, .fv:has(:global(.fav.on)), .fv:focus-within { opacity: 1; }
</style>

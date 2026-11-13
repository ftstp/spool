<script lang="ts">
  // A song's mood board: pictures (drop files or pick them), notes and lyrics written in
  // place, a colour palette. Files live in the song's _moodboard folder; a removed one goes to
  // _moodboard/.corbeille, never deleted.
  import * as api from "../api";
  import type { Item } from "../api";
  import type { Board } from "../api/bindings/Board";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { sid, board }: { sid: number; board: Board } = $props();
  const pictures = $derived([...board.items, ...board.found].filter((i) => i.kind === "image" || i.kind === "gif"));
  const notes = $derived(board.items.filter((i) => i.kind === "text"));
  const others = $derived(board.items.filter((i) => !["image", "gif", "text"].includes(i.kind)));
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
    await app.fileAct(api.boardNote(sid, draft, name === "+lyrics", name.startsWith("+") ? undefined : name));
  }
  async function add(files: FileList | File[] | null | undefined) {
    for (const f of files ?? []) await run(api.boardAdd(sid, f), t("song.board.added", { name: f.name }));
  }
  const setAside = (i: Item) =>
    i.where === "lien"
      ? app.act(api.boardUnlink(sid, i.rel))
      : run(api.boardRemove(sid, i.name), app.physical ? t("song.board.trashed", { name: i.name }) : t("song.board.unlinked", { name: i.name }));
  const palette = (p: string[]) => run(api.boardMeta(sid, { palette: p }));
  /** `autofocus` does nothing on an element created later: focus it once it is there. */
  const focus = (n: HTMLElement) => {
    n.focus();
  };
  const label = (n: Item) => n.name.replace(/\.(txt|md)$/i, "");
</script>

<div class="card" class:over role="group" aria-label={t("song.board.label")}
  ondragover={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); over = true; } }}
  ondragleave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) over = false; }}
  ondrop={(e) => { e.preventDefault(); over = false; add(e.dataTransfer?.files); }}>
  <div class="head"><h2>{t("song.board.title")}</h2>
    <div class="acts">
      <button class="link small" onclick={() => input?.click()}>{t("song.board.addImage")}</button>
      <button class="link small" onclick={() => edit("+idea")}>{t("song.board.addIdea")}</button>
      {#if !hasLyrics}<button class="link small" onclick={() => edit("+lyrics")}>{t("song.board.addLyrics")}</button>{/if}
    </div>
  </div>
  <input bind:this={input} type="file" accept="image/*,video/*,audio/*" multiple hidden onchange={(e) => { add(e.currentTarget.files); e.currentTarget.value = ""; }} />

  {#if pictures.length}
    <div class="pics">
      {#each pictures.slice(0, 12) as p (p.rel)}
        <div class="pic" title={p.where === "dossier" ? t("song.board.inFolder", { name: p.name }) : p.name}>
          {#if api.mediaUrl(p.rel)}<button class="open" onclick={() => (big = p)} aria-label={t("song.board.enlarge", { name: p.name })}><img src={api.mediaUrl(p.rel)} alt={p.name} loading="lazy" /></button>
          {:else}<span class="small faint">{p.name}</span>{/if}
          {#if p.where === "board"}<button class="rm" title={t("song.board.setAside")} aria-label={t("song.board.setAsideName", { name: p.name })} onclick={() => setAside(p)}>×</button>{/if}
        </div>
      {/each}
    </div>
    {#if pictures.length > 12}<p class="small faint">{t("song.board.moreImages", { n: pictures.length - 12 })}</p>{/if}
  {/if}

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
  {#if others.length}
    <p class="small faint">{t("song.board.also", { list: others.map((o) => o.name).join(", ") })}</p>
  {/if}

  <div class="pal">
    {#each board.palette as c, i}
      <span class="sw" style:background={c} title={c}><button aria-label={t("song.board.removeColor", { c })} onclick={() => palette(board.palette.filter((_, j) => j !== i))}>×</button></span>
    {/each}
    <label class="addc" title={t("song.board.addColor")}><input type="color" onchange={(e) => palette([...board.palette, e.currentTarget.value])} />+</label>
  </div>
  {#if !pictures.length && !notes.length && !editing}<p class="small faint">{t("song.board.hint")}</p>{/if}
</div>

{#if big}
  <div class="light" role="presentation" onclick={() => (big = null)} onkeydown={(e) => { if (e.key === "Escape") big = null; }}>
    <img src={api.mediaUrl(big.rel)} alt={big.name} />
    <div class="cap small">{big.name}{big.w ? ` · ${big.w}×${big.h}` : ""}</div>
  </div>
{/if}

<style>
  .card { transition: border-color .15s, background-color .15s; }
  .card.over { border-color: var(--accent); background: var(--accent-bg); }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; }
  .acts { display: flex; gap: 12px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .pics { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .pic { position: relative; aspect-ratio: 1; border-radius: 8px; overflow: hidden; background: var(--panel2); display: grid; place-items: center; text-align: center; padding: 2px; }
  .open { border: 0; padding: 0; width: 100%; height: 100%; cursor: zoom-in; background: none; }
  .open img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .rm, .rm2 { border: 0; cursor: pointer; display: grid; place-items: center; }
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
</style>

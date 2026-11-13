<script lang="ts">
  // The dialogs the right-click menus open (lib/contextMenus.svelte.ts `ui`): move a folder,
  // send files to a song's mood board, type a name.
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { ui } from "../contextMenus.svelte";
  import MoveDialog from "./MoveDialog.svelte";
  import { t } from "../i18n/index.svelte";

  let q = $state("");
  /** Move the files into the board's folder instead of linking them (physical arranging). */
  let move = $state(false);
  let name = $state("");
  $effect(() => {
    if (ui.prompt) name = ui.prompt.value;
  });
  const songs = $derived(app.songs.filter((s) => !s.archived && (!q || (s.title ?? "").toLowerCase().includes(q.toLowerCase()))).slice(0, 14));
  async function send(id: number) {
    const items = ui.toBoard;
    if (!items) return;
    ui.toBoard = null;
    q = "";
    // the file stays where it is and shows on the board; it moves only if asked, switch on
    if (move && app.physical) await app.fileAct(api.fsToBoard(id, items));
    else await app.act(api.boardLink(id, items));
    move = false;
  }
  function ok() {
    const p = ui.prompt;
    ui.prompt = null;
    p?.ok(name.trim());
  }
  function focus(n: HTMLElement) {
    n.focus();
    if (n instanceof HTMLInputElement) n.select();
  }
</script>

{#if ui.move}<MoveDialog src={ui.move} close={() => (ui.move = null)} />{/if}

{#if ui.toBoard}
  <div class="scrim" role="presentation" onclick={() => (ui.toBoard = null)}></div>
  <div class="dlg card" role="dialog" aria-label={t("dialogs.board.label")}>
    <h2>{t("dialogs.board.title")}</h2>
    <p class="small muted ell">{ui.toBoard.length === 1 ? ui.toBoard[0] : t("dialogs.board.items", { n: ui.toBoard.length })}</p>
    <p class="small faint">{move ? t("dialogs.board.moveText") : t("dialogs.board.linkText")}</p>
    {#if app.physical}<label class="small"><input type="checkbox" bind:checked={move} /> {t("dialogs.board.moveCheck")}</label>{/if}
    <input type="search" placeholder={t("dialogs.board.search")} bind:value={q} use:focus onkeydown={(e) => e.key === "Escape" && (ui.toBoard = null)} />
    <div class="list">
      {#each songs as s (s.id)}<button class="row" onclick={() => send(s.id)}><b>{s.title}</b><span class="faint small">{s.dir}</span></button>{/each}
    </div>
    <div class="acts"><button class="btn" onclick={() => (ui.toBoard = null)}>{t("common.cancel")}</button></div>
  </div>
{/if}

{#if ui.prompt}
  <div class="scrim" role="presentation" onclick={() => (ui.prompt = null)}></div>
  <div class="dlg card small-dlg" role="dialog" aria-label={ui.prompt.title}>
    <h2>{ui.prompt.title}</h2>
    <input type="text" bind:value={name} use:focus onkeydown={(e) => { if (e.key === "Enter") ok(); if (e.key === "Escape") ui.prompt = null; }} />
    <div class="acts"><button class="btn" onclick={() => (ui.prompt = null)}>{t("common.cancel")}</button><button class="btn primary" onclick={ok}>{t("dialogs.ok")}</button></div>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .4); z-index: 80; }
  .dlg { position: fixed; z-index: 81; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px)); display: flex; flex-direction: column; gap: 10px; box-shadow: 0 30px 80px rgba(0, 0, 0, .45); padding: 18px 20px; }
  .small-dlg { width: min(380px, calc(100vw - 32px)); }
  h2 { margin: 0; font-size: 15px; }
  p { margin: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .list { overflow: auto; max-height: 320px; border: 1px solid var(--line); border-radius: 9px; padding: 4px; }
  .row { display: flex; flex-direction: column; align-items: flex-start; width: 100%; border: 0; background: none; padding: 6px 8px; border-radius: 7px; cursor: pointer; color: var(--ink); text-align: left; }
  .row:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .acts { display: flex; justify-content: flex-end; gap: 8px; }
</style>

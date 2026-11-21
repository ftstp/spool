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
  /** Send to a song's board, a release's or an artist's; or make it the main image of one. */
  let to = $state<"song" | "release" | "artist">("song");
  // the artists, read when a dialog that offers them opens
  let artistNames = $state<string[]>([]);
  $effect(() => {
    if (!ui.toBoard && !ui.cover) return;
    api.artists().then((l) => (artistNames = l.map((a) => a.name))).catch(() => (artistNames = []));
  });
  const artistList = $derived(artistNames.filter((n) => !q || low(n).includes(low(q))).slice(0, 14));
  // an artist's board is in Spool's data folder: the file is copied there, the original stays
  async function sendArtist(name: string) {
    const items = ui.toBoard;
    if (!items) return;
    ui.toBoard = null;
    q = "";
    try {
      for (const p of items) await api.artistBoardCopy(name, p, false);
      app.say(t("dialogs.board.sent", { n: items.length, title: name }));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // the main image: a song's chosen picture, a release's cover, an artist's photo
  async function coverFor(kind: "song" | "release" | "artist", key: string) {
    const p = ui.cover;
    if (!p) return;
    ui.cover = null;
    q = "";
    try {
      if (kind === "song") {
        const st = await api.setCover(+key, p);
        app.undo = st;
        if (st.label) app.say(st.label, { undo: true });
      } else if (kind === "release") {
        const r = app.releases.find((x) => x.id === key);
        if (!r) return;
        const [, st] = await api.releaseSave({ ...r, cover: p });
        app.undo = st;
        app.releases = await api.releases();
        app.say(t("dialogs.cover.done", { title: r.title }), { undo: true });
      } else {
        await api.artistBoardCopy(key, p, true);
        app.say(t("dialogs.cover.done", { title: key }));
      }
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const close = () => ((ui.toBoard = null), (ui.cover = null), (q = ""));
  const low = (x: string | null | undefined) => (x ?? "").toLowerCase();
  // the songs most recently worked on first
  const songs = $derived(
    app.songs
      .filter((s) => !s.archived && (!q || low(s.title).includes(low(q)) || low(s.artist).includes(low(q))))
      .sort((a, b) => (b.updated ?? b.created ?? 0) - (a.updated ?? a.created ?? 0))
      .slice(0, 14),
  );
  const releases = $derived(app.releases.filter((r) => !q || low(r.title).includes(low(q)) || low(r.artist).includes(low(q))).slice(0, 14));
  // a library file (a sample, a MIDI file: an absolute path) is never moved, only linked
  const fromLibrary = $derived((ui.toBoard ?? []).some((p) => /^[A-Za-z]:|^\//.test(p)));
  async function send(id: number) {
    const items = ui.toBoard;
    if (!items) return;
    ui.toBoard = null;
    q = "";
    // the file stays where it is and shows on the board; it moves only if asked, switch on
    if (move && app.physical && !fromLibrary) await app.fileAct(api.fsToBoard(id, items));
    else await app.act(api.boardLink(id, items));
    const s = app.songs.find((x) => x.id === id);
    app.say(t("dialogs.board.sent", { n: items.length, title: s?.title ?? "" }));
    move = false;
  }
  async function sendRelease(rid: string) {
    const items = ui.toBoard;
    const r = app.releases.find((x) => x.id === rid);
    if (!items || !r) return;
    ui.toBoard = null;
    q = "";
    try {
      const [, st] = await api.releaseSave({ ...r, board: [...new Set([...(r.board ?? []), ...items])] });
      app.undo = st;
      app.releases = await api.releases();
      app.say(t("dialogs.board.sent", { n: items.length, title: r.title }), { undo: true });
    } catch (e) {
      app.say(String(e), { error: true });
    }
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

{#if ui.toBoard || ui.cover}
  {@const cover = !!ui.cover}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="dlg card" role="dialog" aria-label={cover ? t("dialogs.cover.title") : t("dialogs.board.label")}>
    <h2>{cover ? t("dialogs.cover.title") : t("dialogs.board.title")}</h2>
    {#if ui.cover}
      <div class="pv"><img src={api.thumbUrl(ui.cover, 120)} alt="" /><p class="small muted ell">{ui.cover}</p></div>
    {:else if ui.toBoard}
      <p class="small muted ell">{ui.toBoard.length === 1 ? ui.toBoard[0] : t("dialogs.board.items", { n: ui.toBoard.length })}</p>
    {/if}
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={to === "song"} class:on={to === "song"} onclick={() => (to = "song")}>{t("dialogs.board.toSong")}</button>
      <button role="tab" aria-selected={to === "release"} class:on={to === "release"} onclick={() => (to = "release")}>{t("dialogs.board.toRelease")}</button>
      <button role="tab" aria-selected={to === "artist"} class:on={to === "artist"} onclick={() => (to = "artist")}>{t("dialogs.board.toArtist")}</button>
    </div>
    {#if cover}
      <p class="small faint">{t(`dialogs.cover.text.${to}`)}</p>
    {:else if to === "artist"}
      <p class="small faint">{t("dialogs.board.artistText")}</p>
    {:else}
      <p class="small faint">{move && to === "song" && !fromLibrary ? t("dialogs.board.moveText") : t("dialogs.board.linkText")}</p>
      {#if app.physical && to === "song" && !fromLibrary}<label class="small"><input type="checkbox" bind:checked={move} /> {t("dialogs.board.moveCheck")}</label>{/if}
    {/if}
    <input type="search" placeholder={to === "song" ? t("dialogs.board.search") : to === "release" ? t("dialogs.board.searchRelease") : t("dialogs.board.searchArtist")} bind:value={q} use:focus onkeydown={(e) => e.key === "Escape" && close()} />
    <div class="list">
      {#if to === "song"}
        {#each songs as s (s.id)}<button class="row" onclick={() => (cover ? coverFor("song", String(s.id)) : send(s.id))}><b>{s.title}</b><span class="faint small">{s.dir}</span></button>{/each}
      {:else if to === "release"}
        {#each releases as r (r.id)}<button class="row" onclick={() => (cover ? coverFor("release", r.id) : sendRelease(r.id))}><b>{r.title}</b><span class="faint small">{[r.artist, r.date].filter(Boolean).join(" · ")}</span></button>
        {:else}<p class="small faint">{t("dialogs.board.noRelease")}</p>{/each}
      {:else}
        {#each artistList as n (n)}<button class="row" onclick={() => (cover ? coverFor("artist", n) : sendArtist(n))}><b>{n}</b></button>
        {:else}<p class="small faint">{t("dialogs.board.noArtist")}</p>{/each}
      {/if}
    </div>
    <div class="acts"><button class="btn" onclick={close}>{t("common.cancel")}</button></div>
  </div>
{/if}

{#if ui.confirm}
  <div class="scrim" role="presentation" onclick={() => (ui.confirm = null)}></div>
  <div class="dlg card small-dlg" role="alertdialog" aria-label={ui.confirm.title}>
    <h2>{ui.confirm.title}</h2>
    <p class="small muted">{ui.confirm.text}</p>
    <div class="acts">
      <button class="btn" use:focus onclick={() => (ui.confirm = null)}>{t("common.cancel")}</button>
      <button class="btn primary" onclick={() => { const c = ui.confirm; ui.confirm = null; c?.run(); }}>{ui.confirm.ok}</button>
    </div>
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
  .tabs { display: flex; gap: 4px; margin: 2px 0 4px; }
  .tabs button { border: 1px solid var(--line); background: none; color: var(--muted); border-radius: 7px; padding: 3px 10px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .tabs button.on { background: var(--accent-bg); color: var(--ink); border-color: var(--accent); }
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
  .pv { display: flex; gap: 10px; align-items: center; min-width: 0; }
  .pv img { width: 56px; height: 56px; object-fit: cover; border-radius: 7px; flex: none; }
</style>

<script lang="ts">
  // A song's notes, each one apart (removed on its own), and those tied to a moment of the
  // render: "the drop is empty at 1:32", a click on the time plays from there. They are the
  // song's comments (one store with the Renders section's): a note without a moment has no
  // render. The song's older single text, written before, stays as the first note.
  import * as api from "../api";
  import type { Comment } from "../api/bindings/Comment";
  import { app } from "../state.svelte";
  import { player, mmss } from "../player.svelte";
  import { sheetTransport } from "../sheetTransport.svelte";
  import FoldHead from "./FoldHead.svelte";
  import { t, fmtDate } from "../i18n/index.svelte";

  let { sid, title, temoin, legacy, log, list = $bindable([]) }: {
    sid: number; title: string; temoin: string | null; legacy: string | null; log?: import("svelte").Snippet; list?: Comment[];
  } = $props();

  const tr = $derived(sheetTransport.by[sid] ?? null);
  // where the song is now, in the render's seconds: the render playing, else the project's
  // playhead (both follow the same bars), else nowhere
  const now = $derived(
    temoin && player.is(temoin) && player.t > 0 ? player.t : tr && tr.at !== null && tr.at > 0 ? tr.at * tr.secs : null,
  );
  const shown = $derived([...list].sort((a, b) => (a.rel && b.rel ? a.at - b.at : a.rel ? -1 : b.rel ? 1 : a.created - b.created)));
  let text = $state("");
  let timed = $state(false);
  async function act(p: Promise<unknown>) {
    try {
      app.undo = (await p) as typeof app.undo;
      list = await api.comments(sid);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function add() {
    const v = text.trim();
    if (!v) return;
    const at = timed && temoin && now !== null ? now : null;
    await act(api.commentEdit(sid, at === null ? { rel: "", at: 0, text: v } : { rel: temoin!, at, text: v }));
    text = "";
    timed = false;
  }
  function jump(c: Comment) {
    if (tr && tr.secs > 0) {
      tr.seek(Math.min(1, c.at / tr.secs));
      if (!tr.sounding) tr.playPause();
    } else player.play(c.rel, sid, title, c.at);
  }
  const leaf = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
</script>

<section class="card" data-sec="notes">
  <FoldHead {sid} key="notes" title={t("song.notes.title")}>{#if list.length}<span class="small faint">{list.length + (legacy ? 1 : 0)}</span>{/if}</FoldHead>
  <div class="new">
    <textarea rows="2" bind:value={text} placeholder={t("song.notes.placeholder")}
      onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); add(); } }}></textarea>
    <div class="row">
      {#if temoin}
        <label class="small muted at" title={t("song.notes.atTip")}>
          <input type="checkbox" bind:checked={timed} disabled={now === null} />
          {now === null ? t("song.notes.atNone") : t("song.notes.at", { time: mmss(now) })}
        </label>
      {/if}
      <span class="grow"></span>
      <button class="btn sm primary" disabled={!text.trim()} onclick={add}>{t("song.notes.add")}</button>
    </div>
  </div>
  {#if legacy || shown.length}
    <ul>
      {#if legacy}
        <li>
          <span class="when"></span>
          <span class="tx">{legacy}</span>
          <button class="x" aria-label={t("song.notes.remove")} title={t("song.notes.remove")} onclick={() => app.act(api.songEdit(sid, { notes: "" }))}>×</button>
        </li>
      {/if}
      {#each shown as c (c.id)}
        <li class:done={c.done}>
          {#if c.rel}
            <button class="when time" title={t("song.comments.jump") + (c.rel !== temoin ? "\n" + leaf(c.rel) : "")} onclick={() => jump(c)}>{mmss(c.at)}</button>
          {:else}
            <span class="when small faint" title={fmtDate(c.created, { day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" })}>{fmtDate(c.created, { day: "numeric", month: "short" })}</span>
          {/if}
          <span class="tx">{c.text}</span>
          {#if c.rel}<input type="checkbox" checked={c.done} aria-label={t("song.comments.done")} title={t("song.comments.done")} onchange={(e) => act(api.commentEdit(sid, { ...c, done: e.currentTarget.checked }))} />{:else}<span></span>{/if}
          <button class="x" aria-label={t("song.notes.remove")} title={t("song.notes.remove")} onclick={() => act(api.commentEdit(sid, c, true))}>×</button>
        </li>
      {/each}
    </ul>
  {/if}
  {@render log?.()}
</section>

<style>
  .new { display: flex; flex-direction: column; gap: 6px; }
  textarea { width: 100%; resize: vertical; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; padding: 7px 10px; color: inherit; font: inherit; outline: 0; }
  textarea:focus { border-color: var(--accent); }
  .row { display: flex; align-items: center; gap: 8px; }
  .grow { flex: 1; }
  .at { display: inline-flex; align-items: center; gap: 5px; cursor: pointer; }
  ul { list-style: none; margin: 10px 0 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li { display: grid; grid-template-columns: 50px minmax(0, 1fr) 16px 18px; gap: 8px; align-items: start; padding: 4px 4px; border-radius: 6px; font-size: 13px; }
  li:hover { background: var(--panel2); }
  li.done .tx { color: var(--faint); text-decoration: line-through; }
  .when { padding-top: 1px; white-space: nowrap; }
  .time { border: 0; background: none; color: var(--accent); font: inherit; font-size: 12px; font-variant-numeric: tabular-nums; cursor: pointer; padding: 0; text-align: left; }
  .time:hover { text-decoration: underline; }
  .tx { white-space: pre-wrap; overflow-wrap: anywhere; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; line-height: 1; padding: 0; opacity: 0; }
  li:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--ink); }
</style>

<script lang="ts">
  // A song's timed comments: "the drop is empty at 1:32", on the render playing (or the
  // reference render). A click on the time plays from there; ticked when dealt with. Each
  // change is one Ctrl+Z.
  import * as api from "../api";
  import type { Comment } from "../api/bindings/Comment";
  import { app } from "../state.svelte";
  import { player, mmss } from "../player.svelte";
  import { t } from "../i18n/index.svelte";

  let { sid, title, rel, list = $bindable([]) }: { sid: number; title: string; rel: string; list?: Comment[] } = $props();
  $effect(() => {
    void app.version;
    const s = sid;
    api.comments(s).then((v) => s === sid && (list = v)).catch(() => {});
  });
  // the render the comment is about: the one playing, if it is this song's, else the reference
  const on = $derived(player.sid === sid && player.rel ? player.rel : rel);
  const now = $derived(player.sid === sid && player.rel ? player.t : 0);
  let text = $state("");
  let at = $state<number | null>(null);
  let input = $state<HTMLInputElement>();
  const leaf = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
  async function act(p: Promise<unknown>) {
    try {
      app.undo = (await p) as typeof app.undo;
      list = await api.comments(sid);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  function start() {
    at = now;
    queueMicrotask(() => input?.focus());
  }
  async function add() {
    if (!text.trim()) return;
    await act(api.commentEdit(sid, { rel: on, at: at ?? now, text }));
    text = "";
    at = null;
  }
  function jump(c: Comment) {
    player.play(c.rel, sid, title, c.at);
  }
  // the notes without a moment are the Notes section's
  const timed = $derived(list.filter((c) => !!c.rel));
  const open = $derived(timed.filter((c) => !c.done).length);
</script>

<div class="cm">
  <div class="head">
    <span class="small muted">{t("song.comments.title")}{#if timed.length}<span class="faint n">({open}/{timed.length})</span>{/if}</span>
    {#if at === null}
      <button class="btn sm" onclick={start} title={t("song.comments.addTip")}>{t("song.comments.add", { time: mmss(now) })}</button>
    {/if}
  </div>
  {#if at !== null}
    <div class="new">
      <span class="time">{mmss(at)}</span>
      <input bind:this={input} bind:value={text} placeholder={t("song.comments.ph")}
        onkeydown={(e) => { if (e.key === "Enter") add(); if (e.key === "Escape") { at = null; text = ""; } }} />
      <button class="btn sm primary" disabled={!text.trim()} onclick={add}>{t("song.comments.save")}</button>
    </div>
  {/if}
  {#if timed.length}
    <ul>
      {#each timed as c (c.id)}
        <li class:done={c.done}>
          <input type="checkbox" checked={c.done} aria-label={t("song.comments.done")} title={t("song.comments.done")} onchange={(e) => act(api.commentEdit(sid, { ...c, done: e.currentTarget.checked }))} />
          <button class="time" title={t("song.comments.jump")} onclick={() => jump(c)}>{mmss(c.at)}</button>
          <span class="tx">{c.text}{#if c.rel !== rel}<span class="faint small" title={c.rel}> · {leaf(c.rel)}</span>{/if}</span>
          <button class="x" aria-label={t("song.comments.remove")} title={t("song.comments.remove")} onclick={() => act(api.commentEdit(sid, c, true))}>×</button>
        </li>
      {/each}
    </ul>
  {:else if at === null}
    <p class="small faint hint">{t("song.comments.hint")}</p>
  {/if}
</div>

<style>
  .cm { margin-top: 10px; display: flex; flex-direction: column; gap: 4px; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .new { display: flex; gap: 6px; align-items: center; }
  .new input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 4px 8px; font: inherit; font-size: 13px; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
  li { display: grid; grid-template-columns: 16px 44px minmax(0, 1fr) 18px; gap: 6px; align-items: center; padding: 2px 4px; border-radius: 6px; font-size: 13px; }
  li:hover { background: var(--panel2); }
  li.done .tx { color: var(--faint); text-decoration: line-through; }
  .time { border: 0; background: none; color: var(--accent); font: inherit; font-size: 12px; font-variant-numeric: tabular-nums; cursor: pointer; padding: 0; text-align: left; }
  span.time { color: var(--muted); }
  .tx { overflow-wrap: anywhere; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; opacity: 0; }
  li:hover .x, .x:focus-visible { opacity: 1; }
  .hint { margin: 0; }
  .n { margin-left: 0.35em; }
</style>

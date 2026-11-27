<script lang="ts">
  // The sheet's "Comments" section: every note pinned on a moment of the song (on its renders'
  // waveforms, on its projects' bars), by what it was pinned on, in time order. A click on a
  // moment plays from there (the render, or the project with the app's sounds); ticked when
  // dealt with; written at the moment heard. Each change is one Ctrl+Z.
  import type { Comment } from "../api/bindings/Comment";
  import { commentsOf, addComment, editComment, removeComment } from "../comments.svelte";
  import { player, mmss } from "../player.svelte";
  import { projectFromSecs } from "../projectPlay";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";
  import FoldHead from "./FoldHead.svelte";

  let { sid, title, rel }: { sid: number; title: string; rel: string | null } = $props();
  const list = $derived(commentsOf(sid));
  const isProject = (r: string) => /\.als$/i.test(r);
  const leaf = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
  const same = (a: string, b: string | null) => !!b && a.replace(/\\/g, "/").toLowerCase() === b.replace(/\\/g, "/").toLowerCase();

  // the timed ones, by what they are pinned on: the reference render first, then the other
  // renders, then the projects
  const groups = $derived.by(() => {
    const m = new Map<string, Comment[]>();
    for (const c of list.filter((c) => !!c.rel)) {
      const k = c.rel.replace(/\\/g, "/");
      m.set(k, [...(m.get(k) ?? []), c]);
    }
    const rank = (k: string) => (same(k, rel) ? 0 : isProject(k) ? 2 : 1);
    return [...m]
      .map(([src, v]) => ({ src, project: isProject(src), notes: v.sort((a, b) => a.at - b.at) }))
      .sort((a, b) => rank(a.src) - rank(b.src) || a.src.localeCompare(b.src));
  });
  const all = $derived(groups.flatMap((g) => g.notes));
  const open = $derived(all.filter((c) => !c.done).length);

  function play(c: Comment) {
    if (isProject(c.rel)) projectFromSecs(c.rel, title, c.at, (m) => app.say(m)).catch((e) => app.say(String(e), { error: true }));
    else player.play(c.rel, sid, title, c.at);
  }
  const heardHere = (src: string) => (isProject(src) ? false : player.sid === sid && same(src, player.rel));
  const nowOn = (src: string) => heardHere(src) && player.playing;

  // a new one at the moment heard: on the render playing (this song's), else the reference
  const on = $derived(player.sid === sid && player.rel ? player.rel : rel);
  const now = $derived(player.sid === sid && player.rel ? player.t : 0);
  let text = $state("");
  let at = $state<number | null>(null);
  let input = $state<HTMLInputElement>();
  function start() {
    at = now;
    queueMicrotask(() => input?.focus());
  }
  function add() {
    if (!text.trim() || !on) return;
    void addComment(sid, { rel: on, at: at ?? now, text });
    text = "";
    at = null;
  }
  // a text changed in place: saved when left (emptied: kept, the × removes)
  let editing = $state<number | null>(null);
  function save(c: Comment, v: string) {
    editing = null;
    const x = v.trim();
    if (x && x !== c.text) void editComment(sid, { ...c, text: x });
  }
  const focus = (el: HTMLInputElement) => {
    el.focus();
    el.select();
  };
</script>

<section class="card cm" data-sec="comments">
  <FoldHead {sid} key="comments" title={t("song.sections.label.comments")}>
    {#if all.length}<span class="small faint">{t("song.comments.count", { open, n: all.length })}</span>{/if}
    {#snippet actions()}
      {#if on && at === null}
        <button class="btn sm" onclick={start} title={t("song.comments.addTip")}>{t("song.comments.add", { time: mmss(now) })}</button>
      {/if}
    {/snippet}
  </FoldHead>
  {#if at !== null}
    <div class="new">
      <span class="time">{mmss(at)}</span>
      <input bind:this={input} bind:value={text} placeholder={t("song.comments.ph")}
        onkeydown={(e) => { if (e.key === "Enter") add(); if (e.key === "Escape") { at = null; text = ""; } }} />
      <button class="btn sm primary" disabled={!text.trim()} onclick={add}>{t("song.comments.save")}</button>
    </div>
  {/if}
  {#each groups as g (g.src)}
    <h3 class="src small muted" title={g.src}>
      {g.project ? t("song.comments.onProject", { name: leaf(g.src) }) : t("song.comments.onRender", { name: leaf(g.src) })}
      {#if same(g.src, rel)}<span class="faint">{t("song.comments.reference")}</span>{/if}
    </h3>
    <ul>
      {#each g.notes as c (c.id)}
        <li class:done={c.done} class:now={nowOn(g.src) && player.t >= c.at && player.t < c.at + 4}>
          <input type="checkbox" checked={c.done} aria-label={t("song.comments.done")} title={t("song.comments.done")} onchange={(e) => editComment(sid, { ...c, done: e.currentTarget.checked })} />
          <button class="time" title={t("song.comments.jump")} onclick={() => play(c)}>
            <svg viewBox="0 0 24 24" width="9" height="9" fill="currentColor" aria-hidden="true"><path d="M7 5v14l12-7z" /></svg>{mmss(c.at)}
          </button>
          {#if editing === c.id}
            <input class="ed" value={c.text} use:focus onblur={(e) => save(c, e.currentTarget.value)}
              onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); if (e.key === "Escape") editing = null; }} />
          {:else}
            <button class="tx" title={t("song.comments.editTip")} onclick={() => (editing = c.id)}>{c.text}</button>
          {/if}
          <button class="x" aria-label={t("song.comments.remove")} title={t("song.comments.remove")} onclick={() => removeComment(sid, c)}>×</button>
        </li>
      {/each}
    </ul>
  {:else}
    {#if at === null}<p class="small faint hint">{t("song.comments.hint")}</p>{/if}
  {/each}
</section>

<style>
  .cm { display: flex; flex-direction: column; gap: 4px; }
  .new { display: flex; gap: 6px; align-items: center; }
  .new input, .ed { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 3px 8px; font: inherit; font-size: 13px; }
  .new input:focus, .ed:focus { border-color: var(--accent); outline: 0; }
  .src { margin: 8px 0 2px; font-weight: 600; display: flex; gap: 6px; align-items: baseline; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
  li { display: grid; grid-template-columns: 16px 52px minmax(0, 1fr) 18px; gap: 6px; align-items: center; padding: 2px 4px; border-radius: 6px; font-size: 13px; }
  li:hover { background: var(--panel2); }
  li.now { background: color-mix(in srgb, var(--rec) 14%, transparent); }
  li.done .tx { color: var(--faint); text-decoration: line-through; }
  .time { display: inline-flex; align-items: center; gap: 4px; border: 0; background: none; color: var(--accent); font: inherit; font-size: 12px; font-variant-numeric: tabular-nums; cursor: pointer; padding: 0; text-align: left; }
  .time:hover { text-decoration: underline; }
  span.time { color: var(--muted); }
  .tx { border: 0; background: none; color: inherit; font: inherit; text-align: left; padding: 0; cursor: text; overflow-wrap: anywhere; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; opacity: 0; }
  li:hover .x, .x:focus-visible { opacity: 1; }
  .hint { margin: 0; }
</style>

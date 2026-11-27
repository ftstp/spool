<script lang="ts">
  // A song's notes, as post-its: what to remember about the song as a whole, not tied to a
  // moment (those are pinned on the waveform at the top of the sheet, see WaveNotes). Each one
  // apart: written, changed in place, removed on its own. The song's older single text, written
  // before, stays as the first one.
  import type { Comment } from "../api/bindings/Comment";
  import { commentsOf, addComment, editComment, removeComment } from "../comments.svelte";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { t, fmtDate } from "../i18n/index.svelte";

  // shown inside the mood board (its post-its), under a small title of its own
  let { sid, legacy, log }: {
    sid: number; title?: string; temoin?: string | null; legacy: string | null; log?: import("svelte").Snippet;
  } = $props();
  const list = $derived(commentsOf(sid));

  const notes = $derived(list.filter((c) => !c.rel).sort((a, b) => a.created - b.created));
  let text = $state("");
  async function add() {
    const v = text.trim();
    if (!v) return;
    text = "";
    await addComment(sid, { rel: "", at: 0, text: v });
  }
  // one changed in place: saved when left, if it changed (emptied: kept, the × removes)
  let editing = $state<number | null>(null);
  function save(c: Comment, v: string) {
    editing = null;
    const x = v.trim();
    if (x && x !== c.text) void editComment(sid, { ...c, text: x });
  }
  const focus = (el: HTMLTextAreaElement) => {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  };
</script>

<div class="notes">
  <h3 class="small muted nt">{t("song.notes.title")}{#if notes.length || legacy}<span class="faint"> · {notes.length + (legacy ? 1 : 0)}</span>{/if}</h3>
  <div class="wall">
    {#if legacy}
      <div class="it">
        <p class="tx">{legacy}</p>
        <button class="x" aria-label={t("song.notes.remove")} title={t("song.notes.remove")} onclick={() => app.act(api.songEdit(sid, { notes: "" }))}>×</button>
      </div>
    {/if}
    {#each notes as c (c.id)}
      <div class="it">
        {#if editing === c.id}
          <textarea class="tx" rows="3" value={c.text} use:focus onblur={(e) => save(c, e.currentTarget.value)}
            onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); e.currentTarget.blur(); } if (e.key === "Escape") editing = null; }}></textarea>
        {:else}
          <button class="tx" title={t("song.notes.editTip")} onclick={() => (editing = c.id)}>{c.text}</button>
        {/if}
        <span class="when small faint" title={fmtDate(c.created, { day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" })}>{fmtDate(c.created, { day: "numeric", month: "short" })}</span>
        <button class="x" aria-label={t("song.notes.remove")} title={t("song.notes.remove")} onclick={() => removeComment(sid, c)}>×</button>
      </div>
    {/each}
    <div class="it new">
      <textarea class="tx" rows="3" bind:value={text} placeholder={t("song.notes.placeholder")}
        onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); add(); } }} onblur={add}></textarea>
    </div>
  </div>
  <p class="small faint hint">{t("song.notes.pinHint")}</p>
  {@render log?.()}
</div>

<style>
  .notes { margin: 12px 0 4px; }
  .nt { margin: 0 0 6px; font-weight: 600; }
  .wall { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 8px; }
  /* a post-it: warm paper, a slight lift */
  .it { position: relative; display: flex; flex-direction: column; gap: 4px; min-height: 80px; padding: 10px 12px 8px; border-radius: 4px 4px 10px 4px;
    background: color-mix(in srgb, #e3b341 16%, var(--panel)); border: 1px solid color-mix(in srgb, #e3b341 30%, var(--line)); box-shadow: 0 2px 6px rgba(0, 0, 0, .12); }
  .it.new { background: none; border-style: dashed; box-shadow: none; }
  .tx { flex: 1; margin: 0; padding: 0; border: 0; background: none; color: var(--ink); font: inherit; font-size: 13px; line-height: 1.45; text-align: left; white-space: pre-wrap; overflow-wrap: anywhere; cursor: text; resize: none; outline: 0; }
  textarea.tx { width: 100%; min-height: 60px; }
  .when { align-self: flex-start; }
  .x { position: absolute; top: 2px; right: 4px; border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; line-height: 1; padding: 2px; opacity: 0; }
  .it:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--ink); }
  .hint { margin: 8px 0 0; }
</style>

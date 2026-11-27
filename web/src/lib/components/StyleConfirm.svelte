<script lang="ts">
  // "Spool thinks this is dnb. Right?": a song whose style Spool is unsure of, confirmed in
  // one click (or another style, "mixed", "I don't know"). The answer is the song's style, as
  // if chosen by hand, and teaches every song that sounds like it.
  import * as api from "../api";
  import type { StyleAnswer } from "../api/bindings/StyleAnswer";
  import { app } from "../state.svelte";
  import { player } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";

  let {
    sid,
    title = "",
    temoin = null,
    options,
    known = [],
    compact = false,
    done,
  }: {
    sid: number;
    title?: string;
    /** Its reference render: a play button when given. */
    temoin?: string | null;
    /** What Spool thinks, best first. */
    options: string[];
    /** The styles Spool knows, to type another one. */
    known?: string[];
    /** In a song's sheet: one line. */
    compact?: boolean;
    done?: (a: StyleAnswer) => void;
  } = $props();

  let busy = $state(false);
  let other = $state("");
  let typing = $state(false);
  const best = $derived(options[0] ?? "");

  async function answer(style: string | null) {
    if (busy) return;
    busy = true;
    try {
      const a = await api.styleAnswer(sid, style);
      if (style) app.say(a.classed > 0 ? t("styles.q.classed", { n: fmtNum(a.classed), style }) : t("styles.q.noted", { style }));
      done?.(a);
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
  function submit(e: SubmitEvent) {
    e.preventDefault();
    const s = other.trim();
    if (s) answer(s);
  }
</script>

<div class="q" class:compact>
  <p class="ask">
    {#if temoin}
      <button class="play" title={t("styles.q.listen")} aria-label={t("styles.q.listen")} onclick={() => player.play(temoin, sid, title)}>▶</button>
    {/if}
    {#if !compact && title}<a class="title" href="#/song/{sid}">{title}</a><span class="faint"> · </span>{/if}
    {#if best}{t("styles.q.thinks", { style: best })}{:else}{t("styles.q.what")}{/if}
  </p>
  <div class="row">
    {#if best}<button class="btn primary sm" disabled={busy} onclick={() => answer(best)}>{t("styles.q.yes")}</button>{/if}
    {#each options.slice(1) as o (o)}<button class="chip-btn" disabled={busy} onclick={() => answer(o)}>{o}</button>{/each}
    {#if typing}
      <form onsubmit={submit}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={other} list="style-confirm-known" placeholder={t("styles.q.otherPh")} aria-label={t("styles.q.other")} autocomplete="off" spellcheck="false" autofocus />
        <button class="btn sm" disabled={busy || !other.trim()}>{t("styles.q.ok")}</button>
      </form>
      <datalist id="style-confirm-known">{#each known as k (k)}<option value={k}></option>{/each}</datalist>
    {:else}
      <button class="link small" disabled={busy} onclick={() => (typing = true)}>{t("styles.q.other")}</button>
    {/if}
    <button class="link small" disabled={busy} title={t("styles.q.mixedTip")} onclick={() => answer(null)}>{t("styles.q.mixed")}</button>
    <button class="link small" disabled={busy} title={t("styles.q.skipTip")} onclick={() => answer(null)}>{t("styles.q.skip")}</button>
  </div>
</div>

<style>
  .q { display: grid; gap: 8px; }
  .q.compact { gap: 4px; margin-top: 6px; color: var(--muted); font-size: 12.5px; }
  .ask { margin: 0; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .title { color: var(--ink); font-weight: 600; }
  .play { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line); background: none; color: var(--ink); cursor: pointer; font-size: 10px; padding: 0; flex: none; }
  .play:hover { background: var(--panel); }
  .row { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
  .chip-btn { border: 1px solid var(--line); background: none; border-radius: 999px; padding: 1px 9px; font: inherit; color: var(--ink); cursor: pointer; }
  .chip-btn:hover:not(:disabled) { background: var(--panel); }
  .link { border: 0; background: none; color: var(--muted); cursor: pointer; padding: 2px 4px; font: inherit; text-decoration: underline; text-decoration-color: var(--line); text-underline-offset: 3px; }
  .link:hover:not(:disabled) { color: var(--ink); }
  form { display: inline-flex; gap: 6px; }
  form input { width: 12em; }
</style>

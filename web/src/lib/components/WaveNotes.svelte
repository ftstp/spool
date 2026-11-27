<script lang="ts">
  // Notes pinned to a moment of what plays, over the sheet's waveform (the render) or the
  // project's bar: the render's and the project's are kept apart (each on its own file), as
  // their moments are not the same. Hovering shows the exact moment under the pointer; a
  // right-click writes a note there at once. A note is a pin: dragged, it moves; clicked, it
  // opens (play from there, rename, tick, remove). While it plays, the notes just passed show
  // their text by their pin, as on a listening page.
  import type { Snippet } from "svelte";
  import type { Comment } from "../api/bindings/Comment";
  import { mmss } from "../player.svelte";
  import { t } from "../i18n/index.svelte";

  let {
    notes,
    len,
    now,
    playing,
    add,
    edit,
    remove,
    playAt,
    children,
    above = false,
  }: {
    /** This source's notes (seconds into it). */
    notes: Comment[];
    /** Its length in seconds (0: not known yet, nothing is placed). */
    len: number;
    /** Where it plays now, in seconds (null: it does not play). */
    now: number | null;
    playing: boolean;
    add: (at: number, text: string) => Promise<void> | void;
    edit: (c: Comment) => Promise<void> | void;
    remove: (c: Comment) => Promise<void> | void;
    playAt: (at: number) => void;
    children: Snippet;
    /** The note's bubble opens above (a bar at the bottom of the window). */
    above?: boolean;
  } = $props();

  let box = $state<HTMLDivElement>();
  let w = $state(0);
  // the moment under the pointer
  let hx = $state<number | null>(null);
  const atX = (x: number) => (w && len ? Math.max(0, Math.min(len, (x / w) * len)) : 0);
  const xOf = (at: number) => (len ? Math.max(0, Math.min(100, (at / len) * 100)) : 0);
  const localX = (e: MouseEvent) => {
    const r = box!.getBoundingClientRect();
    w = r.width;
    return Math.max(0, Math.min(r.width, e.clientX - r.left));
  };

  // the note being written (new) or opened (an existing one), and where its bubble sits
  let open = $state<{ c: Comment | null; at: number; text: string } | null>(null);
  let input = $state<HTMLInputElement>();
  function write(e: MouseEvent) {
    if (!len) return;
    e.preventDefault();
    e.stopPropagation();
    open = { c: null, at: atX(localX(e)), text: "" };
    queueMicrotask(() => input?.focus());
  }
  function openNote(c: Comment) {
    open = { c, at: c.at, text: c.text };
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  }
  async function commit() {
    const o = open;
    if (!o) return;
    const v = o.text.trim();
    open = null;
    if (!o.c) {
      if (v) await add(o.at, v);
    } else if (v && v !== o.c.text) await edit({ ...o.c, text: v });
  }
  // a click elsewhere closes it (what was typed is kept)
  $effect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (box && e.target instanceof Node && box.contains(e.target)) return;
      void commit();
    };
    const t = setTimeout(() => addEventListener("pointerdown", away, true));
    return () => {
      clearTimeout(t);
      removeEventListener("pointerdown", away, true);
    };
  });

  // a pin dragged along moves its note (a short click opens it)
  let drag = $state<{ c: Comment; x0: number; at: number; moved: boolean } | null>(null);
  function down(e: PointerEvent, c: Comment) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { c, x0: e.clientX, at: c.at, moved: false };
  }
  function moveDrag(e: PointerEvent) {
    if (!drag) return;
    if (Math.abs(e.clientX - drag.x0) > 3) drag.moved = true;
    if (drag.moved) drag.at = atX(localX(e));
  }
  async function up(e: PointerEvent) {
    const d = drag;
    drag = null;
    if (!d) return;
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
    if (!d.moved) return openNote(d.c);
    if (Math.abs(d.at - d.c.at) > 0.05) await edit({ ...d.c, at: d.at });
  }
  const placeOf = (c: Comment) => (drag && drag.c.id === c.id ? drag.at : c.at);

  // what plays now: the notes passed in the last few seconds say their text
  const SHOW_FOR = 4;
  const speaking = $derived(
    playing && now !== null ? notes.filter((c) => !c.done && now! >= c.at && now! < c.at + SHOW_FOR).slice(-2) : [],
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wn" bind:this={box} onmousemove={(e) => (hx = localX(e))} onmouseleave={() => (hx = null)} oncontextmenu={write}>
  {@render children()}
  {#if len}
    {#if hx !== null && !open && !drag}
      <span class="cur" style:left="{hx}px"></span>
      <span class="hl" style:left="{hx}px">{mmss(atX(hx))}</span>
    {/if}
    {#each notes as c (c.id)}
      <button class="pin" class:done={c.done} class:drag={drag?.c.id === c.id} style:left="{xOf(placeOf(c))}%"
        title={drag?.c.id === c.id ? mmss(drag.at) : `${mmss(c.at)} · ${c.text}\n${t("song.wnotes.pinTip")}`}
        aria-label={`${mmss(c.at)} ${c.text}`}
        onpointerdown={(e) => down(e, c)} onpointermove={moveDrag} onpointerup={up} oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); openNote(c); }}><i></i></button>
    {/each}
    {#each speaking as c (c.id)}
      <span class="say" style:left="{xOf(c.at)}%"><b>{mmss(c.at)}</b> {c.text}</span>
    {/each}
    {#if open}
      <div class="pop" class:up={above} class:right={xOf(open.at) > 60} style:left="{xOf(open.at)}%" role="dialog" aria-label={t("song.wnotes.title")}>
        <div class="row">
          <span class="time">{mmss(open.at)}</span>
          <input bind:this={input} bind:value={open.text} placeholder={open.c ? t("song.wnotes.rename") : t("song.wnotes.ph")}
            onkeydown={(e) => { if (e.key === "Enter") void commit(); if (e.key === "Escape") open = null; }} />
        </div>
        {#if open.c}
          {@const c = open.c}
          <div class="acts">
            <button class="btn sm primary" onclick={() => { open = null; playAt(c.at); }}>{t("song.wnotes.play")}</button>
            <label class="small muted"><input type="checkbox" checked={c.done} onchange={(e) => { open = null; void edit({ ...c, done: e.currentTarget.checked }); }} /> {t("song.comments.done")}</label>
            <span class="grow"></span>
            <button class="btn sm danger" onclick={() => { open = null; void remove(c); }}>{t("song.wnotes.remove")}</button>
          </div>
        {:else}
          <p class="small faint">{t("song.wnotes.hint")}</p>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .wn { position: relative; }
  .cur { position: absolute; top: 0; bottom: 0; width: 1px; background: color-mix(in srgb, var(--ink) 55%, transparent); pointer-events: none; z-index: 2; }
  .hl { position: absolute; top: 1px; transform: translateX(-50%); padding: 0 5px; border-radius: 4px; background: var(--ink); color: var(--panel); font-size: 10.5px; font-weight: 600; line-height: 15px; font-variant-numeric: tabular-nums; pointer-events: none; z-index: 6; white-space: nowrap; }
  /* a note's pin: a line through the wave, a head on top to grab */
  .pin { position: absolute; top: -5px; bottom: 0; width: 12px; margin-left: -6px; padding: 0; border: 0; background: none; cursor: grab; z-index: 3; touch-action: none; }
  .pin::before { content: ""; position: absolute; left: 5.5px; top: 6px; bottom: 0; width: 1px; background: color-mix(in srgb, var(--rec) 70%, transparent); }
  .pin i { position: absolute; left: 1.5px; top: 0; width: 9px; height: 9px; border-radius: 50% 50% 50% 0; transform: rotate(-45deg); background: var(--rec); box-shadow: 0 0 0 1.5px var(--panel); }
  .pin:hover i, .pin.drag i { transform: rotate(-45deg) scale(1.25); }
  .pin.drag { cursor: grabbing; }
  .pin.done i { background: var(--faint); }
  .pin.done::before { background: color-mix(in srgb, var(--faint) 60%, transparent); }
  /* a note just passed while it plays: its text by its pin, inside the wave's lower half */
  .say { position: absolute; bottom: 2px; max-width: 260px; padding: 1px 7px; border-radius: 5px; background: color-mix(in srgb, var(--panel) 92%, transparent); border: 1px solid color-mix(in srgb, var(--rec) 50%, var(--line)); color: var(--ink); font-size: 11.5px; line-height: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; pointer-events: none; z-index: 4; animation: sayin .18s ease-out; }
  .say b { color: var(--rec); font-weight: 600; font-variant-numeric: tabular-nums; }
  @keyframes sayin { from { opacity: 0; transform: translateY(3px); } }
  .pop { position: absolute; top: calc(100% + 8px); transform: translateX(-14px); z-index: 40; width: 300px; padding: 8px; border-radius: 10px; border: 1px solid var(--line); background: var(--panel); box-shadow: 0 12px 28px rgba(0, 0, 0, .32); display: flex; flex-direction: column; gap: 6px; }
  .pop.right { transform: translateX(calc(-100% + 14px)); }
  .pop.up { top: auto; bottom: calc(100% + 8px); }
  .row { display: flex; gap: 6px; align-items: center; }
  .time { flex: none; font-size: 12px; font-weight: 600; color: var(--rec); font-variant-numeric: tabular-nums; }
  .row input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--panel2); color: var(--ink); border-radius: 7px; padding: 5px 8px; font: inherit; font-size: 13px; outline: 0; }
  .row input:focus { border-color: var(--accent); }
  .acts { display: flex; gap: 8px; align-items: center; }
  .acts label { display: inline-flex; gap: 4px; align-items: center; cursor: pointer; }
  .grow { flex: 1; }
  .pop p { margin: 0; }
  .danger { color: var(--bad); }
  .danger:hover { border-color: var(--bad); }
  @media (prefers-reduced-motion: reduce) { .say { animation: none; } }
</style>

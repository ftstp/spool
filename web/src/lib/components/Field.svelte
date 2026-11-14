<script lang="ts">
  // A value shown as text, edited in place: click, type, Enter or leaving saves, Escape cancels.
  // With `detected`, an empty field shows what Spool detected (marked as such); hovering tells
  // where it comes from and offers the other candidates. Editing starts empty: leaving without
  // typing writes nothing, so a detected value never becomes the user's by accident.
  import { t } from "../i18n/index.svelte";
  import type { Field as Detected } from "../api/bindings/Field";
  let {
    value,
    placeholder,
    save,
    kind = "text",
    big = false,
    width = "",
    options = [],
    hint = "",
    detected = null,
  }: {
    value: string | number | null | undefined;
    placeholder: string;
    save: (v: string) => void;
    kind?: "text" | "date" | "tempo";
    big?: boolean;
    width?: string;
    /** Choices offered under the field while editing (artists, keys, styles); typing stays free. */
    options?: string[];
    /** Shown on hover: what the app sees for this field, and why. */
    hint?: string;
    /** What Spool detects for this field (crates/core detect.rs). */
    detected?: Detected | null;
  } = $props();
  // the detected value, when the user wrote none
  const auto = $derived(detected?.shown && detected.shown.src !== "noté" ? detected.shown : null);
  const SRC: Record<string, string> = { "noté": "noted", Live: "live", "estimé": "notes", projet: "project", audio: "audio", auto: "auto" };
  const srcLabel = (src: string) => t(`song.src.${SRC[src] ?? "auto"}`);
  const fmt = (v: string) => (kind === "tempo" ? t("common.bpm", { n: v }) : v);
  const pop = $derived(!!detected && !!(detected.shown || detected.alts.length));
  // the bubble opens toward the room left: to the left for a field in the right half
  let flip = $state(false);
  const place = (e: Event) => (flip = (e.currentTarget as HTMLElement).getBoundingClientRect().left > innerWidth * 0.55);
  let hi = $state(-1);
  const choices = $derived.by(() => {
    if (!options.length || !editing) return [];
    const q = draft.trim().toLowerCase();
    const all = q && draft.trim() !== shown ? options.filter((o) => o.toLowerCase().includes(q)) : options;
    return all.filter((o) => o !== draft.trim()).slice(0, 14);
  });
  function pick(o: string) {
    draft = o;
    done(true);
  }
  let editing = $state(false);
  let draft = $state("");
  let input = $state<HTMLInputElement>();
  const shown = $derived(value === null || value === undefined || value === "" ? "" : String(value));

  function start() {
    draft = shown;
    editing = true;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  }
  function done(keep: boolean) {
    if (!editing) return;
    editing = false;
    if (keep && draft.trim() !== shown) save(draft.trim());
  }
</script>

{#if editing}
  <span class="wrap">
  <input
    bind:this={input}
    bind:value={draft}
    class="ed"
    class:big
    type={kind === "date" ? "date" : "text"}
    inputmode={kind === "tempo" ? "decimal" : undefined}
    style:width
    {placeholder}
    onblur={() => done(true)}
    oninput={() => (hi = -1)}
    onkeydown={(e) => {
      if (e.key === "ArrowDown" && choices.length) { e.preventDefault(); hi = (hi + 1) % choices.length; return; }
      if (e.key === "ArrowUp" && choices.length) { e.preventDefault(); hi = (hi - 1 + choices.length) % choices.length; return; }
      if (e.key === "Enter") { if (hi >= 0 && choices[hi]) pick(choices[hi]); else done(true); }
      if (e.key === "Escape") done(false);
    }}
  />
  {#if choices.length}
    <span class="opts" role="listbox">
      {#each choices as o, i (o)}<button role="option" aria-selected={i === hi} class:hi={i === hi} onmousedown={(e) => { e.preventDefault(); pick(o); }}>{o}</button>{/each}
    </span>
  {/if}
  </span>
{:else if pop}
  <span class="fld" class:flip onmouseenter={place} onfocusin={place} role="group">
    <button class="val" class:big class:empty={!shown && !auto} class:auto={!shown && !!auto} class:doubt={!shown && detected?.doubt} onclick={start}>
      {shown ? fmt(shown) : auto ? fmt(auto.value) : placeholder}
    </button>
    <span class="pop" role="tooltip">
      <span class="src small">{shown ? t("song.src.yours") : auto ? t("song.src.from", { src: srcLabel(auto.src) }) : t("song.src.none")}</span>
      {#if !shown && detected?.doubt}<span class="small warn">{t("song.src.doubt")}</span>{/if}
      {#if detected?.alts.length}
        <span class="small faint">{t("song.src.others")}</span>
        <span class="alts">
          {#each detected.alts as a (a.value + a.src)}
            <button class="chip" title={t("song.src.pick")} onclick={() => save(a.value)}>{fmt(a.value)} <span class="faint">{srcLabel(a.src)}</span></button>
          {/each}
        </span>
      {/if}
      {#if shown && auto === null && detected?.shown?.src === "noté" && detected.alts.length}
        <button class="link small" onclick={() => save("")}>{t("song.src.back")}</button>
      {/if}
      {#if hint}<span class="why small faint">{hint}</span>{/if}
      <span class="small faint">{t("song.field.edit")}</span>
    </span>
  </span>
{:else}
  <button class="val" class:big class:empty={!shown} onclick={start} title={hint ? t("song.field.clickToEdit", { hint }) : t("song.field.edit")}>
    {shown ? (kind === "tempo" ? t("common.bpm", { n: shown }) : shown) : placeholder}
  </button>
{/if}

<style>
  .val { border: 1px solid transparent; background: none; padding: 1px 6px; margin: 0 -6px; border-radius: 6px; cursor: text; text-align: left; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: border-color .15s, background-color .15s; }
  .val:hover { border-color: var(--line); background: var(--panel); }
  .val.empty { color: var(--faint); }
  /* detected, not written: the value with a small dotted underline (a doubt: amber) */
  .val.auto { text-decoration: underline dotted var(--faint); text-underline-offset: 4px; }
  .val.auto.doubt { text-decoration-color: var(--lost); }
  .fld { position: relative; display: inline-block; max-width: 100%; flex: none; }
  /* the button sits on a negative margin: its own max-width would cut it by its padding */
  .fld > .val { max-width: none; }
  .pop { display: none; position: absolute; top: calc(100% + 6px); left: -8px; z-index: 60; width: max-content; max-width: 320px; flex-direction: column; gap: 6px;
    padding: 10px 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 12px 30px rgba(0, 0, 0, .35); font-size: 13px; font-weight: 400; letter-spacing: 0; }
  /* a bridge over the gap, so the pointer can reach the bubble */
  .pop::before { content: ""; position: absolute; left: 0; right: 0; top: -8px; height: 8px; }
  .fld:hover .pop, .fld:focus-within .pop { display: flex; }
  .fld.flip .pop { left: auto; right: -8px; }
  .pop .alts { display: flex; flex-wrap: wrap; gap: 6px; }
  .pop .chip { border: 1px solid var(--line); background: var(--panel2); border-radius: 999px; padding: 2px 10px; cursor: pointer; color: var(--ink); font-size: 12.5px; transition: border-color .15s, background-color .15s; }
  .pop .chip:hover { border-color: var(--ink); }
  .pop .link { border: 0; background: none; padding: 0; color: var(--ink); text-decoration: underline; cursor: pointer; text-align: left; }
  .pop .warn { color: var(--lost); }
  .pop .why { white-space: pre-line; max-height: 160px; overflow: auto; }
  .ed { border: 1px solid var(--accent); background: var(--panel); border-radius: 6px; padding: 1px 6px; margin: 0 -7px; outline: 0; min-width: 90px; }
  .wrap { position: relative; display: inline-block; }
  .opts { position: absolute; top: calc(100% + 4px); left: -7px; z-index: 60; min-width: 180px; max-width: 320px; max-height: 280px; overflow: auto; display: flex; flex-direction: column;
    padding: 4px; border-radius: 9px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 12px 30px rgba(0, 0, 0, .35); }
  .opts button { border: 0; background: none; text-align: left; padding: 5px 8px; border-radius: 6px; cursor: pointer; color: var(--ink); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .opts button:hover, .opts button.hi { background: var(--accent-bg); }
  .big { font-size: 26px; font-weight: 650; letter-spacing: -0.015em; line-height: 1.2; }
  .ed.big { width: min(560px, 100%); }
</style>

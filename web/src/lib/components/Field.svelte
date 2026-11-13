<script lang="ts">
  // A value shown as text, edited in place: click, type, Enter or leaving saves, Escape cancels.
  import { t } from "../i18n/index.svelte";
  let {
    value,
    placeholder,
    save,
    kind = "text",
    big = false,
    width = "",
    options = [],
    hint = "",
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
  } = $props();
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
{:else}
  <button class="val" class:big class:empty={!shown} onclick={start} title={hint ? t("song.field.clickToEdit", { hint }) : t("song.field.edit")}>
    {shown ? (kind === "tempo" ? t("common.bpm", { n: shown }) : shown) : placeholder}
  </button>
{/if}

<style>
  .val { border: 1px solid transparent; background: none; padding: 1px 6px; margin: 0 -6px; border-radius: 6px; cursor: text; text-align: left; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: border-color .15s, background-color .15s; }
  .val:hover { border-color: var(--line); background: var(--panel); }
  .val.empty { color: var(--faint); }
  .ed { border: 1px solid var(--accent); background: var(--panel); border-radius: 6px; padding: 1px 6px; margin: 0 -7px; outline: 0; min-width: 90px; }
  .wrap { position: relative; display: inline-block; }
  .opts { position: absolute; top: calc(100% + 4px); left: -7px; z-index: 60; min-width: 180px; max-width: 320px; max-height: 280px; overflow: auto; display: flex; flex-direction: column;
    padding: 4px; border-radius: 9px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 12px 30px rgba(0, 0, 0, .35); }
  .opts button { border: 0; background: none; text-align: left; padding: 5px 8px; border-radius: 6px; cursor: pointer; color: var(--ink); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .opts button:hover, .opts button.hi { background: var(--accent-bg); }
  .big { font-size: 26px; font-weight: 650; letter-spacing: -0.015em; line-height: 1.2; }
  .ed.big { width: min(560px, 100%); }
</style>

<script lang="ts">
  // A filter that takes several values (artists, styles, keys...): a button saying what is
  // chosen, opening a list of boxes to tick, with a search field when the list is long.
  // Nothing ticked: every value passes.
  import { t } from "../i18n/index.svelte";

  let {
    options,
    value = $bindable([]),
    all,
    label = (o: string) => o,
  }: { options: string[]; value?: string[]; all: string; label?: (o: string) => string } = $props();

  let open = $state(false);
  let q = $state("");
  let box = $state<HTMLDivElement>();
  let search = $state<HTMLInputElement>();
  const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const shown = $derived(q.trim() ? options.filter((o) => norm(label(o)).includes(norm(q.trim()))) : options);
  const summary = $derived(!value.length ? all : value.length <= 2 ? value.map(label).join(", ") : t("songs.filter.nChosen", { first: label(value[0]), n: value.length - 1 }));

  function flip(o: string) {
    value = value.includes(o) ? value.filter((x) => x !== o) : [...value, o];
  }
  $effect(() => {
    if (!open) return;
    q = "";
    queueMicrotask(() => search?.focus());
    const away = (e: PointerEvent) => {
      if (box && e.target instanceof Node && !box.contains(e.target)) open = false;
    };
    // Escape closes this list only, not the panel holding it
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      e.preventDefault();
      open = false;
    };
    addEventListener("pointerdown", away, true);
    addEventListener("keydown", key, true);
    return () => {
      removeEventListener("pointerdown", away, true);
      removeEventListener("keydown", key, true);
    };
  });
</script>

<div class="mp" bind:this={box}>
  <button type="button" class="btn-pick" class:set={value.length > 0} aria-expanded={open} onclick={() => (open = !open)}>
    <span>{summary}</span>
    <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 3.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
  </button>
  {#if open}
    <div class="list" role="listbox" aria-multiselectable="true">
      {#if options.length > 8}<input class="q" bind:this={search} bind:value={q} placeholder={t("songs.filter.searchIn")} />{/if}
      {#if value.length}<button type="button" class="clear small" onclick={() => (value = [])}>{t("songs.filter.clearPick")}</button>{/if}
      <div class="opts">
        {#each shown as o (o)}
          <label class="opt" class:on={value.includes(o)}><input type="checkbox" checked={value.includes(o)} onchange={() => flip(o)} /> <span>{label(o)}</span></label>
        {:else}<p class="small faint">{t("songs.filter.noMatch")}</p>{/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .mp { position: relative; width: 100%; }
  .btn-pick { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 6px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); border-radius: 7px; padding: 4px 8px; font: inherit; font-size: 13px; cursor: pointer; text-align: left; }
  .btn-pick span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .btn-pick.set { color: var(--ink); border-color: var(--accent); background: var(--accent-bg); }
  .btn-pick svg { flex: none; }
  .list { position: absolute; top: calc(100% + 4px); left: 0; right: 0; min-width: 200px; z-index: 50; padding: 6px; border: 1px solid var(--line); border-radius: 10px; background: var(--panel); box-shadow: 0 12px 28px rgba(0, 0, 0, .28); display: flex; flex-direction: column; gap: 4px; }
  .q { width: 100%; box-sizing: border-box; border: 1px solid var(--line); background: var(--panel2); color: var(--ink); border-radius: 6px; padding: 4px 7px; font: inherit; font-size: 12.5px; outline: 0; }
  .q:focus { border-color: var(--accent); }
  .clear { align-self: flex-start; border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0 2px; font: inherit; }
  .opts { max-height: 240px; overflow: auto; display: flex; flex-direction: column; }
  .opt { display: flex; align-items: center; gap: 7px; padding: 3px 6px; border-radius: 6px; font-size: 13px; cursor: pointer; }
  .opt:hover { background: var(--panel2); }
  .opt.on { color: var(--ink); font-weight: 600; }
  .opt span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .opts p { margin: 4px 6px; }
</style>

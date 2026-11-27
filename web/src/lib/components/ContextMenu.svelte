<script lang="ts">
  // Draws the right-click menu (lib/menu.svelte.ts), kept inside the window, with one level
  // of sub-menus and keyboard use (arrows, Enter, Escape).
  import { menu, closeMenu, type MenuItem } from "../menu.svelte";
  import { t } from "../i18n/index.svelte";

  let box = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });
  let subOf = $state<number | null>(null);
  let sel = $state(-1);
  $effect(() => {
    if (!menu.open) return;
    subOf = null;
    sel = -1;
    pos = { x: menu.x, y: menu.y };
    queueMicrotask(() => {
      if (!box) return;
      const r = box.getBoundingClientRect();
      pos = { x: Math.min(menu.x, innerWidth - r.width - 8), y: Math.min(menu.y, innerHeight - r.height - 8) };
      box.focus();
    });
  });
  const items = $derived(menu.items);
  const usable = (it: MenuItem) => !("sep" in it) && !it.disabled;
  function run(it: MenuItem, i: number) {
    if ("sep" in it || it.disabled) return;
    if (it.sub) {
      subOf = subOf === i ? null : i;
      return;
    }
    closeMenu();
    it.run?.();
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (subOf !== null) subOf = null;
      else closeMenu();
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      for (let k = 0, i = sel; k < items.length; k++) {
        i = (i + d + items.length) % items.length;
        if (usable(items[i])) {
          sel = i;
          break;
        }
      }
    } else if ((e.key === "Enter" || e.key === "ArrowRight") && sel >= 0) {
      e.preventDefault();
      run(items[sel], sel);
    } else if (e.key === "ArrowLeft") subOf = null;
  }
</script>

<svelte:window onblur={closeMenu} onresize={closeMenu} />
{#if menu.open}
  <div class="catch" role="presentation" onmousedown={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}></div>
  <div class="menu" bind:this={box} role="menu" tabindex="-1" aria-label={menu.title || t("menu.menu")} style:left="{pos.x}px" style:top="{pos.y}px" onkeydown={key}>
    {#if menu.title}<div class="mt">{menu.title}</div>{/if}
    {#each items as it, i}
      {#if "sep" in it}<div class="sep" role="separator"></div>
      {:else}
        <div class="wrap">
          <button role="menuitem" class="mi" class:danger={it.danger} class:hi={sel === i || subOf === i} disabled={it.disabled}
            onclick={() => run(it, i)} onmouseenter={() => { sel = i; if (it.sub) subOf = i; else subOf = null; }}>
            <span class="ck">{it.checked ? "✓" : ""}</span><span class="l">{it.label}</span>
            {#if it.sub}<span class="ar">›</span>{:else if it.hint}<span class="hint">{it.hint}</span>{/if}
          </button>
          {#if it.sub && subOf === i}
            <div class="menu sub" role="menu" class:left={pos.x > innerWidth - 460}>
              {#each it.sub as s}
                {#if "sep" in s}<div class="sep" role="separator"></div>
                {:else}<button role="menuitem" class="mi" class:danger={s.danger} disabled={s.disabled} onclick={() => { closeMenu(); s.run?.(); }}>
                  <span class="ck">{s.checked ? "✓" : ""}</span><span class="l">{s.label}</span>{#if s.hint}<span class="hint">{s.hint}</span>{/if}</button>{/if}
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .catch { position: fixed; inset: 0; z-index: 300; }
  .menu { position: fixed; z-index: 301; min-width: 220px; max-width: 320px; padding: 5px; border-radius: 10px; background: var(--panel); border: 1px solid var(--line);
    box-shadow: 0 16px 40px rgba(0, 0, 0, .4); outline: 0; font-size: 13px; animation: pop .1s ease-out; }
  .menu.sub { position: absolute; left: calc(100% + 4px); top: -6px; }
  .menu.sub.left { left: auto; right: calc(100% + 4px); }
  .wrap { position: relative; }
  .mt { padding: 5px 9px 6px; font-size: 11.5px; color: var(--faint); font-weight: 600; max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mi { display: flex; align-items: center; gap: 6px; width: 100%; padding: 6px 9px 6px 4px; border: 0; background: none; border-radius: 7px; color: var(--ink); cursor: pointer; text-align: left; font: inherit; }
  .mi.hi, .mi:hover:not(:disabled) { background: var(--accent-bg); }
  .mi:disabled { color: var(--faint); cursor: default; }
  .mi.danger { color: var(--bad); }
  .ck { width: 16px; text-align: center; color: var(--accent); flex: none; }
  .l { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ar, .hint { color: var(--faint); font-size: 11.5px; }
  .sep { height: 1px; background: var(--line); margin: 4px 6px; }
  @keyframes pop { from { opacity: 0; transform: scale(.98); } }
  @media (prefers-reduced-motion: reduce) { .menu { animation: none; } }
</style>

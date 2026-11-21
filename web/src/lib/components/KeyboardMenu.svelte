<script lang="ts">
  // In the window's header: the MIDI keyboard, set up where it is played. Its input, its
  // sound and its volume, and the notes it plays now (to check it answers). A dot on the
  // button says a keyboard is plugged in and chosen. Settings keeps the same choices.
  import { keyboard, KEY_SOUNDS } from "../keyboard.svelte";
  import { noteLabel } from "../theory";
  import { i18n, t } from "../i18n/index.svelte";

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);
  const bound = $derived(!!keyboard.input && keyboard.inputs.some((i) => i.id === keyboard.input));
  // the inputs are asked for when the panel opens (the system lists them then)
  function toggle() {
    open = !open;
    if (open) void keyboard.start();
  }
  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  const notes = $derived([...keyboard.down].sort((a, b) => a - b).map((n) => noteLabel(n, i18n.lang === "fr")).join(" "));
</script>

<svelte:window onpointerdown={outside} onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="kb" bind:this={root}>
  <button class="ib" class:on={open} aria-expanded={open} aria-haspopup="dialog" onclick={toggle}
    title={bound ? t("app.keyboard.tipOn", { sound: t(`settings.keyboard.sounds.${keyboard.sound}`) }) : t("app.keyboard.tip")} aria-label={t("app.keyboard.title")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"><rect x="2.5" y="6" width="19" height="12" rx="2" /><path d="M7 6v7M12 6v7M17 6v7" /><path d="M5.5 6v4.5h3V6M10.5 6v4.5h3V6M15.5 6v4.5h3V6" fill="currentColor" stroke="none" /></svg>
    {#if bound}<i class="dot" class:hit={keyboard.down.length > 0}></i>{/if}
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label={t("app.keyboard.title")}>
      <h3>{t("app.keyboard.title")}</h3>
      {#if !keyboard.supported}<p class="small warn">{t("settings.keyboard.unsupported")}</p>
      {:else}
        <label class="row"><span class="small muted">{t("settings.keyboard.input")}</span>
          <select value={keyboard.input} onchange={(e) => keyboard.setInput(e.currentTarget.value)} disabled={!keyboard.inputs.length}>
            <option value="">{t("settings.keyboard.off")}</option>
            {#each keyboard.inputs as i (i.id)}<option value={i.id}>{i.name}</option>{/each}
          </select>
        </label>
        {#if !keyboard.inputs.length}<p class="small faint">{t("settings.keyboard.none")}</p>{/if}
        <div class="small muted lab">{t("settings.keyboard.sound")}</div>
        <div class="sounds" role="radiogroup" aria-label={t("settings.keyboard.sound")}>
          {#each KEY_SOUNDS as k (k)}
            <button role="radio" aria-checked={keyboard.sound === k} class:sel={keyboard.sound === k} onclick={() => keyboard.setSound(k)}>{t(`settings.keyboard.sounds.${k}`)}</button>
          {/each}
        </div>
        <label class="row"><span class="small muted">{t("settings.keyboard.volume")}</span>
          <input type="range" min="0" max="1" step="0.01" value={keyboard.volume} oninput={(e) => keyboard.setVolume(+e.currentTarget.value)} />
        </label>
        {#if bound}<p class="small state">{notes ? t("settings.keyboard.playing", { notes }) : t("settings.keyboard.ready")}</p>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .kb { position: relative; flex: none; }
  .ib { position: relative; width: 34px; height: 34px; border: 0; border-radius: 8px; background: none; color: var(--muted); display: grid; place-items: center; cursor: pointer; }
  .ib svg { width: 20px; height: 20px; }
  .ib:hover, .ib.on { background: var(--panel2); color: var(--ink); }
  .dot { position: absolute; top: 6px; right: 5px; width: 7px; height: 7px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 0 2px var(--bg, var(--panel)); transition: transform .1s; }
  .dot.hit { transform: scale(1.4); background: var(--accent); }
  .pop { position: absolute; right: 0; top: calc(100% + 8px); z-index: 1500; width: 300px; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 14px 40px rgba(0, 0, 0, .28); display: flex; flex-direction: column; gap: 10px; }
  h3 { margin: 0; font-size: 14px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .row select { max-width: 180px; }
  .row input[type="range"] { flex: 1; max-width: 180px; }
  .lab { margin-bottom: -4px; }
  .sounds { display: grid; grid-template-columns: repeat(2, 1fr); gap: 6px; }
  .sounds button { border: 1px solid var(--line); border-radius: 8px; background: none; color: var(--ink); padding: 6px 8px; font: inherit; font-size: 12.5px; cursor: pointer; text-align: left; }
  .sounds button:hover { border-color: var(--faint); }
  .sounds button.sel { border-color: var(--accent); background: var(--accent-bg); font-weight: 600; }
  .state { margin: 0; color: var(--ok); }
  .warn { color: var(--fix); margin: 0; }
  @media (prefers-reduced-motion: reduce) { .dot { transition: none; } }
</style>

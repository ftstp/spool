<script lang="ts">
  // A song's finishing checks. The app's own ones tick themselves when it sees them done (a
  // title and an artist, a cover, lyrics in the mood board); the user adds others by hand, for
  // this song or as a preset: the default preset shows on every song, the others are applied
  // to a song in one click. Each change is one Ctrl+Z.
  import * as api from "../api";
  import type { Check } from "../api";
  import { app } from "../state.svelte";
  import { openMenu, type MenuItem } from "../menu.svelte";
  import FoldHead from "./FoldHead.svelte";
  import { t } from "../i18n/index.svelte";

  let { sid, list }: { sid: number; list: Check[] } = $props();

  const done = $derived(list.filter((c) => c.done).length);
  const presets = $derived(app.config?.check_presets?.presets ?? {});
  const def = $derived(app.config?.check_presets?.default ?? null);
  const defLabels = $derived(def ? (presets[def] ?? []) : []);
  const set = (key: string, v: boolean | "na" | "off" | null) => app.act(api.checkSet(sid, key, v));

  function cycle(c: Check) {
    // not done -> done -> back to what the tool sees (or not done when the tool saw it done)
    const next = c.na ? null : !c.done ? true : c.custom ? false : c.manual ? (c.auto ? false : null) : false;
    set(c.key, next);
  }
  // one of the default preset leaves this song only ("off"); one of its own goes
  const remove = (c: Check) => set(c.key, defLabels.includes(c.label) ? "off" : null);

  let adding = $state("");
  async function add() {
    const label = adding.trim().replace(/\s+/g, " ").slice(0, 80);
    if (!label) return;
    adding = "";
    if (list.some((c) => c.label.toLowerCase() === label.toLowerCase())) return app.say(t("song.checks.already", { label }));
    await set("x:" + label, false);
  }

  // ---- the presets: the checks this song has, saved under a name
  let naming = $state(false);
  let presetName = $state("");
  const mine = $derived(list.filter((c) => c.custom).map((c) => c.label));
  async function save(cp: { presets: Record<string, string[]>; default: string | null }) {
    try {
      app.config = await api.setConfig("check_presets", cp);
      await app.reload();
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function savePreset() {
    const name = presetName.trim().slice(0, 40);
    naming = false;
    presetName = "";
    if (!name || !mine.length) return;
    await save({ presets: { ...presets, [name]: mine }, default: def });
    app.say(t("song.checks.presetSaved", { name, n: mine.length }));
  }
  async function apply(name: string) {
    for (const label of presets[name] ?? []) if (!list.some((c) => c.label === label)) await set("x:" + label, false);
  }
  function presetMenu(e: MouseEvent) {
    const names = Object.keys(presets).sort();
    const items: MenuItem[] = [
      { label: t("song.checks.saveAs"), disabled: !mine.length, hint: mine.length ? undefined : t("song.checks.saveAsNone"), run: () => (naming = true) },
    ];
    if (names.length) {
      items.push({ sep: true });
      for (const n of names) items.push({ label: t("song.checks.apply", { name: n, n: presets[n].length }), run: () => apply(n) });
      items.push({ sep: true });
      items.push({
        label: t("song.checks.default"),
        sub: [
          { label: t("song.checks.defaultNone"), checked: !def, run: () => save({ presets, default: null }) },
          ...names.map((n) => ({ label: n, checked: def === n, run: () => save({ presets, default: n }) })),
        ],
      });
      items.push({
        label: t("song.checks.delete"),
        sub: names.map((n) => ({ label: n, danger: true, run: () => { const p = { ...presets }; delete p[n]; save({ presets: p, default: def === n ? null : def }); } })),
      });
    }
    openMenu(e, items, t("song.checks.presets"));
  }
</script>

<section class="card" data-sec="checks">
  <FoldHead {sid} key="checks" title={t("song.checks.title")}>
    <span class="small faint">{done}/{list.length}</span>
    {#snippet actions()}<button class="btn sm" title={t("song.checks.presetsTip")} onclick={presetMenu}>{t("song.checks.presets")}{#if def}<span class="faint"> · {def}</span>{/if}</button>{/snippet}
  </FoldHead>
  {#each list as c (c.key)}
    <div class="check" class:na={c.na}>
      <button class="box" class:on={c.done && !c.na} role="checkbox" aria-checked={c.done} aria-label={c.label} onclick={() => cycle(c)}>
        {#if c.done && !c.na}<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="3"><path d="M5 12l5 5 9-10" /></svg>{/if}
      </button>
      <div class="grow">
        <div>{c.label}{#if c.manual && !c.na && !c.custom} <span class="faint small">{t("song.checks.manual")}</span>{/if}</div>
        {#if c.why && !c.na}<div class="small faint ell" title={c.why}>{c.why}</div>{/if}
        {#if c.offer}
          <div class="small offer">{t("song.checks.lyricsOffer", { name: c.offer })}
            <button class="link" onclick={() => set("lyrics", true)}>{t("song.checks.lyricsYes")}</button></div>
        {/if}
      </div>
      {#if c.custom}
        <button class="x" title={t("song.checks.remove")} aria-label={t("song.checks.removeName", { label: c.label })} onclick={() => remove(c)}>×</button>
      {:else}
        <button class="link small" title={c.na ? t("song.checks.appliesTip") : t("song.checks.naTip")} onclick={() => set(c.key, c.na ? null : "na")}>{c.na ? t("song.checks.applies") : t("song.checks.na")}</button>
      {/if}
    </div>
  {/each}
  <div class="add">
    <input bind:value={adding} placeholder={t("song.checks.addPh")} maxlength="80" onkeydown={(e) => e.key === "Enter" && add()} />
    <button class="btn sm" disabled={!adding.trim()} onclick={add}>{t("song.checks.add")}</button>
  </div>
  {#if naming}
    <div class="add">
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={presetName} placeholder={t("song.checks.presetPh")} maxlength="40" autofocus
        onkeydown={(e) => { if (e.key === "Enter") savePreset(); if (e.key === "Escape") naming = false; }} />
      <button class="btn sm primary" disabled={!presetName.trim()} onclick={savePreset}>{t("song.checks.presetSave")}</button>
      <button class="btn sm" onclick={() => (naming = false)}>{t("common.cancel")}</button>
    </div>
  {/if}
</section>

<style>
  .check { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--line); }
  .check.na { opacity: .5; }
  .check.na .grow > div:first-child { text-decoration: line-through; }
  .grow { flex: 1; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .box { width: 18px; height: 18px; flex: none; border-radius: 5px; border: 1.5px solid var(--faint); background: none; cursor: pointer; display: grid; place-items: center; color: #111; padding: 0; transition: background-color .15s, border-color .15s; }
  .box.on { background: var(--ok); border-color: var(--ok); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; font: inherit; }
  .link:hover { text-decoration: underline; }
  .offer { color: var(--muted); margin-top: 2px; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 16px; padding: 0 4px; opacity: .5; }
  .check:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--ink); }
  .add { display: flex; gap: 6px; margin-top: 8px; }
  .add input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--panel2); color: var(--ink); border-radius: 7px; padding: 4px 8px; font: inherit; font-size: 13px; }
  .add input:focus { outline: 0; border-color: var(--accent); }
  @media (prefers-reduced-motion: reduce) { .box { transition: none; } }
</style>

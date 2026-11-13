<script lang="ts">
  // One plugin, everything known about it: what to do, which version to install and where it
  // came from, what is on this PC and on the old Windows, the presets it was loaded with, and
  // the songs using it (with their preset). A panel over the right side of Repair.
  import { statusSeverity } from "../severity";
  import * as api from "../api";
  import type { PluginNeed } from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { p, ignored, kept, titles, close }:
    { p: PluginNeed; ignored: boolean; kept: boolean; titles: Record<string, string | null>; close: () => void } = $props();

  type Cand = { name: string; format: string; version?: string };
  const FMT = (f: string) => (f === "au" ? "AU" : f.toUpperCase());
  const ver = (v?: string) => (v && v !== "1" ? " v" + v : "");
  const candText = (c: Cand) => `${c.name} (${FMT(c.format)}${ver(c.version)})`;
  const needText = $derived(
    p.needs.length ? `v${p.needs[0][0]}${p.needs.length > 1 ? t("repair.sheet.needAlso", { list: p.needs.slice(1).map((n) => "v" + n[0]).join(", ") }) : ""}` : "",
  );
  const STATUSES = ["renamed", "trial", "vst3", "other_vst2", "newer", "to_vst3", "other_vst3", "missing", "ok"];
  const statusText = (s: string) => (STATUSES.includes(s) ? t(`repair.sheet.status.${s}`) : s);
  const cands = $derived(p.candidates as Cand[]);
  // Which version to install. Live finds a plugin by its identifier, which a vendor keeps
  // along a series (Ozone 6.x) and changes with a new product (Ozone 12): any version of the
  // project's series opens it, from the one it was made with.
  const series = $derived.by(() => {
    const v = p.needs[0]?.[0] ?? "";
    const major = /^(\d+)[.\d]*$/.exec(v)?.[1];
    if (!major) return "";
    return t("repair.sheet.series", { major, v });
  });
  // what to do, in one line, then why (the prototype's wording)
  const todo = $derived.by((): [string, string] => {
    const fmt = FMT(p.format), c = cands[0];
    const both = p.old_formats.includes("vst2") && p.old_formats.includes("vst3");
    const need = needText ? ", " + needText.split(" (")[0] : "";
    const T = (k: string, vars?: Record<string, string | number | null | undefined>) => t(`repair.sheet.todo.${k}`, vars);
    if (p.status === "trial" && p.format === "au") return [T("nothing"), T("trialAu", { trial: p.trial })];
    if (p.status === "trial") return [T("nothing"), T("trial", { trial: p.trial })];
    if (p.status === "vst3") {
      const x = p.vst3_targets?.find((y) => y.uid === p.vst3);
      return [T("nothing"), T(x?.compatible ? "vst3Safe" : "vst3Listen", { name: x ? x.name : T("itsVst3") })];
    }
    if (p.status === "renamed" || p.status === "other_vst2") return [T("nothing"), T("renamed", { name: c?.name })];
    if (/portable/i.test(p.name)) return [T("portable", { fmt }), T("portableWhy", { path: p.old_path || T("portableFolder") })];
    if (p.status === "newer") return [T("install", { name: p.name, fmt, need: series || need }), T("newerWhy", { have: c ? candText(c) : T("anotherVersion") })];
    if (p.status === "to_vst3") return [T("addFormat", { fmt, need: series || need }), p.vst3_targets?.length ? T("toVst3Try") : T("toVst3")];
    if (p.status === "other_vst3") return [T("install", { name: p.name, fmt, need: "" }), c ? T("here", { have: candText(c) }) : ""];
    if (p.format === "au") return [T("au"), T("auWhy")];
    return [T("installIn", { fmt, need: series || need }), both && p.format === "vst2" ? T("bothFormats") : p.old_formats.length ? "" : T("elsewhere")];
  });
  async function act(x: Promise<unknown>, ok: string) {
    try {
      await app.track(x);
      app.say(ok);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />
<div class="scrim" role="presentation" onclick={close}></div>
<aside class="sheet" aria-label={t("repair.sheet.aria", { name: p.name })}>
  <header>
    <div class="grow">
      <h2>{p.name} <span class="fmt">{FMT(p.format)}</span></h2>
      <p class="small muted">{p.vendor || t("repair.sheet.unknownVendor")} · <span class="pill s{statusSeverity(p.status)}">{statusText(p.status)}</span>{ignored ? t("repair.sheet.ignoredTag") : ""}</p>
    </div>
    <button class="x" onclick={close} aria-label={t("common.close")}>✕</button>
  </header>

  <section class="card">
    <h3>{t("repair.sheet.toDo")}</h3>
    <b>{todo[0]}</b>
    {#if todo[1]}<p class="small muted">{todo[1]}</p>{/if}
    {#if ignored}
      <p class="small">{t("repair.sheet.ignoredIn")} <b>{kept ? t("repair.sheet.keptManual") : t("repair.sheet.removed")}</b>.</p>
      <div class="row">
        <button class="btn sm" onclick={() => act(api.setIgnored(p.name, true, !kept), kept ? t("repair.sheet.toastRemoved") : t("repair.sheet.toastKept"))}>{kept ? t("repair.sheet.removeFromCopies") : t("repair.sheet.keepToReplace")}</button>
        <button class="btn sm" onclick={() => act(api.setIgnored(p.name, false), t("repair.ignored.countsAgain", { name: p.name })).then(close)}>{t("repair.ignored.unignore")}</button>
      </div>
    {:else}
      <h4>{t("repair.sheet.noLongerUsed")}</h4>
      <div class="row">
        <button class="btn sm" title={t("repair.sheet.removeTitle")} onclick={() => act(api.setIgnored(p.name, true, false), t("repair.sheet.toastIgnoredRemoved")).then(close)}>{t("repair.sheet.removeFromCopies")}</button>
        <button class="btn sm" title={t("repair.sheet.keepTitle")} onclick={() => act(api.setIgnored(p.name, true, true), t("repair.sheet.toastIgnoredKept")).then(close)}>{t("repair.sheet.keepShort")}</button>
      </div>
    {/if}
  </section>

  <section class="card">
    <h3>{t("repair.sheet.versions")}</h3>
    <dl>
      <dt>{p.status === "trial" ? t("repair.sheet.origVersion") : t("repair.sheet.toInstall")}</dt>
      <dd>{p.name} {FMT(p.format)}{#if needText} <span class="okc">{needText}</span>{/if}
        <div class="small muted">
          {#if p.needs.length}{t("repair.sheet.versionSource", { src: p.need_source })}{:else}<b>{p.need_label}</b>{#if p.any}{t("repair.sheet.anyUpdate")}{/if}{/if}
          {#if p.bundles.length} {t("repair.sheet.oldBundles")} <b>{(p.bundles as string[]).join(", ")}</b>.{/if}
          {#if p.filenames.length}{t("repair.sheet.expectedFile", { files: p.filenames.join(", ") })}{/if}
        </div>
      </dd>
      <dt>{t("repair.sheet.onThisPc")}</dt>
      <dd>{#if cands.length}{#each cands as c}<div>{candText(c)}</div>{/each}{:else}{t("repair.sheet.nothing")}{/if}</dd>
      <dt>{t("repair.sheet.oldWindows")}</dt>
      <dd>{p.old_path ? p.old_path + (p.old_version && p.old_version !== "1" ? ` (${p.old_version})` : "") + (p.on_disk ? t("repair.sheet.stillOnDisk") : "") : t("repair.sheet.unknown")}</dd>
      <dt>{t("repair.sheet.formatsBefore")}</dt>
      <dd>{(p.old_formats as string[]).map(FMT).join(", ") || "-"}</dd>
      <dt>{t("repair.sheet.instances")}</dt>
      <dd>{t("repair.sheet.instancesIn", { count: p.instances, n: p.files })}</dd>
    </dl>
  </section>

  {#if !ignored && p.trial_targets?.length}
    <section class="card">
      <h3>{p.trial ? t("repair.sheet.trialActiveTitle") : t("repair.sheet.trialTryTitle")}</h3>
      <p class="small muted">
        {#if p.format === "au"}{t("repair.sheet.trialAu", { name: p.name })}
        {:else}{t("repair.sheet.trialText", { name: p.name })}{/if}
        {t("repair.sheet.trialNote")}
      </p>
      {#if p.trial}<p class="small">{t("repair.sheet.trialActive")} <b>{p.trial}</b>.</p>{/if}
      <div class="row">
        {#each p.trial_targets.filter((x) => x !== p.trial) as tt}
          <button class="btn sm" class:primary={!p.trial} onclick={() => act(api.setTrial(p.name, p.format, tt), t("repair.sheet.trialSet", { name: tt }))}>{t("repair.sheet.tryWith", { name: tt })}</button>
        {/each}
        {#if p.trial}<button class="btn sm" onclick={() => act(api.setTrial(p.name, p.format, null), t("repair.sheet.trialUnset"))}>{t("repair.sheet.stopReplacing")}</button>{/if}
      </div>
    </section>
  {/if}

  {#if !ignored && p.vst3_targets?.length}
    <section class="card">
      <h3>{t("repair.sheet.vst3Title")}</h3>
      <p class="small muted">{t("repair.sheet.vst3Text", { name: p.name })}</p>
      {#if p.automated}<p class="small warn">{t("repair.sheet.vst3Automated", { n: p.automated })}</p>{/if}
      {#each p.vst3_targets as v3 (v3.uid)}
        <div class="v3">
          <div class="grow">
            <b>{v3.name}</b> <span class="fmt">VST3</span>
            <div class="small okc">{t("repair.sheet.vst3Safe")}</div>
          </div>
          {#if p.vst3 === v3.uid}
            <span class="small okc">{t("repair.sheet.active")}</span>
            <button class="btn sm" onclick={() => act(api.setVst3(p.name, null), t("repair.sheet.vst3Unset"))}>{t("repair.sheet.stopVst3")}</button>
          {:else}
            <button class="btn sm primary" onclick={() => act(api.setVst3(p.name, v3.uid), t("repair.sheet.vst3Set", { name: p.name, to: v3.name }))}>{t("repair.sheet.useVst3")}</button>
          {/if}
        </div>
      {/each}
    </section>
  {/if}

  {#if p.presets.length}
    <section class="card">
      <h3>{t("repair.sheet.presetsTitle")} <span class="faint">{p.presets.length}</span></h3>
      <p class="small muted">{t("repair.sheet.presetsText")}</p>
      <div class="presets">{#each p.presets as [n, c]}<div class="small"><b>{n}</b> <span class="faint">{t("repair.sheet.times", { n: c })}</span></div>{/each}</div>
    </section>
  {/if}

  <section class="card">
    <h3>{t("repair.sheet.songsTitle")} <span class="faint">{p.n}</span></h3>
    <div class="songs">
      {#each p.songs as id}
        <a class="srow" href="#/song/{id}" onclick={close}>
          <span class="ell">{titles[id] ?? "#" + id}</span>
          <span class="small faint ell">{(p.song_presets[id] ?? []).join(", ") || t("repair.sheet.unreadablePreset")}</span>
        </a>
      {/each}
    </div>
  </section>
</aside>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .35); z-index: 60; animation: fade .15s ease-out; }
  .sheet { position: fixed; top: 0; right: 0; bottom: 0; width: min(560px, 100vw); background: var(--bg); border-left: 1px solid var(--line); z-index: 61;
    overflow: auto; padding: 18px 20px 40px; display: flex; flex-direction: column; gap: 12px; animation: slide .2s ease-out; box-shadow: -20px 0 50px rgba(0, 0, 0, .3); }
  header { display: flex; gap: 12px; align-items: flex-start; }
  .grow { flex: 1; min-width: 0; }
  h2 { font-size: 18px; margin: 0 0 4px; }
  h3 { font-size: 13px; margin: 0 0 8px; }
  h4 { font-size: 12px; margin: 12px 0 6px; color: var(--muted); }
  .x { border: 0; background: none; color: var(--muted); cursor: pointer; font-size: 16px; padding: 4px 8px; border-radius: 6px; }
  .x:hover { background: var(--panel); color: var(--ink); }
  .fmt { font-size: 11px; padding: 0 6px; border-radius: 5px; border: 1px solid var(--line); color: var(--muted); font-weight: 500; vertical-align: middle; }
  .pill { padding: 0 7px; border-radius: 99px; font-size: 11.5px; }
  .pill.s1 { color: var(--fix); background: color-mix(in srgb, var(--fix) 14%, transparent); }
  .pill.s2 { color: var(--lost); background: color-mix(in srgb, var(--lost) 14%, transparent); }
  .pill.s3 { color: var(--bad); background: color-mix(in srgb, var(--bad) 14%, transparent); }
  .row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  p { margin: 6px 0 0; }
  dl { display: grid; grid-template-columns: 9.5em 1fr; gap: 8px 12px; margin: 0; font-size: 12.5px; }
  dt { color: var(--muted); }
  dd { margin: 0; font-weight: 500; min-width: 0; overflow-wrap: anywhere; }
  .okc { color: var(--ok); }
  .v3 { display: flex; align-items: center; gap: 10px; padding: 8px 0; border-top: 1px solid var(--line); margin-top: 8px; }
  .warn { color: var(--lost); }
  .presets { max-height: 200px; overflow: auto; display: flex; flex-direction: column; gap: 2px; margin-top: 8px; }
  .songs { display: flex; flex-direction: column; }
  .srow { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 10px; padding: 6px 4px; border-bottom: 1px solid var(--line); color: var(--ink); }
  .srow:last-child { border-bottom: 0; }
  .srow:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @keyframes slide { from { transform: translateX(30px); opacity: 0; } }
  @keyframes fade { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .sheet, .scrim { animation: none; } }
</style>

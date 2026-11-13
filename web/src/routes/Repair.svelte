<script lang="ts">
  // What is missing across the active projects and what to do about it: the copies the app
  // writes by itself, what to install first, the plugins set aside, the lost samples.
  import { onDestroy } from "svelte";
  import * as api from "../lib/api";
  import type { PluginNeed, RepairState, RepairView } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { stageLabel, STAGES } from "../lib/format";
  import { t, compare, fmtDate } from "../lib/i18n/index.svelte";
  import type { Preset } from "../lib/api";
  import StackBar from "../lib/charts/StackBar.svelte";
  import Lines from "../lib/charts/Lines.svelte";
  import BarList from "../lib/charts/BarList.svelte";
  import PluginSheet from "../lib/components/PluginSheet.svelte";
  import { focus } from "../lib/focus.svelte";
  import Atelier from "./Atelier.svelte";
  import WrittenCopies from "../lib/components/WrittenCopies.svelte";
  import { statusSeverity } from "../lib/severity";

  type Tab = "fix" | "atelier" | "install" | "ignored" | "samples" | "copies";
  /** `initial`: the tab asked by the address (#/atelier opens the treatments). */
  let { initial }: { initial?: Tab } = $props();
  let tab = $state<Tab>((() => {
    if (initial) return initial;
    try {
      return (localStorage.getItem("pf:repairTab") as Tab) || "fix";
    } catch {
      return "fix";
    }
  })());
  $effect(() => {
    try {
      localStorage.setItem("pf:repairTab", tab);
    } catch {
      /* not remembered */
    }
  });
  let years = $state<number | null>(null);
  let v = $state<RepairView | null>(null);
  let picked = $state<Set<string>>(new Set());
  let onlyComplete = $state(false);
  let run = $state<RepairState | null>(null);
  let q = $state("");
  let timer = 0;
  // the plugin whose sheet is open
  let open = $state<string | null>(null);
  // asked from a song's sheet: that plugin's sheet, on its tab
  if (focus.plugin) {
    open = focus.plugin;
    tab = "install";
    focus.plugin = "";
  }
  // "To install" filters: severity class, format, vendor, stage of the songs it blocks
  let sevF = $state<number | null>(null);
  let fmtF = $state("");
  let vendorF = $state("");
  let stageF = $state("");
  // "To repair": a treatments preset applied in the same copy
  let presets = $state<Preset[]>([]);
  let presetName = $state("");
  api.atelierPresets().then((p) => (presets = p)).catch(() => {});

  let seq = 0;
  $effect(() => {
    void app.version; // fetched again after every change
    const y = years;
    const n = ++seq;
    app.track(api.repairView(y ?? undefined)).then((r) => {
      if (years === y && n === seq) v = r;
    });
  });
  async function poll() {
    run = await api.repairState();
    clearTimeout(timer);
    if (run.running) timer = window.setTimeout(poll, 600);
  }
  poll();
  onDestroy(() => clearTimeout(timer));

  const STATUSES = ["renamed", "other_vst2", "trial", "vst3", "newer", "to_vst3", "other_vst3", "missing"];
  const statusText = (s: string) => (STATUSES.includes(s) ? t(`repair.status.${s}`) : s);
  const fixable = $derived(
    (v?.fixable ?? []).slice().sort((a, b) => Number(b.complete) - Number(a.complete) || compare(a.title ?? "", b.title ?? "")),
  );
  const shown = $derived(onlyComplete ? fixable.filter((f) => f.complete) : fixable);
  const sel = $derived(shown.filter((f) => picked.has(f.rel)));
  const sevCount = $derived([1, 2, 3].map((k) => (v?.plugins ?? []).filter((p) => statusSeverity(p.status) === k).length));
  const formats = $derived([...new Set((v?.plugins ?? []).map((p) => p.format))].sort());
  const vendors = $derived([...new Set((v?.plugins ?? []).map((p) => p.vendor).filter(Boolean))].sort(compare));
  const toInstall = $derived(
    (v?.plugins ?? []).filter(
      (p) =>
        (!q || (p.name + " " + p.vendor).toLowerCase().includes(q.toLowerCase())) &&
        (sevF === null || statusSeverity(p.status) === sevF) &&
        (!fmtF || p.format === fmtF) &&
        (!vendorF || p.vendor === vendorF) &&
        (!stageF || (p.stages[stageF] ?? 0) > 0),
    ),
  );
  // VST2 plugins whose installed VST3 declares it replaces them, not switched yet
  const safeVst3 = $derived((v?.plugins ?? []).filter((p) => !p.vst3 && p.vst3_targets?.some((t) => t.compatible) && p.automated < p.files));
  const autoKept = $derived(safeVst3.reduce((n, p) => n + p.automated, 0));
  let switching = $state(false);
  async function allVst3() {
    const list = safeVst3.map((p) => [p.name, p.vst3_targets!.find((x) => x.compatible)!.uid] as [string, string]);
    switching = true;
    try {
      await app.track(api.setVst3Many(list));
      app.say(t("repair.install.vst3Switched", { n: list.length }));
    } catch (e) {
      app.say(String(e), { error: true });
    } finally {
      switching = false;
    }
  }
  const filtered = $derived(!!(q || sevF !== null || fmtF || vendorF || stageF));
  const sheet = $derived(open ? (v?.plugins.find((p) => p.key === open) ?? v?.ignored.find((p) => p.key === open) ?? null) : null);
  const opensPct = $derived(v && v.files ? Math.round((100 * (v.sev[0] + v.sev[1])) / v.files) : 0);
  const SEVC = ["var(--ok)", "var(--fix)", "var(--lost)", "var(--bad)"];
  const proj = $derived(v ? [v.complete_now, ...v.order.map((o) => v!.complete_now + o.complete)] : []);
  const projX = $derived(v ? [t("repair.today"), ...v.order.map((o, i) => `${i + 1}. ${o.name}`)] : []);
  const pct = $derived(run && run.total ? Math.round((run.done / run.total) * 100) : 0);
  const periods = $derived.by(() => {
    const ages = v?.ages ?? [];
    const upTo = (n: number) => ages.filter(([a]) => a < n).reduce((sum, [, c]) => sum + c, 0);
    return [[null, t("repair.period.all"), v?.all_files ?? 0], [1, t("repair.period.lessThanYear"), upTo(1)], [2, t("repair.period.years2"), upTo(2)], [5, t("repair.period.years5"), upTo(5)]] as const;
  });

  const mo = (b: number) => (b >= 1e9 ? t("repair.size.gb", { n: (b / 1e9).toFixed(1) }) : t("repair.size.mb", { n: Math.max(1, Math.round(b / 1e6)) }));
  const name = (rel: string) => rel.split("/").pop() ?? rel;
  function toggle(rel: string) {
    const n = new Set(picked);
    if (n.has(rel)) n.delete(rel);
    else n.add(rel);
    picked = n;
  }
  async function act(p: Promise<unknown>, ok?: string) {
    try {
      await app.track(p);
      if (ok) app.say(ok);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function go(rels: string[]) {
    const preset = presets.find((p) => p.name === presetName);
    await act(preset ? api.repairRunPreset(rels, preset) : api.repairRun(rels));
    picked = new Set();
    poll();
  }
  async function deep() {
    await act(api.deepSearch(), t("repair.samples.deepStarted"));
  }
  async function rescan() {
    await act(api.scan(true), t("repair.rescanned"));
  }
  const ignore = (p: PluginNeed, keep: boolean) =>
    act(api.setIgnored(p.name, true, keep), keep ? t("repair.install.ignoredKept", { name: p.name }) : t("repair.install.ignoredRemoved", { name: p.name }));
  // "Set the old Windows in {link}.": the text around the link to Settings
  const oldDriveText = $derived(t("repair.samples.oldDriveFirst").split("{link}"));
  const stagesOf = (p: PluginNeed) => ["none", ...STAGES].filter((k) => p.stages[k]).map((k) => [k, p.stages[k]] as const);
</script>

<div class="page">
  <header class="head">
    <div>
      <h1>{t("repair.title")}{#if app.busy}<span class="spin" title={t("repair.updating")} aria-label={t("repair.updatingAria")}></span>{/if}</h1>
      {#if v}
        <p class="muted sub">{t("repair.sub", { n: v.files, open: v.sev[0], fix: v.sev[1], lost: v.sev[2], missing: v.sev[3] })}</p>
      {/if}
    </div>
    <div class="hr">
      <div class="period" role="radiogroup" aria-label={t("repair.period.aria")}>
        {#each periods as [y, label, n]}
          <button role="radio" aria-checked={years === y} class:on={years === y} onclick={() => (years = y)}>{label} <span class="faint">{n}</span></button>
        {/each}
      </div>
      <button class="btn sm" title={t("repair.rescanTitle")} disabled={app.scan?.running} onclick={rescan}>{t("repair.rescan")}</button>
    </div>
  </header>

  {#if v}
    <div class="dash">
      <section class="card big">
        <b>{t("repair.pct", { n: opensPct })}</b>
        <span class="small muted">{t("repair.opensShare")}</span>
        <StackBar parts={v.sev.map((n, i) => ({ key: String(i), label: t(`repair.sev.${i}`), n, color: SEVC[i] }))} legend height={10} unit={t("repair.unit.projects")} />
      </section>
      <section class="card">
        <div class="dh"><span class="small muted">{t("repair.completeIfOrder")}</span><span class="small faint">{t("repair.completeRange", { from: v.complete_now, to: proj.at(-1) ?? v.complete_now, total: v.files })}</span></div>
        {#if v.order.length}<Lines xs={projX} unit={t("repair.unit.completeProjects")} series={[{ label: t("repair.completeSeries"), color: "var(--fix)", values: proj }]} height={110} />
        {:else}<p class="small faint">{t("repair.nothingToInstall")}</p>{/if}
      </section>
    </div>
  {/if}

  <nav class="tabs" aria-label={t("repair.tabs.aria")}>
    <button class:on={tab === "fix"} onclick={() => (tab = "fix")}>{t("repair.tabs.fix")} <span class="faint">{fixable.length}</span></button>
    <button class:on={tab === "atelier"} onclick={() => (tab = "atelier")} title={t("repair.tabs.atelierTitle")}>{t("repair.tabs.atelier")}</button>
    <button class:on={tab === "install"} onclick={() => (tab = "install")}>{t("repair.tabs.install")} <span class="faint">{v?.plugins.length ?? ""}</span></button>
    <button class:on={tab === "ignored"} onclick={() => (tab = "ignored")}>{t("repair.tabs.ignored")} <span class="faint">{v?.ignored.length ?? ""}</span></button>
    <button class:on={tab === "samples"} onclick={() => (tab = "samples")}>{t("repair.tabs.samples")} <span class="faint">{v?.samples.lost ?? ""}</span></button>
    <button class:on={tab === "copies"} onclick={() => (tab = "copies")} title={t("repair.tabs.copiesTitle")}>{t("repair.tabs.copies")}</button>
  </nav>

  {#if run && (run.running || run.results.length) && tab === "fix"}
    {@const ok = run.results.filter((r) => r.ok).length}
    <section class="card run">
      <div class="rh"><b>{run.running ? t("repair.run.running", { done: run.done, total: run.total }) : t("repair.run.last", { n: ok })}</b>
        {#if run.running}<span class="small faint">{t("repair.pct", { n: pct })}</span>{/if}</div>
      {#if run.running}<div class="bar"><i style:width="{pct}%"></i></div>{/if}
      <div class="res">
        {#each run.results.slice().reverse().slice(0, 30) as r (r.rel)}
          <div class="line small">
            <span class="dot" class:ok={r.ok} class:bad={!r.ok}></span>
            <span class="grow ell" title={r.rel}>{r.ok ? name(r.out ?? "") : name(r.rel)}</span>
            <span class="faint">{r.ok
              ? [r.renamed && t("repair.run.renamed", { n: r.renamed }), r.samples && t("repair.run.samples", { n: r.samples }), r.removed && t("repair.run.removed", { n: r.removed }), r.tried && t("repair.run.tried", { n: r.tried }), !r.complete && t("repair.run.stillMissing"), r.error].filter(Boolean).join(" · ")
              : r.error}</span>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if tab === "atelier"}<Atelier embedded />
  {:else if tab === "copies"}<WrittenCopies />
  {:else if !v}<p class="muted">{t("common.loading")}</p>
  {:else if tab === "fix"}
    <div class="bar2">
      <p class="small faint">{t("repair.fix.intro")}
        {t("repair.fix.completeCount", { n: fixable.filter((f) => f.complete).length })}</p>
      <label class="small muted"><input type="checkbox" bind:checked={onlyComplete} /> {t("repair.fix.onlyComplete")}</label>
      <label class="small muted" title={t("repair.fix.withPresetTitle")}>{t("repair.fix.withPreset")}
        <select bind:value={presetName}><option value="">{t("repair.fix.no")}</option>{#each presets as p}<option value={p.name}>{p.name}</option>{/each}</select>
      </label>
      <button class="btn primary" disabled={!sel.length || run?.running} onclick={() => go(sel.map((f) => f.rel))}>
        {presetName
          ? sel.length ? t("repair.fix.repairPresetN", { n: sel.length }) : t("repair.fix.repairPresetSel")
          : sel.length ? t("repair.fix.repairN", { n: sel.length }) : t("repair.fix.repairSel")}
      </button>
    </div>
    {#if !shown.length}
      <div class="empty card"><h2>{t("repair.fix.emptyTitle")}</h2><p class="muted small">{t("repair.fix.emptyText")}</p></div>
    {:else}
      <div class="table">
        <div class="tr th small faint">
          <input type="checkbox" aria-label={t("repair.fix.selectAll")} checked={sel.length === shown.length && shown.length > 0}
            onchange={(e) => (picked = new Set(e.currentTarget.checked ? shown.map((f) => f.rel) : []))} />
          <span>{t("repair.fix.colProject")}</span><span>{t("repair.fix.colFixes")}</span><span>{t("repair.fix.colAfter")}</span>
        </div>
        {#each shown as f (f.rel)}
          <label class="tr">
            <input type="checkbox" checked={picked.has(f.rel)} onchange={() => toggle(f.rel)} />
            <span class="p"><a href="#/song/{f.song}" onclick={(e) => e.stopPropagation()}>{f.title}</a>
              <span class="small faint ell" title={f.rel}>{name(f.rel)}{f.again ? t("repair.fix.again") : ""}</span></span>
            <span class="chips small">
              {#if f.renames}<span class="chip">{t("repair.fix.renames", { n: f.renames })}</span>{/if}
              {#if f.copies}<span class="chip">{t("repair.fix.copies", { n: f.copies, size: mo(f.bytes) })}</span>{/if}
              {#if f.removes}<span class="chip">{t("repair.fix.removes", { n: f.removes })}</span>{/if}
              {#if f.trials}<span class="chip">{t("repair.fix.trials", { n: f.trials })}</span>{/if}
            </span>
            <span class="small" class:okc={f.complete} class:muted={!f.complete}>{f.complete ? t("repair.fix.complete") : t("repair.fix.incomplete")}</span>
          </label>
        {/each}
      </div>
    {/if}
  {:else if tab === "install"}
    <div class="two">
      <section class="card order">
        <h2>{t("repair.install.orderTitle")}</h2>
        <p class="small faint">{t("repair.install.orderIntro", { now: v.complete_now, total: v.files })}</p>
        <ol>
          {#each v.order.slice(0, 40) as o, i (o.key)}
            <li>
              <span class="no">{i + 1}</span>
              <span class="grow ell" title={o.name}>{o.name} <span class="faint small">{o.format.toUpperCase()}</span></span>
              <span class="small faint" title={t("repair.install.usedBy")}>{o.files}</span>
              <span class="prog" title={t("repair.install.completeAfter", { n: o.complete })}><i style:width="{v.need_any ? (o.complete / v.need_any) * 100 : 0}%"></i></span>
            </li>
          {/each}
        </ol>
        {#if v.order.length > 40}<p class="small faint">{t("repair.install.more", { n: v.order.length - 40 })}</p>{/if}
        <h2 class="mt">{t("repair.install.byVendor")}</h2>
        <BarList rows={v.vendors.filter((x) => x.to_install).slice(0, 12).map((x) => ({ label: x.vendor || t("repair.install.unknownVendor"), n: x.to_install, note: t("repair.install.songsConcerned", { n: x.songs }) }))} unit={t("repair.unit.pluginsToInstall")}
          pick={(l) => (vendorF = vendorF === l ? "" : l)} />
      </section>
      <section class="plist">
        {#if safeVst3.length}
          <div class="v3all card small">
            <span><b>{safeVst3.length}</b> {t("repair.install.vst3All", { n: safeVst3.length, names: safeVst3.slice(0, 4).map((p) => p.name).join(", ") + (safeVst3.length > 4 ? "..." : "") })}{#if autoKept} {t("repair.install.vst3Automated", { n: autoKept })}{/if}</span>
            <button class="btn sm primary" disabled={switching} onclick={allVst3}>{switching ? t("repair.install.vst3Switching") : t("repair.install.vst3Switch")}</button>
          </div>
        {/if}
        <div class="ph"><h2>{t("repair.install.problemTitle")} <span class="faint">{toInstall.length}</span></h2><input type="search" placeholder={t("repair.install.search")} bind:value={q} /></div>
        <div class="filters small">
          <div class="seg" role="radiogroup" aria-label={t("repair.install.stateAria")}>
            <button role="radio" aria-checked={sevF === null} class:on={sevF === null} onclick={() => (sevF = null)}>{t("repair.install.all")}</button>
            {#each [1, 2, 3] as k}<button role="radio" aria-checked={sevF === k} class:on={sevF === k} onclick={() => (sevF = sevF === k ? null : k)}><i class="d s{k}"></i>{t(`repair.sevName.${k}`)} <span class="faint">{sevCount[k - 1]}</span></button>{/each}
          </div>
          <select bind:value={fmtF} aria-label={t("repair.install.formatAria")}><option value="">{t("repair.install.allFormats")}</option>{#each formats as f}<option value={f}>{f.toUpperCase()}</option>{/each}</select>
          <select bind:value={vendorF} aria-label={t("repair.install.vendorAria")}><option value="">{t("repair.install.allVendors")}</option>{#each vendors as x}<option value={x}>{x}</option>{/each}</select>
          <select bind:value={stageF} aria-label={t("repair.install.stageAria")}><option value="">{t("repair.install.blocksStage")}</option>{#each STAGES as s}<option value={s}>{stageLabel(s)}</option>{/each}</select>
          {#if filtered}<button class="link" onclick={() => { q = ""; sevF = null; fmtF = ""; vendorF = ""; stageF = ""; }}>{t("repair.install.showAll")}</button>{/if}
        </div>
        {#each toInstall as p (p.key)}
          <div class="pl card" role="button" tabindex="0" onclick={() => (open = p.key)} onkeydown={(e) => e.key === "Enter" && (open = p.key)}>
            <div class="pt">
              <div class="grow">
                <b class="pn">{p.name}</b> <span class="fmt">{p.format.toUpperCase()}</span>
                {#if p.vst3}<span class="v3tag small">{t("repair.install.vst3Active")}</span>{:else if p.vst3_targets?.length}<span class="v3tag off small">{p.vst3_targets.some((x) => x.compatible) ? t("repair.install.vst3Safe") : t("repair.install.vst3Try")}</span>{/if}
                <div class="small muted">{statusText(p.status)}{p.needs.length ? " · " + t("repair.install.version", { v: p.needs[0][0] }) : ""}{p.vendor ? ` · ${p.vendor}` : ""}</div>
              </div>
              <div class="small r">
                <div>{t("repair.songs", { n: p.n })} · {t("repair.projects", { n: p.files })}</div>
                <div class="st">{#each stagesOf(p) as [k, n]}<i class="s-{k}" style:flex={n} title={t("repair.install.stageCount", { stage: stageLabel(k === "none" ? null : k), n })}></i>{/each}</div>
              </div>
            </div>
            {#if p.need_label || p.candidates.length}
              <div class="small faint">{p.need_label}{p.candidates.length ? (p.need_label ? " · " : "") + t("repair.install.installed", { names: (p.candidates as { name: string }[]).map((c) => c.name).join(", ") }) : ""}</div>
            {/if}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="pa small" onclick={(e) => e.stopPropagation()}>
              {#if p.trial_targets?.length}
                <label>{t("repair.install.replaceWith")}
                  <select value={p.trial ?? ""} onchange={(e) => act(api.setTrial(p.name, p.format, e.currentTarget.value || null), e.currentTarget.value ? t("repair.install.trialSet", { name: p.name, to: e.currentTarget.value }) : t("repair.install.trialUnset", { name: p.name }))}>
                    <option value="">{t("repair.fix.no")}</option>{#each p.trial_targets as tt}<option>{tt}</option>{/each}
                  </select></label>
              {/if}
              <button class="link" onclick={() => ignore(p, false)} title={t("repair.install.ignoreTitle")}>{t("repair.install.ignore")}</button>
              <button class="link" onclick={() => ignore(p, true)} title={t("repair.install.ignoreKeepTitle")}>{t("repair.install.ignoreKeep")}</button>
            </div>
          </div>
        {:else}
          <p class="muted small">{t("repair.nothingToInstall")}</p>
        {/each}
      </section>
    </div>
  {:else if tab === "ignored"}
    <p class="small faint">{t("repair.ignored.intro")}</p>
    <div class="table">
      {#each v.ignored as p (p.key)}
        {@const kept = v.ignored_keep.includes(p.name.toLowerCase())}
        <div class="tr ig">
          <span class="p"><button class="lnk" onclick={() => (open = p.key)}><b>{p.name}</b></button><span class="small faint">{p.format.toUpperCase()} · {statusText(p.status)}</span></span>
          <span class="small muted">{t("repair.songs", { n: p.n })} · {t("repair.projects", { n: p.files })}</span>
          <label class="small"><input type="checkbox" checked={kept} onchange={(e) => act(api.setIgnored(p.name, true, e.currentTarget.checked))} /> {t("repair.ignored.kept")}</label>
          <button class="btn sm" onclick={() => act(api.setIgnored(p.name, false), t("repair.ignored.countsAgain", { name: p.name }))}>{t("repair.ignored.unignore")}</button>
        </div>
      {:else}
        <p class="muted small pad">{t("repair.ignored.none")}</p>
      {/each}
    </div>
  {:else}
    <div class="stats">
      <div class="card"><b>{v.samples.total}</b><span class="small muted">{t("repair.samples.used")}</span></div>
      <div class="card"><b>{v.samples.moved}</b><span class="small muted">{t("repair.samples.moved")}</span></div>
      <div class="card"><b class="bad">{v.samples.lost}</b><span class="small muted">{t("repair.samples.lost")}</span></div>
    </div>
    <div class="deep">
      <p class="small faint">{#if v.deep}{v.deep.at
          ? t("repair.samples.deepLastOn", { root: v.deep.root, files: v.deep.files ?? 0, date: fmtDate(v.deep.at, { day: "2-digit", month: "2-digit", year: "numeric" }) })
          : t("repair.samples.deepLast", { root: v.deep.root, files: v.deep.files ?? 0 })}{:else}{t("repair.samples.deepNone")}{/if}
        {#if !app.config?.old_drive} {oldDriveText[0]}<a href="#/settings">{t("repair.samples.settings")}</a>{oldDriveText[1] ?? ""}{/if}</p>
      <button class="btn sm" disabled={!app.config?.old_drive || app.scan?.running} onclick={deep}>{t("repair.samples.deep")}</button>
    </div>
    <div class="table">
      <div class="tr th sm small faint"><span>{t("repair.samples.colFrom")}</span><span>{t("repair.samples.colSamples")}</span><span>{t("repair.samples.colSongs")}</span><span>{t("repair.samples.colOld")}</span></div>
      {#each v.roots as r (r.root)}
        <div class="tr sm">
          <span class="ell" title={r.root}>{r.root}</span><span>{r.files}</span><span>{r.songs}</span>
          <span class="small faint ell" title={r.old ?? ""}>{r.old ? (r.old_exists ? t("repair.samples.oldSearched") : t("repair.samples.oldAbsent")) : "-"}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if sheet && v}
  <PluginSheet p={sheet} ignored={v.ignored.includes(sheet)} kept={v.ignored_keep.includes(sheet.name.toLowerCase())} titles={v.titles} close={() => (open = null)} />
{/if}

<style>
  .page { padding: 24px 28px 60px; }
  .hr { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; justify-content: flex-end; }
  .dash { display: grid; grid-template-columns: minmax(260px, 1fr) minmax(0, 2fr); gap: 12px; margin-bottom: 16px; }
  @media (max-width: 1000px) { .dash { grid-template-columns: 1fr; } }
  .big { display: flex; flex-direction: column; gap: 6px; }
  .big > b { font-size: 32px; letter-spacing: -0.02em; font-variant-numeric: tabular-nums; line-height: 1.1; }
  .dh { display: flex; justify-content: space-between; gap: 10px; margin-bottom: 6px; }
  .mt { margin-top: 18px; }
  .filters { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .filters select { padding: 3px 8px; font-size: 12px; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .seg button { border: 0; background: none; padding: 3px 9px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12px; display: inline-flex; align-items: center; gap: 5px; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .d { width: 7px; height: 7px; border-radius: 50%; display: inline-block; }
  .d.s1 { background: var(--fix); } .d.s2 { background: var(--lost); } .d.s3 { background: var(--bad); }
  .pl { cursor: pointer; transition: border-color .15s; }
  .pl:hover { border-color: var(--faint); }
  .pl:hover .pn { color: var(--accent); }
  .lnk { border: 0; background: none; padding: 0; color: inherit; cursor: pointer; text-align: left; }
  .lnk:hover { color: var(--accent); }
  .v3all { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 14px; border-color: color-mix(in srgb, var(--fix) 45%, var(--line)); }
  .v3tag { margin-left: 6px; padding: 0 7px; border-radius: 99px; color: var(--fix); background: color-mix(in srgb, var(--fix) 14%, transparent); }
  .v3tag.off { color: var(--muted); background: var(--panel2); }
  .deep { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
  .deep p { margin: 0; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 20px; margin-bottom: 12px; }
  .sub { margin: 0; }
  .period, .tabs { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .period button, .tabs button { border: 0; background: none; padding: 4px 11px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  .period button.on, .tabs button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .tabs { margin-bottom: 14px; }
  .bar2 { display: flex; align-items: center; gap: 16px; margin-bottom: 12px; }
  .bar2 p { margin: 0; flex: 1; }
  .bar2 label { display: inline-flex; gap: 6px; align-items: center; cursor: pointer; }
  .btn:disabled { opacity: .45; cursor: default; }
  .run { margin-bottom: 14px; }
  .rh { display: flex; justify-content: space-between; align-items: baseline; }
  .bar { height: 6px; border-radius: 3px; background: var(--line); margin: 8px 0; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width .3s ease; }
  .res { max-height: 220px; overflow: auto; margin-top: 6px; }
  .line { display: flex; gap: 10px; align-items: center; padding: 3px 0; }
  .grow { flex: 1; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .dot.ok { background: var(--ok); } .dot.bad { background: var(--bad); }
  .table { border: 1px solid var(--line); border-radius: var(--radius); overflow: hidden; background: var(--panel); }
  .tr { display: grid; grid-template-columns: 28px minmax(220px, 1.2fr) minmax(0, 2fr) 140px; gap: 12px; align-items: center; padding: 9px 14px; border-bottom: 1px solid var(--line); cursor: pointer; }
  .tr.ig { grid-template-columns: minmax(220px, 1.4fr) 200px 190px auto; cursor: default; }
  .tr.sm { grid-template-columns: minmax(0, 3fr) 90px 90px minmax(0, 1.2fr); cursor: default; }
  .tr:last-child { border-bottom: 0; }
  .tr:hover:not(.th) { background: color-mix(in srgb, var(--line) 35%, transparent); }
  .th { cursor: default; background: var(--panel2); }
  .p { display: flex; flex-direction: column; min-width: 0; }
  .p a { color: var(--ink); font-weight: 600; width: fit-content; }
  .p a:hover { color: var(--accent); }
  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
  .chip { padding: 1px 8px; border-radius: 99px; border: 1px solid var(--line); white-space: nowrap; }
  .okc { color: var(--ok); }
  .empty { text-align: center; padding: 40px; }
  .pad { padding: 14px; }
  .two { display: grid; grid-template-columns: minmax(300px, 1fr) minmax(0, 2fr); gap: 16px; align-items: start; }
  @media (max-width: 1200px) { .two { grid-template-columns: 1fr; } }
  .order ol { list-style: none; margin: 8px 0 0; padding: 0; }
  .order li { display: flex; align-items: center; gap: 10px; padding: 4px 0; font-size: 12.5px; }
  .no { width: 20px; text-align: right; color: var(--faint); font-variant-numeric: tabular-nums; }
  .prog { width: 70px; height: 5px; border-radius: 3px; background: var(--line); overflow: hidden; flex: none; }
  .prog i { display: block; height: 100%; background: var(--ok); }
  .plist { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .ph { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .ph h2 { margin: 0; }
  .ph input { width: 260px; }
  .pl { padding: 10px 14px; display: flex; flex-direction: column; gap: 4px; }
  .pt { display: flex; gap: 12px; align-items: flex-start; }
  .fmt { font-size: 10.5px; padding: 0 6px; border-radius: 5px; border: 1px solid var(--line); color: var(--muted); }
  .r { text-align: right; flex: none; width: 170px; }
  .st { display: flex; gap: 2px; height: 5px; margin-top: 5px; border-radius: 3px; overflow: hidden; }
  .st i { background: var(--s-none); }
  .s-draft { background: var(--s-draft) !important; } .s-arrangement { background: var(--s-arrangement) !important; } .s-mix { background: var(--s-mix) !important; }
  .s-master { background: var(--s-master) !important; } .s-done { background: var(--s-done) !important; } .s-pause { background: var(--s-pause) !important; }
  .pa { display: flex; gap: 14px; align-items: center; flex-wrap: wrap; }
  .pa label { display: inline-flex; gap: 6px; align-items: center; }
  .pa select { padding: 2px 6px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin-bottom: 12px; }
  .stats .card { display: flex; flex-direction: column; gap: 2px; }
  .stats b { font-size: 22px; }
  .bad { color: var(--bad); }
</style>

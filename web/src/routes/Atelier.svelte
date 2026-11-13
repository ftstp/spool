<script lang="ts">
  // Treatments of projects, each optional, set by presets. The original is
  // never modified: each pass writes a copy "Name [codes].als" next to it, and a copy
  // retouched in Live is never overwritten.
  import { onDestroy } from "svelte";
  import * as api from "../lib/api";
  import type { AtelierRun, Preset, RepairState, Track } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { t } from "../lib/i18n/index.svelte";

  /** Shown inside Repair (its own tab): no page title. */
  let { embedded = false }: { embedded?: boolean } = $props();

  const CODES = "RCNOPMG";
  const ROLES = ["drums", "perc", "bass", "keys", "pad", "pluck", "lead", "vox", "fx", "other", "return"];
  /** A role's name in the user's language (an unknown role shows as is). */
  const role = (r: string) => (ROLES.includes(r) ? t(`atelier.role.${r}`) : r);
  // Live's palette: 14 hues x 5 shades (index = shade * 14 + hue). Shade 1 is the vivid one;
  // the others are approximations for the preview.
  const HUES = ["#FF3636", "#F66C03", "#99724B", "#FFF034", "#87FF67", "#3DC300", "#00BFAF", "#19E9FF", "#10A4EE", "#007DC0", "#886CE4", "#B677C6", "#FF39D4", "#D0D0D0"];
  function liveColor(i: number) {
    const hue = HUES[i % 14];
    const mix = [["#ffffff", 35], null, ["#ffffff", 60], ["#808080", 45], ["#000000", 45]][Math.floor(i / 14)];
    return mix ? `color-mix(in srgb, ${hue}, ${mix[0]} ${mix[1]}%)` : hue;
  }

  let presets = $state<Preset[]>([]);
  let def = $state<Preset | null>(null);
  let cur = $state(0);
  let naming = $state<"" | "new" | "rename">("");
  let nameDraft = $state("");
  let target = $state<"song" | "all">("song");
  let songQ = $state("");
  let song = $state<number | null>(null);
  let preview = $state<AtelierRun | null>(null);
  let previewing = $state(false);
  let run = $state<RepairState | null>(null);
  let saveT = 0;
  let pollT = 0;

  Promise.all([api.atelierPresets(), api.atelierDefault()]).then(([p, d]) => {
    presets = p;
    def = d;
  });
  const p = $derived(presets[cur] ?? presets[0]);
  const codes = (x: Preset) => CODES.split("").filter((c) => x[c] === true || x[c]?.on).join("");
  const live = $derived(app.songs.filter((s) => !s.archived));
  const matches = $derived(songQ ? live.filter((s) => (s.title ?? "").toLowerCase().includes(songQ.toLowerCase())).slice(0, 8) : []);
  const chosen = $derived(live.find((s) => s.id === song));

  function save() {
    preview = null;
    clearTimeout(saveT);
    saveT = window.setTimeout(() => api.atelierSavePresets($state.snapshot(presets) as Preset[]).catch((e) => app.say(String(e), { error: true })), 400);
  }
  function setOpt(code: string, key: string | null, val: unknown) {
    if (key === null) p[code] = val;
    else p[code] = { ...(p[code] ?? {}), [key]: val };
    save();
  }
  function nameDone() {
    const n = nameDraft.trim();
    if (n && naming === "new" && def) {
      presets.push({ ...structuredClone($state.snapshot(def)), name: n });
      cur = presets.length - 1;
    } else if (n && naming === "rename") p.name = n;
    naming = "";
    if (n) save();
  }
  function dup() {
    presets.push({ ...structuredClone($state.snapshot(p)), name: p.name + t("atelier.copySuffix") });
    cur = presets.length - 1;
    save();
  }
  function del() {
    presets.splice(cur, 1);
    cur = 0;
    save();
  }
  function move(i: number, d: number) {
    const o = p.O.order as string[];
    [o[i], o[i + d]] = [o[i + d], o[i]];
    save();
  }
  async function doPreview() {
    if (!song) return;
    previewing = true;
    try {
      preview = await api.atelierPreview(song, $state.snapshot(p) as Preset);
    } catch (e) {
      app.say(String(e), { error: true });
    }
    previewing = false;
  }
  async function poll() {
    run = await api.repairState();
    clearTimeout(pollT);
    if (run.running) pollT = window.setTimeout(poll, 800);
  }
  async function go() {
    const ids = target === "all" ? live.map((s) => s.id) : song ? [song] : [];
    if (!ids.length) return app.say(t("atelier.pickSong"), { error: true });
    try {
      await api.atelierRun(ids, $state.snapshot(p) as Preset);
      poll();
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  poll();
  onDestroy(() => {
    clearTimeout(pollT);
    clearTimeout(saveT);
  });

  const facts = (r: AtelierRun) =>
    [
      r.R && t("atelier.facts.R", { renamed: r.R.renamed, tried: r.R.tried, samples: r.R.samples }),
      r.P && t("atelier.facts.P", { tracks: r.P.tracks, devices: r.P.devices, returns: r.P.returns }) + (r.P.simplified ? t("atelier.facts.Psimplified", { n: r.P.simplified }) : ""),
      r.O && t("atelier.facts.O", { moved: r.O.moved }) + (r.O.groups ? t("atelier.facts.Ogroups", { n: r.O.groups }) : ""),
      r.C && t("atelier.facts.C", { tracks: r.C.tracks, clips: r.C.clips }),
      r.N && t("atelier.facts.N", { tracks: r.N.tracks, clips: r.N.clips, scenes: r.N.scenes }),
      r.M && (r.M.skipped ? t("atelier.facts.Mnone", { why: r.M.skipped }) : t("atelier.facts.M", { n: r.M.markers })),
      r.G && t("atelier.facts.G", { list: Object.entries(r.G).map(([k, v]) => `${k} ${v}`).join(", ") || t("atelier.facts.Gnothing") }),
    ].filter(Boolean) as string[];
  const SECTIONS = ["R", "C", "N", "O", "P", "M", "G"];
  // "Sections of {input} bars...": the text around the number field
  const barsText = $derived(t("atelier.M.sections").split("{input}"));
  const isOn = (c: string) => (c === "R" ? !!p.R : !!p[c]?.on);
</script>

{#snippet chip(code: string, key: string, label: string, title = "")}
  <label class="chip" class:on={!!p[code]?.[key]} {title}><input type="checkbox" checked={!!p[code]?.[key]} onchange={(e) => setOpt(code, key, e.currentTarget.checked)} />{label}</label>
{/snippet}

{#snippet trackRow(tr: Track)}
  <div class="trk" style:padding-left="{tr.depth * 14}px">
    <i style:background={liveColor(tr.color)}></i><span class="ell grow">{tr.name || t("atelier.noName")}</span><span class="faint">{role(tr.role)}</span>
  </div>
{/snippet}

<div class:page={!embedded}>
  {#if !embedded}<h1>{t("atelier.title")}</h1>{/if}
  <p class="muted sub">{t("atelier.intro")}</p>

  {#if p}
    <div class="studio">
      <div class="left">
        <div class="card bar">
          <select value={cur} onchange={(e) => { cur = +e.currentTarget.value; preview = null; }}>
            {#each presets as x, i}<option value={i}>{x.name}</option>{/each}
          </select>
          {#if naming}
            <input placeholder={t("atelier.presetName")} bind:value={nameDraft} onkeydown={(e) => { if (e.key === "Enter") nameDone(); if (e.key === "Escape") naming = ""; }} />
            <button class="btn sm primary" onclick={nameDone}>{t("atelier.ok")}</button>
          {:else}
            <button class="btn sm" onclick={() => { naming = "new"; nameDraft = ""; }}>{t("atelier.new")}</button>
            <button class="btn sm" onclick={dup}>{t("atelier.duplicate")}</button>
            <button class="btn sm" onclick={() => { naming = "rename"; nameDraft = p.name; }}>{t("atelier.rename")}</button>
            {#if presets.length > 1}<button class="btn sm" onclick={del}>{t("atelier.delete")}</button>{/if}
          {/if}
          <span class="small faint grow r">{t("atelier.copiesLabel")} <b>{t("atelier.copiesPattern", { codes: codes(p) || "-" })}</b> {t("atelier.autosaved")}</span>
        </div>

        {#each SECTIONS as c}
          <section class="card sect" class:off={!isOn(c)}>
            <div class="sh">
              <h2><span class="code">{c}</span> {t(`atelier.section.${c}`)}</h2>
              <label class="sw"><input type="checkbox" checked={isOn(c)} onchange={(e) => (c === "R" ? setOpt("R", null, e.currentTarget.checked) : setOpt(c, "on", e.currentTarget.checked))} />{isOn(c) ? t("atelier.on") : t("atelier.off")}</label>
            </div>
            {#if isOn(c)}
              {#if c === "R"}
                <p class="small muted">{t("atelier.R.text")}</p>
              {:else if c === "C"}
                <div class="chips">{@render chip("C", "clips", t("atelier.C.clips"))}</div>
                {#each ROLES as r}
                  <div class="hues"><span class="small">{role(r)}</span>
                    {#each HUES as h, i}<button class="hue" class:sel={p.C.hue?.[r] === i} style:background={h} title={String(i)} aria-label={t("atelier.C.hueAria", { role: role(r), i })}
                      onclick={() => setOpt("C", "hue", { ...(p.C.hue ?? {}), [r]: i })}></button>{/each}
                  </div>
                {/each}
              {:else if c === "N"}
                <div class="chips">{@render chip("N", "tracks", t("atelier.N.tracks"))}{@render chip("N", "clips", t("atelier.N.clips"))}{@render chip("N", "scenes", t("atelier.N.scenes"))}{@render chip("N", "only_auto", t("atelier.N.onlyAuto"), t("atelier.N.onlyAutoTitle"))}</div>
                <div class="labels">
                  {#each ROLES.filter((r) => r !== "return") as r}
                    <label class="small"><span>{role(r)}</span><input value={p.N.labels?.[r] ?? ""} onchange={(e) => setOpt("N", "labels", { ...(p.N.labels ?? {}), [r]: e.currentTarget.value.trim() })} /></label>
                  {/each}
                </div>
              {:else if c === "O"}
                <div class="chips">{@render chip("O", "inside_groups", t("atelier.O.insideGroups"))}{@render chip("O", "make_groups", t("atelier.O.makeGroups"), t("atelier.O.makeGroupsTitle"))}</div>
                <ol class="ord">
                  {#each (p.O.order ?? []) as r, i (r)}
                    <li class="small"><span class="faint">{i + 1}</span><span class="grow">{role(r)}</span>
                      <button class="btn sm" disabled={!i} onclick={() => move(i, -1)} aria-label={t("atelier.O.up")}>↑</button>
                      <button class="btn sm" disabled={i === p.O.order.length - 1} onclick={() => move(i, 1)} aria-label={t("atelier.O.down")}>↓</button></li>
                  {/each}
                </ol>
              {:else if c === "P"}
                <div class="chips">{@render chip("P", "empty_tracks", t("atelier.P.emptyTracks"), t("atelier.P.emptyTracksTitle"))}{@render chip("P", "disabled_devices", t("atelier.P.disabledDevices"), t("atelier.P.disabledDevicesTitle"))}{@render chip("P", "unused_returns", t("atelier.P.unusedReturns"), t("atelier.P.unusedReturnsTitle"))}{@render chip("P", "simplify_groups", t("atelier.P.simplifyGroups"), t("atelier.P.simplifyGroupsTitle"))}</div>
                <p class="small muted">{t("atelier.P.text")}</p>
              {:else if c === "M"}
                <div class="chips">{@render chip("M", "only_if_none", t("atelier.M.onlyIfNone"))}</div>
                <label class="small row">{barsText[0]}<input type="number" min="2" max="64" value={p.M.min_bars ?? 8} onchange={(e) => setOpt("M", "min_bars", +e.currentTarget.value || 8)} />{barsText[1] ?? ""}</label>
              {:else if c === "G"}
                <div class="g">
                  <span class="small">{t("atelier.G.tempo")}</span><input type="number" step="0.01" value={p.G.tempo ?? ""} placeholder={t("atelier.G.tempoPlaceholder")} onchange={(e) => setOpt("G", "tempo", e.currentTarget.value === "" ? null : +e.currentTarget.value)} />
                  <span class="small">{t("atelier.G.scale")}</span><input value={p.G.scale ?? ""} placeholder={t("atelier.G.scalePlaceholder")} onchange={(e) => setOpt("G", "scale", e.currentTarget.value.trim())} />
                  <span class="small">{t("atelier.G.rack")}</span><input value={p.G.main_rack ?? ""} placeholder={t("atelier.G.rackPlaceholder")} onchange={(e) => setOpt("G", "main_rack", e.currentTarget.value.trim())} />
                </div>
              {/if}
            {/if}
          </section>
        {/each}
      </div>

      <div class="right">
        <section class="card">
          <h2>{t("atelier.target")}</h2>
          <div class="seg">
            <button class:on={target === "song"} onclick={() => (target = "song")}>{t("atelier.oneSong")}</button>
            <button class:on={target === "all"} onclick={() => { target = "all"; preview = null; }}>{t("atelier.allSongs", { n: live.length })}</button>
          </div>
          {#if target === "song"}
            <input class="songq" type="search" placeholder={t("atelier.songSearch")} bind:value={songQ} />
            {#if chosen && !songQ}<p class="small">{t("atelier.songLabel")} <b>{chosen.title}</b> <span class="faint">{chosen.rel.split("/").pop()}</span></p>{/if}
            {#each matches as s (s.id)}<button class="opt" onclick={() => { song = s.id; songQ = ""; preview = null; }}>{s.title}<span class="faint small">{s.dir.split("/").slice(-1)[0]}</span></button>{/each}
          {/if}
          <div class="acts">
            {#if target === "song"}<button class="btn" disabled={!song || previewing} onclick={doPreview}>{previewing ? t("atelier.computing") : t("atelier.preview")}</button>{/if}
            <button class="btn primary" disabled={run?.running || (target === "song" && !song)} onclick={go}>
              {target === "all" ? t("atelier.createN", { n: live.length }) : t("atelier.createOne")}</button>
          </div>
          {#if run && (run.running || run.results.length)}
            <div class="prog small">
              {#if run.running}<div class="pbar"><i style:width="{run.total ? (run.done / run.total) * 100 : 0}%"></i></div>{t("atelier.progress", { done: run.done, total: run.total })}
              {:else}{t("atelier.done", { n: run.results.filter((r) => r.ok).length })}{run.results.some((r) => !r.ok) ? t("atelier.notDone", { n: run.results.filter((r) => !r.ok).length }) : ""}.{/if}
              {#each run.results.filter((r) => !r.ok || r.error).slice(0, 6) as r}<div class="bad">{t("atelier.fileError", { file: r.rel.split("/").pop(), error: r.error })}</div>{/each}
            </div>
          {/if}
        </section>

        {#if preview}
          <section class="card">
            <h2>{t("atelier.preview")}</h2>
            {#if !preview.ok}<p class="small">{preview.error ?? t("atelier.nothingToDo")}</p>
            {:else}
              <p class="small">{t("atelier.copyLabel")} <b>{preview.out?.split("/").pop()}</b>{#if preview.source !== preview.rel}<br /><span class="faint">{t("atelier.redone")}</span>{/if}</p>
              {#each facts(preview) as f}<div class="small">{f}</div>{/each}
              <div class="ba">
                <div><h3>{t("atelier.before")}</h3>{#each preview.before as tr}{@render trackRow(tr)}{/each}</div>
                <div><h3>{t("atelier.after")}</h3>{#each preview.after as tr}{@render trackRow(tr)}{/each}</div>
              </div>
              <p class="small faint">{t("atelier.approxColors")}</p>
            {/if}
          </section>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .sub { margin: 0 0 16px; max-width: 900px; }
  .studio { display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(340px, 1fr); gap: 16px; align-items: start; }
  @media (max-width: 1200px) { .studio { grid-template-columns: 1fr; } }
  .left, .right { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .right { position: sticky; top: 0; }
  .bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .grow { flex: 1; min-width: 0; }
  .r { text-align: right; }
  .sect.off { opacity: .72; }
  .sh { display: flex; justify-content: space-between; align-items: center; }
  .sh h2 { margin: 0; }
  .code { display: inline-grid; place-items: center; width: 20px; height: 20px; border-radius: 5px; background: var(--accent-bg); color: var(--accent); font-size: 11.5px; margin-right: 4px; }
  .sw { display: inline-flex; gap: 6px; align-items: center; font-size: 12.5px; color: var(--muted); cursor: pointer; }
  .sect > :not(.sh) { margin-top: 8px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { display: inline-flex; align-items: center; padding: 3px 10px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; font-size: 12.5px; }
  .chip.on { background: var(--accent-bg); border-color: var(--accent); }
  .chip input { display: none; }
  .hues { display: flex; align-items: center; gap: 5px; margin: 3px 0; }
  .hues span { width: 130px; }
  .hue { width: 16px; height: 16px; border-radius: 4px; border: 0; padding: 0; cursor: pointer; }
  .hue.sel { outline: 2px solid var(--ink); outline-offset: 1px; }
  .labels { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 4px 12px; }
  .labels label { display: grid; grid-template-columns: 110px minmax(0, 1fr); align-items: center; gap: 6px; }
  .labels input { min-width: 0; padding: 3px 8px; }
  .ord { list-style: none; padding: 0; margin: 8px 0 0; }
  .ord li { display: flex; align-items: center; gap: 8px; padding: 2px 0; }
  .ord li > span:first-child { width: 18px; }
  .btn:disabled { opacity: .4; cursor: default; }
  .row input { width: 60px; margin: 0 4px; }
  .g { display: grid; grid-template-columns: auto 1fr; gap: 6px 10px; align-items: center; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .seg button { border: 0; background: none; padding: 4px 11px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .songq { width: 100%; margin-top: 10px; }
  .opt { display: flex; justify-content: space-between; gap: 10px; width: 100%; text-align: left; border: 0; background: none; color: var(--ink); padding: 5px 6px; border-radius: 6px; cursor: pointer; }
  .opt:hover { background: color-mix(in srgb, var(--line) 55%, transparent); }
  .acts { display: flex; gap: 8px; margin-top: 12px; }
  .prog { margin-top: 10px; }
  .pbar { height: 6px; border-radius: 3px; background: var(--line); margin-bottom: 6px; overflow: hidden; }
  .pbar i { display: block; height: 100%; background: var(--accent); transition: width .3s; }
  .bad { color: var(--lost); }
  .ba { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; margin-top: 10px; }
  .ba h3 { margin: 0 0 4px; font-size: 13px; }
  .trk { display: flex; align-items: center; gap: 6px; padding: 2px 0; font-size: 12px; }
  .trk i { width: 10px; height: 10px; border-radius: 2px; flex: none; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

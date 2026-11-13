<script lang="ts">
  // A new song started from how the user works: for a style, one chain per part drawn among
  // those of their own projects (the style's usual ones more often), a structure drawn among
  // theirs and fitted to the length wanted. Written in a NEW folder; the projects it borrows
  // from are only read.
  import * as api from "../lib/api";
  import type { CreatePlan, DevView } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { t } from "../lib/i18n/index.svelte";
  import Devices from "../lib/components/Devices.svelte";

  const PARTS = ["kick", "snare", "hats", "kit", "perc", "bass", "keys", "pad", "lead"];
  const partName = (p: string) => (PARTS.includes(p) ? t(`create.part.${p}`) : p);
  const DEFAULT_PARTS = ["kick", "snare", "hats", "bass", "keys", "lead"];
  const LENGTHS = [null, 150, 180, 210, 240, 300, 360];
  const lengthName = (v: number | null) => t(v === null ? "create.length.usual" : `create.length.s${v}`);
  const newSeed = () => Math.floor(Math.random() * 2 ** 31);

  let plan = $state<CreatePlan | null>(null);
  let style = $state<string | null>(null);
  let live = $state<string | null>(null);
  /** Parts kept as they are (a chain chosen or locked): a new draw leaves them. */
  let locked = $state<Record<string, string>>({});
  let seed = $state<number | null>(newSeed());
  let tempo = $state<number | null>(null);
  let seconds = $state<number | null>(null);
  let name = $state("");
  let picked = $state<Set<string> | null>(null);
  let err = $state("");
  let making = $state(false);
  let made = $state<{ rel: string; out: string; parts: number; samples: number } | null>(null);
  let views = $state<Record<string, DevView[] | "loading" | "error">>({});
  /** Where the "<name> Project" folder goes (null: the default, plan.folder). */
  let dest = $state<string | null>(null);
  // the name as the app writes it on disk (crate::create::clean_name)
  const clean = (n: string) => n.replace(/[<>:"/\\|?*\u0000-\u001f]/g, " ").split(/\s+/).filter(Boolean).join(" ").replace(/^\.+|\.+$/g, "").trim();
  const sep = $derived((plan?.folder ?? "").includes("\\") ? "\\" : "/");
  async function pickDest() {
    try {
      const f = await api.pickFolder(t("create.pickDest"));
      if (f) dest = f;
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }

  $effect(() => {
    const req = { style, live, overrides: { ...locked }, tempo, seed, seconds };
    err = "";
    app.track(api.createPlan(req))
      .then((p) => {
        if (style !== req.style || live !== req.live || seed !== req.seed) return; // a newer request is on its way
        plan = p;
        if (!picked) {
          const ok = p.parts.filter((x) => x.pick).map((x) => x.part);
          let d = DEFAULT_PARTS.filter((x) => ok.includes(x));
          if (!d.some((x) => ["kick", "snare", "hats"].includes(x)) && ok.includes("kit")) d = ["kit", ...d];
          picked = new Set(d);
        }
        // what each track holds, read once per track
        for (const x of p.parts) {
          const k = x.pick && x.pick.rel + "|" + x.pick.track;
          if (!k || views[k]) continue;
          views[k] = "loading";
          api.trackView(x.pick!.rel, x.pick!.track).then((v) => (views[k] = v)).catch(() => (views[k] = "error"));
        }
      })
      .catch((e) => (err = String(e)));
  });

  function setStyle(s: string) {
    style = s;
    locked = {};
    tempo = null;
    picked = null;
  }
  const reroll = () => (seed = newSeed());
  function habits() {
    seed = null;
    locked = {};
  }
  function tick(part: string) {
    const n = new Set(picked ?? []);
    if (n.has(part)) n.delete(part);
    else n.add(part);
    picked = n;
  }
  function lock(part: string, key: string | undefined) {
    const n = { ...locked };
    if (key === undefined || n[part] === key) delete n[part];
    else n[part] = key;
    locked = n;
  }
  // another chain for this part only, drawn as the app draws (usual ones more often)
  function rerollPart(part: string) {
    const p = plan?.parts.find((x) => x.part === part);
    if (!p?.pick) return;
    const pool = p.alts.filter((a) => a.key !== p.pick!.key);
    if (!pool.length) return;
    const w = pool.map((a) => 1 + a.songs + 4 * a.in_style);
    let at = Math.random() * w.reduce((t, x) => t + x, 0);
    const i = Math.max(0, w.findIndex((x) => (at -= x) < 0));
    locked = { ...locked, [part]: pool[i].key };
  }
  const why = (p: NonNullable<CreatePlan["parts"][number]["pick"]>, pool: number) =>
    locked[p.part]
      ? t("create.why.kept")
      : p.why === "drawn"
        ? t("create.why.drawn", { n: pool })
        : p.why === "auto"
          ? t("create.why.auto")
          : p.why === "choice"
            ? t("create.why.choice")
            : t("create.why.other");

  const order = $derived(plan ? plan.parts.filter((p) => picked?.has(p.part) && p.pick).map((p) => p.part) : []);
  const total = $derived(plan ? plan.structure.sections.reduce((a, s) => a + Number(s.bars), 0) : 0);
  const mmss = (s: number) => t("create.mmss", { m: Math.floor(s / 60), s: String(Math.round(s % 60)).padStart(2, "0") });

  async function make() {
    if (!plan) return;
    making = true;
    err = "";
    // exactly what is on screen: every part's chain named, the same draw for the structure
    const overrides = Object.fromEntries(plan.parts.filter((p) => p.pick).map((p) => [p.part, p.pick!.key]));
    try {
      const r = await app.track(api.createSong({ style: plan.style, live: plan.live, overrides, parts: order, tempo, name, seed, seconds, folder: dest }));
      made = { rel: String(r.rel ?? ""), out: String(r.out ?? ""), parts: Array.isArray(r.added) ? r.added.length : order.length, samples: Number(r.samples_copied ?? 0) };
      app.say(t("create.created", { name, out: made.out }));
    } catch (e) {
      err = String(e);
    } finally {
      making = false;
    }
  }
</script>

<div class="page">
  <header class="head">
    <h1>{t("create.title")}</h1>
    <p class="muted sub">{t("create.intro")}</p>
  </header>

  {#if !plan}
    {#if err}<p class="err">{err}</p>{:else}<p class="muted">{t("create.reading")}</p>{/if}
  {:else if made}
    <section class="card done">
      <h2>{t("create.ready", { name })}</h2>
      <p class="muted small">{t("create.loaded", { n: made.parts })}{made.samples ? t("create.copied", { n: made.samples }) : ""}.</p>
      <p class="small path">{made.out}</p>
      <div class="row">
        <button class="btn primary" onclick={() => api.openInLive(made!.rel).catch((e) => app.say(String(e), { error: true }))}>{t("create.openLive")}</button>
        <button class="btn" onclick={() => api.reveal(made!.rel).catch((e) => app.say(String(e), { error: true }))}>{t("create.showFolder")}</button>
        <button class="btn" onclick={() => { made = null; name = ""; reroll(); }}>{t("create.another")}</button>
      </div>
    </section>
  {:else}
    <div class="grid">
      <aside class="card form">
        <label><span class="small muted">{t("create.name")}</span><input bind:value={name} placeholder={t("create.namePh")} /></label>
        <label><span class="small muted">{t("create.style")}</span>
          <select value={plan.style} onchange={(e) => setStyle(e.currentTarget.value)}>
            {#each plan.styles as s}<option value={s.label}>{s.label}</option>{/each}
          </select>
        </label>
        <div class="two">
          <label><span class="small muted">{t("create.tempo")}</span>
            <input type="number" min="40" max="250" placeholder={plan.structure.bpm ? String(plan.structure.bpm) : t("create.bpm")} value={tempo ?? ""}
              onchange={(e) => (tempo = e.currentTarget.value ? +e.currentTarget.value : null)} />
          </label>
          <label><span class="small muted">{t("create.length_label")}</span>
            <select value={seconds ?? ""} onchange={(e) => (seconds = e.currentTarget.value ? +e.currentTarget.value : null)}>
              {#each LENGTHS as v}<option value={v ?? ""}>{lengthName(v)}</option>{/each}
            </select>
          </label>
        </div>
        <p class="small faint">{plan.structure.bpm ? t("create.usualBpm", { bpm: plan.structure.bpm, style: plan.style }) : ""}</p>

        <button class="btn dice" onclick={reroll} title={t("create.rerollTitle")}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="18" height="18" rx="4" /><circle cx="8" cy="8" r="1.3" fill="currentColor" /><circle cx="16" cy="16" r="1.3" fill="currentColor" /><circle cx="12" cy="12" r="1.3" fill="currentColor" /><circle cx="16" cy="8" r="1.3" fill="currentColor" /><circle cx="8" cy="16" r="1.3" fill="currentColor" /></svg>
          {t("create.reroll")}
        </button>
        {#if seed !== null || Object.keys(locked).length}<button class="link small" onclick={habits}>{t("create.habits")}</button>{/if}

        <div class="grow"></div>
        <div class="dest">
          <span class="small muted">{t("create.dest")}</span>
          <code>{dest ?? plan.folder}{sep}<b>{clean(name) || t("create.nameFallback")} Project</b>{sep}{clean(name) || t("create.nameFallback")}.als</code>
          <span class="small faint">{t("create.newFolder", { v: plan.live })}</span>
          <div class="row2">
            <button class="link small" onclick={pickDest}>{t("create.otherFolder")}</button>
            {#if dest}<button class="link small" onclick={() => (dest = null)}>{t("create.backDefault")}</button>{/if}
          </div>
        {#if err}<p class="err small">{err}</p>{/if}
        <button class="btn primary" disabled={making || !name.trim() || !order.length} onclick={make}>
          {making ? t("create.making") : t("create.make", { n: order.length })}
        </button>
        </div>
        <details class="adv">
          <summary class="small muted">{t("create.liveVersion")}</summary>
          <p class="small faint">{t("create.liveHint")}</p>
          <select value={plan.live} onchange={(e) => { live = e.currentTarget.value; locked = {}; picked = null; }}>
            {#each plan.versions as v}<option value={v.label}>{t("create.live", { v: v.label })}</option>{/each}
          </select>
        </details>
      </aside>

      <div class="main">
        <section class="card">
          <div class="sh">
            <h2>{t("create.structure")}</h2>
            <span class="small faint">{t("create.structInfo", { bars: total, len: mmss(plan.structure.seconds), bpm: plan.structure.bpm })}
              {seed !== null && plan.structure.variants > 1 ? t("create.structDrawn", { n: plan.structure.variants, style: plan.style }) : plan.structure.learnt_from ? t("create.structLearnt", { n: Number(plan.structure.learnt_from) }) : t("create.structDefault")}</span>
          </div>
          <div class="bars">
            {#each plan.structure.sections as s, i (i + s.name)}
              <div class="sec t-{s.name.split(' ')[0].toLowerCase()}" style:flex-grow={Number(s.bars)} title={t("create.section", { name: s.name, bars: s.bars, from: Number(s.bar) + 1 })}><b>{s.name}</b><span>{s.bars}</span></div>
            {/each}
          </div>
          <p class="small faint">{t("create.markers")}</p>
        </section>

        {#each plan.parts as p (p.part)}
          {@const on = !!(picked?.has(p.part) && p.pick)}
          {@const k = p.pick ? p.pick.rel + "|" + p.pick.track : ""}
          {@const v = views[k]}
          <section class="card part" class:off={!p.pick} class:idle={!on}>
            <div class="ph">
              <label class="pn"><input type="checkbox" disabled={!p.pick} checked={on} onchange={() => tick(p.part)} />{partName(p.part)}</label>
              {#if p.pick}
                <span class="src small muted" title={t("create.srcTitle", { title: p.pick.title, track: p.pick.track_name })}>{t("create.src", { title: p.pick.title, track: p.pick.track_name })}</span>
                <span class="why small" class:kept={!!locked[p.part]}>{why(p.pick, Number(p.pool))}</span>
                <div class="ctl">
                  <select value={p.pick.key} onchange={(e) => lock(p.part, e.currentTarget.value)} title={t("create.chooseChain")}>
                    {#each p.alts as a (a.key)}<option value={a.key}>{a.label || t("create.emptyTrack")} · {a.in_style ? t("create.inStyle", { n: a.in_style, style: plan.style }) : t("songs.nSongs", { n: a.songs })}</option>{/each}
                    {#if !p.alts.some((a) => a.key === p.pick!.key)}<option value={p.pick.key}>{p.label}</option>{/if}
                  </select>
                  <button class="ib" disabled={p.alts.length < 2} onclick={() => rerollPart(p.part)} title={t("create.rerollPartTitle")} aria-label={t("create.rerollPart", { part: partName(p.part) })}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 5v6h-6" /></svg></button>
                  <button class="ib" class:on={!!locked[p.part]} onclick={() => lock(p.part, locked[p.part] ? undefined : p.pick!.key)}
                    title={locked[p.part] ? t("create.keptTitle") : t("create.keepTitle")} aria-label={t("create.keep", { part: partName(p.part) })}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="11" width="14" height="10" rx="2" /><path d={locked[p.part] ? "M8 11V7a4 4 0 0 1 8 0v4" : "M8 11V7a4 4 0 0 1 7.5-2"} /></svg></button>
                </div>
              {:else}
                <span class="small faint">{t("create.noChain", { v: plan.live })}</span>
              {/if}
            </div>
            {#if p.pick && on}
              <div class="dv">
                {#if v === "loading" || v === undefined}<span class="small faint">{t("create.readingTrack")}</span>
                {:else if v === "error"}<span class="small faint">{p.label}</span>
                {:else}<Devices devices={v} />{/if}
              </div>
            {/if}
          </section>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .sub { margin: 0 0 18px; max-width: 860px; }
  h2 { margin: 0; font-size: 15px; }
  .grid { display: grid; grid-template-columns: minmax(250px, 290px) minmax(0, 1fr); gap: 16px; align-items: start; }
  @media (max-width: 1000px) { .grid { grid-template-columns: 1fr; } }
  .main { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .form { display: flex; flex-direction: column; gap: 10px; position: sticky; top: 0; }
  .form label { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .form p { margin: 0; }
  .two { display: grid; grid-template-columns: 90px minmax(0, 1fr); gap: 8px; }
  .grow { min-height: 4px; }
  .dice { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 9px 12px; font-weight: 600; }
  .dice svg { width: 18px; height: 18px; transition: transform .35s ease; }
  .dice:hover svg { transform: rotate(90deg); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; text-align: left; }
  .adv summary { cursor: pointer; }
  .dest { display: flex; flex-direction: column; gap: 4px; padding: 10px; border-radius: 8px; background: var(--panel2); border: 1px solid var(--line); }
  .dest code { font-family: ui-monospace, monospace; font-size: 11.5px; color: var(--muted); word-break: break-all; line-height: 1.45; }
  .dest code b { color: var(--ink); }
  .row2 { display: flex; gap: 12px; flex-wrap: wrap; }
  .adv select { margin-top: 6px; width: 100%; }
  .part { padding: 10px 14px; }
  .part.off, .part.idle { opacity: .6; }
  .ph { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .pn { display: flex; gap: 8px; align-items: center; font-weight: 600; cursor: pointer; min-width: 140px; }
  .src { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; flex: 1 1 160px; }
  .why { padding: 1px 8px; border-radius: 99px; background: var(--accent-bg); color: var(--ink); white-space: nowrap; }
  .why.kept { background: color-mix(in srgb, var(--ok) 22%, transparent); }
  .ctl { display: flex; gap: 4px; align-items: center; margin-left: auto; }
  .ctl select { max-width: 260px; }
  .ib { width: 30px; height: 30px; border: 1px solid var(--line); border-radius: 7px; background: var(--panel); color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .ib svg { width: 15px; height: 15px; }
  .ib:hover:not(:disabled) { color: var(--ink); border-color: var(--faint); }
  .ib.on { color: var(--ok); border-color: var(--ok); }
  .ib:disabled { opacity: .35; cursor: default; }
  .dv { margin-top: 10px; }
  .sh { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .bars { display: flex; gap: 3px; margin: 10px 0 8px; }
  .sec { flex-basis: 0; min-width: 0; padding: 7px 8px; border-radius: 6px; display: flex; flex-direction: column; gap: 1px; overflow: hidden;
    background: color-mix(in srgb, var(--muted) 16%, transparent); }
  .sec b { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sec span { font-size: 11px; opacity: .75; }
  .t-intro, .t-outro { background: color-mix(in srgb, var(--muted) 18%, transparent); }
  .t-buildup { background: color-mix(in srgb, var(--lost) 24%, transparent); }
  .t-drop { background: color-mix(in srgb, var(--accent) 30%, transparent); }
  .t-break { background: color-mix(in srgb, var(--ok) 22%, transparent); }
  .t-verse { background: color-mix(in srgb, var(--fix) 22%, transparent); }
  .err { color: var(--bad); margin: 0; }
  .done { max-width: 720px; }
  .done .path { font-family: ui-monospace, monospace; color: var(--muted); word-break: break-all; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; }
</style>

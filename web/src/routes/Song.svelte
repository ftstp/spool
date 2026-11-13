<script lang="ts">
  // One song's sheet: its fields, stage, reference render, projects, finishing checks, mood board and
  // devices. Every change goes through the core and can be undone (Ctrl+Z).
  import * as api from "../lib/api";
  import type { Check, SongDetail } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { STAGES, ago, bpm, dur, hue, stageLabel } from "../lib/format";
  import { t, fmtDate, compare } from "../lib/i18n/index.svelte";
  import Field from "../lib/components/Field.svelte";
  import Stars from "../lib/components/Stars.svelte";
  import { focus } from "../lib/focus.svelte";
  import type { Device } from "../lib/api/bindings/Device";
  import { statusSeverity } from "../lib/severity";
  // a plugin's state, as Repair counts it: 1 the app repairs it, 2 a version to install,
  // 3 not installed (an ignored one counts for nothing)
  const PSAY_KEYS = ["ok", "renamed", "trial", "vst3", "other_vst2", "newer", "to_vst3", "other_vst3", "missing"];
  const psay = (st?: string | null) => (st && PSAY_KEYS.includes(st) ? t(`song.psay.${st}`) : st);
  const devSev = (x: Device) => (x.native || x.ignored ? 0 : statusSeverity(x.status));
  const devTip = (x: Device) =>
    x.native ? t("song.dev.native") : [x.name, x.ignored ? t("song.dev.ignored") : psay(x.status), x.instead && t("song.dev.here", { name: x.instead }), x.count > 1 && t("song.dev.instances", { n: x.count }), t("song.dev.clickHint")].filter(Boolean).join("\n");
  function openPlugin(x: Device) {
    if (x.native || !x.key) return;
    focus.plugin = x.key;
    location.hash = "#/repair";
  }
  import Player from "../lib/components/Player.svelte";
  import MoodBoard from "../lib/components/MoodBoard.svelte";
  import Merges from "../lib/components/Merges.svelte";
  import { player } from "../lib/player.svelte";
  import { openSongMenu } from "../lib/contextMenus.svelte";

  /** `embedded`: shown in a tab beside the songs list (no way back: the list is there). */
  let { id, embedded = false }: { id: number; embedded?: boolean } = $props();
  let d = $state<SongDetail | null>(null);
  let missing = $state(false);

  // the newest answer only: two fetches close together may come back out of order
  let seqSong = 0;
  let seqGuess = 0;
  $effect(() => {
    void app.version; // fetched again after every change
    const want = id;
    const n = ++seqSong;
    api.song(want).then((r) => {
      if (want !== id || n !== seqSong) return;
      d = r;
      missing = !r;
    });
  });

  const s = $derived(d?.song);
  /** What the app would suggest for the style and the key, shown on hover even once noted. */
  let guess = $state<api.StyleOpt[]>([]);
  $effect(() => {
    void app.version;
    const want = id;
    const n = ++seqGuess;
    api.styleGuess(want).then((g) => want === id && n === seqGuess && (guess = g), () => (guess = []));
  });
  const styleHint = $derived.by(() => {
    if (!guess.length) return t("song.hint.styleNone");
    return t("song.hint.head") + "\n" + guess.map((o) => `${t("song.hint.stylePct", { style: o.style, pct: o.pct })}${o.why.length ? "\n   " + o.why.join("\n   ") : ""}`).join("\n");
  });
  const keyHintText = $derived.by(() => {
    const l: string[] = [];
    if (main?.key) l.push(t("song.hint.liveKey", { key: main.key }));
    if (main?.key_raw) {
      const pct = Math.round((main.key_conf ?? 0) * 100);
      const sure = !!main.key_est;
      l.push(
        t(main.key_bass ? "song.hint.midiBass" : "song.hint.midi", { key: main.key_raw, pct }) +
          (sure ? "" : "\n   " + t("song.hint.unsure")),
      );
    }
    if (!l.length) return t("song.hint.keyNone");
    return t("song.hint.head") + "\n" + l.join("\n");
  });
  /** Which of its projects a repair would improve. */
  let plans = $state<Record<string, api.RepairPlan | null>>({});
  $effect(() => {
    const files = d?.files ?? [];
    Promise.all(files.map((f) => api.repairPlan(f.rel).then((p) => [f.rel, p] as const))).then((l) => (plans = Object.fromEntries(l)));
  });
  async function repairNow(rel: string) {
    try {
      await api.repairRun([rel]);
      app.say(t("song.repairStarted"));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const edit = (body: Record<string, unknown>) => app.act(api.songEdit(id, body));
  // what the fields offer: the artists known (settings, folders, songs), the styles used,
  // every key
  const uniq = (xs: (string | null | undefined)[]) =>
    [...new Set(xs.map((x) => (x ?? "").trim()).filter(Boolean))].sort(compare);
  const artistOpts = $derived(uniq([...(app.config?.artists ?? []), ...(app.config?.roots ?? []).map((r) => r.artist), ...app.songs.map((x) => x.artist)]));
  const styleOpts = $derived(uniq(app.songs.flatMap((x) => (x.style ?? "").split(/\s*,\s*/))));
  const NOTES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
  const keyOpts = $derived(uniq([...NOTES.map((n) => n + "m"), ...NOTES, ...app.songs.map((x) => x.key)]));
  const kindOpts = $derived(uniq(["Original", "Remix", "Collab", "Remake", "Cover", "Edit", "VIP", "Live", ...app.songs.flatMap((x) => x.kinds)]));
  async function copy(rel: string) {
    try {
      app.say(t("song.pathCopied", { path: await api.copyPath(rel) }));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function show(rel: string) {
    try {
      await api.reveal(rel);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const main = $derived(d?.files.find((f) => f.latest) ?? d?.files[0]);
  const temoin = $derived(
    d && s?.temoin ? [...d.temoins, ...d.other_audio].find((c) => c.rel === s.temoin || s.temoin!.replace(/\\/g, "/").endsWith("/" + c.rel)) : undefined,
  );
  const cover = $derived(s?.cover ? api.mediaUrl(s.cover) : "");
  const keyHint = $derived(main ? main.key ?? (main.key_est ? "~" + main.key_est : "") : "");
  const doneChecks = $derived(d ? d.checklist.filter((c) => c.done).length : 0);

  const STATUS_KEYS = ["renamed", "other_vst2", "trial", "vst3", "newer", "to_vst3", "other_vst3", "missing"];
  const statusLabel = (st: string) => (STATUS_KEYS.includes(st) ? t(`song.status.${st}`) : st);
  const sevLabel = (n: number) => (n >= 0 && n <= 3 ? t(`song.sev.s${n}`) : undefined);

  function cycle(c: Check) {
    // not done -> done -> back to what the tool sees (or not done when the tool saw it done)
    const next = c.na ? null : !c.done ? true : c.manual ? (c.auto ? false : null) : false;
    app.act(api.checkSet(id, c.key, next));
  }
  const day = (ts: number) => fmtDate(ts, { day: "numeric", month: "short", year: "numeric" });
</script>

{#if missing}
  <div class="page"><p class="muted">{t("song.gone")} <a href="#/songs">{t("song.backToSongs")}</a></p></div>
{:else if d && s}
  <!-- data-song: a right-click anywhere on the sheet opens the song's menu -->
  <div class="page" data-song={s.id}>
    {#if s.archived}
      <div class="archived small">{t("song.archived")}
        <button class="btn sm" onclick={() => edit({ archived: false })}>{t("song.unarchive")}</button></div>
    {/if}
    <header class="hero">
      <div class="art" style="--h:{hue(s.title ?? '')}">
        {#if cover}<img src={cover} alt="" />{:else}{(s.title || "?").slice(0, 1).toUpperCase()}{/if}
      </div>
      <div class="id">
        {#if !embedded}<a class="back small" href="#/songs">{t("song.back")}</a>{/if}
        <Field big value={s.title} placeholder={t("song.ph.title")} save={(v) => edit({ title: v })} />
        <div class="meta">
          <Field value={s.artist} placeholder={t("song.ph.artist")} options={artistOpts} save={(v) => edit({ artist: v })} />
          <span class="dot">·</span>
          <Field value={s.style} placeholder={t("song.ph.style")} options={styleOpts} hint={styleHint} save={(v) => edit({ style: v })} />
          <span class="dot">·</span>
          <Field value={s.key} placeholder={keyHint ? t("song.ph.keyProject", { key: keyHint }) : t("song.ph.key")} options={keyOpts} hint={keyHintText} save={(v) => edit({ key: v })} />
          <span class="dot">·</span>
          <Field kind="tempo" value={s.tempo} placeholder={main?.tempo ? t("song.ph.tempoProject", { n: Math.round(main.tempo * 10) / 10 }) : t("song.ph.tempo")} save={(v) => edit({ tempo: v })} />
          <span class="dot">·</span>
          <Field kind="date" value={s.date} placeholder={t("song.ph.date")} save={(v) => edit({ date: v })} />
        </div>
        <div class="meta">
          <Stars value={s.rating} set={(n) => edit({ rating: n })} />
          <span class="dot">·</span>
          <Field value={(s.kinds ?? []).join(", ")} placeholder={t("song.ph.kind")} options={kindOpts} save={(v) => edit({ kinds: v.split(",").map((x) => x.trim()).filter(Boolean) })} />
          <span class="dot">·</span>
          <Field value={s.ft} placeholder={t("song.ph.ft")} save={(v) => edit({ ft: v })} />
        </div>
        <div class="stages" role="radiogroup" aria-label={t("song.stage")}>
          {#each STAGES as st}
            <button role="radio" aria-checked={s.stage === st} class="seg s-{st}" class:on={s.stage === st}
              onclick={() => edit({ stage: s.stage === st ? null : st })}><i></i>{stageLabel(st)}</button>
          {/each}
        </div>
      </div>
      <div class="acts">
        {#if main}<button class="btn" onclick={() => api.openInLive(main.rel).catch((e) => app.say(String(e), { error: true }))}>{t("song.openInLive")}</button>{/if}
        <!-- the rest (folder, paths, release, archive) in the song's menu: here, or a right-click anywhere on the sheet -->
        <button class="btn more" title={t("song.moreTip")} aria-label={t("song.more")} onclick={(e) => openSongMenu(e, s.id)}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg></button>
      </div>
    </header>

    {#each d.suggestions as g (g.id)}
      <div class="sugg">
        <div><b>{t("song.probableStage", { stage: stageLabel(g.value) })}</b><div class="small muted">{g.reasons.join(" · ")}</div></div>
        <div class="row-btns">
          <button class="btn sm primary" onclick={() => app.act(api.suggestionAct(g.id, true))}>{t("song.accept")}</button>
          <button class="btn sm" onclick={() => app.act(api.suggestionAct(g.id, false))}>{t("song.decline")}</button>
        </div>
      </div>
    {/each}
    {#if d.style_sugg && !s.style}
      {@const g = d.style_sugg}
      <div class="sugg">
        <div><b>{t("song.probableStyle")}</b><div class="small muted">{t("song.styleClick")}</div></div>
        <div class="row-btns">
          {#each g.options as o}
            <button class="chip" title={o.why.join("\n")} onclick={() => app.act(api.suggestionAct(g.id, true, o.style))}>{o.style} <b>{t("song.hint.pct", { pct: o.pct })}</b></button>
          {/each}
          <button class="btn sm" onclick={() => app.act(api.suggestionAct(g.id, false))}>{t("song.noStyle")}</button>
        </div>
      </div>
    {/if}

    <div class="grid">
      <div class="col">
        <section class="card">
          <div class="head"><h2>{t("song.temoin.title")}</h2>
            <span class="small faint">{s.temoin_source === "user" ? t("song.temoin.byYou") : temoin ? t("song.temoin.byApp") : ""}</span></div>
          {#if temoin}
            <Player rel={s.temoin ?? temoin.rel} sid={id} title={s.title ?? ""} />
          {:else}
            <p class="small faint">{t("song.temoin.none")}</p>
          {/if}
          {#if d.temoins.length}
            <details class="more">
              <summary class="small muted">{t("song.temoin.renders", { n: d.temoins.length })}</summary>
              {#each d.temoins as c (c.rel)}
                <div class="line">
                  <div class="grow ell" title={c.rel}>{c.rel.split("/").pop()}
                    <span class="small faint">{c.where === "MP3" ? "- MP3" : ""} {c.kind} {ago(c.mtime)}</span></div>
                  <button class="icon" title={t("song.temoin.listen")} aria-label={t("song.temoin.listen")} onclick={() => player.play(c.rel, id, s.title ?? "")}>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" /></svg></button>
                  {#if c === temoin}<span class="small tag">{t("song.temoin.tag")}</span>
                  {:else if c.pickable}<button class="btn sm" onclick={() => edit({ temoin: c.rel })}>{t("song.temoin.choose")}</button>{/if}
                </div>
              {/each}
              {#if s.temoin_source === "user"}<button class="link small" onclick={() => edit({ temoin: null })}>{t("song.temoin.letApp")}</button>{/if}
            </details>
          {/if}
          {#if d.other_audio.length}
            <details class="more" open={!d.temoins.length}>
              <summary class="small muted">{t("song.temoin.otherAudio", { n: d.other_audio.length })}</summary>
              <p class="small faint">{t("song.temoin.otherAudioHint")}</p>
              {#each d.other_audio as c (c.rel)}
                <div class="line" data-media={c.rel} data-kind="audio">
                  <div class="grow ell" title={c.rel}>{c.rel.split("/").pop()} <span class="small faint">{ago(c.mtime)}</span></div>
                  <button class="icon" title={t("song.temoin.listen")} aria-label={t("song.temoin.listen")} onclick={() => player.play(c.rel, id, s.title ?? "")}>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor"><path d="M8 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 8 5.5z" /></svg></button>
                  {#if c === temoin}<span class="small tag">{t("song.temoin.tag")}</span>
                  {:else}<button class="btn sm" onclick={() => edit({ temoin: c.rel })}>{t("song.temoin.choose")}</button>{/if}
                </div>
              {/each}
            </details>
          {/if}
          {#if d.near_temoin.length}
            <div class="small faint near">{t("song.temoin.near")}
              {#each d.near_temoin as n, i}{i ? ", " : " "}<span title={n.rel}>{n.name} ({[t("song.temoin.nearDays", { n: n.days, d: (n.days > 0 ? "+" : "") + n.days }), n.old && t("song.temoin.oldVersion"), !n.same && n.title].filter(Boolean).join(", ")})</span>{/each}
            </div>
          {/if}
        </section>

        <section class="card">
          <div class="head"><h2>{t("song.projects.title")}</h2><span class="small faint">{d.hidden ? t("song.projects.oldVersions", { n: d.hidden }) : ""}{d.backups ? ` · ${t("song.projects.backups", { n: d.backups })}` : ""}</span></div>
          {#each d.files as f (f.rel)}
            <div class="file">
              <div class="line">
                <span class="sev s{f.sev}" title={sevLabel(f.sev)}></span>
                <div class="grow">
                  <div class="ell" title={f.rel}><b>{f.name}</b>{#if f.latest} <span class="tag small">{t("song.projects.latest")}</span>{/if}</div>
                  <div class="small faint ell">{[sevLabel(f.sev), f.live && "Live " + f.live, f.tempo && bpm(f.tempo), dur(f.len), ago(f.mtime)].filter(Boolean).join(" · ")}</div>
                </div>
                {#if plans[f.rel]?.useful}<button class="btn sm" title={plans[f.rel]?.complete ? t("song.projects.repairComplete") : t("song.projects.repairPartial")} onclick={() => repairNow(f.rel)}>{t("song.projects.repair")}</button>{/if}
                <button class="icon" title={t("song.projects.copyFullPath")} aria-label={t("song.projects.copyPath")} onclick={() => copy(f.rel)}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V5a1 1 0 0 1 1-1h10" /></svg>
                </button>
                <button class="icon" title={t("song.projects.reveal")} aria-label={t("song.projects.reveal")} onclick={() => show(f.rel)}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg>
                </button>
                <button class="icon" title={t("song.openInLive")} aria-label={t("song.openInLive")} onclick={() => api.openInLive(f.rel)}>
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" /></svg>
                </button>
              </div>
              {#if f.error}<div class="small bad">{t("song.projects.unreadable", { error: f.error })}</div>{/if}
              {#if f.issues.length || f.samples_lost.length}
                <div class="issues small">
                  {#each f.issues as i}
                    {@const c = (i.candidates as { name: string }[])[0]}
                    <button class="iss v{i.ignored ? 0 : statusSeverity(i.status)}" class:ign={i.ignored}
                      title={[psay(i.status), c && t("song.dev.here", { name: c.name }), i.need_label, t("song.dev.clickHint")].filter(Boolean).join("\n")}
                      onclick={() => { focus.plugin = `${i.format}|${i.name}|${i.status}`; location.hash = "#/repair"; }}>{i.name}{i.n > 1 ? ` ×${i.n}` : ""} <span class="faint">{i.ignored ? t("song.statusIgnored", { status: statusLabel(i.status) }) : statusLabel(i.status)}</span></button>
                  {/each}
                  {#if f.samples_lost.length}<span class="iss">{t("song.projects.samplesLost", { n: f.samples_lost.length })}</span>{/if}
                </div>
              {/if}
            </div>
          {/each}
          {#if d.versions.length}
            <details class="more">
              <summary class="small muted">{t("song.projects.oldVersions", { n: d.versions.length })}</summary>
              {#each d.versions as v (v.rel)}<div class="line small"><span class="grow ell" title={v.rel}>{v.name}</span><span class="faint">{ago(v.mtime)}</span></div>{/each}
            </details>
          {/if}
          <Merges sid={id} {d} />
        </section>
      </div>

      <div class="col">
        <section class="card">
          <div class="head"><h2>{t("song.checks.title")}</h2><span class="small faint">{doneChecks}/{d.checklist.length}</span></div>
          {#each d.checklist as c (c.key)}
            <div class="check" class:na={c.na}>
              <button class="box" class:on={c.done && !c.na} role="checkbox" aria-checked={c.done} aria-label={c.label} onclick={() => cycle(c)}>
                {#if c.done && !c.na}<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="3"><path d="M5 12l5 5 9-10" /></svg>{/if}
              </button>
              <div class="grow"><div>{c.label}{#if c.manual && !c.na} <span class="faint small">{t("song.checks.manual")}</span>{/if}</div>
                {#if c.why && !c.na}<div class="small faint ell" title={c.why}>{c.why}</div>{/if}</div>
              <button class="link small" onclick={() => app.act(api.checkSet(id, c.key, c.na ? null : "na"))}>{c.na ? t("song.checks.applies") : t("song.checks.na")}</button>
            </div>
          {/each}
        </section>

        <section class="card">
          <h2>{t("song.notes.title")}</h2>
          <textarea rows="5" placeholder={t("song.notes.placeholder")} value={s.notes ?? ""}
            onblur={(e) => { const v = e.currentTarget.value.trim(); if (v !== (s.notes ?? "")) edit({ notes: v }); }}></textarea>
          {#if d.log.length}
            <div class="small faint log">{#each d.log as l, i}{i ? " → " : ""}{stageLabel(l.stage)} <span title={day(l.at)}>({ago(l.at)})</span>{/each}</div>
          {/if}
        </section>
      </div>

      <div class="col">
        <MoodBoard sid={id} board={d.board} />

        <section class="card">
          <h2>{t("song.devices.title")}</h2>
          {#each [[t("song.devices.instr"), d.devices.instr], [t("song.devices.fx"), d.devices.fx], [t("song.devices.midi"), d.devices.midi]] as const as [label, list]}
            {#if list.length}
              <h3 class="small muted">{label}</h3>
              <div class="devs">{#each list.slice(0, 14) as x}
                {#if x.native}<span class="dev" title={devTip(x)}>{x.name}{x.count > 1 ? ` ×${x.count}` : ""}</span>
                {:else}<button class="dev plug v{devSev(x)}" class:ign={x.ignored} title={devTip(x)} onclick={() => openPlugin(x)}>{x.name}{x.count > 1 ? ` ×${x.count}` : ""}</button>{/if}
              {/each}</div>
            {/if}
          {/each}
          {#if d.main_chain.length}<p class="small faint">{t("song.devices.chain", { chain: d.main_chain.join(" → ") })}</p>{/if}
        </section>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { padding: 20px 28px 60px; container-type: inline-size; }
  .hero { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 20px; align-items: start; margin-bottom: 16px; }
  .art { width: 112px; height: 112px; border-radius: 14px; overflow: hidden; display: grid; place-items: center; font-size: 40px; font-weight: 650; color: #fff;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .id { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .back { color: var(--muted); width: fit-content; }
  .back:hover { color: var(--ink); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 12px; color: var(--muted); }
  .dot { color: var(--faint); }
  .stages { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  .seg { display: inline-flex; align-items: center; gap: 7px; padding: 4px 11px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; font-size: 12.5px; color: var(--muted); transition: border-color .15s, color .15s; }
  .seg:hover { color: var(--ink); border-color: var(--faint); }
  .seg i { width: 7px; height: 7px; border-radius: 50%; background: var(--s-none); }
  .seg.on { color: var(--ink); font-weight: 600; border-color: currentColor; }
  .s-draft i { background: var(--s-draft); } .s-arrangement i { background: var(--s-arrangement); } .s-mix i { background: var(--s-mix); }
  .s-master i { background: var(--s-master); } .s-done i { background: var(--s-done); } .s-pause i { background: var(--s-pause); }
  .s-draft.on { border-color: var(--s-draft); } .s-arrangement.on { border-color: var(--s-arrangement); } .s-mix.on { border-color: var(--s-mix); }
  .s-master.on { border-color: var(--s-master); } .s-done.on { border-color: var(--s-done); } .s-pause.on { border-color: var(--s-pause); }
  .acts { display: flex; gap: 8px; }
  .sugg { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 14px; margin-bottom: 10px; border-radius: var(--radius); border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--line)); background: var(--accent-bg); }
  .row-btns { display: flex; gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
  .chip { font-size: 12px; padding: 3px 10px; border-radius: 99px; border: 1px solid var(--line); background: var(--panel); cursor: pointer; }
  .chip:hover { border-color: var(--accent); }
  .grid { display: grid; grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr) minmax(0, 1fr); gap: 16px; align-items: start; margin-top: 6px; }
  /* by the room the sheet has, not the window: beside the list it is narrower */
  @container (max-width: 1250px) { .grid { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); } .grid > .col:last-child { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr; } }
  @container (max-width: 760px) { .grid, .grid > .col:last-child { grid-template-columns: minmax(0, 1fr); } .hero { grid-template-columns: auto minmax(0, 1fr); } .hero .acts { grid-column: 1 / -1; } }
  .col { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; }
  .line { display: flex; align-items: center; gap: 10px; padding: 5px 0; min-width: 0; }
  .grow { flex: 1; min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { padding: 0 7px; border-radius: 99px; background: var(--accent-bg); color: var(--accent); font-size: 11px; font-weight: 600; }
  .more { margin-top: 8px; }
  .more summary { cursor: pointer; padding: 3px 0; }
  .near { margin-top: 8px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .file { padding: 4px 0; border-bottom: 1px solid var(--line); }
  .file:last-of-type { border-bottom: 0; }
  .sev { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--ok); }
  .sev.s1 { background: var(--fix); } .sev.s2 { background: var(--lost); } .sev.s3 { background: var(--bad); }
  .icon { width: 26px; height: 26px; flex: none; border-radius: 7px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .icon:hover { color: var(--accent); border-color: var(--accent); }
  .issues { display: flex; flex-wrap: wrap; gap: 4px 8px; padding: 0 0 6px 18px; }
  .iss { padding: 1px 8px; border-radius: 6px; background: color-mix(in srgb, var(--lost) 12%, transparent); border: 0; font: inherit; font-size: 12px; color: var(--ink); }
  button.iss { cursor: pointer; }
  button.iss:hover { outline: 1px solid var(--accent); }
  .iss.v1 { background: color-mix(in srgb, var(--fix) 14%, transparent); }
  .iss.v2 { background: color-mix(in srgb, var(--lost) 16%, transparent); }
  .iss.v3 { background: color-mix(in srgb, var(--bad) 16%, transparent); }
  .iss.ign { opacity: .55; }
  .bad { color: var(--bad); }
  .check { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--line); }
  .check:last-child { border-bottom: 0; }
  .check.na { opacity: .5; }
  .check.na .grow > div:first-child { text-decoration: line-through; }
  .box { width: 18px; height: 18px; flex: none; border-radius: 5px; border: 1.5px solid var(--faint); background: none; cursor: pointer; display: grid; place-items: center; color: #111; padding: 0; transition: background-color .15s, border-color .15s; }
  .box.on { background: var(--ok); border-color: var(--ok); }
  textarea { width: 100%; resize: vertical; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; color: inherit; font: inherit; outline: 0; }
  textarea:focus { border-color: var(--accent); }
  .log { margin-top: 8px; }
  h3 { margin: 10px 0 4px; font-weight: 600; }
  .devs { display: flex; flex-wrap: wrap; gap: 4px; }
  .dev { font-size: 12px; padding: 1px 8px; border-radius: 6px; background: var(--panel2); border: 1px solid var(--line); }
  .dev.plug { border-color: color-mix(in srgb, var(--accent) 35%, var(--line)); cursor: pointer; color: var(--ink); font: inherit; font-size: 12px; }
  .dev.plug:hover { border-color: var(--accent); }
  .dev.v1 { border-color: var(--fix); }
  .dev.v2 { border-color: var(--lost); color: var(--lost); }
  .dev.v3 { border-color: var(--bad); color: var(--bad); }
  .dev.ign { opacity: .55; text-decoration: line-through; }
  .archived { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 12px; margin-bottom: 12px; border-radius: 9px;
    background: color-mix(in srgb, var(--lost) 14%, transparent); }
  .more { padding: 6px 9px; display: inline-grid; place-items: center; }
</style>

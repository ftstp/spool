<script lang="ts">
  // One release: its cover, title, kind, date, and the tracklist in order (drag a track to
  // move it, add a song from the field below, play the reference renders one after another). Every
  // change is saved at once and undone by Ctrl+Z.
  import * as api from "../lib/api";
  import type { Release, SongItem } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { player } from "../lib/player.svelte";
  import { openSong } from "../lib/tabs.svelte";
  import Art from "../lib/components/Art.svelte";
  import Stage from "../lib/components/Stage.svelte";
  import PlayBtn from "../lib/components/PlayBtn.svelte";
  import { KIND_KEYS, kinds, countdown, daysUntil, kindLabel, longDate, minutes, summary } from "../lib/releases";
  import { dur } from "../lib/format";
  import { t } from "../lib/i18n/index.svelte";

  let { id }: { id: string } = $props();
  const r = $derived(app.releases.find((x) => x.id === id) ?? null);
  const s = $derived(r ? summary(r, app.songs) : null);

  async function save(patch: Partial<Release>) {
    if (!r) return;
    try {
      const [, st] = await app.track(api.releaseSave({ ...r, ...patch }));
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function remove() {
    // nothing on disk is touched, and Ctrl+Z gives it back: no question asked
    if (!r) return;
    try {
      app.undo = await api.releaseDelete(r.id);
      if (app.undo.label) app.say(app.undo.label, { undo: true });
      location.hash = "#/releases";
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }

  // drag a track onto another to move it there
  let drag = $state<number | null>(null);
  let over = $state<number | null>(null);
  function drop(to: number) {
    if (!r || drag === null || drag === to) return reset();
    const songs = [...r.songs];
    const [m] = songs.splice(drag, 1);
    songs.splice(to, 0, m);
    reset();
    save({ songs });
  }
  const reset = () => ((drag = null), (over = null));
  const removeAt = (i: number) => r && save({ songs: r.songs.filter((_, j) => j !== i) });

  // add a song: a field that finds it by title, artist or folder
  let q = $state("");
  const norm = (x: string | null | undefined) => (x ?? "").normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const found = $derived.by((): SongItem[] => {
    const n = norm(q.trim());
    if (!n || !r) return [];
    return app.songs.filter((x) => !x.archived && !r.songs.includes(x.id) && [x.title, x.artist, x.dir].some((v) => norm(v).includes(n))).slice(0, 8);
  });
  function add(sid: number) {
    if (!r) return;
    q = "";
    save({ songs: [...r.songs, sid] });
  }

  // play the reference renders one after another, from a track
  let chain = $state<number[]>([]);
  function playFrom(i: number) {
    if (!s) return;
    chain = s.tracks.slice(i).filter((x): x is SongItem => !!x?.temoin).map((x) => x.id);
    next();
  }
  function next() {
    const sid = chain.shift();
    if (sid != null) player.playSong(sid, app.songs.find((x) => x.id === sid)?.title ?? "");
  }
  // when a reference render of the chain ends, the next starts
  $effect(() => {
    if (chain.length && !player.playing && player.len && player.t >= player.len - 0.3) next();
  });
  const playable = $derived(s ? s.tracks.filter((x) => x?.temoin).length : 0);

  // "Ranger sur le disque": what would move is shown first, nothing moves before the click
  let arranging = $state(false);
  let parent = $state<string | null>(null);
  let plan = $state<import("../lib/api/bindings/ArrangePlan").ArrangePlan | null>(null);
  let planErr = $state("");
  let moving = $state(false);
  $effect(() => {
    if (!arranging || !app.physical || !r) return;
    void app.version;
    planErr = "";
    api.releaseArrangePlan(r.id, parent).then((p) => (plan = p)).catch((e) => ((plan = null), (planErr = String(e))));
  });
  async function pickParent() {
    const f = await api.pickFolder(t("releases.page.pickParent")).catch(() => null);
    if (f) parent = f;
  }
  async function arrange() {
    if (!r) return;
    moving = true;
    const res = await app.fileAct(api.releaseArrange(r.id, parent));
    moving = false;
    if (res) {
      const left = res.skipped.length;
      if (left)
        app.say(
          t("releases.page.arranged", {
            done: t("releases.page.arrangedDone", { n: res.done.length }),
            left: t("releases.page.arrangedLeft", { n: left }),
            why: res.skipped.map((x) => x.why).join(", "),
          }),
          { undo: true },
        );
      arranging = false;
      parent = null;
    }
  }
  const physOff = $derived(t("releases.page.physOff").split("{link}"));
</script>

{#if !r || !s}
  <div class="page"><p class="muted">{t("releases.page.gone")} <a href="#/releases">{t("releases.page.all")}</a></p></div>
{:else}
  <div class="page">
    <a class="back small" href="#/releases">{t("releases.page.back")}</a>
    <header class="hero">
      <div class="cv">
        <Art title={r.title} cover={s.cover} size="100%" radius={12} />
        {#if s.present.some((x) => x.cover)}
          <select class="cvsel small" value={r.cover_song ?? ""} onchange={(e) => save({ cover_song: e.currentTarget.value ? +e.currentTarget.value : null })} title={t("releases.page.coverTitle")}>
            <option value="">{t("releases.page.coverFirst")}</option>
            {#each s.present.filter((x) => x.cover) as c (c.id)}<option value={c.id}>{t("releases.page.coverOf", { title: c.title })}</option>{/each}
          </select>
        {/if}
      </div>
      <div class="meta">
        <div class="kindrow">
          <select class="kind" value={r.kind} onchange={(e) => save({ kind: e.currentTarget.value })}>
            {#each kinds() as [k, l]}<option value={k}>{l}</option>{/each}
          </select>
          <span class="small muted">{t("releases.nTracks", { n: s.count })}{s.seconds ? ` · ${minutes(s.seconds)}` : ""}</span>
        </div>
        <input class="title" value={r.title} onchange={(e) => save({ title: e.currentTarget.value })} aria-label={t("releases.page.titleLabel")} />
        <input class="artist" value={r.artist} placeholder={t("releases.page.artist")} onchange={(e) => save({ artist: e.currentTarget.value })} aria-label={t("releases.page.artist")} />
        <div class="daterow">
          <label class="small muted">{t("releases.page.date")} <input type="date" value={r.date ?? ""} onchange={(e) => save({ date: e.currentTarget.value || null })} /></label>
          {#if r.date}<span class="cd" class:soon={(daysUntil(r.date) ?? 0) > 1 && s.todo.length}>{countdown(r.date)}</span>{/if}
        </div>
        <div class="prog"><b style:width="{Math.round(s.progress * 100)}%" class:warn={s.blocked > 0}></b></div>
        <p class="small" class:muted={!s.todo.length}>
          {#if s.todo.length}{t("releases.page.todo", { list: s.todo.join(", ") })}{:else if s.count}{r.date ? t("releases.page.allReadyFor", { date: longDate(r.date) }) : t("releases.page.allReady")}{:else}{t("releases.page.addBelow")}{/if}
        </p>
        <div class="acts">
          <button class="btn sm primary" disabled={!playable} onclick={() => playFrom(0)}>{KIND_KEYS.includes(r.kind) ? t(`releases.page.listen.${r.kind}`) : t("releases.page.listen.other", { kind: kindLabel(r.kind) })}</button>
          <button class="btn sm" class:on={arranging} onclick={() => (arranging = !arranging)} title={t("releases.page.arrangeTitle")}>{t("releases.page.arrange")}</button>
          <button class="btn sm" onclick={remove}>{t("releases.page.delete")}</button>
        </div>
        {#if arranging}
          <div class="arr">
            {#if !app.physical}
              <p class="small">{physOff[0]}<a href="#/settings">{t("releases.page.settingsLink")}</a>{physOff[1] ?? ""}</p>
            {:else if planErr}
              <p class="small err">{planErr}</p>
            {:else if plan}
              {#if !plan.moves.length}
                <p class="small">{plan.already ? t("releases.page.nothingAlready", { target: plan.target }) : t("releases.page.nothingCant")}</p>
              {:else}
                <p class="small">{t(plan.exists ? "releases.page.movesExisting" : "releases.page.movesNew", { n: plan.moves.length })}</p>
                <code class="tgt">{plan.target}</code>
                <ul class="mv small muted">{#each plan.moves as m (m)}<li>{m.split("/").pop()}</li>{/each}</ul>
              {/if}
              {#if plan.already && plan.moves.length}<p class="small faint">{t("releases.page.already", { n: plan.already })}</p>{/if}
              {#if plan.skipped.length}<p class="small faint">{t("releases.page.stay", { list: plan.skipped.map((x) => `${x.src.split("/").pop()} (${x.why})`).join(", ") })}</p>{/if}
              <p class="small faint">{t("releases.page.checked")}</p>
              <div class="acts">
                {#if plan.moves.length}<button class="btn sm primary" disabled={moving} onclick={arrange}>{moving ? t("releases.page.arranging") : t("releases.page.arrangeNow")}</button>{/if}
                <button class="btn sm" onclick={pickParent}>{t("releases.page.otherFolder")}</button>
                {#if parent}<button class="link small" onclick={() => (parent = null)}>{t("releases.page.suggested")}</button>{/if}
              </div>
            {:else}<p class="small faint">{t("releases.page.computing")}</p>{/if}
          </div>
        {/if}
      </div>
    </header>

    <section class="tl" aria-label={t("releases.page.tracks")}>
      {#each s.tracks as tk, i (i + ":" + (r.songs[i] ?? "x"))}
        <div class="tk" class:over={over === i && drag !== i} class:dragging={drag === i} role="listitem" draggable="true"
          ondragstart={(e) => { drag = i; e.dataTransfer!.effectAllowed = "move"; }} ondragover={(e) => { e.preventDefault(); over = i; }}
          ondrop={(e) => { e.preventDefault(); drop(i); }} ondragend={reset}>
          <span class="grip" title={t("releases.page.grip")}>⋮⋮</span>
          <span class="n">{i + 1}</span>
          {#if tk}
            <Art title={tk.title} cover={tk.cover} size={36} radius={6} />
            <a class="tt" href="#/song/{tk.id}" onclick={() => openSong(tk.id)}>{tk.title}<span class="small muted">{tk.artist ?? ""}</span></a>
            <span class="pb">{#if tk.temoin}<PlayBtn sid={tk.id} title={tk.title ?? ""} size={26} />{:else}<span class="small faint" title={t("releases.page.noRenderTitle")}>{t("releases.page.noRender")}</span>{/if}</span>
            <Stage stage={tk.stage} />
            <span class="sev">{#if tk.sev >= 3}<span class="bad small">{t("releases.page.bad")}</span>{:else if tk.sev === 2}<span class="lost small">{t("releases.page.lost")}</span>{/if}</span>
            <span class="d small muted">{Number(tk.len) ? dur(Number(tk.len)) : ""}</span>
          {:else}
            <span></span><span class="tt faint">{t("releases.page.missing")}</span><span></span><span></span><span></span><span></span>
          {/if}
          <button class="x" onclick={() => removeAt(i)} title={t("releases.page.removeTitle")} aria-label={t("releases.page.remove")}>✕</button>
        </div>
      {/each}
      <div class="add">
        <input bind:value={q} placeholder={t("releases.page.addPh")} onkeydown={(e) => e.key === "Enter" && found[0] && add(found[0].id)} />
        {#if found.length}
          <div class="res">
            {#each found as f (f.id)}
              <button onclick={() => add(f.id)}><Art title={f.title} cover={f.cover} size={26} radius={5} /><b>{f.title}</b><span class="small muted">{f.artist ?? ""} · {f.dir.split("/").pop()}</span></button>
            {/each}
          </div>
        {/if}
      </div>
    </section>
    {#if r.folder}<p class="small faint">{t("releases.page.fromFolder", { folder: r.folder })}</p>{/if}
  </div>
{/if}

<style>
  .page { padding: 18px 28px 60px; max-width: 1100px; }
  .back { color: var(--muted); }
  .hero { display: grid; grid-template-columns: 200px minmax(0, 1fr); gap: 24px; margin: 12px 0 22px; align-items: start; }
  .cv { display: flex; flex-direction: column; gap: 6px; }
  .cvsel { width: 100%; }
  .meta { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .kindrow, .daterow { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .kind { font-size: 12px; padding: 2px 6px; }
  .title { font-size: 28px; font-weight: 700; border: 1px solid transparent; background: none; padding: 2px 6px; margin-left: -7px; border-radius: 8px; color: var(--ink); }
  .artist { font-size: 15px; border: 1px solid transparent; background: none; padding: 2px 6px; margin-left: -7px; border-radius: 8px; color: var(--muted); }
  .title:hover, .artist:hover, .title:focus, .artist:focus { border-color: var(--line); background: var(--panel); }
  .daterow input { margin-left: 6px; }
  .cd { font-size: 12px; padding: 2px 9px; border-radius: 99px; background: var(--accent-bg); }
  .cd.soon { background: color-mix(in srgb, var(--lost) 22%, transparent); }
  .prog { height: 6px; border-radius: 6px; background: var(--line); overflow: hidden; max-width: 420px; margin-top: 6px; }
  .prog b { display: block; height: 100%; background: var(--ok); }
  .prog b.warn { background: var(--lost); }
  .meta p { margin: 2px 0 0; }
  .acts { display: flex; gap: 8px; margin-top: 8px; align-items: center; flex-wrap: wrap; }
  .btn.on { border-color: var(--accent); background: var(--accent-bg); }
  .arr { margin-top: 10px; padding: 12px 14px; border-radius: 10px; border: 1px solid var(--line); background: var(--panel); max-width: 640px; }
  .arr p { margin: 0 0 6px; }
  .tgt { display: block; font-family: ui-monospace, monospace; font-size: 12px; word-break: break-all; margin: 2px 0 6px; }
  .mv { margin: 0 0 6px; padding-left: 18px; max-height: 140px; overflow: auto; }
  .err { color: var(--bad); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .tl { display: flex; flex-direction: column; border-top: 1px solid var(--line); }
  .tk { display: grid; grid-template-columns: 16px 22px 36px minmax(0, 1fr) 70px 110px 130px 48px 26px; gap: 12px; align-items: center; padding: 7px 8px; border-bottom: 1px solid var(--line); border-radius: 6px; }
  .tk:hover { background: color-mix(in srgb, var(--line) 35%, transparent); }
  .tk.over { box-shadow: inset 0 2px 0 var(--accent); }
  .tk.dragging { opacity: .4; }
  .grip { color: var(--faint); cursor: grab; font-size: 11px; letter-spacing: -2px; }
  .n { color: var(--faint); text-align: right; font-variant-numeric: tabular-nums; }
  .tt { display: flex; flex-direction: column; min-width: 0; color: var(--ink); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tt span { font-weight: 400; }
  .bad { color: var(--bad); } .lost { color: var(--lost); }
  .d { text-align: right; font-variant-numeric: tabular-nums; }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; border-radius: 5px; padding: 3px; opacity: 0; }
  .tk:hover .x { opacity: 1; }
  .x:hover { color: var(--ink); background: var(--line); }
  .add { position: relative; margin-top: 10px; max-width: 520px; }
  .add input { width: 100%; }
  .res { position: absolute; top: 40px; left: 0; right: 0; z-index: 20; background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 4px; box-shadow: 0 14px 30px rgba(0, 0, 0, .3); }
  .res button { display: grid; grid-template-columns: 26px auto minmax(0, 1fr); gap: 10px; align-items: center; width: 100%; border: 0; background: none; padding: 5px 8px; border-radius: 7px; cursor: pointer; text-align: left; color: var(--ink); }
  .res button:hover { background: var(--accent-bg); }
  .res span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

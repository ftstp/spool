<script lang="ts">
  // The Media page's Audio list: the audio files no song uses (not a reference render, not a
  // sample a project plays, not a reference track), by the folder they sit in. Each can be
  // heard, its folder opened, and linked by hand: as a song's reference render, onto a song's
  // mood board, or as a reference for a song, an artist or a release. Nothing is moved on disk.
  import * as api from "../api";
  import type { LooseAudio } from "../api/bindings/LooseAudio";
  import type { SongRef } from "../api/bindings/SongRef";
  import { app } from "../state.svelte";
  import { player, playOrPause, playingFile } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { ago } from "../format";
  import PlayIcon from "./PlayIcon.svelte";

  let { items, q = "", selected = $bindable(new Set<string>()) }: { items: LooseAudio[]; q?: string; selected?: Set<string> } = $props();

  const words = $derived(q.toLowerCase().split(/\s+/).filter(Boolean));
  const shown = $derived(items.filter((a) => words.every((w) => a.rel.toLowerCase().includes(w))));
  type Group = { dir: string; files: LooseAudio[]; songs: SongRef[] };
  const groups = $derived.by(() => {
    const m = new Map<string, Group>();
    for (const a of shown) {
      const dir = a.rel.includes("/") ? a.rel.slice(0, a.rel.lastIndexOf("/")) : "";
      let g = m.get(dir);
      if (!g) m.set(dir, (g = { dir, files: [], songs: a.songs }));
      g.files.push(a);
    }
    return [...m.values()].sort((a, b) => a.dir.localeCompare(b.dir));
  });
  // many folders: the first ones until asked
  const STEP = 40;
  let upTo = $state(STEP);
  let shut = $state<Set<string>>(new Set());
  const toggle = (d: string) => {
    const s = new Set(shut);
    if (s.has(d)) s.delete(d);
    else s.add(d);
    shut = s;
  };
  const pick = (rels: string[], on: boolean) => {
    const s = new Set(selected);
    for (const r of rels) on ? s.add(r) : s.delete(r);
    selected = s;
  };

  const name = (rel: string) => rel.split("/").pop() ?? rel;
  const sizeOf = (b: number) => (b > 1e6 ? `${(b / 1e6).toFixed(1)} Mo` : `${Math.max(1, Math.round(b / 1e3))} ko`);
  const fail = (e: unknown) => app.say(String(e), { error: true });

  // ---- linking one file: what it becomes, and to what
  type As = "temoin" | "board" | "refSong" | "refArtist" | "refRelease";
  type Target = { key: string; label: string; sub?: string; hint?: boolean };
  let open = $state<string | null>(null);
  let as = $state<As>("temoin");
  let find = $state("");
  let songs = $state<{ id: number; title: string | null; artist: string | null }[] | null>(null);
  let artists = $state<string[] | null>(null);
  let releases = $state<{ id: string; title: string; artist: string }[] | null>(null);
  async function openFor(a: LooseAudio) {
    if (open === a.rel) return (open = null);
    open = a.rel;
    find = "";
    if (!songs) songs = (await api.songs().catch(() => [])).filter((s) => !s.archived).map((s) => ({ id: s.id, title: s.title, artist: s.artist }));
  }
  $effect(() => {
    if (as === "refArtist" && !artists) api.artists().then((x) => (artists = x.map((a) => a.name))).catch(fail);
    if (as === "refRelease" && !releases) api.releases().then((x) => (releases = x.map((r) => ({ id: r.id, title: r.title, artist: r.artist })))).catch(fail);
  });
  function targets(a: LooseAudio): Target[] {
    const w = find.toLowerCase().split(/\s+/).filter(Boolean);
    const fits = (s: string) => w.every((x) => s.toLowerCase().includes(x));
    let all: Target[];
    if (as === "refArtist") all = (artists ?? []).map((n) => ({ key: n, label: n }));
    else if (as === "refRelease") all = (releases ?? []).map((r) => ({ key: r.id, label: r.title, sub: r.artist }));
    else {
      // the songs of its folder and those with a close name first
      const hinted = new Set([...a.songs, ...a.near].map((s) => s.id));
      all = (songs ?? [])
        .map((s) => ({ key: String(s.id), label: s.title || "?", sub: s.artist ?? undefined, hint: hinted.has(s.id) }))
        .sort((x, y) => Number(!!y.hint) - Number(!!x.hint));
    }
    return all.filter((x) => fits(`${x.label} ${x.sub ?? ""}`)).slice(0, 12);
  }
  async function link(a: LooseAudio, to: Target) {
    const nm = name(a.rel);
    try {
      if (as === "temoin") {
        await api.songEdit(Number(to.key), { temoin: a.rel });
        app.say(t("media.audio.doneTemoin", { name: nm, title: to.label }));
      } else if (as === "board") {
        const st = await api.boardLink(Number(to.key), [a.rel]);
        app.undo = st;
        app.say(t("media.audio.doneBoard", { name: nm, title: to.label }), { undo: !!st.label });
      } else {
        const kind = as === "refSong" ? "song" : as === "refArtist" ? "artist" : "release";
        await api.refMark(a.rel, kind, to.key, true);
        app.say(t("media.audio.doneRef", { name: nm, title: to.label }));
      }
      open = null;
      app.version++;
    } catch (e) {
      fail(e);
    }
  }
</script>

{#if !groups.length}<p class="faint">{t("media.audio.none")}</p>
{:else}
  {#each groups.slice(0, upTo) as g (g.dir)}
    <section class="grp">
      <div class="gh">
        <input type="checkbox" aria-label={t("media.pick.folder")} checked={g.files.every((a) => selected.has(a.rel))} onchange={(e) => pick(g.files.map((a) => a.rel), e.currentTarget.checked)} />
        <button class="dname" aria-expanded={!shut.has(g.dir)} onclick={() => toggle(g.dir)}>
          <span class="chev" class:shut={shut.has(g.dir)}>▾</span><span class="ell" title={g.dir}>{g.dir || "/"}</span>
        </button>
        {#each g.songs as s (s.id)}<a class="songtag" href="#/song/{s.id}">{s.title || "?"}</a>{/each}
        <span class="faint small">{t("media.audio.count", { n: g.files.length })}</span>
        <button class="btn sm" onclick={() => api.reveal(g.dir).catch(fail)}>{t("media.audio.reveal")}</button>
      </div>
      {#if !shut.has(g.dir)}
        {#each g.files as a (a.rel)}
          <div class="row" class:sel={selected.has(a.rel)} data-media={a.rel} data-kind="audio">
            <input type="checkbox" aria-label={t("media.pick.one", { name: name(a.rel) })} checked={selected.has(a.rel)} onchange={(e) => pick([a.rel], e.currentTarget.checked)} />
            <button class="icon" title={t("media.audio.listen")} aria-label={t("media.audio.listen")} onclick={() => playOrPause(a.rel, () => player.play(a.rel, null, name(a.rel)))}>
              <PlayIcon on={playingFile(a.rel)} /></button>
            <span class="nm ell" title={a.rel}>{name(a.rel)}</span>
            {#if a.near.length}<span class="small near">{t("media.audio.near")} {#each a.near as s, i (s.id)}{i ? ", " : ""}<a href="#/song/{s.id}">{s.title || "?"}</a>{/each}</span>{/if}
            <span class="faint small nw">{sizeOf(a.size)} · {ago(a.mtime)}</span>
            <button class="btn sm" class:on={open === a.rel} aria-expanded={open === a.rel} onclick={() => openFor(a)}>{open === a.rel ? t("media.audio.close") : t("media.audio.link")}</button>
          </div>
          {#if open === a.rel}
            <div class="pick">
              <div class="as">
                <span class="small faint">{t("media.audio.as")}</span>
                <select bind:value={as}>
                  <option value="temoin">{t("media.audio.asTemoin")}</option>
                  <option value="board">{t("media.audio.asBoard")}</option>
                  <option value="refSong">{t("media.audio.asRef")} {t("media.audio.refSong")}</option>
                  <option value="refArtist">{t("media.audio.asRef")} {t("media.audio.refArtist")}</option>
                  <option value="refRelease">{t("media.audio.asRef")} {t("media.audio.refRelease")}</option>
                </select>
                <!-- svelte-ignore a11y_autofocus -->
                <input type="search" placeholder={t("media.audio.find")} bind:value={find} autofocus />
              </div>
              <div class="targets">
                {#each targets(a) as to (to.key)}
                  <button class="target" onclick={() => link(a, to)}>
                    <b>{to.label}</b>{#if to.sub}<span class="faint"> · {to.sub}</span>{/if}
                    {#if to.hint}<span class="small tag">{t("media.audio.suggested")}</span>{/if}
                  </button>
                {:else}<span class="faint small">{t("media.audio.noTarget")}</span>{/each}
              </div>
            </div>
          {/if}
        {/each}
      {/if}
    </section>
  {/each}
  {#if groups.length > upTo}<button class="link small" onclick={() => (upTo += STEP)}>{t("media.audio.loadMore", { n: Math.min(STEP, groups.length - upTo) })}</button>{/if}
{/if}

<style>
  .grp { border-top: 1px solid var(--line); padding: 8px 0; }
  .gh { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .dname { display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; border: 0; background: none; color: var(--ink); font: inherit; font-weight: 600; text-align: left; padding: 3px 4px; border-radius: 6px; cursor: pointer; }
  .dname:hover { background: var(--accent-bg); }
  .chev { color: var(--faint); transition: transform .15s; display: inline-block; }
  .chev.shut { transform: rotate(-90deg); }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .songtag { font-size: 11px; color: var(--accent); border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent); border-radius: 4px; padding: 0 5px; text-decoration: none; white-space: nowrap; }
  .row { display: flex; align-items: center; gap: 10px; padding: 3px 4px 3px 4px; border-radius: 6px; font-size: 13px; }
  .row:hover, .row.sel { background: var(--accent-bg); }
  .nm { flex: 1; }
  .near { color: var(--muted); white-space: nowrap; }
  .near a { color: var(--accent); }
  .nw { white-space: nowrap; }
  .icon { border: 0; background: none; color: var(--ink); cursor: pointer; padding: 2px; display: grid; place-items: center; }
  .btn.on { background: var(--accent-bg); }
  .pick { margin: 4px 0 8px 22px; padding: 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel); display: flex; flex-direction: column; gap: 8px; }
  .as { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .as select, .as input { border: 1px solid var(--line); background: var(--panel2); color: var(--ink); border-radius: 7px; padding: 4px 8px; font: inherit; font-size: 13px; }
  .as input { flex: 1; min-width: 160px; }
  .targets { display: flex; flex-wrap: wrap; gap: 6px; }
  .target { border: 1px solid var(--line); background: var(--panel2); color: var(--ink); border-radius: 7px; padding: 4px 10px; cursor: pointer; font: inherit; font-size: 12.5px; display: inline-flex; align-items: center; gap: 6px; }
  .target:hover { border-color: var(--accent); }
  .tag { border: 1px solid var(--accent); color: var(--accent); border-radius: 4px; padding: 0 5px; }
</style>

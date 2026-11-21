<script lang="ts">
  // Folders as releases: the folder holding the song folders is the EP or the album. Grouped
  // by folder, or by artist / style with their folders inside. The real folders can be
  // rearranged here: a song dragged onto another card moves its folder there, a card moves
  // whole, folders get created, the whole tree reorganised by artist; loose media go to a
  // song's mood board. Every move is checked (no sample lost, nothing overwritten) and
  // undone with Ctrl+Z.
  import type { SongItem } from "../api";
  import { dragSource, dropTarget } from "../pdrag.svelte";
  import Splitter from "../components/Splitter.svelte";
  import { f } from "../filters.svelte";
  import { STAGES, bpm, dur, hue, stageLabel } from "../format";
  import { t, compare } from "../i18n/index.svelte";
  import Art from "../components/Art.svelte";
  import PlayBtn from "../components/PlayBtn.svelte";
  import MoveDialog from "../components/MoveDialog.svelte";
  import ReorgDialog from "../components/ReorgDialog.svelte";
  import MediaPanel from "../components/MediaPanel.svelte";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { ui } from "../contextMenus.svelte";
  import { loadHints, folderRelease, releaseFromFolder, releaseFromHint, songDrag } from "../releaseSuggest.svelte";
  import { kindLabel } from "../releases";

  let { list }: { list: SongItem[] } = $props();
  // what each folder is for the releases (a release already, one that looks like it)
  $effect(() => {
    void app.version;
    loadHints();
  });
  type Album = { dir: string; songs: SongItem[] };

  const albumOf = (s: SongItem) => {
    const p = (s.dir || "").split("/");
    return p.length > 1 ? p.slice(0, -1).join("/") : "";
  };
  function albums(songs: SongItem[]): Album[] {
    const m = new Map<string, SongItem[]>();
    for (const s of songs) {
      const a = albumOf(s);
      if (!m.has(a)) m.set(a, []);
      m.get(a)!.push(s);
    }
    return [...m.entries()]
      .map(([dir, songs]) => ({ dir, songs: songs.sort((a, b) => compare(a.title || "", b.title || "")) }))
      .sort((a, b) => b.songs.length - a.songs.length || a.dir.localeCompare(b.dir));
  }
  const groups = $derived.by(() => {
    if (f.by === "folder") return [{ name: "", albums: albums(list), n: list.length }];
    const keyOf = (s: SongItem) => (f.by === "artist" ? s.artist || t("songs.noArtist") : (s.style || "").split(/\s*,\s*/)[0].toLowerCase() || t("songs.noStyle"));
    const m = new Map<string, SongItem[]>();
    for (const s of list) {
      const k = keyOf(s);
      if (!m.has(k)) m.set(k, []);
      m.get(k)!.push(s);
    }
    return [...m.entries()].sort((a, b) => b[1].length - a[1].length).map(([name, songs]) => ({ name, albums: albums(songs), n: songs.length }));
  });
  const ORDER = ["none", ...STAGES];
  const stagesOf = (songs: SongItem[]) => {
    const c: Record<string, number> = {};
    for (const s of songs) c[s.stage || "none"] = (c[s.stage || "none"] ?? 0) + 1;
    return ORDER.filter((k) => c[k]).map((k) => [k, c[k]] as const);
  };
  // The cards in columns of their own, filled in turn into the shortest (as a masonry wall).
  // CSS `columns` did it before, but a fragmented flow is laid out whole: hovering one row
  // re-laid every card of the view (~50 ms a hover on 400 songs, more with more).
  let paneW = $state(0);
  const CARD = 330;
  const GAP = 14;
  const ncols = $derived(Math.max(1, Math.floor((paneW - 44 + GAP) / (CARD + GAP))));
  // a card's height, guessed (header + rows): also its placeholder size while off screen
  const guess = (a: Album) => 112 + 25 * a.songs.length;
  function columnsOf(list: Album[], n: number): Album[][] {
    const cols: Album[][] = Array.from({ length: n }, () => []);
    const h = new Array(n).fill(0);
    for (const a of list) {
      const i = h.indexOf(Math.min(...h));
      cols[i].push(a);
      h[i] += guess(a) + GAP;
    }
    return cols.filter((c) => c.length);
  }
  const BY = [["folder", "byFolder"], ["artist", "byArtist"], ["style", "byStyle"]] as const;

  // ---- moving
  let moving = $state<string | null>(null);
  let reorg = $state(false);
  let media = $state(false);
  /** The media panel takes most of the width (a wall of pictures). */
  let mediaWide = $state(false);
  let mediaW = $state(380);
  let newDir = $state<string | null>(null);
  // songs per folder: a folder holding several songs moves only whole, from its card
  const perDir = $derived.by(() => {
    const m = new Map<string, number>();
    for (const s of app.songs) if (!s.archived) m.set(s.dir, (m.get(s.dir) ?? 0) + 1);
    return m;
  });
  const movable = (s: SongItem) => app.physical && !!s.dir && (perDir.get(s.dir) ?? 0) <= 1 && s.dir.includes("/");
  let over = $state<string | null>(null);
  const songOf = (id: unknown) => app.songs.find((x) => x.id === id);
  async function drop(album: string, s: SongItem | undefined) {
    over = null;
    if (!s || !album || s.dir.startsWith(album + "/") && s.dir.split("/").length === album.split("/").length + 1) return;
    try {
      const plan = await api.fsPlan(s.dir, album);
      if (plan.risks.length) {
        app.say(t("songs.folders.risk", { title: s.title, n: plan.risks.length }), { error: true });
        return;
      }
    } catch (e) {
      app.say(String(e), { error: true });
      return;
    }
    // a drop moves a real folder: said first, done only on "Move"
    const leaf = (p: string) => p.split("/").pop() ?? p;
    ui.confirm = {
      title: t("songs.folders.confirmTitle", { name: leaf(s.dir) }),
      text: t("songs.folders.confirmText", { name: leaf(s.dir), to: album }),
      ok: t("songs.folders.confirmOk"),
      run: async () => {
        const r = await app.fileAct(api.fsMove([[s.dir, album]]));
        if (r?.skipped.length) app.say(r.skipped[0].why, { error: true });
      },
    };
  }
  async function mkdir() {
    const n = (newDir ?? "").trim();
    if (!n) return;
    const r = await app.fileAct(api.fsMkdir(n));
    if (r) newDir = null;
  }
</script>

<div class="fwrap" class:withmedia={media} style:--mw="{mediaW}px">
<div class="pane" bind:clientWidth={paneW}>
  <div class="tools">
    <div class="seg">{#each BY as [k, l]}<button class:on={f.by === k} onclick={() => (f.by = k)}>{t(`songs.folders.${l}`)}</button>{/each}</div>
    <span class="small faint grow">{app.physical ? t("songs.folders.physicalOn") : t("songs.folders.physicalOff")}</span>
    {#if !app.physical}
    {:else if newDir !== null}
      <input class="nd" type="text" placeholder={t("songs.folders.newPlaceholder")} bind:value={newDir} onkeydown={(e) => { if (e.key === "Enter") mkdir(); if (e.key === "Escape") newDir = null; }} />
      <button class="btn sm" onclick={mkdir}>{t("songs.folders.create")}</button>
    {:else}<button class="btn sm" onclick={() => (newDir = "")}>{t("songs.folders.newFolder")}</button>{/if}
    {#if app.physical}<button class="btn sm" title={t("songs.folders.reorgTitle")} onclick={() => (reorg = true)}>{t("songs.folders.reorg")}</button>{/if}
    <button class="btn sm" class:on={media} title={t("songs.folders.mediaTitle")} onclick={() => (media = !media)}>{t("songs.folders.media")}</button>
  </div>
  {#if !list.length}<p class="empty muted">{t("songs.empty")}</p>{/if}
  {#each groups as g (g.name)}
    {#if g.name}<h2 class="gh">{g.name} <span class="faint">{g.n}</span></h2>{/if}
    <div class="albums">
      {#each columnsOf(g.albums, ncols) as col, ci (ci)}
      <div class="col">
      {#each col as a (a.dir)}
        {@const name = a.dir ? a.dir.split("/").pop() ?? "" : t("songs.folders.root")}
        {@const covers = a.songs.filter((s) => s.cover).slice(0, 4)}
        {@const artists = [...new Set(a.songs.map((s) => s.artist).filter(Boolean))]}
        {@const total = a.songs.reduce((t, s) => t + Number(s.len || 0), 0)}
        <section class="album card" style:contain-intrinsic-size="auto {guess(a)}px" class:drop={over === a.dir && !!a.dir} role="group" aria-label={name} data-folder={a.dir || undefined}
          use:dropTarget={{
            accept: (it) => { const s = it.kind === "song" ? songOf(it.data) : undefined; return !!a.dir && !!s && movable(s); },
            over: () => (over = a.dir),
            leave: () => { if (over === a.dir) over = null; },
            drop: (it) => drop(a.dir, songOf(it.data)),
          }}>
          <div class="head">
            <div class="cov n{covers.length}" style="--h:{hue(name)}">
              {#if covers.length >= 2}{#each covers as c (c.id)}<Art title={c.title} cover={c.cover} size="100%" radius={0} />{/each}
              {:else if covers.length === 1}<Art title={covers[0].title} cover={covers[0].cover} size="100%" radius={0} />
              {:else}<span>{name.replace(/^[^\p{L}\p{N}]+/u, "").slice(0, 1).toUpperCase() || "?"}</span>{/if}
            </div>
            <div class="txt">
              <div class="t" title={a.dir || t("songs.folders.rootTitle")}>{name}
                {#if a.dir}<span class="acts">
                  <button class="ic" title={t("songs.folders.reveal")} aria-label={t("songs.folders.reveal")} onclick={() => api.reveal(a.dir).catch((e) => app.say(String(e), { error: true }))}>
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg></button>
                  {#if app.physical}<button class="ic" title={t("songs.folders.move")} aria-label={t("songs.folders.move")} onclick={() => (moving = a.dir)}>
                    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M13 6l6 6-6 6" /></svg></button>{/if}
                </span>{/if}
              </div>
              <div class="m">{[a.dir.split("/").slice(0, -1).join(" / "), artists.slice(0, 2).join(", ") + (artists.length > 2 ? "..." : "")].filter(Boolean).join(" · ")}</div>
              <div class="m">{t("songs.nSongs", { n: a.songs.length })}{total ? " · " + dur(total) : ""}</div>
              {#if a.dir}
                {@const fr = folderRelease(a.dir)}
                <div class="rel small">
                  {#if fr.release}<a class="relb" href="#/release/{fr.release.id}" title={t("songs.folders.isRelease")}>◉ {kindLabel(fr.release.kind)} · {fr.release.title}</a>
                  {:else if a.songs.length > 25}<span class="faint" title={t("songs.folders.tooManyTip")}>{t("songs.folders.tooMany")}</span>
                  {:else if fr.inside.length}<span class="faint" title={t("songs.folders.holdsReleases", { list: fr.inside.join(", ") })}>{t("songs.folders.holdsN", { n: fr.inside.length })}</span>
                  {:else}
                    {#if fr.hint}<span class="faint">{t("songs.folders.looksLike", { kind: kindLabel(fr.hint.kind) })}</span>{/if}
                    <button class="link" onclick={() => (fr.hint ? releaseFromHint(fr.hint) : releaseFromFolder(a.dir, a.songs.map((s) => s.id)))}>{t("songs.folders.makeRelease")}</button>
                  {/if}
                </div>
              {/if}
              <div class="bar" title={stagesOf(a.songs).map(([k, n]) => t("songs.folders.stageCount", { stage: stageLabel(k === "none" ? null : k), n })).join("\n")}>
                {#each stagesOf(a.songs) as [k, n]}<i class="s-{k}" style:flex={n}></i>{/each}
              </div>
            </div>
          </div>
          <ol>
            {#each a.songs as s, i (s.id)}
              <li><a href="#/song/{s.id}" class:grab={movable(s)} use:dragSource={songDrag(s.id, s.title)}
                title={movable(s) ? t("songs.folders.dragHint") : s.dir.includes("/") ? t("songs.folders.shared") : undefined}
                ><span class="no"><span class="num">{i + 1}</span>{#if s.temoin || s.rel}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={20} queue={{ songs: a.songs, name }} project={s.temoin ? "" : s.rel} /></span>{/if}</span><span class="st">{s.title}</span>
                <span class="sm">{[bpm(s.tempo), s.key].filter(Boolean).join(" · ")}</span><i class="dot s-{s.stage || 'none'}" title={stageLabel(s.stage)}></i></a></li>
            {/each}
          </ol>
        </section>
      {/each}
      </div>
      {/each}
    </div>
  {/each}
</div>
{#if media}<Splitter bind:width={mediaW} side="right" key="folders-media" min={280} max={1200} label={t("songs.resize")} /><MediaPanel close={() => (media = false)} bind:wide={mediaWide} />{/if}
</div>
{#if moving}<MoveDialog src={moving} close={() => (moving = null)} />{/if}
{#if reorg}<ReorgDialog close={() => (reorg = false)} />{/if}

<style>
  .rel { display: flex; gap: 8px; align-items: center; margin-top: 2px; }
  .relb { color: var(--accent); font-weight: 600; }
  .fwrap { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr); }
  /* the media panel at the width set with the bar */
  .fwrap.withmedia { grid-template-columns: minmax(0, 1fr) auto minmax(0, var(--mw)); }
  .pane { overflow: auto; min-height: 0; padding: 14px 22px 40px; }
  .tools { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; flex-wrap: wrap; }
  .grow { flex: 1; min-width: 160px; }
  .nd { width: 240px; }
  .btn.on { border-color: var(--accent); background: var(--accent-bg); }
  .album.drop { border-color: var(--accent); background: var(--accent-bg); }
  .acts { float: right; display: inline-flex; gap: 2px; opacity: 0; transition: opacity .15s; }
  .album:hover .acts, .acts:focus-within { opacity: 1; }
  .ic { border: 0; background: none; color: var(--muted); cursor: pointer; padding: 2px 4px; border-radius: 5px; display: grid; place-items: center; }
  .ic:hover { color: var(--ink); background: var(--line); }
  li a.grab { cursor: grab; }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .seg button { border: 0; background: none; padding: 3px 10px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  .gh { font-size: 16px; margin: 18px 0 10px; text-transform: capitalize; }
  .albums { display: flex; gap: 14px; align-items: flex-start; }
  .col { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 14px; }
  /* a card off screen is neither laid out nor painted (its guessed size holds its place) */
  .album { padding: 12px; content-visibility: auto; }
  .head { display: flex; gap: 12px; margin-bottom: 8px; }
  .cov { width: 74px; height: 74px; flex: none; border-radius: 9px; overflow: hidden; display: grid; place-items: center; color: #fff; font-size: 26px; font-weight: 650;
    background: linear-gradient(135deg, hsl(var(--h) 40% 46%), hsl(calc(var(--h) + 60) 40% 30%)); }
  .cov.n2, .cov.n3, .cov.n4 { grid-template-columns: 1fr 1fr; grid-template-rows: 1fr 1fr; }
  .cov.n2 { grid-template-rows: 1fr; }
  .txt { min-width: 0; flex: 1; }
  .t { font-weight: 650; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { display: flex; gap: 2px; height: 5px; margin-top: 7px; border-radius: 3px; overflow: hidden; }
  .bar i, .dot { background: var(--s-none); }
  .s-draft { background: var(--s-draft) !important; } .s-arrangement { background: var(--s-arrangement) !important; } .s-mix { background: var(--s-mix) !important; }
  .s-master { background: var(--s-master) !important; } .s-done { background: var(--s-done) !important; } .s-pause { background: var(--s-pause) !important; }
  ol { list-style: none; margin: 0; padding: 0; }
  li a { display: flex; align-items: center; gap: 10px; padding: 4px 6px; border-radius: 6px; color: inherit; font-size: 12.5px; }
  li a:hover { background: color-mix(in srgb, var(--line) 50%, transparent); }
  .no { width: 20px; flex: none; position: relative; text-align: right; color: var(--faint); font-variant-numeric: tabular-nums; }
  .pb { position: absolute; inset: -2px 0 0 0; display: none; justify-content: flex-end; align-items: center; }
  li a:hover .pb, .pb:focus-within, .pb:has(:global(.play.on)) { display: flex; }
  li a:hover .num { visibility: hidden; }
  .st { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sm { color: var(--faint); font-size: 11.5px; white-space: nowrap; }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .empty { padding: 40px; text-align: center; }
</style>

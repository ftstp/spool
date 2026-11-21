<script lang="ts">
  import Skeleton from "../lib/components/Skeleton.svelte";
  import LooseAudioList from "../lib/components/LooseAudioList.svelte";
  import LooseMedia from "../lib/components/LooseMedia.svelte";
  import type { MediaFolder } from "../lib/api";
  import type { LooseAudio } from "../lib/api/bindings/LooseAudio";
  import { ui } from "../lib/contextMenus.svelte";
  // Every song's media in one place: the pictures, videos, fonts, presets and notes of the
  // mood boards and of the song folders, by song or as the folder tree they sit in. A picture
  // can become the song's own (its card, its sheet); nothing is moved or written on disk.
  import * as api from "../lib/api";
  import type { MediaSong } from "../lib/api/bindings/MediaSong";
  import type { Item } from "../lib/api/bindings/Item";
  import { app } from "../lib/state.svelte";
  import { t, fmtNum } from "../lib/i18n/index.svelte";

  let list = $state<MediaSong[]>([]);
  let loading = $state(true);
  let error = $state("");
  $effect(() => {
    void app.version;
    api.mediaList()
      // the songs with pictures first, then the most files
      .then((x) => (list = x.sort((a, b) => pics(b) - pics(a) || b.items.length - a.items.length)))
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false));
  });

  // two sources: the songs' media (their folders and mood boards), and what lies outside
  // every project (the same files as the Folders view's panel)
  type Scope = "songs" | "loose";
  let scope = $state<Scope>((() => { try { return localStorage.getItem("spool.media.scope") === "loose" ? "loose" : "songs"; } catch { return "songs"; } })());
  function setScope(s: Scope) {
    scope = s;
    selected = new Set();
    try { localStorage.setItem("spool.media.scope", s); } catch { /* not kept */ }
  }
  // read once (a walk of the projects folders), again on demand
  let folders = $state<MediaFolder[] | null>(null);
  let looseErr = $state("");
  const loadFolders = () => {
    folders = null;
    looseErr = "";
    api.fsMedia().then((x) => (folders = x)).catch((e) => (looseErr = String(e)));
  };
  loadFolders();
  const looseN = $derived((folders ?? []).reduce((n, f) => n + f.items.length, 0));
  // the audio files no song uses (renders nobody picked), with the songs of a close name
  let unused = $state<LooseAudio[]>([]);
  $effect(() => {
    void app.version;
    api.looseAudio().then((x) => (unused = x)).catch(() => (unused = []));
  });
  const unusedIn = $derived(unused.filter((a) => a.songs.length));
  const near = $derived(new Map(unused.map((a) => [a.rel, a])));
  // several files picked, for a mood board at once
  let selected = $state<Set<string>>(new Set());
  const pick = (rel: string, on: boolean) => {
    const s = new Set(selected);
    if (on) s.add(rel);
    else s.delete(rel);
    selected = s;
  };

  const pics = (s: MediaSong) => s.items.filter((i) => i.kind === "image" || i.kind === "gif").length;
  // a long list of files (sixty presets) shows its first ones until asked
  const MANY = 12;
  let allOf = $state<Set<number>>(new Set());
  // "audio": not a song's media but the audio files no song uses (their own list)
  type Kind = "" | "image" | "video" | "font" | "preset" | "text" | "other" | "audio";
  const KINDS: Kind[] = ["", "image", "video", "font", "preset", "text", "other"];
  const kindOf = (i: Item): Kind => (i.kind === "gif" ? "image" : (["image", "video", "font", "preset", "text"] as string[]).includes(i.kind) ? (i.kind as Kind) : "other");
  let kind = $state<Kind>("");
  let q = $state("");
  type View = "songs" | "tree";
  let view = $state<View>((() => { try { return localStorage.getItem("spool.media.view") === "tree" ? "tree" : "songs"; } catch { return "songs"; } })());
  function setView(v: View) {
    view = v;
    try { localStorage.setItem("spool.media.view", v); } catch { /* not kept */ }
  }
  const words = $derived(q.toLowerCase().split(/\s+/).filter(Boolean));
  const keep = (s: MediaSong, i: Item) => (!kind || kindOf(i) === kind) && words.every((w) => `${i.rel} ${s.title ?? ""} ${s.artist ?? ""}`.toLowerCase().includes(w));
  const shown = $derived(list.map((s) => ({ s, items: s.items.filter((i) => keep(s, i)) })).filter((x) => x.items.length));
  const counts = $derived.by(() => {
    const m = new Map<Kind, number>();
    for (const s of list) for (const i of s.items) {
      m.set(kindOf(i), (m.get(kindOf(i)) ?? 0) + 1);
      m.set("", (m.get("") ?? 0) + 1);
    }
    return m;
  });
  const total = $derived(shown.reduce((n, x) => n + x.items.length, 0));

  // ---- the tree: the folders the files sit in, from the projects folders down
  type Node = { name: string; path: string; dirs: Map<string, Node>; files: { i: Item; s: MediaSong }[]; songs: MediaSong[]; n: number };
  const tree = $derived.by(() => {
    const root: Node = { name: "", path: "", dirs: new Map(), files: [], songs: [], n: 0 };
    for (const { s, items } of shown) {
      for (const i of items) {
        const parts = i.rel.split("/");
        parts.pop();
        let n = root;
        n.n++;
        let path = "";
        for (const p of parts) {
          path = path ? `${path}/${p}` : p;
          let c = n.dirs.get(p);
          if (!c) n.dirs.set(p, (c = { name: p, path, dirs: new Map(), files: [], songs: [], n: 0 }));
          c.n++;
          n = c;
        }
        n.files.push({ i, s });
      }
      // the song named where its folder is
      let n = root;
      for (const p of s.dir.split("/").filter(Boolean)) {
        const c = n.dirs.get(p);
        if (!c) break;
        n = c;
      }
      if (n !== root && !n.songs.includes(s)) n.songs.push(s);
    }
    // a folder with a single subfolder and no file of its own folds into it ("a/b/c")
    const squash = (n: Node): Node => {
      for (const [k, c] of [...n.dirs]) {
        let x = c;
        while (x.dirs.size === 1 && !x.files.length && !x.songs.length) {
          const only = [...x.dirs.values()][0];
          x = { ...only, name: `${x.name}/${only.name}` };
        }
        n.dirs.set(k, squash(x));
      }
      return n;
    };
    return squash(root);
  });
  let openDirs = $state<Set<string>>(new Set());
  function toggle(p: string) {
    const s = new Set(openDirs);
    if (s.has(p)) s.delete(p);
    else s.add(p);
    openDirs = s;
  }

  let big = $state<Item | null>(null);
  async function makeCover(s: MediaSong, i: Item) {
    try {
      const st = await api.setCover(s.id, s.cover_chosen && s.cover === i.rel ? null : i.rel);
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const ICON: Record<string, string> = { video: "▶", font: "Aa", preset: "◈", text: "≡", other: "·" };
  const sizeOf = (b: number) => (b > 1e6 ? `${(b / 1e6).toFixed(1)} Mo` : `${Math.max(1, Math.round(b / 1e3))} ko`);
</script>

<div class="page">
  <header class="top">
    <h1>{t("media.title")}</h1>
    <div class="seg" role="radiogroup" aria-label={t("media.scopeLabel")}>
      <button role="radio" aria-checked={scope === "songs"} class:on={scope === "songs"} onclick={() => setScope("songs")}>{t("media.scope.songs", { n: fmtNum(counts.get("") ?? 0) })}</button>
      <button role="radio" aria-checked={scope === "loose"} class:on={scope === "loose"} onclick={() => setScope("loose")}>{t("media.scope.loose", { n: folders ? fmtNum(looseN) : "..." })}</button>
    </div>
    <span class="grow"></span>
    <input class="search" type="search" placeholder={t("media.search")} bind:value={q} />
    {#if scope === "songs" && kind !== "audio"}
      <div class="seg" role="radiogroup" aria-label={t("media.viewLabel")}>
        <button role="radio" aria-checked={view === "songs"} class:on={view === "songs"} onclick={() => setView("songs")}>{t("media.bySong")}</button>
        <button role="radio" aria-checked={view === "tree"} class:on={view === "tree"} onclick={() => setView("tree")}>{t("media.tree")}</button>
      </div>
    {:else if scope === "loose"}
      <button class="btn sm" onclick={loadFolders} title={t("song.media.reload")}>↻</button>
    {/if}
  </header>
  {#if selected.size}
    <div class="selbar" role="region" aria-label={t("media.pick.bar")}>
      <b>{t("media.pick.count", { n: selected.size })}</b>
      <button class="btn sm primary" onclick={() => { ui.toBoard = [...selected]; selected = new Set(); }}>{t("media.pick.send")}</button>
      <button class="link small" onclick={() => (selected = new Set())}>{t("media.pick.clear")}</button>
    </div>
  {/if}
  {#if scope === "loose"}
    {#if looseErr}<p class="bad small">{looseErr}</p>
    {:else if !folders}<Skeleton kind="list" />
    {:else}<LooseMedia {folders} {near} {q} bind:selected />{/if}
  {:else}
  <div class="kinds">
    {#each KINDS as k (k)}
      {#if !k || counts.get(k)}
        <button class="chip" class:on={kind === k} onclick={() => (kind = k)}>{t(`media.kind.${k || "all"}`)} <span class="faint">{fmtNum(counts.get(k) ?? 0)}</span></button>
      {/if}
    {/each}
    {#if unusedIn.length}<button class="chip" class:on={kind === "audio"} onclick={() => (kind = "audio")}>{t("media.kind.audioUnused")} <span class="faint">{fmtNum(unusedIn.length)}</span></button>{/if}
  </div>
  {#if kind === "audio"}
    <p class="small faint lede">{t("media.audio.lede")}</p>
    <LooseAudioList items={unusedIn} {q} bind:selected />
  {:else}
  <p class="small faint lede">{t("media.lede")}</p>

  {#if loading}<Skeleton kind="list" />
  {:else if error}<p class="bad small">{error}</p>
  {:else if !shown.length}<p class="faint">{list.length ? t("media.noneHere") : t("media.none")}</p>
  {:else if view === "songs"}
    {#each shown as { s, items } (s.id)}
      {@const imgs = items.filter((i) => kindOf(i) === "image")}
      {@const files = items.filter((i) => kindOf(i) !== "image")}
      <section class="song">
        <div class="shead">
          <span class="art">{#if s.cover}<img src={api.thumbUrl(s.cover, 96)} alt="" loading="lazy" />{:else}{(s.title || "?").slice(0, 1).toUpperCase()}{/if}</span>
          <div class="grow">
            <a href="#/song/{s.id}"><b>{s.title || "?"}</b></a>{#if s.artist}<span class="faint"> · {s.artist}</span>{/if}
            <div class="small faint ell" title={s.dir}>{s.dir}</div>
          </div>
          <span class="small faint">{fmtNum(items.length)}</span>
        </div>
        {#if imgs.length}<div class="tiles">
          {#each imgs as i (i.rel)}
            {#if true}
              <figure class="tile" class:cover={s.cover === i.rel} class:sel={selected.has(i.rel)} data-media={i.rel} data-kind="image">
                <button class="img" onclick={() => (big = i)} aria-label={t("media.enlarge", { name: i.name })}><img src={api.thumbUrl(i.rel, 240)} alt={i.name} loading="lazy" /></button>
                <input class="ck" type="checkbox" aria-label={t("media.pick.one", { name: i.name })} checked={selected.has(i.rel)} onchange={(e) => pick(i.rel, e.currentTarget.checked)} />
                <button class="star" class:on={s.cover === i.rel} title={s.cover === i.rel ? (s.cover_chosen ? t("media.coverUnset") : t("media.coverAuto")) : t("media.coverSet")} onclick={() => makeCover(s, i)}>★</button>
                <figcaption class="ell" title={i.rel}>{i.name}</figcaption>
              </figure>
            {/if}
          {/each}
        </div>{/if}
        {#if files.length}
          <div class="files">
            {#each allOf.has(s.id) ? files : files.slice(0, MANY) as i (i.rel)}
              <div class="file" class:sel={selected.has(i.rel)} data-media={i.rel} data-kind={i.kind} title={i.rel}><input type="checkbox" aria-label={t("media.pick.one", { name: i.name })} checked={selected.has(i.rel)} onchange={(e) => pick(i.rel, e.currentTarget.checked)} /><span class="ic">{ICON[kindOf(i)] ?? "·"}</span><span class="ell">{i.name}</span><span class="faint small sz">{sizeOf(i.size)}</span></div>
            {/each}
          </div>
          {#if files.length > MANY && !allOf.has(s.id)}<button class="link small more" onclick={() => (allOf = new Set([...allOf, s.id]))}>{t("media.showAll", { n: files.length - MANY })}</button>{/if}
        {/if}
      </section>
    {/each}
  {:else}
    {#snippet node(n: Node, depth: number)}
      {#each [...n.dirs.values()].sort((a, b) => a.name.localeCompare(b.name)) as c (c.path)}
        <div class="dir" style:--d={depth}>
          <button class="dname" aria-expanded={openDirs.has(c.path)} onclick={() => toggle(c.path)}>
            <span class="chev" class:shut={!openDirs.has(c.path)}>▾</span>
            <span class="ell">{c.name}</span>
            {#each c.songs as s (s.id)}<a class="songtag" href="#/song/{s.id}" onclick={(e) => e.stopPropagation()}>{s.title}</a>{/each}
            <span class="faint small n">{fmtNum(c.n)}</span>
          </button>
          {#if openDirs.has(c.path)}
            {@render node(c, depth + 1)}
            {#each c.files as { i, s } (i.rel)}
              <div class="leaf" style:--d={depth + 1} data-media={i.rel} data-kind={i.kind} title={i.rel}>
                {#if kindOf(i) === "image"}<button class="mini" onclick={() => (big = i)}><img src={api.thumbUrl(i.rel, 64)} alt="" loading="lazy" /></button>
                {:else}<span class="ic">{ICON[kindOf(i)] ?? "·"}</span>{/if}
                <span class="ell">{i.name}</span>
                {#if s.cover === i.rel}<span class="small tag">{t("media.isCover")}</span>{/if}
                <span class="faint small">{sizeOf(i.size)}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/each}
    {/snippet}
    <div class="tree">{@render node(tree, 0)}</div>
  {/if}
  {/if}
  {/if}
</div>

{#if big}
  <div class="big" role="presentation" onclick={() => (big = null)} onkeydown={(e) => e.key === "Escape" && (big = null)}>
    <img src={api.mediaUrl(big.rel)} alt={big.name} data-media={big.rel} data-kind="image" />
    <p class="small">{big.rel}</p>
  </div>
{/if}

<style>
  .page { padding: 20px 24px 40px; max-width: 1400px; }
  .top { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .top h1 { margin: 0; }
  .grow { flex: 1; min-width: 0; }
  .search { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 8px; padding: 5px 10px; font: inherit; font-size: 13px; width: 220px; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: none; color: var(--muted); padding: 4px 12px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .seg button + button { border-left: 1px solid var(--line); }
  .seg button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .kinds { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 12px; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--muted); border-radius: 999px; padding: 3px 11px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .chip.on { border-color: var(--accent); color: var(--ink); background: var(--accent-bg); }
  .lede { margin: 8px 0 14px; }
  .song { border-top: 1px solid var(--line); padding: 12px 0; }
  .shead { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
  .shead a { color: var(--ink); text-decoration: none; }
  .shead a:hover { text-decoration: underline; }
  .art { width: 36px; height: 36px; border-radius: 6px; background: var(--panel2); display: grid; place-items: center; overflow: hidden; font-weight: 700; flex: none; }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tiles { display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-start; }
  .tile { position: relative; margin: 0; width: 132px; }
  .tile .img { display: block; width: 132px; height: 132px; padding: 0; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--panel2); cursor: zoom-in; }
  .tile img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .tile.cover .img { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .tile figcaption { font-size: 11px; color: var(--muted); margin-top: 3px; }
  .star { position: absolute; top: 5px; right: 5px; width: 24px; height: 24px; border-radius: 50%; border: 0; background: color-mix(in srgb, #000 55%, transparent); color: #fff; cursor: pointer; opacity: 0; transition: opacity .15s; font-size: 13px; line-height: 1; }
  .tile:hover .star, .star.on { opacity: 1; }
  .star.on { color: #f2c94c; }
  .files { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 2px 14px; margin-top: 8px; }
  .file { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 3px 6px; border-radius: 6px; font-size: 12.5px; }
  .file:hover, .file.sel { background: var(--accent-bg); }
  .tile .ck { position: absolute; top: 6px; left: 6px; width: 16px; height: 16px; opacity: 0; transition: opacity .15s; }
  .tile:hover .ck, .tile.sel .ck, .ck:focus-visible { opacity: 1; }
  .tile.sel .img { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent); }
  .selbar { position: sticky; top: 0; z-index: 5; display: flex; align-items: center; gap: 12px; margin: 10px 0 0; padding: 8px 12px; border: 1px solid var(--accent); border-radius: 10px; background: var(--panel); }
  .file .ell { flex: 1; min-width: 0; }
  .sz { white-space: nowrap; flex: none; }
  .more { margin-top: 4px; }
  .ic { width: 20px; text-align: center; color: var(--muted); font-weight: 700; flex: none; }
  .tree { font-size: 13px; }
  .dname { display: flex; align-items: center; gap: 6px; width: 100%; border: 0; background: none; color: var(--ink); font: inherit; text-align: left; padding: 3px 6px 3px calc(6px + var(--d) * 16px); border-radius: 6px; cursor: pointer; }
  .dname:hover { background: var(--accent-bg); }
  .chev { color: var(--faint); transition: transform .15s; display: inline-block; }
  .chev.shut { transform: rotate(-90deg); }
  .songtag { font-size: 11px; color: var(--accent); border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent); border-radius: 4px; padding: 0 5px; text-decoration: none; white-space: nowrap; }
  .n { margin-left: auto; }
  .leaf { display: flex; align-items: center; gap: 8px; padding: 2px 6px 2px calc(28px + var(--d) * 16px); }
  .mini { width: 28px; height: 28px; padding: 0; border: 1px solid var(--line); border-radius: 4px; overflow: hidden; cursor: zoom-in; flex: none; background: var(--panel2); }
  .mini img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .tag { border: 1px solid var(--accent); color: var(--accent); border-radius: 4px; padding: 0 5px; }
  .big { position: fixed; inset: 0; z-index: 3000; background: color-mix(in srgb, #000 82%, transparent); display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; cursor: zoom-out; padding: 30px; }
  .big img { max-width: 92vw; max-height: 84vh; object-fit: contain; border-radius: 6px; }
  .big p { color: #ddd; margin: 0; }
</style>

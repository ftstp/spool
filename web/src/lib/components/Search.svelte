<script lang="ts">
  // The search at the top. On Songs it filters the songs on screen (every view). Elsewhere
  // it finds a song, a folder or a command and goes there, from a list under the field
  // (arrows + Enter, Escape). Ctrl+K from anywhere. A song in the list is heard from it
  // (its button, or Ctrl+Space on the line chosen); moving through the list while one plays
  // plays the line chosen instead.
  import { untrack } from "svelte";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { f, setView } from "../filters.svelte";
  import { openSong, closeAll } from "../tabs.svelte";
  import { bpm, hue, ago } from "../format";
  import { t } from "../i18n/index.svelte";
  import { norm, parse, matches, releasesBySong } from "../songQuery";
  import PlayBtn from "./PlayBtn.svelte";
  import { player } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { playProjectRel, sessionKey } from "../projectPlay";

  let { page, fold }: { page: string; fold: () => void } = $props();
  const onSongs = $derived(page === "songs" || page.startsWith("song/"));
  let q = $state("");
  let open = $state(false);
  let sel = $state(0);

  type Item = { sec: string; title: string; meta?: string; mark?: string; h?: number; run: () => void; song?: (typeof app.songs)[number] };

  const go = (h: string) => (location.hash = h);
  const VIEWS = ["list", "folders", "board", "cal"] as const;
  // built at render time: the labels follow a change of language
  const COMMANDS = $derived<[string, () => void][]>([
    [t("app.search.cmd.home"), () => go("#/home")],
    [t("app.search.cmd.songs"), () => go("#/songs")],
    [t("app.search.cmd.repair"), () => go("#/repair")],
    [t("app.search.cmd.create"), () => go("#/create")],
    [t("app.search.cmd.releases"), () => go("#/releases")],
    [t("app.search.cmd.artists"), () => go("#/artists")],
    [t("app.search.cmd.atelier"), () => go("#/atelier")],
    [t("app.search.cmd.stats"), () => go("#/stats")],
    [t("app.search.cmd.copies"), () => go("#/copies")],
    [t("app.search.cmd.export"), () => go("#/settings")],
    [t("app.search.cmd.settings"), () => go("#/settings")],
    ...VIEWS.map((k) => [t("app.search.cmd.songsView", { view: t(`app.search.view.${k}`) }), () => { setView(k); go("#/songs"); }] as [string, () => void]),
    [t("app.search.cmd.scan"), () => { api.scan(false); app.say(t("app.search.scanStarted")); }],
    [t("app.search.cmd.rescan"), () => { api.scan(true); app.say(t("app.search.rescanStarted")); }],
    [t("app.search.cmd.addProjects"), () => go("#/settings")],
    [t("app.search.cmd.addSamples"), () => go("#/settings")],
    [t("app.search.cmd.closeAll"), () => closeAll()],
    [t("app.search.cmd.fold"), () => fold()],
    [t("app.search.cmd.withAudio"), () => { f.audio = "yes"; go("#/songs"); }],
    [t("app.search.cmd.toFix"), () => { f.plugins = ["fix"]; go("#/songs"); }],
  ]);
  // every folder holding songs, and the folders above them
  const folders = $derived.by(() => {
    const set = new Set<string>();
    for (const s of app.songs) {
      const parts = (s.dir || "").split("/").filter(Boolean);
      for (let i = 1; i <= parts.length; i++) set.add(parts.slice(0, i).join("/"));
    }
    return [...set];
  });
  // every style given to a song, with how many songs have it (as the Style filter lists them)
  const styleCounts = $derived.by(() => {
    const m = new Map<string, number>();
    for (const s of app.songs) {
      if (s.archived) continue;
      for (const st of (s.style || "").toLowerCase().split(/\s*,\s*/).filter(Boolean)) m.set(st, (m.get(st) ?? 0) + 1);
    }
    return [...m].sort((a, b) => b[1] - a[1]);
  });
  // a song as a line of the list: opened by Enter, heard from its button
  const songItem = (s: (typeof app.songs)[number], sec: string, meta: (string | null | undefined)[]): Item => ({
    sec, title: s.title ?? "", h: hue(s.title ?? ""), mark: (s.title || "?").slice(0, 1).toUpperCase(),
    meta: meta.filter(Boolean).join(" · "),
    run: () => { openSong(s.id); go(`#/song/${s.id}`); },
    song: s.temoin || s.rel ? s : undefined,
  });
  // the field clicked, nothing typed yet: the songs worked on last; on Songs the styles used
  // most (a filter in one click), elsewhere a new song and a check
  const start = $derived.by((): Item[] => {
    const recent = app.songs
      .filter((s) => !s.archived && s.mtime > 0)
      .sort((a, b) => b.mtime - a.mtime)
      .slice(0, 6)
      .map((s) => songItem(s, t("app.search.sec.recent"), [s.artist, bpm(s.tempo), s.key, ago(s.mtime)]));
    const more: Item[] = onSongs
      ? styleCounts.slice(0, 5).map(([st, c]) => ({
          sec: t("app.search.sec.styles"), title: st, meta: t("app.search.songCount", { n: c }), mark: "♪",
          run: () => { f.style = [st]; app.query = ""; go("#/songs"); },
        }))
      : [
          { sec: t("app.search.sec.commands"), title: t("app.search.cmd.create"), mark: "+", run: () => go("#/create") },
          { sec: t("app.search.sec.commands"), title: t("app.search.cmd.scan"), mark: "→", run: () => { api.scan(false); app.say(t("app.search.scanStarted")); } },
        ];
    return [...recent, ...more];
  });
  const items = $derived.by((): Item[] => {
    const n = norm(q.trim());
    if (!n) return start;
    const styles = styleCounts
      .filter(([st]) => norm(st).includes(n))
      .slice(0, onSongs ? 6 : 4)
      .map(([st, c]) => ({
        sec: t("app.search.sec.styles"), title: st, meta: t("app.search.songCount", { n: c }), mark: "♪",
        run: () => { f.style = [st]; app.query = ""; go("#/songs"); },
      }));
    // on Songs the field already filters the songs: only the styles are offered under it
    if (onSongs) return styles;
    // every word found somewhere in the song: text, a tempo, a key ("dnb 174 fm lea")
    const words = parse(q);
    const onRel = releasesBySong(app.releases);
    const found = app.songs.filter((s) => !s.archived && matches(s, words, onRel));
    const songs = found.slice(0, 8).map((s) => songItem(s, t("app.search.sec.songs"), [s.artist, s.style, bpm(s.tempo), s.key]));
    // the artists named: their songs on the Songs page
    const artists = [...new Map(app.songs.filter((s) => !s.archived && s.artist && norm(s.artist).includes(n)).map((s) => [s.artist!, 0])).keys()]
      .slice(0, 4)
      .map((a) => ({
        sec: t("app.search.sec.artists"), title: a, meta: t("app.search.songCount", { n: app.songs.filter((s) => !s.archived && s.artist === a).length }), mark: "☺",
        run: () => go(`#/artist/${encodeURIComponent(a)}`),
      }));
    // more songs than shown: all of them on the Songs page, this search as its filter
    const all = found.length > 8
      ? [{ sec: t("app.search.sec.songs"), title: t("app.search.allSongs", { n: found.length }), mark: "≡", run: () => { app.query = q.trim(); setView("list"); go("#/songs"); } }]
      : [];
    const dirs = folders
      .filter((d) => norm(d.split("/").pop()).includes(n))
      .slice(0, 5)
      .map((d) => ({
        sec: t("app.search.sec.folders"), title: d.split("/").pop() ?? d, meta: d, mark: "▤",
        run: () => { app.query = d; setView("list"); go("#/songs"); },
      }));
    const cmds = COMMANDS.filter(([l]) => norm(l).includes(n))
      .slice(0, 5)
      .map(([l, run]) => ({ sec: t("app.search.sec.commands"), title: l, mark: "→", run }));
    const rels = app.releases
      .filter((r) => words.every((w) => norm([r.title, r.artist, r.kind, r.folder, r.date].join(" ")).includes(w.text)))
      .slice(0, 4)
      .map((r) => ({ sec: t("app.search.sec.releases"), title: r.title, meta: t("app.search.trackCount", { n: r.songs.length }) + (r.artist ? " · " + r.artist : ""), mark: "◉", run: () => go(`#/release/${r.id}`) }));
    return [...styles, ...artists, ...rels, ...songs, ...all, ...dirs, ...cmds];
  });

  function input() {
    if (onSongs) app.query = q;
    open = true;
    sel = 0;
  }
  function pick(i: number) {
    const it = items[i];
    if (!it) return;
    q = "";
    open = false;
    (document.getElementById("search") as HTMLInputElement | null)?.blur();
    it.run();
  }
  type Song = NonNullable<Item["song"]>;
  // the song heard now from the list: its render, or its project when it has none
  const hearing = (s: Song) => (s.temoin ? player.sid === s.id && player.playing : midi.rel === "notes:" + sessionKey(s.rel ?? "") && (midi.playing || midi.loading));
  const heard = $derived(items.find((it) => it.song && hearing(it.song))?.song ?? null);
  function listen(s: Song) {
    if (hearing(s)) return void (s.temoin ? player.toggle() : midi.stop());
    player.endQueue();
    if (s.temoin) return void player.playSong(s.id, s.title ?? "");
    playProjectRel(s.rel ?? "", s.title ?? "", (m) => app.say(m)).catch((err) => app.say(String(err), { error: true }));
  }
  let list = $state<HTMLDivElement>();
  // the line chosen stays in sight; while a song of the list plays, the one chosen plays instead
  function choose(i: number) {
    sel = i;
    list?.querySelector<HTMLElement>(`[data-i="${i}"]`)?.scrollIntoView({ block: "nearest" });
    const s = items[i]?.song;
    if (heard && s && s.id !== heard.id) listen(s);
  }
  // the first line of the next (or previous) section
  function jump(dir: 1 | -1) {
    const starts = items.map((it, i) => (i === 0 || items[i - 1].sec !== it.sec ? i : -1)).filter((i) => i >= 0);
    const cur = starts.filter((i) => i <= sel).pop() ?? 0;
    const k = starts.indexOf(cur) + dir;
    return starts[(k + starts.length) % starts.length];
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (q) {
        q = "";
        input();
      }
      open = false;
      (e.currentTarget as HTMLInputElement).blur();
      return;
    }
    if (!open || !items.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      choose((sel + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length);
    } else if (e.key === "PageDown" || e.key === "PageUp") {
      e.preventDefault();
      choose(jump(e.key === "PageDown" ? 1 : -1));
    } else if (e.key === " " && e.ctrlKey) {
      e.preventDefault();
      const s = items[sel]?.song;
      if (s) listen(s);
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(sel);
    }
  }
  // on Songs the field shows the filter; elsewhere it starts empty
  // (typing on Songs writes the filter, which must not close the styles offered meanwhile)
  $effect(() => {
    const v = onSongs ? app.query : "";
    if (v !== untrack(() => q)) {
      q = v;
      open = false;
    }
  });
</script>

<div class="wrap">
  <label class="search">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7" /><path d="M20 20l-3.5-3.5" /></svg>
    <input id="search" type="search" autocomplete="off" role="combobox" aria-expanded={open && !!items.length} aria-controls="search-results"
      placeholder={onSongs ? t("app.search.placeholderSongs") : t("app.search.placeholder")}
      bind:value={q} oninput={input} onkeydown={key} onfocus={() => { open = true; sel = 0; }} onclick={() => (open = true)} onblur={() => setTimeout(() => (open = false), 120)} />
    <kbd>Ctrl K</kbd>
  </label>
  {#if open && (items.length > 0 || (!onSongs && !!q.trim()))}
    <!-- the field keeps the focus: a click in the list (a line, its play button) never takes it -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="res" id="search-results" onmousedown={(e) => e.preventDefault()}>
      <div class="lines" role="listbox" tabindex="-1" bind:this={list}>
        {#each items as it, i (it.sec + it.title + i)}
          {#if i === 0 || items[i - 1].sec !== it.sec}<div class="sec">{it.sec}</div>{/if}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="it" class:hi={i === sel} class:on={!!it.song && heard?.id === it.song.id} role="option" tabindex="-1" aria-selected={i === sel} data-i={i} onclick={() => pick(i)} onmousemove={() => (sel = i)}>
            <span class="mk" class:art={it.h !== undefined} style:--h={it.h}>{it.mark}{#if it.song}<span class="pb"><PlayBtn sid={it.song.id} title={it.song.title ?? ""} size={26} play={() => listen(it.song!)} project={it.song.temoin ? "" : (it.song.rel ?? "")} /></span>{/if}</span>
            <span class="t">{it.title}</span>
            {#if it.meta}<span class="m">{it.meta}</span>{/if}
          </div>
        {:else}
          <div class="none small faint">{t("app.search.nothing", { q })}</div>
        {/each}
      </div>
      {#if items.length}
        <div class="keys">
          <span><kbd>↑</kbd><kbd>↓</kbd> {t("app.search.keys.move")}</span>
          {#if new Set(items.map((x) => x.sec)).size > 1}<span><kbd>{t("app.search.keys.page")}</kbd> {t("app.search.keys.section")}</span>{/if}
          <span><kbd>{t("app.search.keys.enter")}</kbd> {t("app.search.keys.open")}</span>
          {#if items.some((x) => x.song)}<span><kbd>Ctrl</kbd><kbd>{t("app.search.keys.space")}</kbd> {t("app.search.keys.listen")}</span>{/if}
          <span><kbd>{t("app.search.keys.esc")}</kbd> {t("app.search.keys.close")}</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap { flex: 1; max-width: 620px; position: relative; }
  .search { display: flex; align-items: center; gap: 8px; height: 36px; padding: 0 10px 0 12px; border-radius: 10px; background: var(--panel); border: 1px solid var(--line); }
  .search:focus-within { border-color: var(--accent); }
  .search svg { width: 16px; height: 16px; color: var(--muted); flex: none; }
  .search input { flex: 1; border: 0; background: none; padding: 0; outline: 0; height: 100%; }
  .search kbd { font: inherit; font-size: 10.5px; color: var(--faint); border: 1px solid var(--line); border-radius: 5px; padding: 1px 5px; }
  .res { position: absolute; top: 42px; left: 0; right: 0; z-index: 70; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 6px; box-shadow: 0 18px 40px rgba(0, 0, 0, .35); max-height: 70vh; display: flex; flex-direction: column; overflow: hidden; }
  .lines { padding: 6px; overflow: auto; overscroll-behavior: contain; scroll-padding-block: 28px 4px; }
  .keys { display: flex; flex-wrap: wrap; gap: 4px 14px; padding: 7px 12px; border-top: 1px solid var(--line); font-size: 11px; color: var(--faint); }
  .keys kbd { font: inherit; font-size: 10.5px; border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; margin-right: 2px; color: var(--muted); }
  .sec { font-size: 11px; color: var(--faint); padding: 8px 8px 4px; text-transform: none; font-weight: 600; }
  .it { display: grid; grid-template-columns: 26px minmax(0, auto) minmax(0, 1fr); align-items: center; gap: 10px; width: 100%; padding: 6px 8px; border: 0; background: none; border-radius: 8px; cursor: pointer; text-align: left; color: var(--ink); }
  .it.hi { background: var(--accent-bg); }
  /* a song's play button over its initial: seen on the line chosen, or while it plays */
  .pb { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0; transition: opacity .12s; }
  .it.hi .pb, .it.on .pb, .pb:focus-within { opacity: 1; }
  .mk { position: relative; width: 26px; height: 26px; border-radius: 7px; display: grid; place-items: center; font-size: 12px; color: var(--muted); background: var(--panel2); }
  .mk.art { color: #fff; font-weight: 650; background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .none { padding: 10px; }
</style>

<script lang="ts">
  // The search at the top. On Songs it filters the songs on screen (every view). Elsewhere
  // it finds a song, a folder or a command and goes there, from a list under the field
  // (arrows + Enter, Escape). Ctrl+K from anywhere.
  import { untrack } from "svelte";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { f, setView } from "../filters.svelte";
  import { openSong, closeAll } from "../tabs.svelte";
  import { bpm, hue } from "../format";
  import { t } from "../i18n/index.svelte";

  let { page, fold }: { page: string; fold: () => void } = $props();
  const onSongs = $derived(page === "songs" || page.startsWith("song/"));
  let q = $state("");
  let open = $state(false);
  let sel = $state(0);

  const norm = (s: string | null | undefined) => (s ?? "").normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  type Item = { sec: string; title: string; meta?: string; mark?: string; h?: number; run: () => void };

  const go = (h: string) => (location.hash = h);
  const VIEWS = ["list", "folders", "board", "cal"] as const;
  // built at render time: the labels follow a change of language
  const COMMANDS = $derived<[string, () => void][]>([
    [t("app.search.cmd.home"), () => go("#/home")],
    [t("app.search.cmd.songs"), () => go("#/songs")],
    [t("app.search.cmd.repair"), () => go("#/repair")],
    [t("app.search.cmd.create"), () => go("#/create")],
    [t("app.search.cmd.releases"), () => go("#/releases")],
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
    [t("app.search.cmd.toFix"), () => { f.plugins = "fix"; go("#/songs"); }],
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
  const items = $derived.by((): Item[] => {
    const n = norm(q.trim());
    if (!n) return [];
    const styles = styleCounts
      .filter(([st]) => norm(st).includes(n))
      .slice(0, onSongs ? 6 : 4)
      .map(([st, c]) => ({
        sec: t("app.search.sec.styles"), title: st, meta: t("app.search.songCount", { n: c }), mark: "♪",
        run: () => { f.style = st; app.query = ""; go("#/songs"); },
      }));
    // on Songs the field already filters the songs: only the styles are offered under it
    if (onSongs) return styles;
    const songs = app.songs
      .filter((s) => !s.archived && [s.title, s.artist, s.style, s.dir, ...s.instr].some((v) => norm(v).includes(n)))
      .slice(0, 7)
      .map((s) => ({
        sec: t("app.search.sec.songs"), title: s.title ?? "", h: hue(s.title ?? ""), mark: (s.title || "?").slice(0, 1).toUpperCase(),
        meta: [s.artist, s.style, bpm(s.tempo), s.key].filter(Boolean).join(" · "),
        run: () => { openSong(s.id); go(`#/song/${s.id}`); },
      }));
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
      .filter((r) => norm(r.title).includes(n) || norm(r.artist).includes(n))
      .slice(0, 4)
      .map((r) => ({ sec: t("app.search.sec.releases"), title: r.title, meta: t("app.search.trackCount", { n: r.songs.length }) + (r.artist ? " · " + r.artist : ""), mark: "◉", run: () => go(`#/release/${r.id}`) }));
    return [...styles, ...rels, ...songs, ...dirs, ...cmds];
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
      sel = (sel + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
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
      bind:value={q} oninput={input} onkeydown={key} onfocus={() => { if (q) open = true; }} onblur={() => setTimeout(() => (open = false), 120)} />
    <kbd>Ctrl K</kbd>
  </label>
  {#if open && q.trim() && (!onSongs || items.length)}
    <div class="res" id="search-results" role="listbox">
      {#each items as it, i (it.sec + it.title + i)}
        {#if i === 0 || items[i - 1].sec !== it.sec}<div class="sec">{it.sec}</div>{/if}
        <button class="it" class:hi={i === sel} role="option" aria-selected={i === sel} onmousedown={(e) => e.preventDefault()} onclick={() => pick(i)} onmousemove={() => (sel = i)}>
          <span class="mk" class:art={it.h !== undefined} style:--h={it.h}>{it.mark}</span>
          <span class="t">{it.title}</span>
          {#if it.meta}<span class="m">{it.meta}</span>{/if}
        </button>
      {:else}
        <div class="none small faint">{t("app.search.nothing", { q })}</div>
      {/each}
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
  .res { position: absolute; top: 42px; left: 0; right: 0; z-index: 70; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 6px; box-shadow: 0 18px 40px rgba(0, 0, 0, .35); max-height: 70vh; overflow: auto; }
  .sec { font-size: 11px; color: var(--faint); padding: 8px 8px 4px; text-transform: none; font-weight: 600; }
  .it { display: grid; grid-template-columns: 26px minmax(0, auto) minmax(0, 1fr); align-items: center; gap: 10px; width: 100%; padding: 6px 8px; border: 0; background: none; border-radius: 8px; cursor: pointer; text-align: left; color: var(--ink); }
  .it.hi { background: var(--accent-bg); }
  .mk { width: 26px; height: 26px; border-radius: 7px; display: grid; place-items: center; font-size: 12px; color: var(--muted); background: var(--panel2); }
  .mk.art { color: #fff; font-weight: 650; background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .none { padding: 10px; }
</style>

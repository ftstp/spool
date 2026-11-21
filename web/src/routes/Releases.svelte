<script lang="ts">
  // The releases (single, EP, album...): each with its cover, how far it is, its date. Below,
  // the folders whose name says EP / LP / album, made a release in one click. A release lives
  // in the app only: no file moves.
  import * as api from "../lib/api";
  import type { ReleaseHint } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Art from "../lib/components/Art.svelte";
  import { KIND_KEYS, countdown, kindLabel, longDate, minutes, natural, summary } from "../lib/releases";
  import PlayBtn from "../lib/components/PlayBtn.svelte";
  import { tracksOf, releasePlaying, playRelease } from "../lib/releasePlay.svelte";
  import { t } from "../lib/i18n/index.svelte";
  import { norm } from "../lib/songQuery";

  let hints = $state<ReleaseHint[]>([]);
  $effect(() => {
    void app.version; // again after every change (one made a release)
    api.releaseHints().then((h) => (hints = h)).catch(() => (hints = []));
  });
  const byId = $derived(new Map(app.songs.map((s) => [s.id, s])));
  // the artist most of its songs carry
  function artistOf(ids: number[]) {
    const n = new Map<string, number>();
    for (const id of ids) {
      const a = byId.get(id)?.artist;
      if (a) n.set(a, (n.get(a) ?? 0) + 1);
    }
    return [...n].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "";
  }
  // one release per click: a second click while the first is saved does nothing
  let creating = $state(false);
  async function create(h?: ReleaseHint) {
    if (creating) return;
    creating = true;
    const ids = h ? [...h.songs].sort((a, b) => natural(byId.get(a)?.title ?? "", byId.get(b)?.title ?? "")) : [];
    try {
      const [id, st] = await app.track(
        api.releaseSave({
          id: "", title: h?.title ?? t("releases.newTitle"), kind: h?.kind ?? "ep", artist: h ? h.artist || artistOf(h.songs) : "", date: null,
          cover_song: null, cover: null, notes: "", songs: ids, folder: h?.folder ?? null, created: 0, board: [],
        }),
      );
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
      // only the releases, then the page: the whole catalogue reloads behind (the
      // "changed" event), which a scan can make slow
      app.releases = await api.releases();
      location.hash = `#/release/${id}`;
    } catch (e) {
      app.say(String(e), { error: true });
    } finally {
      creating = false;
    }
  }
  const all = $derived(app.releases.map((r) => ({ r, s: summary(r, app.songs) })));

  // a release played whole, from its card (see lib/releasePlay)
  const isPlaying = releasePlaying;
  // filtered like the artists: words found in the title, artist, kind, date, notes or the
  // tracks' titles; a kind; out yet or not
  let q = $state("");
  let kind = $state("");
  let when = $state<"" | "soon" | "out" | "undated">("");
  const today = new Date().toISOString().slice(0, 10);
  // the folders that look like releases: by the same words
  const shownHints = $derived(
    (kind ? hints.filter((h) => h.kind === kind) : hints).filter((h) => !q.trim() || norm([h.title, h.artist, h.folder, kindLabel(h.kind)].join(" ")).includes(norm(q.trim()))),
  );
  // the formats offered: the usual ones, and any other a release uses
  const kinds = $derived([...new Set([...KIND_KEYS.filter((k) => k !== "compil"), ...app.releases.map((r) => r.kind)])]);
  // the songs marked "single" (their type) that no release holds yet: singles too
  const loneSingles = $derived(
    app.songs.filter(
      (x) => !x.archived && x.kinds.some((k) => k.toLowerCase() === "single") && !app.releases.some((r) => r.songs.includes(x.id)) &&
        (!q.trim() || norm(q).split(/\s+/).filter(Boolean).every((w) => norm([x.title, x.artist, x.dir].join(" ")).includes(w))),
    ),
  );
  const showSingles = $derived((kind === "" || kind === "single") && !when && loneSingles.length > 0);
  async function singleRelease(id: number, title: string, artist: string) {
    try {
      const [rid, st] = await api.releaseSave({ id: "", title, kind: "single", artist, date: null, cover_song: id, cover: null, notes: "", songs: [id], folder: null, created: 0, board: [] });
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
      app.releases = await api.releases();
      location.hash = `#/release/${rid}`;
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const list = $derived(
    all.filter(({ r }) => {
      if (kind && r.kind !== kind) return false;
      if (when === "soon" && !(r.date && r.date >= today)) return false;
      if (when === "out" && !(r.date && r.date < today)) return false;
      if (when === "undated" && r.date) return false;
      if (!q.trim()) return true;
      const hay = norm([r.title, r.artist, kindLabel(r.kind), r.date, r.notes, ...r.songs.map((id) => (id != null ? byId.get(id)?.title : ""))].join(" "));
      return norm(q).split(/\s+/).filter(Boolean).every((w) => hay.includes(w));
    }),
  );
</script>

<div class="page">
  <header class="head">
    <div>
      <h1>{t("releases.title")}</h1>
      <p class="muted sub">{t("releases.intro")}</p>
    </div>
    <button class="btn primary" disabled={creating} aria-busy={creating} onclick={() => create()}>{t("releases.new")}</button>
  </header>

  <!-- by format, always: everything, the singles (songs marked single included), EPs, albums -->
  {#if all.length || loneSingles.length}
    <div class="types" role="radiogroup" aria-label={t("releases.filter.kind")}>
      <button role="radio" aria-checked={kind === ""} class:on={kind === ""} onclick={() => (kind = "")}>{t("releases.filter.kindAll")} <span class="faint">{all.length + loneSingles.length}</span></button>
      {#each kinds as k (k)}
        {@const n = all.filter(({ r }) => r.kind === k).length + (k === "single" ? loneSingles.length : 0)}
        <button role="radio" aria-checked={kind === k} class:on={kind === k} onclick={() => (kind = kind === k ? "" : k)}>{kindLabel(k)} <span class="faint">{n}</span></button>
      {/each}
    </div>
  {/if}
  {#if all.length + hints.length > 3}
    <div class="filters">
      <input type="search" placeholder={t("releases.filter.q")} aria-label={t("releases.filter.q")} bind:value={q} />
      {#if all.length > 1}<select bind:value={when} aria-label={t("releases.filter.when")}>
        <option value="">{t("releases.filter.whenAll")}</option><option value="soon">{t("releases.filter.soon")}</option>
        <option value="out">{t("releases.filter.out")}</option><option value="undated">{t("releases.filter.undated")}</option>
      </select>{/if}
      {#if q || kind || when}<span class="small faint">{t("releases.filter.shown", { n: list.length + (showSingles ? loneSingles.length : 0), total: all.length + loneSingles.length })}</span>{/if}
    </div>
  {/if}

  {#if all.length && !list.length && !showSingles}<p class="small faint">{t("releases.filter.none")}</p>{/if}
  {#if list.length}
    <div class="grid">
      {#each list as { r, s } (r.id)}
        <a class="rel" href="#/release/{r.id}" data-release={r.id}>
          <div class="cv"><Art title={r.title} cover={s.cover} size="100%" radius={10} />
            {#if tracksOf(r).length}
              {@const on = isPlaying(r)}
              <button class="rplay" class:on title={on ? t("song.player.pause") : t("releases.playAll", { n: tracksOf(r).length })} aria-label={on ? t("song.player.pause") : t("releases.playAll", { n: tracksOf(r).length })} onclick={(e) => playRelease(e, r)}>
                {#if on}<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
                {:else}<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
              </button>
            {/if}
          </div>
          <div class="info">
            <b class="t">{r.title}</b>
            <span class="small muted">{kindLabel(r.kind)}{r.artist ? ` · ${r.artist}` : ""}</span>
            <span class="small faint">{t("releases.nTracks", { n: s.count })}{s.seconds ? ` · ${minutes(s.seconds)}` : ""}</span>
            <div class="prog" title={t("releases.readyTitle", { n: s.ready, total: s.count })}>
              <b style:width="{Math.round(s.progress * 100)}%" class:warn={s.blocked > 0}></b>
            </div>
            <span class="small" class:muted={!r.date}>{r.date ? t("releases.readyDate", { ready: s.ready, count: s.count, date: longDate(r.date), countdown: countdown(r.date) }) : t("releases.ready", { ready: s.ready, count: s.count })}</span>
          </div>
        </a>
      {/each}
    </div>
  {:else if !all.length}
    <section class="card empty">
      <h2>{t("releases.firstTitle")}</h2>
      <p class="muted">{t("releases.firstText")}</p>
      <button class="btn primary" disabled={creating} aria-busy={creating} onclick={() => create()}>{t("releases.create")}</button>
    </section>
  {/if}

  {#if showSingles}
    <section class="singles">
      <h2>{t("releases.singles.title")} <span class="faint">{loneSingles.length}</span></h2>
      <p class="small muted">{t("releases.singles.text")}</p>
      <div class="slist">
        {#each loneSingles as x (x.id)}
          <div class="srow card">
            <Art title={x.title} cover={x.cover} size={44} radius={7} />
            <div class="grow"><a href="#/song/{x.id}"><b>{x.title}</b></a><div class="small muted">{[x.artist, x.style].filter(Boolean).join(" · ")}</div></div>
            {#if x.temoin || x.rel}<PlayBtn sid={x.id} title={x.title ?? ""} size={26} queue={{ songs: loneSingles, name: t("releases.singles.title") }} project={x.temoin ? "" : x.rel} />{/if}
            <button class="btn sm" onclick={() => singleRelease(x.id, x.title ?? "", x.artist ?? "")}>{t("releases.singles.make")}</button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if hints.length}
    <section class="hints">
      <h2>{t("releases.hintsTitle")}</h2>
      <p class="small muted">{t("releases.hintsText")}</p>
      <div class="hl">
        {#each shownHints as h (h.folder)}
          {@const cov = h.songs.map((id) => byId.get(id)).find((x) => x?.cover)}
          <div class="hint">
            <Art title={h.title} cover={cov?.cover} size={44} radius={7} />
            <div class="hi">
              <b>{h.title}</b>
              <span class="small muted">{kindLabel(h.kind)} · {t("songs.nSongs", { n: h.songs.length })}{h.artist || artistOf(h.songs) ? ` · ${h.artist || artistOf(h.songs)}` : ""}</span>
              <span class="small faint path" title={h.folder}>{t("releases.inFolder", { path: h.folder.split("/").slice(-3, -1).join(" / ") || t("releases.mainFolder") })}</span>
            </div>
            <button class="btn sm" disabled={creating} onclick={() => create(h)}>{t("releases.make")}</button>
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 20px; margin-bottom: 18px; }
  .sub { margin: 4px 0 0; max-width: 720px; }
  h2 { font-size: 15px; margin: 0 0 4px; }
  .types { display: inline-flex; flex-wrap: wrap; border: 1px solid var(--line); border-radius: 9px; overflow: hidden; margin-bottom: 10px; }
  .types button { border: 0; background: none; color: var(--muted); padding: 5px 12px; cursor: pointer; font: inherit; font-size: 13px; }
  .types button + button { border-left: 1px solid var(--line); }
  .types button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .singles { margin-top: 22px; }
  .singles h2 { margin: 0 0 4px; }
  .slist { display: grid; grid-template-columns: repeat(auto-fill, minmax(330px, 1fr)); gap: 10px; margin-top: 10px; }
  .srow { display: flex; gap: 12px; align-items: center; }
  .srow .grow { flex: 1; min-width: 0; }
  .srow b { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .srow a { color: inherit; }
  .filters { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 14px; }
  .filters input { width: 240px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(330px, 1fr)); gap: 12px; }
  .rel { display: grid; grid-template-columns: 112px minmax(0, 1fr); gap: 14px; padding: 12px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); color: inherit;
    transition: border-color .15s, transform .15s; }
  .rel:hover { border-color: var(--faint); transform: translateY(-1px); }
  .cv { width: 112px; position: relative; }
  /* the whole release: on the cover, shown on hover, and while it plays */
  .rplay { position: absolute; right: 6px; bottom: 6px; width: 36px; height: 36px; border-radius: 50%; border: 0; background: var(--accent); color: var(--on-accent); display: grid; place-items: center; cursor: pointer; padding: 0;
    box-shadow: 0 4px 14px rgba(0, 0, 0, .3); opacity: 0; transform: translateY(4px); transition: opacity .15s, transform .2s cubic-bezier(.2, .8, .2, 1); }
  .rel:hover .rplay, .rplay:focus-visible, .rplay.on { opacity: 1; transform: none; }
  .rplay:hover { transform: scale(1.08); }
  .rplay:active { transform: scale(.96); }
  @media (prefers-reduced-motion: reduce) { .rplay { transition: none; transform: none; } }
  .info { display: flex; flex-direction: column; gap: 3px; min-width: 0; justify-content: center; }
  .t { font-size: 16px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .prog { height: 5px; border-radius: 5px; background: var(--line); overflow: hidden; margin: 6px 0 2px; }
  .prog b { display: block; height: 100%; background: var(--ok); border-radius: 5px; }
  .prog b.warn { background: var(--lost); }
  .empty { max-width: 560px; }
  .empty p { margin: 6px 0 14px; }
  .hints { margin-top: 30px; }
  .hints > p { margin: 0 0 10px; }
  .hl { display: grid; grid-template-columns: repeat(auto-fill, minmax(420px, 1fr)); gap: 8px; }
  .hint { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 9px 12px; border-radius: 10px; border: 1px dashed var(--line); }
  .hi { display: flex; flex-direction: column; min-width: 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

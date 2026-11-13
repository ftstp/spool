<script lang="ts">
  // The releases (single, EP, album...): each with its cover, how far it is, its date. Below,
  // the folders whose name says EP / LP / album, made a release in one click. A release lives
  // in the app only: no file moves.
  import * as api from "../lib/api";
  import type { ReleaseHint } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Art from "../lib/components/Art.svelte";
  import { countdown, kindLabel, longDate, minutes, natural, summary } from "../lib/releases";
  import { t } from "../lib/i18n/index.svelte";

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
  async function create(h?: ReleaseHint) {
    const ids = h ? [...h.songs].sort((a, b) => natural(byId.get(a)?.title ?? "", byId.get(b)?.title ?? "")) : [];
    try {
      const [id, st] = await app.track(
        api.releaseSave({
          id: "", title: h?.title ?? t("releases.newTitle"), kind: h?.kind ?? "ep", artist: h ? h.artist || artistOf(h.songs) : "", date: null,
          cover_song: null, notes: "", songs: ids, folder: h?.folder ?? null, created: 0,
        }),
      );
      app.undo = st;
      if (st.label) app.say(st.label, { undo: true });
      await app.reload();
      location.hash = `#/release/${id}`;
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const list = $derived(app.releases.map((r) => ({ r, s: summary(r, app.songs) })));
</script>

<div class="page">
  <header class="head">
    <div>
      <h1>{t("releases.title")}</h1>
      <p class="muted sub">{t("releases.intro")}</p>
    </div>
    <button class="btn primary" onclick={() => create()}>{t("releases.new")}</button>
  </header>

  {#if list.length}
    <div class="grid">
      {#each list as { r, s } (r.id)}
        <a class="rel" href="#/release/{r.id}">
          <div class="cv"><Art title={r.title} cover={s.cover} size="100%" radius={10} /></div>
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
  {:else}
    <section class="card empty">
      <h2>{t("releases.firstTitle")}</h2>
      <p class="muted">{t("releases.firstText")}</p>
      <button class="btn primary" onclick={() => create()}>{t("releases.create")}</button>
    </section>
  {/if}

  {#if hints.length}
    <section class="hints">
      <h2>{t("releases.hintsTitle")}</h2>
      <p class="small muted">{t("releases.hintsText")}</p>
      <div class="hl">
        {#each hints as h (h.folder)}
          {@const cov = h.songs.map((id) => byId.get(id)).find((x) => x?.cover)}
          <div class="hint">
            <Art title={h.title} cover={cov?.cover} size={44} radius={7} />
            <div class="hi">
              <b>{h.title}</b>
              <span class="small muted">{kindLabel(h.kind)} · {t("songs.nSongs", { n: h.songs.length })}{h.artist || artistOf(h.songs) ? ` · ${h.artist || artistOf(h.songs)}` : ""}</span>
              <span class="small faint path" title={h.folder}>{t("releases.inFolder", { path: h.folder.split("/").slice(-3, -1).join(" / ") || t("releases.mainFolder") })}</span>
            </div>
            <button class="btn sm" onclick={() => create(h)}>{t("releases.make")}</button>
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
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(330px, 1fr)); gap: 12px; }
  .rel { display: grid; grid-template-columns: 112px minmax(0, 1fr); gap: 14px; padding: 12px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); color: inherit;
    transition: border-color .15s, transform .15s; }
  .rel:hover { border-color: var(--faint); transform: translateY(-1px); }
  .cv { width: 112px; }
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

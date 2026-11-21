<script lang="ts">
  import { whoMenu, roleOf } from "../lib/artistRoles.svelte";
  import Skeleton from "../lib/components/Skeleton.svelte";
  // An artist's page: their profile (who they are, how to reach them, where they are heard,
  // their rights identifiers, notes), written in place and kept by Spool; their photo and
  // mood board (in Spool's own data folder); and what the catalogue knows of them: their
  // songs, the songs they are featured on, their releases, styles, keys and tempos.
  import * as api from "../lib/api";
  import type { ArtistPage } from "../lib/api/bindings/ArtistPage";
  import { app } from "../lib/state.svelte";
  import { hue, bpm } from "../lib/format";
  import { f } from "../lib/filters.svelte";
  import SongCard from "../lib/components/SongCard.svelte";
  import RefList from "../lib/components/RefList.svelte";
  import MoodBoard, { type BoardOps } from "../lib/components/MoodBoard.svelte";
  import PicturePicker from "../lib/components/PicturePicker.svelte";
  import { t } from "../lib/i18n/index.svelte";

  let { name }: { name: string } = $props();
  let a = $state<ArtistPage | null>(null);
  let error = $state("");
  let broken = $state(false);
  let seq = 0;
  $effect(() => {
    app.version; // again after every change
    const n = ++seq;
    api.artist(name).then((x) => n === seq && ((a = x), (error = ""))).catch((e) => n === seq && (error = String(e)));
  });

  // what is kept about an artist, in groups; each field saved when it is left
  const GROUPS: [string, [string, string][]][] = [
    ["who", [["real_name", "text"], ["location", "text"], ["bio", "area"]]],
    ["contact", [["email", "email"], ["phone", "tel"], ["manager", "text"]]],
    ["links", [["spotify", "url"], ["apple", "url"], ["deezer", "url"], ["soundcloud", "url"], ["youtube", "url"], ["instagram", "url"], ["tiktok", "url"], ["beatport", "url"], ["website", "url"]]],
    ["rights", [["society", "text"], ["member_id", "text"], ["ipi", "text"]]],
    ["notes", [["notes", "area"]]],
  ];
  const val = (k: string) => (a?.profile[k] as string | undefined) ?? "";
  async function save(k: string, v: string) {
    if (!a || v.trim() === val(k).trim()) return;
    try {
      await api.artistSave(a.name, { [k]: v.trim() });
      a.profile[k] = v.trim();
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  // a link opens in the browser; one typed without https:// gets it
  const linkOf = (k: string, v: string) => (k === "email" ? `mailto:${v}` : /^https?:\/\//i.test(v) ? v : `https://${v}`);
  const links = $derived(GROUPS[2][1].map(([k]) => [k, val(k)] as const).filter(([, v]) => v));

  const mine = $derived(a ? app.songs.filter((s) => a!.songs.includes(s.id) && !s.archived) : []);
  const mineRecent = $derived([...mine].sort((p, q) => q.mtime - p.mtime).slice(0, 8));
  const featured = $derived(a ? app.songs.filter((s) => a!.ft_songs.includes(s.id) && !s.archived) : []);
  const rels = $derived(a ? app.releases.filter((r) => a!.releases.includes(r.id)) : []);
  const count = (xs: (string | null | undefined)[]) => {
    const m = new Map<string, number>();
    for (const x of xs) if (x) m.set(x, (m.get(x) ?? 0) + 1);
    return [...m].sort((p, q) => q[1] - p[1]);
  };
  const styles = $derived(count(mine.flatMap((s) => (s.style ?? "").split(",").map((x) => x.trim().toLowerCase()))).slice(0, 6));
  const keys = $derived(count(mine.map((s) => s.key)).slice(0, 5));
  const tempos = $derived(mine.map((s) => s.tempo).filter((x): x is number => !!x).sort((p, q) => p - q));
  // their photo only: a song's cover shown in its place was a guess, and changed with the
  // songs (Madeby showed the artwork of one track). A click on it changes it.
  const pic = $derived(a?.photo ?? null);
  $effect(() => {
    void pic;
    broken = false;
  });
  let picking = $state(false);
  // offered: the pictures of their board, then their songs' covers and folder pictures
  const candidates = $derived([
    ...(a?.board.items ?? []).filter((i) => i.kind === "image").map((i) => ({ path: i.rel, label: i.name })),
    ...mine.filter((s) => s.cover).map((s) => ({ path: s.cover!, label: s.title ?? "" })),
  ]);
  async function setPhoto(path: string) {
    if (!a) return;
    await api.artistBoardCopy(a.name, path, true);
    app.say(t("artists.photoSet"));
  }
  async function uploadPhoto(file: File) {
    if (!a) return;
    const n = await api.artistBoardAdd(a.name, file);
    await api.artistSave(a.name, { photo: n });
    app.say(t("artists.photoSet"));
  }
  const initials = (n: string) => n.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join("").toUpperCase();

  const ops = $derived<BoardOps>({
    note: (text, lyrics, old) => api.artistBoardNote(name, text, lyrics, old),
    add: (file) => api.artistBoardAdd(name, file),
    remove: (file) => api.artistBoardRemove(name, file),
    meta: (patch) => api.artistBoardMeta(name, patch),
  });
  const songsOf = () => {
    f.artist = a?.name ?? name;
    app.query = "";
    location.hash = "#/songs";
  };
  // who this name is (one of the user's names, a guest...): changed from the button by the name
  const whoLabel = $derived.by(() => {
    if (!a) return "";
    const r = roleOf(app.config, a.name);
    return r.role === "unset" ? t("artists.who.unset") : t(`artists.who.is.${r.role}`);
  });
</script>

<div class="page">
  <a class="small back" href="#/artists">← {t("artists.title")}</a>
  {#if error}<p class="bad">{error}</p>
  {:else if !a}<Skeleton kind="page" />
  {:else}
    <header class="top">
      <button class="ph" style:--h={hue(a.name)} data-media={pic ?? undefined} data-kind="image" title={t("artists.changePhoto")} aria-label={t("artists.changePhoto")} onclick={() => (picking = true)}>
        {#if pic && api.mediaUrl(pic) && !broken}<img src={api.thumbUrl(pic, 400)} alt={a.name} onerror={() => (broken = true)} />{:else}<span class="ini">{initials(a.name)}</span>{/if}
        <span class="edit">{pic ? t("artists.changePhotoShort") : t("artists.addPhoto")}</span>
      </button>
      <div class="who">
        <div class="nameline"><h1>{a.name}</h1><button class="whob small" title={t("artists.who.tip")} onclick={(e) => whoMenu(e, a!.name, true)}>{whoLabel} ▾</button></div>
        {#if val("real_name") || val("location")}<p class="muted">{[val("real_name"), val("location")].filter(Boolean).join(" · ")}</p>{/if}
        {#if a.aliases.length}<p class="small faint">{t("artists.aliases", { names: a.aliases.join(", ") })}</p>{/if}
        <p class="small">
          {#if mine.length}<a href="#/songs" onclick={(e) => { e.preventDefault(); songsOf(); }}>{t("artists.songs", { n: mine.length })}</a>{/if}
          {[featured.length && t("artists.ft", { n: featured.length }), rels.length && t("artists.releases", { n: rels.length }), tempos.length && (tempos[0] === tempos.at(-1) ? bpm(tempos[0]) : `${Math.round(tempos[0])}-${Math.round(tempos.at(-1)!)} bpm`)].filter(Boolean).map((x) => " · " + x).join("")}
        </p>
        {#if links.length}
          <div class="links">{#each links as [k, v] (k)}<button class="chip" title={v} onclick={() => api.openLink(linkOf(k, v)).catch((e) => app.say(String(e), { error: true }))}>{t(`artists.field.${k}`)}</button>{/each}</div>
        {/if}
      </div>
    </header>

    {#if picking}
      <PicturePicker title={t("artists.photoTitle", { name: a.name })} current={pic} {candidates} songs={mine.map((s) => s.id)}
        pick={setPhoto} upload={uploadPhoto} close={() => (picking = false)} />
    {/if}

    <div class="cols">
      <div class="main">
        {#if styles.length || keys.length}
          <section class="card facts">
            {#if styles.length}<div><span class="small muted">{t("artists.styles")}</span><div class="chips">{#each styles as [s, n] (s)}<span class="chip">{s} <span class="faint">{n}</span></span>{/each}</div></div>{/if}
            {#if keys.length}<div><span class="small muted">{t("artists.keys")}</span><div class="chips">{#each keys as [k, n] (k)}<span class="chip">{k} <span class="faint">{n}</span></span>{/each}</div></div>{/if}
          </section>
        {/if}

        {#if mine.length}
          <section>
            <div class="sh"><h2>{t("artists.theirSongs")} <span class="faint">{mine.length}</span></h2>{#if mine.length > 8}<a class="small" href="#/songs" onclick={(e) => { e.preventDefault(); songsOf(); }}>{t("artists.allSongs")}</a>{/if}</div>
            <div class="cards">{#each mineRecent as s (s.id)}<SongCard song={s} queue={{ songs: mineRecent, name: name }} />{/each}</div>
          </section>
        {/if}
        {#if featured.length}
          <section>
            <h2>{t("artists.featuredOn")} <span class="faint">{featured.length}</span></h2>
            <div class="cards">{#each featured.slice(0, 8) as s (s.id)}<SongCard song={s} queue={{ songs: featured.slice(0, 8), name: name }} />{/each}</div>
          </section>
        {/if}
        {#if rels.length}
          <section class="card">
            <h2>{t("artists.theirReleases")}</h2>
            {#each rels as r (r.id)}<a class="row" href="#/release/{r.id}"><b>{r.title}</b><span class="small muted">{[r.kind, r.date, t("artists.tracks", { n: r.songs.length })].filter(Boolean).join(" · ")}</span></a>{/each}
          </section>
        {/if}

        <section class="card"><RefList kind="artist" key={a.name} /></section>

        <MoodBoard board={a.board} {ops} picked={(a.profile.photo as string) ?? null} pickLabel={t("artists.usePhoto")}
          pick={(i) => save("photo", i.name)} />
      </div>

      <aside class="card profile">
        <h2>{t("artists.profile")}</h2>
        {#each GROUPS as [g, fields] (g)}
          <h3>{t(`artists.group.${g}`)}</h3>
          {#each fields as [k, kind] (k)}
            <label class="fld">
              <span class="small muted">{t(`artists.field.${k}`)}</span>
              {#if kind === "area"}
                <textarea rows={k === "bio" ? 5 : 4} value={val(k)} onblur={(e) => save(k, e.currentTarget.value)}></textarea>
              {:else}
                <input type="text" inputmode={kind === "email" ? "email" : kind === "tel" ? "tel" : kind === "url" ? "url" : "text"} value={val(k)} placeholder={kind === "url" ? t("artists.urlHint") : ""} onblur={(e) => save(k, e.currentTarget.value)}
                  onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
              {/if}
            </label>
          {/each}
        {/each}
        <p class="small faint">{t("artists.keptHere")}</p>
      </aside>
    </div>
  {/if}
</div>

<style>
  .nameline { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .nameline h1 { margin: 0; }
  .whob { border: 1px solid var(--line); border-radius: 6px; background: var(--panel); color: var(--muted); padding: 2px 8px; font: inherit; font-size: 11.5px; cursor: pointer; }
  .whob:hover { color: var(--ink); border-color: var(--faint); }
  .page { padding: 20px 28px 60px; }
  .back { color: var(--muted); }
  .top { display: flex; gap: 20px; align-items: center; margin: 12px 0 20px; }
  .ph { position: relative; width: 112px; height: 112px; flex: none; border-radius: 50%; overflow: hidden; display: grid; place-items: center; color: #fff; font: inherit; font-weight: 650; font-size: 34px;
    border: 0; padding: 0; cursor: pointer; background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .ph img { width: 100%; height: 100%; object-fit: cover; }
  .ph .ini, .ph img { grid-area: 1 / 1; }
  .ph .edit { position: absolute; inset: auto 0 0 0; padding: 6px 0 9px; font-size: 11.5px; font-weight: 600; background: rgba(0, 0, 0, .55); opacity: 0; transition: opacity .15s; }
  .ph:hover .edit, .ph:focus-visible .edit { opacity: 1; }
  .who h1 { margin: 0 0 2px; }
  .who p { margin: 2px 0; }
  .links { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  .chip { background: var(--panel); color: inherit; font: inherit; font-size: 12px; padding: 2px 9px; border-radius: 99px; border: 1px solid var(--line); white-space: nowrap; }
  button.chip { cursor: pointer; }
  button.chip:hover { border-color: var(--accent); }
  .cols { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: 18px; align-items: start; }
  @media (max-width: 1100px) { .cols { grid-template-columns: minmax(0, 1fr); } }
  .main { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .facts { display: flex; gap: 24px; flex-wrap: wrap; }
  .chips { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 4px; }
  .sh { display: flex; justify-content: space-between; align-items: baseline; }
  h2 { margin: 0 0 10px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
  .row { display: flex; justify-content: space-between; gap: 10px; padding: 7px 0; border-bottom: 1px solid var(--line); color: inherit; }
  .row:last-child { border-bottom: 0; }
  .row:hover b { color: var(--accent); }
  .profile h3 { margin: 14px 0 6px; font-size: 12.5px; font-weight: 600; color: var(--muted); }
  .fld { display: flex; flex-direction: column; gap: 3px; margin-bottom: 8px; }
  .fld input, .fld textarea { width: 100%; box-sizing: border-box; }
  .fld textarea { resize: vertical; font: inherit; }
</style>

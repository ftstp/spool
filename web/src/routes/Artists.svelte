<script lang="ts">
  import Skeleton from "../lib/components/Skeleton.svelte";
  // The Artists page: who is who. The user's own names first (each with the other names read
  // as it), then the guests and the other artists the catalogue names, and above both the
  // names found in the folders that nobody has said anything about yet, each asked once:
  // who is it? (see lib/artistRoles). A card opens the artist's page.
  import * as api from "../lib/api";
  import type { ArtistCard } from "../lib/api/bindings/ArtistCard";
  import { app } from "../lib/state.svelte";
  import { ui } from "../lib/contextMenus.svelte";
  import { hue, ago } from "../lib/format";
  import { norm } from "../lib/songQuery";
  import { t } from "../lib/i18n/index.svelte";
  import { myNames, roleOf, aliasesOf, setRole, whoMenu } from "../lib/artistRoles.svelte";
  import { openMenu } from "../lib/menu.svelte";

  let list = $state<ArtistCard[] | null>(null);
  let found = $state<api.ArtistGuess[]>([]);
  // to sort: the names that look like artists (in file names AND in several folders); the
  // others (an album's folder, a category: "Pop Tunes", "Broken Notes") only on demand
  let showUnsure = $state(false);
  const sure = $derived(found.filter((g) => g.sure));
  const unsure = $derived(found.filter((g) => !g.sure));
  const toSort = $derived(showUnsure ? [...sure, ...unsure] : sure);
  let q = $state("");
  // pictures that would not load: the initials instead
  let broken = $state<Record<string, boolean>>({});
  let seq = 0;
  $effect(() => {
    app.version; // again after every change
    const n = ++seq;
    api.artists().then((x) => n === seq && (list = x)).catch(() => n === seq && (list = []));
    api.artistSuggestions().then((x) => n === seq && (found = x)).catch(() => n === seq && (found = []));
  });
  const cfg = $derived(app.config);
  const match = (a: ArtistCard) => !q.trim() || norm([a.name, ...a.styles, ...aliasesOf(cfg, a.name)].join(" ")).includes(norm(q.trim()));
  const isMine = (a: ArtistCard) => roleOf(cfg, a.name).role === "me";
  // a name read as another one is shown on that one's card, not on its own
  const visible = $derived((list ?? []).filter((a) => roleOf(cfg, a.name).role !== "alias" && roleOf(cfg, a.name).role !== "none"));
  const mine = $derived(visible.filter((a) => isMine(a) && match(a)));
  const others = $derived(visible.filter((a) => !isMine(a) && match(a)));
  const href = (name: string) => `#/artist/${encodeURIComponent(name)}`;
  const initials = (n: string) => n.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join("").toUpperCase();
  // a name of the user's own, added by hand (their songs are filed under it at once)
  function add() {
    ui.prompt = {
      title: t("artists.newName"),
      value: "",
      ok: async (v) => {
        const name = v.trim();
        if (name) await setRole(name, "me");
      },
    };
  }
  // never answered for: featured only, it is a guest already (a "(ft X)" in a folder name)
  const label = (a: ArtistCard) => {
    const r = roleOf(cfg, a.name);
    if (r.role === "unset") return a.ft && !a.songs ? t("artists.who.is.guest") : t("artists.who.unset");
    return t(`artists.who.is.${r.role}`);
  };
  function aliasMenu(e: MouseEvent, name: string) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu({ preventDefault() {}, stopPropagation() {}, clientX: r.left, clientY: r.bottom + 4 } as MouseEvent,
      myNames(cfg).map((m) => ({ label: m, run: () => setRole(name, "alias", m) })), t("artists.who.aliasOf", { name }));
  }
</script>

{#snippet card(a: ArtistCard)}
  {@const cover = a.photo}
  {@const also = aliasesOf(cfg, a.name)}
  <a class="card artist" href={href(a.name)} oncontextmenu={(e) => whoMenu(e, a.name)}>
    <span class="ph" style:--h={hue(a.name)}>
      {#if cover && api.mediaUrl(cover) && !broken[cover]}<img src={api.thumbUrl(cover, 240)} alt="" loading="lazy" onerror={() => (broken[cover] = true)} />{:else}{initials(a.name)}{/if}
    </span>
    <span class="txt">
      <b class="nm">{a.name}</b>
      {#if also.length}<span class="small muted" title={also.join(", ")}>{t("artists.aliases", { names: also.join(", ") })}</span>{/if}
      <span class="small muted">{[a.songs && t("artists.songs", { n: a.songs }), a.ft && t("artists.ft", { n: a.ft }), a.releases && t("artists.releases", { n: a.releases })].filter(Boolean).join(" · ") || t("artists.empty")}</span>
      {#if a.styles.length}<span class="small faint">{a.styles.join(", ")}</span>{/if}
      {#if a.last}<span class="small faint">{t("artists.last", { when: ago(a.last) })}{#if !a.profile} · <span title={t("artists.noProfileTip")}>{t("artists.noProfile")}</span>{/if}</span>{/if}
    </span>
    <button class="who small" title={t("artists.who.tip")} onclick={(e) => { e.preventDefault(); e.stopPropagation(); whoMenu(e, a.name, true); }}>{label(a)} ▾</button>
  </a>
{/snippet}

<div class="page">
  <header class="head">
    <div>
      <h1>{t("artists.title")}</h1>
      <p class="muted sub">{list ? t("artists.sub2", { mine: mine.length, others: others.length }) : " "}</p>
    </div>
    <div class="hr">
      <input type="search" class="filter" placeholder={t("artists.filter")} bind:value={q} aria-label={t("artists.filter")} />
      <button class="btn sm primary" onclick={add}>{t("artists.addMine")}</button>
    </div>
  </header>

  {#if toSort.length}
    <!-- names found in the folders, never answered for: until they are, they stay in titles -->
    <section class="sort card">
      <div class="sh"><h2>{t("artists.toSort.title")} <span class="count">{toSort.length}</span></h2><p class="small muted">{t("artists.toSort.hint")}</p></div>
      <ul>
        {#each toSort as g (g.name)}
          <li>
            <span class="gn"><b>{g.name}</b> <span class="small faint" title={t("artists.toSort.seen", { files: g.files, folders: g.folders })}>{t("artists.toSort.n", { n: g.files || g.songs })}</span></span>
            <span class="acts">
              <button class="btn sm" onclick={() => setRole(g.name, "me")}>{t("artists.who.me")}</button>
              <button class="btn sm" disabled={!myNames(cfg).length} onclick={(e) => aliasMenu(e, g.name)}>{t("artists.who.aliasShort")} ▾</button>
              <button class="btn sm" onclick={() => setRole(g.name, "guest")}>{t("artists.who.guestShort")}</button>
              <button class="btn sm ghost" onclick={() => setRole(g.name, "none")}>{t("artists.who.noneShort")}</button>
            </span>
          </li>
        {/each}
      </ul>
      {#if unsure.length}<button class="unsure small" onclick={() => (showUnsure = !showUnsure)}>{showUnsure ? t("artists.toSort.hideUnsure") : t("artists.toSort.showUnsure", { n: unsure.length })}</button>{/if}
    </section>
  {:else if unsure.length}
    <p class="small faint more"><button class="unsure small" onclick={() => (showUnsure = true)}>{t("artists.toSort.showUnsure", { n: unsure.length })}</button></p>
  {/if}

  {#if !list}<Skeleton kind="list" />
  {:else if !mine.length && !others.length}<p class="small faint">{q ? t("artists.noMatch", { q }) : t("artists.none")}</p>
  {:else}
    <h2 class="gh">{t("artists.mine")} <span class="faint">{mine.length}</span></h2>
    {#if mine.length}<div class="grid">{#each mine as a (a.name)}{@render card(a)}{/each}</div>
    {:else}<p class="small faint">{t("artists.mineNone")}</p>{/if}
    {#if others.length}
      <h2 class="gh">{t("artists.others")} <span class="faint">{others.length}</span></h2>
      <p class="small muted gsub">{t("artists.othersHint")}</p>
      <div class="grid">{#each others as a (a.name)}{@render card(a)}{/each}</div>
    {/if}
  {/if}
</div>

<style>
  .page { padding: 24px 28px 60px; }
  .head { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; margin-bottom: 16px; }
  .head h1 { margin: 0; }
  .sub { margin: 4px 0 0; }
  .hr { display: flex; gap: 8px; align-items: center; }
  .filter { width: 220px; }
  .gh { margin: 22px 0 10px; font-size: 15px; }
  .gh .faint { font-weight: 400; font-size: 13px; }
  .gsub { margin: -6px 0 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .artist { position: relative; display: flex; gap: 12px; align-items: center; color: inherit; transition: border-color .15s; }
  .artist:hover { border-color: var(--faint); }
  .ph { width: 64px; height: 64px; flex: none; border-radius: 50%; overflow: hidden; display: grid; place-items: center; color: #fff; font-weight: 650; font-size: 20px;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .ph img { width: 100%; height: 100%; object-fit: cover; }
  .txt { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .nm { font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .txt span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* who it is: on every card, a click changes it */
  .who { flex: none; align-self: flex-start; border: 1px solid var(--line); border-radius: 6px; background: var(--panel); color: var(--muted); padding: 1px 7px; font: inherit; font-size: 11px; cursor: pointer; }
  .who:hover { color: var(--ink); border-color: var(--faint); }
  /* the names to sort */
  .sort { border-color: color-mix(in srgb, var(--accent) 55%, var(--line)); margin-bottom: 8px; }
  .sh h2 { margin: 0 0 2px; display: flex; align-items: center; gap: 8px; }
  .sh p { margin: 0 0 10px; }
  .count { font-size: 12px; font-weight: 700; padding: 1px 8px; border-radius: 10px; background: var(--accent); color: var(--on-accent); }
  .sort ul { list-style: none; margin: 0; padding: 0; max-height: 320px; overflow-y: auto; }
  .sort li { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 0; border-top: 1px solid var(--line); }
  .gn { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .acts { display: flex; gap: 6px; flex: none; }
  .btn.ghost { background: none; color: var(--muted); }
  .unsure { border: 0; background: none; padding: 0; margin-top: 8px; font: inherit; color: var(--muted); cursor: pointer; text-decoration: underline; text-underline-offset: 2px; }
  .unsure:hover { color: var(--ink); }
  .more { margin: 0 0 8px; }
</style>

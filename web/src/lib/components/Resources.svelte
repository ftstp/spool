<script lang="ts">
  // A song's resources: the samples its project plays (in its own Samples folder, in a
  // library elsewhere, or not found) and what its Samples folders keep that no clip uses.
  // Read when the section opens; each row plays, drags into Live, and shows in Explorer.
  import * as api from "../api";
  import type { Resource } from "../api/bindings/Resource";
  import type { Resources } from "../api/bindings/Resources";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";
  import { player, playOrPause } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { openMenu, type MenuItem } from "../menu.svelte";
  import PlayIcon from "./PlayIcon.svelte";

  // `rel`: the project the sheet shows (its latest version), read for what it plays
  let { sid, rel = "" }: { sid: number; rel?: string } = $props();
  let res = $state<Resources | null>(null);
  let err = $state("");
  let all = $state(false);
  $effect(() => {
    const id = sid;
    const r0 = rel;
    void app.version;
    api.songResources(id, r0 || undefined).then((r) => { if (id === sid) res = r; }).catch((e) => (err = String(e)));
  });
  const fail = (e: unknown) => app.say(String(e), { error: true });
  const used = $derived(res?.items.filter((r) => r.used) ?? []);
  const unused = $derived(res?.items.filter((r) => !r.used) ?? []);
  const count = (l: Resource[], k: string) => l.filter((r) => r.kind === k).length;
  const missing = $derived(used.filter((r) => r.place === "missing").length);
  // a long list shows its start: what the project plays comes first
  const SHOW = 40;
  const shown = (l: Resource[]) => (all ? l : l.slice(0, SHOW));

  const isPlaying = (r: Resource) => (r.kind === "midi" ? midi.playing && midi.rel === r.path : player.is(r.path) && player.playing);
  function play(r: Resource) {
    if (r.place === "missing") return;
    if (r.kind === "midi") midi.play(r.path, "", r.name);
    else playOrPause(r.path, () => player.play(r.path, null, r.name));
  }
  function menu(e: MouseEvent, r: Resource) {
    const items: MenuItem[] = [];
    if (r.place !== "missing") {
      items.push({ label: isPlaying(r) ? t("menu.media.stop") : t("menu.media.listen"), run: () => play(r) });
      items.push({ sep: true });
      if (r.kind === "audio") items.push({ label: t("song.session.menu.similarSamples"), run: () => (location.hash = "#/samples?alike=" + encodeURIComponent(r.path)) });
      items.push({ label: t("song.session.menu.revealSample"), run: () => api.reveal(r.path).catch(fail) });
    } else {
      items.push({ label: t("song.resources.missingTip"), disabled: true, run: () => {} });
    }
    items.push({ label: t("menu.folder.copyPath"), run: () => api.copyPath(r.path).then((x) => app.say(t("menu.pathCopied", { path: x }))).catch(fail) });
    openMenu(e, items, r.name);
  }
</script>

{#snippet row(r: Resource)}
  <li class="r" class:gone={r.place === "missing"} oncontextmenu={(e) => { e.preventDefault(); menu(e, r); }}
    draggable={r.place !== "missing"} ondragstart={(e) => { e.preventDefault(); api.dragOut(r.path).catch(fail); }}>
    <button class="pl" disabled={r.place === "missing"} title={r.place === "missing" ? t("song.resources.missingTip") : t("menu.media.listen")} onclick={() => play(r)}>
      {#if r.kind === "midi"}<span class="k">MIDI</span>{:else}<PlayIcon on={isPlaying(r)} />{/if}
    </button>
    <span class="nm ell" title={r.path}>{r.name}</span>
    <span class="dir ell faint small" title={r.dir}>{r.dir}</span>
    <span class="tag small {r.place}">{t(`song.resources.place.${r.place}`)}</span>
  </li>
{/snippet}

{#if err}<p class="small err">{err}</p>
{:else if !res}<p class="small faint">{t("song.resources.loading")}</p>
{:else if !res.items.length}<p class="small faint">{t("song.resources.none")}</p>
{:else}
  <p class="small muted">
    {t("song.resources.summary", { audio: count(used, "audio"), midi: count(used, "midi") })}{#if missing}<span class="miss"> · {t("song.resources.missing", { n: missing })}</span>{/if}
    {#if res.project}<span class="faint"> · {t("song.resources.from", { file: res.project.split("/").pop() ?? res.project })}</span>{/if}
  </p>
  {#if used.length}<ul class="list">{#each shown(used) as r (r.path)}{@render row(r)}{/each}</ul>{/if}
  {#if unused.length}
    <h3 class="small muted">{t("song.resources.unused", { n: unused.length })}</h3>
    <ul class="list">{#each shown(unused) as r (r.path)}{@render row(r)}{/each}</ul>
  {/if}
  {#if !all && (used.length > SHOW || unused.length > SHOW)}<button class="link small" onclick={() => (all = true)}>{t("song.resources.showAll")}</button>{/if}
  {#if res.more}<p class="small faint">{t("song.resources.more", { n: res.more })}</p>{/if}
{/if}

<style>
  .list { list-style: none; margin: 4px 0 8px; padding: 0; display: flex; flex-direction: column; }
  .r { display: grid; grid-template-columns: 28px minmax(0, 1fr) minmax(0, 0.8fr) auto; gap: 8px; align-items: center; padding: 3px 4px; border-radius: 6px; cursor: grab; }
  .r:hover { background: var(--hover, color-mix(in srgb, var(--ink) 6%, transparent)); }
  .r.gone { cursor: default; }
  .r.gone .nm { color: var(--muted); text-decoration: line-through; text-decoration-color: var(--faint); }
  .pl { width: 26px; height: 22px; display: grid; place-items: center; border: 0; background: none; color: var(--ink); cursor: pointer; padding: 0; }
  .pl:disabled { opacity: .35; cursor: default; }
  .k { font-size: 9px; font-weight: 700; letter-spacing: .04em; color: var(--muted); }
  .nm { min-width: 0; }
  .dir { min-width: 0; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { padding: 0 7px; border-radius: 99px; border: 1px solid var(--line); color: var(--muted); white-space: nowrap; }
  .tag.missing, .miss { color: var(--bad, #c55); }
  .tag.missing { border-color: currentColor; }
  h3 { margin: 10px 0 2px; }
</style>

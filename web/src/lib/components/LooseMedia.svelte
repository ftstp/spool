<script lang="ts">
  // The Media page's "outside projects": every picture, video, sound and note in a folder with
  // no project, outside every song's folder (the same files as the Folders view's panel), by
  // the folder they sit in. They can be picked, several at once, for a mood board; a sound
  // can also become a song's reference render. Nothing is moved on disk.
  import * as api from "../api";
  import type { MediaFolder } from "../api";
  import type { LooseAudio } from "../api/bindings/LooseAudio";
  import { app } from "../state.svelte";
  import { player, playOrPause, playingFile } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { ago } from "../format";
  import PlayIcon from "./PlayIcon.svelte";
  import LooseAudioList from "./LooseAudioList.svelte";

  let { folders, near, q = "", selected = $bindable(new Set<string>()) }: { folders: MediaFolder[]; near: Map<string, LooseAudio>; q?: string; selected?: Set<string> } = $props();

  type Kind = "" | "image" | "video" | "audio" | "text";
  const KINDS: Kind[] = ["", "image", "video", "audio", "text"];
  const kindOf = (k: string): Kind => (k === "image" || k === "gif" ? "image" : k === "video" || k === "audio" ? k : k === "text" || k === "doc" ? "text" : "");
  let kind = $state<Kind>("");
  const words = $derived(q.toLowerCase().split(/\s+/).filter(Boolean));
  type It = MediaFolder["items"][number];
  const fits = (i: It) => (!kind || kindOf(i.kind) === kind) && words.every((w) => i.rel.toLowerCase().includes(w));
  const groups = $derived(folders.map((f) => ({ dir: f.dir, items: f.items.filter(fits) })).filter((g) => g.items.length));
  const count = (k: Kind) => folders.reduce((n, f) => n + f.items.filter((i) => !k || kindOf(i.kind) === k).length, 0);
  // the sounds, in the audio list's shape (with the songs of a close name when known)
  const sounds = $derived(
    kind !== "audio"
      ? []
      : groups.flatMap((g) => g.items).map((i) => near.get(i.rel) ?? { rel: i.rel, size: i.size, mtime: i.mtime, songs: [], near: [] }),
  );

  const STEP = 30;
  let upTo = $state(STEP);
  $effect(() => {
    void kind;
    void q;
    upTo = STEP;
  });
  const pick = (rels: string[], on: boolean) => {
    const s = new Set(selected);
    for (const r of rels) on ? s.add(r) : s.delete(r);
    selected = s;
  };
  let big = $state<It | null>(null);
  const sizeOf = (b: number) => (b > 1e6 ? `${(b / 1e6).toFixed(1)} Mo` : `${Math.max(1, Math.round(b / 1e3))} ko`);
  const fail = (e: unknown) => app.say(String(e), { error: true });
</script>

<div class="kinds">
  {#each KINDS as k (k)}
    {#if !k || count(k)}<button class="chip" class:on={kind === k} onclick={() => (kind = k)}>{t(`media.kind.${k || "all"}`)} <span class="faint">{fmtNum(count(k))}</span></button>{/if}
  {/each}
</div>
<p class="small faint lede">{t("media.loose.lede")}</p>

{#if kind === "audio"}
  <LooseAudioList items={sounds} bind:selected />
{:else if !groups.length}<p class="faint">{t("media.noneHere")}</p>
{:else}
  {#each groups.slice(0, upTo) as g (g.dir)}
    {@const imgs = g.items.filter((i) => kindOf(i.kind) === "image")}
    {@const rest = g.items.filter((i) => kindOf(i.kind) !== "image")}
    <section class="grp" data-folder={g.dir}>
      <div class="gh">
        <input type="checkbox" aria-label={t("media.pick.folder")} checked={g.items.every((i) => selected.has(i.rel))} onchange={(e) => pick(g.items.map((i) => i.rel), e.currentTarget.checked)} />
        <b class="ell" title={g.dir}>{g.dir || "/"}</b>
        <span class="faint small">{t("media.audio.count", { n: g.items.length })}</span>
        <span class="grow"></span>
        <button class="btn sm" onclick={() => api.reveal(g.dir).catch(fail)}>{t("media.audio.reveal")}</button>
      </div>
      {#if imgs.length}<div class="tiles">
        {#each imgs as i (i.rel)}
          <figure class="tile" class:sel={selected.has(i.rel)} data-media={i.rel} data-kind="image">
            <button class="img" onclick={() => (big = i)} aria-label={t("media.enlarge", { name: i.name })}><img src={api.thumbUrl(i.rel, 240)} alt={i.name} loading="lazy" /></button>
            <input class="ck" type="checkbox" aria-label={t("media.pick.one", { name: i.name })} checked={selected.has(i.rel)} onchange={(e) => pick([i.rel], e.currentTarget.checked)} />
            <figcaption class="ell" title={i.rel}>{i.name}</figcaption>
          </figure>
        {/each}
      </div>{/if}
      {#each rest as i (i.rel)}
        <div class="row" class:sel={selected.has(i.rel)} data-media={i.rel} data-kind={i.kind}>
          <input type="checkbox" aria-label={t("media.pick.one", { name: i.name })} checked={selected.has(i.rel)} onchange={(e) => pick([i.rel], e.currentTarget.checked)} />
          {#if i.kind === "audio"}<button class="icon" title={t("media.audio.listen")} aria-label={t("media.audio.listen")} onclick={() => playOrPause(i.rel, () => player.play(i.rel, null, i.name))}><PlayIcon on={playingFile(i.rel)} /></button>
          {:else}<span class="k small faint">{i.kind}</span>{/if}
          <span class="nm ell" title={i.rel}>{i.name}</span>
          <span class="faint small nw">{sizeOf(i.size)} · {ago(i.mtime)}</span>
        </div>
      {/each}
    </section>
  {/each}
  {#if groups.length > upTo}<button class="link small" onclick={() => (upTo += STEP)}>{t("media.audio.loadMore", { n: Math.min(STEP, groups.length - upTo) })}</button>{/if}
{/if}

{#if big}
  <div class="big" role="presentation" onclick={() => (big = null)} onkeydown={(e) => e.key === "Escape" && (big = null)}>
    <img src={api.mediaUrl(big.rel)} alt={big.name} data-media={big.rel} data-kind="image" />
    <p class="small">{big.rel}</p>
  </div>
{/if}

<style>
  .kinds { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 12px; }
  .chip { border: 1px solid var(--line); background: var(--panel); color: var(--muted); border-radius: 999px; padding: 3px 11px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .chip.on { border-color: var(--accent); color: var(--ink); background: var(--accent-bg); }
  .lede { margin: 8px 0 14px; }
  .grp { border-top: 1px solid var(--line); padding: 10px 0; }
  .gh { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; min-width: 0; }
  .grow { flex: 1; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .tiles { display: flex; flex-wrap: wrap; gap: 8px; align-items: flex-start; margin-bottom: 4px; }
  .tile { position: relative; margin: 0; width: 120px; }
  .tile .img { display: block; width: 120px; height: 120px; padding: 0; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; background: var(--panel2); cursor: zoom-in; }
  .tile.sel .img { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent); }
  .tile img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .tile .ck { position: absolute; top: 6px; left: 6px; width: 16px; height: 16px; opacity: 0; transition: opacity .15s; }
  .tile:hover .ck, .tile.sel .ck, .ck:focus-visible { opacity: 1; }
  .tile figcaption { font-size: 11px; color: var(--muted); margin-top: 3px; }
  .row { display: flex; align-items: center; gap: 10px; padding: 3px 4px; border-radius: 6px; font-size: 13px; }
  .row:hover, .row.sel { background: var(--accent-bg); }
  .nm { flex: 1; }
  .nw { white-space: nowrap; }
  .k { width: 34px; text-align: center; }
  .icon { border: 0; background: none; color: var(--ink); cursor: pointer; padding: 2px; display: grid; place-items: center; }
  .big { position: fixed; inset: 0; z-index: 3000; background: color-mix(in srgb, #000 82%, transparent); display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; cursor: zoom-out; padding: 30px; }
  .big img { max-width: 92vw; max-height: 84vh; object-fit: contain; border-radius: 6px; }
  .big p { color: #ddd; margin: 0; }
</style>

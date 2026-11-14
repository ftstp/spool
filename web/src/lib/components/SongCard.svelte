<script lang="ts">
  import type { SongItem } from "../api";
  import { openInLive } from "../api";
  import { ago, bpm, hue } from "../format";
  import Stage from "./Stage.svelte";
  import PlayBtn from "./PlayBtn.svelte";
  import { t } from "../i18n/index.svelte";
  let { song }: { song: SongItem } = $props();
  const meta = $derived([song.artist, bpm(song.tempo), song.key].filter(Boolean).join(" · "));
</script>

<div class="rcard" style="--h:{hue(song.title ?? '')}">
  <div class="art">{(song.title || "?").slice(0, 1).toUpperCase()}{#if song.temoin}<span class="pb"><PlayBtn sid={song.id} title={song.title ?? ""} size={30} /></span>{/if}</div>
  <div class="body">
    <a class="t" href="#/song/{song.id}" title={song.title}>{song.title}</a>
    <div class="m">{meta || " "}</div>
    <div class="f"><Stage stage={song.stage} /><span class="faint">{ago(song.mtime)}</span></div>
  </div>
  <button class="live" title={t("song.card.openInLive")} onclick={() => openInLive(song.rel)}>
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" /></svg>
  </button>
</div>

<style>
  .rcard { position: relative; display: flex; gap: 12px; align-items: center; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); transition: border-color .15s; min-width: 0; }
  .rcard:hover { border-color: var(--faint); }
  .art { width: 52px; height: 52px; flex: none; border-radius: 9px; display: grid; place-items: center; font-size: 20px; font-weight: 600; color: #fff;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .body { min-width: 0; flex: 1; }
  .art { position: relative; }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; z-index: 1; opacity: 0; transition: opacity .15s; }
  .rcard:hover .pb, .pb:focus-within { opacity: 1; }
  .t::after { content: ""; position: absolute; inset: 0; border-radius: inherit; }
  .t { display: block; color: inherit; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .f { display: flex; gap: 8px; align-items: center; font-size: 11.5px; margin-top: 3px; }
  .live { z-index: 1; position: absolute; top: 8px; right: 8px; width: 28px; height: 28px; border-radius: 8px; border: 1px solid var(--line); background: var(--panel); color: var(--muted); cursor: pointer; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .rcard:hover .live, .live:focus-visible { opacity: 1; }
  .live:hover { color: var(--accent); border-color: var(--accent); }
</style>

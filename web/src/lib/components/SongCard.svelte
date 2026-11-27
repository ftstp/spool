<script lang="ts">
  import { songDrag } from "../releaseSuggest.svelte";
  import { dragSource } from "../pdrag.svelte";
  import type { SongItem } from "../api";
  import LiveButton from "./LiveButton.svelte";
  import { ago, bpm, hue } from "../format";
  import Stage from "./Stage.svelte";
  import PlayBtn from "./PlayBtn.svelte";
  import { t } from "../i18n/index.svelte";
  let { song, queue }: { song: SongItem; queue?: import("../player.svelte").SongQueue } = $props();
  const meta = $derived([song.artist, bpm(song.tempo), song.key].filter(Boolean).join(" · "));
</script>

<div class="rcard" style="--h:{hue(song.title ?? '')}">
  <div class="art">{(song.title || "?").slice(0, 1).toUpperCase()}{#if song.temoin || song.rel}<span class="pb"><PlayBtn sid={song.id} title={song.title ?? ""} size={30} {queue} project={song.temoin ? "" : song.rel} /></span>{/if}</div>
  <div class="body">
    <a class="t" href="#/song/{song.id}" use:dragSource={songDrag(song.id, song.title)} title={song.title}>{song.title}</a>
    <div class="m">{meta || " "}</div>
    <div class="f"><Stage stage={song.stage} /><span class="faint">{ago(song.mtime)}</span></div>
  </div>
  <span class="live"><LiveButton rel={song.rel} /></span>
</div>

<style>
  .rcard { position: relative; display: flex; gap: 12px; align-items: center; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); transition: border-color .15s; min-width: 0; }
  .rcard:hover { border-color: var(--faint); }
  .art { width: 52px; height: 52px; flex: none; border-radius: 9px; display: grid; place-items: center; font-size: 20px; font-weight: 600; color: #fff;
    background: linear-gradient(135deg, hsl(var(--h) 30% 50%), hsl(calc(var(--h) + 40) 26% 34%)); }
  .body { min-width: 0; flex: 1; }
  .art { position: relative; }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; z-index: 1; opacity: 0; transition: opacity .15s; }
  .rcard:hover .pb, .pb:focus-within, .pb:has(:global(.play.on)) { opacity: 1; }
  .t::after { content: ""; position: absolute; inset: 0; border-radius: inherit; }
  .t { display: block; color: inherit; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .f { display: flex; gap: 8px; align-items: center; font-size: 11.5px; margin-top: 3px; }
  .live { z-index: 1; position: absolute; top: 8px; right: 8px; opacity: 0; transition: opacity .15s; }
  .rcard:hover .live, .live:focus-within { opacity: 1; }
</style>

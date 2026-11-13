<script lang="ts">
  // Every song as a square card with its picture.
  import type { SongItem } from "../api";
  import { openInLive } from "../api";
  import { ago, bpm } from "../format";
  import Art from "../components/Art.svelte";
  import Stage from "../components/Stage.svelte";
  import PlayBtn from "../components/PlayBtn.svelte";
  import { t } from "../i18n/index.svelte";
  let { list }: { list: SongItem[] } = $props();
  const meta = (s: SongItem) => [s.artist, bpm(s.tempo), s.key && (s.key_src === "estimé" ? "~" + s.key : s.key)].filter(Boolean).join(" · ");
</script>

<div class="pane">
  {#if !list.length}<p class="empty muted">{t("songs.empty")}</p>{/if}
  <div class="grid">
    {#each list as s (s.id)}
      <a class="card" href="#/song/{s.id}">
        <div class="pic"><Art title={s.title} cover={s.cover} size="100%" radius={10} />
          {#if s.temoin}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={44} /></span>{/if}
          <button class="live" title={t("songs.grid.openTitle")} aria-label={t("songs.grid.open")} onclick={(e) => { e.preventDefault(); openInLive(s.rel); }}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" /></svg>
          </button>
        </div>
        <div class="t" title={s.title}>{s.title}</div>
        <div class="m">{meta(s) || " "}</div>
        <div class="f"><Stage stage={s.stage} /><span class="faint">{ago(s.mtime)}</span></div>
      </a>
    {/each}
  </div>
</div>

<style>
  .pane { overflow: auto; flex: 1; min-height: 0; padding: 18px 22px 40px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 18px 16px; }
  .card { color: inherit; min-width: 0; border-radius: 12px; padding: 8px; margin: -8px; transition: background-color .15s; }
  .card:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .pic { position: relative; margin-bottom: 8px; }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .f { display: flex; justify-content: space-between; gap: 8px; font-size: 11.5px; margin-top: 2px; }
  .live { position: absolute; top: 8px; right: 8px; width: 28px; height: 28px; border-radius: 8px; border: 0; background: rgba(0, 0, 0, .55); color: #fff; cursor: pointer; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .card:hover .live, .live:focus-visible, .card:hover .pb, .pb:focus-within { opacity: 1; }
  .pb { position: absolute; left: 10px; bottom: 10px; opacity: 0; transition: opacity .15s; filter: drop-shadow(0 4px 10px rgba(0, 0, 0, .4)); }
  .empty { padding: 40px; text-align: center; }
</style>

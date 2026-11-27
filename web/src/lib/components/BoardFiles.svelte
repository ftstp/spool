<script lang="ts" module>
  /** What a board file is, from its name. */
  export function fileKind(path: string): "audio" | "midi" | "image" | "other" {
    const p = path.toLowerCase();
    if (/\.(wav|mp3|aiff?|flac|m4a|ogg)$/.test(p)) return "audio";
    if (/\.midi?$/.test(p)) return "midi";
    if (/\.(png|jpe?g|jfif|webp|avif|gif|bmp|svg)$/.test(p)) return "image";
    return "other";
  }
</script>

<script lang="ts">
  // The sounds and MIDI files of a mood board (a song's or a release's), as rows: listen,
  // drag into Live (always a copy), take off the board (the file itself is never touched).
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { player } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { t } from "../i18n/index.svelte";

  let { files, remove }: { files: { path: string; name?: string }[]; remove?: (path: string) => void } = $props();
  const leaf = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
  const abs = (p: string) => (/^[A-Za-z]:|^\//.test(p) ? Promise.resolve(p) : api.absPath(p));
  function listen(path: string) {
    const k = fileKind(path);
    if (k === "midi") midi.play(path);
    else if (player.is(path)) player.toggle();
    else player.play(path, null, leaf(path));
  }
  const on = (path: string) => (fileKind(path) === "midi" ? midi.is(path) : player.is(path) && player.playing);
</script>

{#if files.length}
  <ul class="bf">
    {#each files as f (f.path)}
      {@const k = fileKind(f.path)}
      <li draggable="true" title={t("song.board.dragTip")} data-media={f.path} data-kind={k}
        ondragstart={(e) => { e.preventDefault(); abs(f.path).then((p) => api.dragOut(p)).catch((x) => app.say(String(x), { error: true })); }}>
        {#if k === "audio" || k === "midi"}
          <button class="pl" class:on={on(f.path)} aria-label={t("song.board.listen")} title={t("song.board.listen")} onclick={() => listen(f.path)}>
            {#if on(f.path)}<svg viewBox="0 0 24 24" width="11" height="11" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5" /></svg>
            {:else}<svg viewBox="0 0 24 24" width="11" height="11" fill="currentColor"><path d="M6.6 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 6.6 5.5z" /></svg>{/if}
          </button>
        {:else if k === "image"}
          <img src={api.thumbUrl(f.path, 64)} alt="" />
        {:else}<span class="pl"></span>{/if}
        <span class="nm" title={f.path}>{f.name ?? leaf(f.path)}</span>
        <span class="tag small">{t(`song.board.kind.${k}`)}</span>
        {#if remove}<button class="x" title={t("song.board.unlinkTip")} aria-label={t("song.board.unlinkTip")} onclick={() => remove(f.path)}>×</button>{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .bf { list-style: none; margin: 6px 0 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li { display: grid; grid-template-columns: 24px minmax(0, 1fr) auto 20px; gap: 8px; align-items: center; padding: 3px 6px; border-radius: 7px; cursor: pointer; font-size: 12.5px; }
  li:hover { background: var(--panel2); }
  .pl { width: 24px; height: 24px; border-radius: 50%; border: 0; background: none; color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .pl:hover { color: var(--ink); background: var(--panel); }
  .pl.on { background: var(--accent); color: var(--on-accent); }
  img { width: 24px; height: 24px; object-fit: cover; border-radius: 4px; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { border: 1px solid var(--line); border-radius: 6px; padding: 0 6px; color: var(--muted); }
  .x { border: 0; background: none; color: var(--faint); cursor: pointer; font-size: 15px; opacity: 0; }
  li:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--ink); }
</style>

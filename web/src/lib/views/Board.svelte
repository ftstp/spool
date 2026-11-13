<script lang="ts">
  // One column per stage: a card dropped on another column takes that stage (Ctrl+Z undoes).
  import type { SongItem } from "../api";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { ago, bpm, stageLabel, STAGES } from "../format";
  import Art from "../components/Art.svelte";
  import Stage from "../components/Stage.svelte";
  import PlayBtn from "../components/PlayBtn.svelte";
  import { t } from "../i18n/index.svelte";
  let { list }: { list: SongItem[] } = $props();
  const COLS = ["none", ...STAGES];
  const MAX = 200;
  let dragged = $state<number | null>(null);
  let over = $state<string | null>(null);
  /** Moved at once on screen, saved behind; the reload after the save confirms it. */
  let moved = $state<Record<number, string>>({});
  $effect(() => {
    void app.version;
    moved = {};
  });
  const where = (s: SongItem) => moved[s.id] ?? (s.stage || "none");

  async function drop(k: string) {
    const id = dragged;
    dragged = null;
    over = null;
    const s = list.find((x) => x.id === id);
    if (!s || where(s) === k) return;
    moved = { ...moved, [s.id]: k };
    await app.act(api.songEdit(s.id, { stage: k === "none" ? null : k }));
    if (app.toast?.error) moved = {};
  }
  const meta = (s: SongItem) => [s.artist, bpm(s.tempo), s.key].filter(Boolean).join(" · ");
</script>

<div class="kanban">
  {#each COLS as k}
    {@const l = list.filter((s) => where(s) === k)}
    <section class="kcol" class:drop={over === k} aria-label={stageLabel(k === "none" ? null : k)}
      ondragover={(e) => { if (dragged != null) { e.preventDefault(); over = k; } }}
      ondragleave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) over = null; }}
      ondrop={(e) => { e.preventDefault(); drop(k); }}>
      <div class="khead"><Stage stage={k === "none" ? null : k} /><span class="faint">{l.length}</span></div>
      <div class="klist">
        {#each l.slice(0, MAX) as s (s.id)}
          <a class="kcard" href="#/song/{s.id}" draggable="true" class:dragging={dragged === s.id}
            ondragstart={(e) => { dragged = s.id; e.dataTransfer?.setData("text/plain", String(s.id)); if (e.dataTransfer) e.dataTransfer.effectAllowed = "move"; }}
            ondragend={() => { dragged = null; over = null; }}>
            <div class="a"><Art title={s.title} cover={s.cover} size={40} radius={7} />{#if s.temoin}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={26} /></span>{/if}</div>
            <div class="b">
              <div class="t" title={s.title}>{s.title}</div>
              <div class="m">{meta(s) || " "}</div>
              <div class="f"><span title={t("songs.checks")}>{s.check}/{s.checks}</span>
                {#if s.sugg}<span class="sg" title={t("songs.board.sugg")}>{t("songs.board.suggLabel", { stage: stageLabel(s.sugg) })}</span>{/if}
                <span class="faint">{ago(s.mtime)}</span></div>
            </div>
          </a>
        {/each}
        {#if l.length > MAX}<div class="small faint more">{t("songs.more", { n: l.length - MAX })}</div>{/if}
        {#if !l.length}<div class="small faint more">{t("songs.board.dropHere")}</div>{/if}
      </div>
    </section>
  {/each}
</div>

<style>
  .kanban { flex: 1; min-height: 0; display: grid; grid-auto-flow: column; grid-auto-columns: minmax(188px, 1fr); gap: 10px; padding: 14px 18px 18px; overflow-x: auto; }
  .kcol { display: flex; flex-direction: column; min-height: 0; border-radius: var(--radius); background: var(--panel2); border: 1px solid var(--line); transition: border-color .15s, background-color .15s; }
  .kcol.drop { border-color: var(--accent); background: var(--accent-bg); }
  .khead { display: flex; justify-content: space-between; align-items: center; padding: 10px 12px 6px; font-weight: 600; }
  .klist { flex: 1; overflow-y: auto; padding: 4px 8px 10px; display: flex; flex-direction: column; gap: 6px; }
  .kcard { display: flex; gap: 10px; align-items: center; padding: 8px; border-radius: 9px; background: var(--panel); border: 1px solid var(--line); color: inherit; cursor: grab; transition: border-color .15s, opacity .15s; }
  .kcard:hover { border-color: var(--faint); }
  .kcard.dragging { opacity: .4; }
  .b { min-width: 0; flex: 1; }
  .a { position: relative; }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .kcard:hover .pb, .pb:focus-within { opacity: 1; }
  .t { font-weight: 600; font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 11.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .f { display: flex; gap: 8px; font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden; }
  .f .faint { margin-left: auto; }
  .sg { color: var(--accent); }
  .more { padding: 6px; }
</style>

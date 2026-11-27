<script lang="ts">
  // The top of a song's sheet, kept in sight while it scrolls: plays the reference render (its
  // waveform, with the comments marked on it) or the project, from where the other one was.
  // The project's playback is the arrangement's (see sheetTransport); without an arrangement,
  // it plays the render alone.
  import { mediaUrl } from "../api";
  import { player, mmss, infoFor, loudness } from "../player.svelte";
  import type { AudioInfo } from "../api";
  import { sheetTransport } from "../sheetTransport.svelte";
  import Wave from "./Wave.svelte";
  import WaveNotes from "./WaveNotes.svelte";
  import type { Comment } from "../api/bindings/Comment";
  import { commentsOf, addComment, editComment, removeComment } from "../comments.svelte";
  import PlayIcon from "./PlayIcon.svelte";
  import { t } from "../i18n/index.svelte";

  let {
    sid,
    title,
    temoin,
    chosenBy,
    openRenders,
  }: { sid: number; title: string; temoin: string | null; chosenBy: string; openRenders: () => void } = $props();
  const comments = $derived(commentsOf(sid));

  const tr = $derived(sheetTransport.by[sid] ?? null);
  const src = $derived(tr ? tr.src : "temoin");
  const name = $derived(temoin ? (temoin.replace(/\\/g, "/").split("/").pop() ?? temoin) : "");
  const mine = $derived(!!temoin && player.is(temoin));
  const playing = $derived(tr ? tr.sounding : mine && player.playing);

  let info = $state<AudioInfo | null>(null);
  $effect(() => {
    const want = temoin;
    info = null;
    if (want) infoFor(want).then((i) => temoin === want && (info = i));
  });

  function play() {
    if (tr) return tr.playPause();
    if (!temoin) return;
    if (mine) player.toggle();
    else player.play(temoin, sid, title);
  }
  // back to the first bar: the project's start, or the render's
  function toStart() {
    if (tr) return tr.toStart();
    if (mine) player.seek(0);
  }
  function seekWave(f: number) {
    if (!temoin) return;
    if (tr && tr.src !== "temoin") tr.setSrc("temoin");
    if (mine) player.seek(f);
    else player.play(temoin, sid, title, f * (info?.duration ?? 0));
  }
  function seekBar(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    tr?.seek(Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)));
  }
  // ---- the notes pinned on what plays: the render's on its waveform, the project's on its bar
  const same = (a: string, b: string | null | undefined) => !!b && a.replace(/\\/g, "/").toLowerCase() === b.replace(/\\/g, "/").toLowerCase();
  const noteSrc = $derived(src === "project" && tr ? tr.rel : temoin);
  const notesHere = $derived(noteSrc ? comments.filter((c) => same(c.rel, noteSrc)) : []);
  const nPinned = $derived(comments.filter((c) => !!c.rel).length);
  const noteLen = $derived(src === "project" && tr ? tr.secs : (info?.duration ?? 0));
  const noteNow = $derived(src === "project" && tr ? (tr.at !== null ? tr.at * tr.secs : null) : mine ? player.t : null);
  const notePlaying = $derived(src === "project" && tr ? tr.sounding : mine && player.playing);
  const addNote = (at: number, text: string) => (noteSrc ? addComment(sid, { rel: noteSrc, at, text }) : undefined);
  const editNote = (c: Comment) => editComment(sid, c);
  const removeNote = (c: Comment) => removeComment(sid, c);
  async function playNoteAt(at: number) {
    if (src === "project" && tr) {
      if (tr.secs > 0) tr.seek(Math.min(1, at / tr.secs));
      if (!tr.sounding) tr.playPause();
    } else if (temoin && info?.duration) seekWave(Math.min(1, at / info.duration));
    else if (temoin) player.play(temoin, sid, title, at);
  }

  const time = $derived.by(() => {
    if (src === "project" && tr) return `${mmss((tr.at ?? 0) * tr.secs)} / ${mmss(tr.secs)}`;
    if (mine) return `${mmss(player.t)} / ${mmss(player.len)}`;
    return info?.duration ? mmss(info.duration) : "";
  });
</script>

{#if temoin || tr}
  <div class="tp">
    <button class="pp" onclick={play} disabled={!tr && !mediaUrl(temoin ?? "")}
      aria-label={playing ? t("song.player.pause") : t("song.player.play")} title={playing ? t("song.player.pause") : t("song.session.playFrom", { src: src === "temoin" ? t("song.session.playRender") : t("song.session.playMidi") })}>
      {#if tr?.loading}…{:else}<PlayIcon on={playing} size={14} />{/if}
    </button>
    <button class="top" onclick={toStart} aria-label={t("song.session.toStart")} title={t("song.session.toStart")}>
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M3 2.5v11" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /><path d="M13 3v10L5.5 8z" fill="currentColor" /></svg>
    </button>
    {#if tr && temoin}
      <div class="srcs" role="radiogroup" aria-label={t("song.session.srcLabel")}>
        <button role="radio" aria-checked={src === "temoin"} class:on={src === "temoin"} title={t("song.session.playRenderTip")} onclick={() => tr.setSrc("temoin")}>{t("song.session.playRender")}</button>
        <button role="radio" aria-checked={src === "project"} class:on={src === "project"} title={t("song.session.playMidiTip")} onclick={() => tr.setSrc("project")}>{t("song.session.playMidi")}</button>
      </div>
    {/if}
    <div class="mid">
      <WaveNotes notes={notesHere} len={noteLen} now={noteNow} playing={notePlaying} add={addNote} edit={editNote} remove={removeNote} playAt={playNoteAt}>
        {#if src === "temoin" && temoin}
          <Wave peaks={info?.peaks ?? null} height={40} progress={mine && player.len ? player.t / player.len : 0} seek={seekWave} />
        {:else if tr}
          <div class="prog" role="presentation" onclick={seekBar}><i style:width="{(tr.at ?? 0) * 100}%"></i></div>
        {/if}
      </WaveNotes>
    </div>
    <span class="time small">{time}</span>
    <div class="sub small faint">
        {#if temoin}<span class="nm" title={[temoin, loudness(info)].filter(Boolean).join("\n")}>{name}</span><span class="by">{chosenBy}</span>
        {:else}<span>{t("song.temoin.none")}</span>{/if}
        {#if nPinned}<span>{t("song.sections.comments", { n: nPinned })}</span>{:else}<span class="hint">{t("song.wnotes.howTo")}</span>{/if}
        <button class="link" onclick={openRenders}>{t("song.sections.rendersLink")}</button>
      </div>
  </div>
{/if}

<style>
  /* one row: the button, the source, the waveform and the time on one line, centred on the
     waveform; what the render is, under it */
  .tp { display: grid; grid-template-columns: auto auto auto minmax(0, 1fr) auto; grid-template-areas: "pp top srcs mid time" ". . . sub sub"; align-items: center; column-gap: 12px; row-gap: 2px; padding: 6px 12px 5px; border: 1px solid var(--line); border-radius: 12px; background: var(--panel); }
  .tp > .pp { grid-area: pp; }
  .tp > .top { grid-area: top; }
  .tp > .srcs { grid-area: srcs; }
  .tp > .mid { grid-area: mid; }
  .tp > .time { grid-area: time; }
  .tp > .sub { grid-area: sub; }
  .pp { width: 32px; height: 32px; flex: none; border-radius: 50%; border: 0; background: var(--accent); color: var(--on-accent); cursor: pointer; display: grid; place-items: center; transition: transform .12s; }
  .pp:hover:not(:disabled) { transform: scale(1.06); }
  .pp:active:not(:disabled) { transform: scale(0.96); }
  .pp:disabled { background: var(--line); color: var(--faint); cursor: default; }
  .top { width: 26px; height: 26px; margin-left: -6px; border: 0; border-radius: 50%; background: none; color: var(--muted); cursor: pointer; display: grid; place-items: center; }
  .top:hover { background: var(--panel2); color: var(--ink); }
  .srcs { display: inline-flex; flex: none; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .srcs button { border: 0; background: none; color: var(--muted); padding: 4px 10px; cursor: pointer; font: inherit; font-size: 12px; }
  .srcs button + button { border-left: 1px solid var(--line); }
  .srcs button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .mid { flex: 1; min-width: 0; }
  .prog { height: 40px; border-radius: 6px; background: var(--panel2); cursor: pointer; position: relative; overflow: hidden; }
  .prog i { position: absolute; inset: 0 auto 0 0; background: color-mix(in srgb, var(--accent) 35%, transparent); border-right: 2px solid var(--rec); }
  .sub { display: flex; align-items: baseline; gap: 10px; min-width: 0; white-space: nowrap; }
  .sub > * { flex: none; }
  .sub .hint { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .sub .nm { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--muted); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; font: inherit; }
  .link:hover { text-decoration: underline; }
  .time { flex: none; color: var(--muted); font-variant-numeric: tabular-nums; }
  @container (max-width: 560px) { .srcs button { padding: 4px 7px; } .time, .sub .by { display: none; } }
  @media (prefers-reduced-motion: reduce) { .pp { transition: none; } }
</style>

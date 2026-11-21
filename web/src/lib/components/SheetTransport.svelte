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
  import PlayIcon from "./PlayIcon.svelte";
  import { t } from "../i18n/index.svelte";

  let {
    sid,
    title,
    temoin,
    chosenBy,
    marks = [],
    openRenders,
  }: { sid: number; title: string; temoin: string | null; chosenBy: string; marks?: number[]; openRenders: () => void } = $props();

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
    {#if tr && temoin}
      <div class="srcs" role="radiogroup" aria-label={t("song.session.srcLabel")}>
        <button role="radio" aria-checked={src === "temoin"} class:on={src === "temoin"} title={t("song.session.playRenderTip")} onclick={() => tr.setSrc("temoin")}>{t("song.session.playRender")}</button>
        <button role="radio" aria-checked={src === "project"} class:on={src === "project"} title={t("song.session.playMidiTip")} onclick={() => tr.setSrc("project")}>{t("song.session.playMidi")}</button>
      </div>
    {/if}
    <div class="mid">
      {#if src === "temoin" && temoin}
        <Wave peaks={info?.peaks ?? null} height={40} marks={info?.duration ? marks.map((m) => m / info!.duration) : []}
          progress={mine && player.len ? player.t / player.len : 0} seek={seekWave} />
      {:else if tr}
        <div class="prog" role="presentation" title={t("song.session.rulerTip")} onclick={seekBar}><i style:width="{(tr.at ?? 0) * 100}%"></i></div>
      {/if}
    </div>
    <span class="time small">{time}</span>
    <div class="sub small faint">
        {#if temoin}<span class="nm" title={[temoin, loudness(info)].filter(Boolean).join("\n")}>{name}</span><span class="by">{chosenBy}</span>
        {:else}<span>{t("song.temoin.none")}</span>{/if}
        {#if marks.length}<span>{t("song.sections.comments", { n: marks.length })}</span>{/if}
        <button class="link" onclick={openRenders}>{t("song.sections.rendersLink")}</button>
      </div>
  </div>
{/if}

<style>
  /* one row: the button, the source, the waveform and the time on one line, centred on the
     waveform; what the render is, under it */
  .tp { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; grid-template-areas: "pp srcs mid time" ". . sub sub"; align-items: center; column-gap: 12px; row-gap: 2px; padding: 6px 12px 5px; border: 1px solid var(--line); border-radius: 12px; background: var(--panel); }
  .tp > .pp { grid-area: pp; }
  .tp > .srcs { grid-area: srcs; }
  .tp > .mid { grid-area: mid; }
  .tp > .time { grid-area: time; }
  .tp > .sub { grid-area: sub; }
  .pp { width: 32px; height: 32px; flex: none; border-radius: 50%; border: 0; background: var(--accent); color: var(--on-accent); cursor: pointer; display: grid; place-items: center; transition: transform .12s; }
  .pp:hover:not(:disabled) { transform: scale(1.06); }
  .pp:active:not(:disabled) { transform: scale(0.96); }
  .pp:disabled { background: var(--line); color: var(--faint); cursor: default; }
  .srcs { display: inline-flex; flex: none; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .srcs button { border: 0; background: none; color: var(--muted); padding: 4px 10px; cursor: pointer; font: inherit; font-size: 12px; }
  .srcs button + button { border-left: 1px solid var(--line); }
  .srcs button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  .mid { flex: 1; min-width: 0; }
  .prog { height: 40px; border-radius: 6px; background: var(--panel2); cursor: pointer; position: relative; overflow: hidden; }
  .prog i { position: absolute; inset: 0 auto 0 0; background: color-mix(in srgb, var(--accent) 35%, transparent); border-right: 2px solid var(--rec); }
  .sub { display: flex; align-items: baseline; gap: 10px; min-width: 0; white-space: nowrap; }
  .sub > * { flex: none; }
  .sub .nm { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--muted); }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; font: inherit; }
  .link:hover { text-decoration: underline; }
  .time { flex: none; color: var(--muted); font-variant-numeric: tabular-nums; }
  @container (max-width: 560px) { .srcs button { padding: 4px 7px; } .time, .sub .by { display: none; } }
  @media (prefers-reduced-motion: reduce) { .pp { transition: none; } }
</style>

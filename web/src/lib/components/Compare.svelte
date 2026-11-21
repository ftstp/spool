<script lang="ts">
  // Compare a song's reference render (A) with another of its versions or with a released
  // song (B): switch between them at the same moment, at the same loudness, and see where the
  // streaming services would put it.
  import * as api from "../api";
  import type { AudioInfo } from "../api";
  import { app } from "../state.svelte";
  import { player, infoFor } from "../player.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";
  import { ago } from "../format";

  type Opt = { rel: string; label: string; ref?: boolean };
  let { sid, title, a, versions }: { sid: number; title: string; a: string; versions: { rel: string; mtime: number }[] } = $props();

  let refs = $state<string[]>([]);
  $effect(() => {
    api.refTracks().then((r) => (refs = r)).catch(() => {});
  });
  const leaf = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
  const opts = $derived<Opt[]>([
    ...versions.filter((v) => v.rel !== a).map((v) => ({ rel: v.rel, label: `${leaf(v.rel)} · ${ago(v.mtime)}` })),
    ...refs.map((r) => ({ rel: r, label: leaf(r), ref: true })),
  ]);
  let b = $state("");
  // the other side chosen by default: the previous version, when there is one
  $effect(() => {
    if (!b || !opts.some((o) => o.rel === b)) b = opts[0]?.rel ?? "";
  });
  let match = $state(true);
  let ia = $state<AudioInfo | null>(null);
  let ib = $state<AudioInfo | null>(null);
  $effect(() => {
    const x = a;
    infoFor(x).then((i) => x === a && (ia = i));
  });
  $effect(() => {
    const x = b;
    ib = null;
    if (x) infoFor(x).then((i) => x === b && (ib = i));
  });
  const side = $derived(player.is(a) ? "a" : b && player.is(b) ? "b" : null);
  // the louder one lowered to the other's loudness
  function gain(which: "a" | "b") {
    if (!match || ia?.lufs == null || ib?.lufs == null) return 1;
    const [mine, other] = which === "a" ? [ia.lufs, ib.lufs] : [ib.lufs, ia.lufs];
    return mine > other ? 10 ** ((other - mine) / 20) : 1;
  }
  function go(which: "a" | "b") {
    const to = which === "a" ? a : b;
    if (!to) return;
    const at = side ? player.t : 0;
    const was = side ? player.playing : true;
    player.play(to, sid, title, at);
    player.setVolume(gain(which));
    if (!was) player.toggle();
  }
  $effect(() => {
    void match;
    if (side) player.setVolume(gain(side));
  });
  async function addRef() {
    try {
      const p = await api.pickFile(t("song.compare.pickRef"), ["wav", "mp3", "flac", "aif", "aiff", "m4a", "ogg"]);
      if (!p) return;
      refs = await api.refTrack(p, true);
      b = refs.find((r) => leaf(r) === leaf(p)) ?? b;
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function dropRef() {
    try {
      refs = await api.refTrack(b, false);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const isRef = $derived(refs.includes(b));

  // where the services put it: each turns a louder song down to its level
  const TARGETS: [string, number][] = [["Spotify, YouTube, Tidal, Amazon", -14], ["Deezer", -15], ["Apple Music", -16]];
  // `|| 0`: a peak of -0.02 dB rounds to -0, shown "-0"
  const f1 = (x: number) => fmtNum(Math.round(x * 10) / 10 || 0);
  const row = (i: AudioInfo | null) =>
    i ? [i.lufs != null ? `${f1(i.lufs)} LUFS` : "", t("song.compare.peak", { v: f1(i.peak_db) }), i.lra != null ? `LRA ${f1(i.lra)} LU` : ""].filter(Boolean).join(" · ") : "…";
</script>

<div class="cmp">
  <div class="top">
    <span class="small muted">{t("song.compare.title")}</span>
    <div class="ab" role="group" aria-label={t("song.compare.title")}>
      <button class:on={side === "a"} onclick={() => go("a")} title={t("song.compare.aTip")}>A</button>
      <button class:on={side === "b"} disabled={!b} onclick={() => go("b")} title={t("song.compare.bTip")}>B</button>
    </div>
    <select bind:value={b} aria-label={t("song.compare.bLabel")}>
      {#if !opts.length}<option value="">{t("song.compare.nothing")}</option>{/if}
      {#if opts.some((o) => !o.ref)}<optgroup label={t("song.compare.versions")}>{#each opts.filter((o) => !o.ref) as o (o.rel)}<option value={o.rel}>{o.label}</option>{/each}</optgroup>{/if}
      {#if refs.length}<optgroup label={t("song.compare.refs")}>{#each opts.filter((o) => o.ref) as o (o.rel)}<option value={o.rel}>{o.label}</option>{/each}</optgroup>{/if}
    </select>
    <button class="link small" onclick={addRef}>{t("song.compare.addRef")}</button>
    {#if isRef}<button class="link small" onclick={dropRef}>{t("song.compare.dropRef")}</button>{/if}
    <label class="small match" title={t("song.compare.matchTip")}><input type="checkbox" bind:checked={match} /> {t("song.compare.match")}</label>
  </div>
  <div class="ld small">
    <span class="k">A</span><span>{row(ia)}</span>
    {#if b}<span class="k">B</span><span>{row(ib)}</span>{/if}
    {#if match && ia?.lufs != null && ib?.lufs != null && Math.abs(ia.lufs - ib.lufs) >= 0.5}
      <span></span><span class="faint">{t("song.compare.lowered", { which: ia.lufs > ib.lufs ? "A" : "B", db: f1(Math.abs(ia.lufs - ib.lufs)) })}</span>
    {/if}
  </div>
  {#if ia?.lufs != null}
    <ul class="svc small">
      {#each TARGETS as [who, lv] (who)}
        <li><b>{who}</b> ({lv} LUFS){t("song.compare.colon")}{ia.lufs > lv + 0.4 ? t("song.compare.down", { db: f1(ia.lufs - lv) }) : ia.lufs < lv - 0.4 ? t("song.compare.quiet", { db: f1(lv - ia.lufs) }) : t("song.compare.asIs")}</li>
      {/each}
      {#if ia.peak_db > -1}<li class="warn">{t("song.compare.peakWarn", { v: f1(ia.peak_db) })}</li>{/if}
    </ul>
  {/if}
</div>

<style>
  .cmp { margin-top: 10px; padding: 10px 12px; border-radius: 10px; border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .top { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .ab { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .ab button { border: 0; background: none; color: var(--muted); font-weight: 700; padding: 3px 12px; cursor: pointer; font: inherit; font-weight: 700; }
  .ab button + button { border-left: 1px solid var(--line); }
  .ab button.on { background: var(--accent); color: var(--on-accent); }
  .ab button:disabled { color: var(--faint); cursor: default; }
  select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 3px 6px; font: inherit; font-size: 12.5px; max-width: 260px; }
  .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; }
  .match { display: inline-flex; gap: 4px; align-items: center; color: var(--muted); margin-left: auto; }
  .ld { display: grid; grid-template-columns: 16px 1fr; gap: 2px 8px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .k { font-weight: 700; color: var(--ink); }
  .svc { margin: 0; padding-left: 16px; color: var(--muted); display: grid; gap: 1px; }
  .svc b { color: var(--ink); font-weight: 600; }
  .warn { color: var(--lost); }
</style>

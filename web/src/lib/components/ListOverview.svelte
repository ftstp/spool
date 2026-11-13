<script lang="ts">
  // Beside the song list when no song is open: what the songs shown (with their filters) are
  // made of. A bar or a row filters the list; a song opens beside it.
  import type { SongItem } from "../api";
  import { STAGES, ago, stageLabel } from "../format";
  import { f } from "../filters.svelte";
  import StackBar from "../charts/StackBar.svelte";
  import BarList from "../charts/BarList.svelte";
  import Art from "./Art.svelte";
  import PlayBtn from "./PlayBtn.svelte";
  import { t } from "../i18n/index.svelte";

  let { list }: { list: SongItem[] } = $props();
  const STAGE_C: Record<string, string> = {
    none: "var(--s-none)", draft: "var(--s-draft)", arrangement: "var(--s-arrangement)", mix: "var(--s-mix)", master: "var(--s-master)", done: "var(--s-done)", pause: "var(--s-pause)",
  };
  const SEV = [["ok", "var(--ok)"], ["fix", "var(--fix)"], ["lost", "var(--lost)"], ["bad", "var(--bad)"]];
  const nSongs = (n: number) => t("songs.nSongs", { n });

  const count = <T,>(xs: T[]) => {
    const m = new Map<T, number>();
    for (const x of xs) m.set(x, (m.get(x) ?? 0) + 1);
    return [...m].sort((a, b) => b[1] - a[1]);
  };
  const stages = $derived(["none", ...STAGES].map((k) => ({ key: k, label: stageLabel(k === "none" ? null : k), n: list.filter((s) => (s.stage || "none") === k).length, color: STAGE_C[k] })));
  const sev = $derived(SEV.map(([key, color], i) => ({ key, label: t(`songs.sev.${key}`), color, n: list.filter((s) => Math.min(3, s.sev) === i).length })));
  const styles = $derived(count(list.flatMap((s) => (s.style || "").toLowerCase().split(/\s*,\s*/).filter(Boolean))).slice(0, 6).map(([label, n]) => ({ label, n })));
  const keys = $derived(count(list.map((s) => s.key).filter((k): k is string => !!k)).slice(0, 6).map(([label, n]) => ({ label, n })));
  const tempos = $derived(list.map((s) => s.tempo).filter((t): t is number => !!t).sort((a, b) => a - b));
  const median = $derived(tempos.length ? Math.round(tempos[Math.floor(tempos.length / 2)]) : null);
  const audio = $derived(list.filter((s) => s.temoin).length);
  const recent = $derived([...list].sort((a, b) => b.mtime - a.mtime).slice(0, 6));
</script>

<div class="ov">
  <header>
    <h2>{t("songs.ov.title")}</h2>
    <p class="small muted">{t("songs.ov.intro", { n: list.length })}</p>
  </header>

  <section>
    <h3>{t("songs.ov.stages")}</h3>
    <StackBar parts={stages} legend height={10} unit={nSongs} pick={(k) => (f.stages = [k])} />
  </section>

  <section>
    <h3>{t("songs.ov.plugins")}</h3>
    <StackBar parts={sev} legend height={10} unit={nSongs} pick={(k) => (f.plugins = k)} />
  </section>

  <section class="nums">
    <div><b>{audio}</b><span class="small muted">{t("songs.ov.audio", { pct: list.length ? Math.round((100 * audio) / list.length) : 0 })}</span></div>
    <div><b>{median ?? "-"}</b><span class="small muted">{tempos.length ? t("songs.ov.medianRange", { min: Math.round(tempos[0]), max: Math.round(tempos.at(-1)!) }) : t("songs.ov.median")}</span></div>
  </section>

  <div class="two">
    {#if styles.length}<section><h3>{t("songs.ov.styles")}</h3><BarList rows={styles} unit={nSongs} pick={(l) => (f.style = l)} /></section>{/if}
    {#if keys.length}<section><h3>{t("songs.ov.keys")}</h3><BarList rows={keys} unit={nSongs} pick={(l) => (f.key = l)} /></section>{/if}
  </div>

  <section>
    <h3>{t("songs.ov.recent")}</h3>
    <div class="rec">
      {#each recent as s (s.id)}
        <a class="r" href="#/song/{s.id}">
          <span class="pic"><Art title={s.title} cover={s.cover} size={34} radius={6} />{#if s.temoin}<span class="pb"><PlayBtn sid={s.id} title={s.title ?? ""} size={22} /></span>{/if}</span>
          <span class="t">{s.title}</span><span class="small faint">{ago(s.mtime)}</span>
        </a>
      {/each}
    </div>
  </section>
</div>

<style>
  .ov { padding: 22px 26px 40px; display: flex; flex-direction: column; gap: 22px; max-width: 760px; }
  header h2 { margin: 0 0 4px; font-size: 18px; }
  header p { margin: 0; }
  h3 { font-size: 12px; font-weight: 600; color: var(--muted); margin: 0 0 8px; }
  .nums { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .nums div { display: flex; flex-direction: column; padding: 12px 14px; border-radius: 10px; background: var(--panel); border: 1px solid var(--line); }
  .nums b { font-family: var(--mono); font-size: 22px; font-weight: 600; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 22px; }
  .rec { display: flex; flex-direction: column; }
  .r { display: grid; grid-template-columns: 34px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 5px 6px; border-radius: 8px; color: var(--ink); }
  .r:hover { background: color-mix(in srgb, var(--line) 45%, transparent); }
  .pic { position: relative; }
  .pb { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0; transition: opacity .15s; }
  .r:hover .pb { opacity: 1; }
  .t { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

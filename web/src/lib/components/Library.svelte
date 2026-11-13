<script lang="ts">
  // What the projects folders hold besides the projects: the MIDI files (what kind, which
  // keys, the chord progressions that come back) and the media outside the songs (pictures,
  // videos, sounds, notes). Shown when the first analysis ends, and in the home statistics.
  import * as api from "../api";
  import type { MidiSummary, MediaFolder } from "../api";
  import type { MidiStats } from "../api/bindings/MidiStats";
  import { app } from "../state.svelte";
  import StackBar from "../charts/StackBar.svelte";
  import { t, fmtNum } from "../i18n/index.svelte";

  let { compact = false }: { compact?: boolean } = $props();
  let m = $state<MidiSummary | null>(null);
  let media = $state<MediaFolder[] | null>(null);
  $effect(() => {
    void app.version;
    api.midiSummary().then((x) => (m = x)).catch(() => (m = null));
    api.fsMedia().then((x) => (media = x)).catch(() => (media = null));
  });
  const KIND: Record<string, string> = {
    chords: "var(--s-mix)",
    melody: "var(--s-arrangement)",
    bass: "var(--s-master)",
    drums: "var(--s-draft)",
    multi: "var(--s-done)",
  };
  const fr = (n: number) => fmtNum(n);
  const pct = (a: number, total: number) => t("stats.pct", { n: total ? Math.round((a / total) * 100) : 0 });
  const topTempo = (st: MidiStats) => {
    const x = [...st.tempos].sort((a, b) => b.n - a.n)[0];
    return t("stats.lib.tempoRange", { a: x.label, b: +x.label + 9, pct: pct(x.n, st.tempos.reduce((a, c) => a + c.n, 0)) });
  };
  const MEDIA = ["image", "gif", "video", "audio", "text", "pdf"];
  const mediaCount = (k: string, n: number) => (MEDIA.includes(k) ? t(`stats.lib.mediaKind.${k}`, { n }) : `${n} ${k}`);
  const mediaCounts = $derived.by(() => {
    const c: Record<string, number> = {};
    for (const f of media ?? []) for (const [k, n] of Object.entries(f.counts)) c[k] = (c[k] ?? 0) + n;
    return Object.entries(c).sort((a, b) => b[1] - a[1]);
  });
</script>

<div class="lib" class:compact>
  <section class="card">
    <h3>{t("stats.lib.midi")} <span class="faint">{m ? fr(m.files) : "..."}</span></h3>
    {#if m && m.files}
      {#each [[t("stats.lib.mine"), m.mine], [t("stats.lib.library"), m.library]] as [title, s] (title)}
        {@const st = s as MidiStats}
        {#if st.files}
          <div class="src">
            {#if m.library.files}<h4>{title} <span class="faint">{fr(st.files)}</span></h4>{/if}
            <StackBar parts={st.kinds.map((k) => ({ key: k.label, label: KIND[k.label] ? t(`stats.lib.kind.${k.label}`) : k.label, n: k.n, color: KIND[k.label] ?? "var(--s-none)" }))} legend height={9} unit={(n) => t("stats.lib.files", { n })} />
            {#if st.keys.length}
              <p class="small muted">{t("stats.lib.minor", { pct: pct(st.minor, st.minor + st.major), keys: st.keys.slice(0, 5).map((k) => `${k.label} (${fr(k.n)})`).join(", ") })}</p>
            {/if}
            {#if st.tempos.length}<p class="small muted">{t("stats.lib.topTempo", { v: topTempo(st) })}</p>{/if}
            {#if st.progressions.length}
              <div class="progs small">
                <span class="muted">{t("stats.lib.progs")}</span>
                {#each st.progressions.slice(0, compact ? 3 : 5) as p (p.degrees)}
                  <div class="pg" title={t("stats.lib.example", { name: p.example })}><b>{p.degrees}</b> <span class="faint">{p.chords.join(" ")}</span>{#if p.n > 1}<span class="n">×{fr(p.n)}</span>{/if}</div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
      <p class="small faint">{t("stats.lib.matter")}</p>
    {:else if m && app.scan?.running && /fichiers MIDI|MIDI files/i.test(app.scan?.stage ?? "")}
      <p class="small faint">{t("stats.lib.reading")}</p>
    {:else if m}
      <p class="small faint">{t("stats.lib.noMidi")}</p>
    {/if}
  </section>
  <section class="card">
    <h3>{t("stats.lib.media")} <span class="faint">{media ? media.reduce((a, f) => a + f.n, 0) : "..."}</span></h3>
    {#if media && media.length}
      <p class="small">{mediaCounts.map(([k, n]) => mediaCount(k, n)).join(" · ")}</p>
      <p class="small faint">{t("stats.lib.inFolders", { n: media.length })}</p>
    {:else if media}
      <p class="small faint">{t("stats.lib.noMedia")}</p>
    {/if}
  </section>
</div>

<style>
  .lib { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(300px, 100%), 1fr)); gap: 12px; }
  h3 { margin: 0 0 8px; font-size: 13px; display: flex; justify-content: space-between; }
  h3 .faint { font-weight: 400; font-variant-numeric: tabular-nums; }
  p { margin: 8px 0 0; }
  .src + .src { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--line); }
  h4 { margin: 0 0 8px; font-size: 12px; font-weight: 600; display: flex; justify-content: space-between; }
  h4 .faint { font-weight: 400; font-variant-numeric: tabular-nums; }
  .progs { display: flex; flex-direction: column; gap: 3px; margin-top: 8px; }
  .pg { display: flex; gap: 8px; align-items: baseline; }
  .pg b { font-family: ui-monospace, monospace; font-size: 12px; }
  .n { margin-left: auto; color: var(--muted); font-variant-numeric: tabular-nums; }
</style>

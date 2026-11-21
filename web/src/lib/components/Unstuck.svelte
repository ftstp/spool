<script lang="ts">
  // "Unstick this song": material of the user's own that fits it (its key or the relative,
  // its tempo or half / double): the MIDI clips written in their other projects (progressions,
  // basslines, melodies, beats) and their sample loops. Each heard alone, or over the
  // reference render from its next bar at the song's tempo, then dragged into Live.
  import * as api from "../api";
  import type { MidiRow } from "../api/bindings/MidiRow";
  import type { SampleRow } from "../api/bindings/SampleRow";
  import { app } from "../state.svelte";
  import { player, layer, playingFile } from "../player.svelte";
  import { sectionOpen } from "../tabs.svelte";
  import FoldHead from "./FoldHead.svelte";
  import PlayIcon from "./PlayIcon.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { lockRate } from "../tempo";
  import { t } from "../i18n/index.svelte";
  import { sheetTransport } from "../sheetTransport.svelte";

  let { sid, title, songKey, bpm, temoin = null }: { sid: number; title: string; songKey: string | null; bpm: number; temoin?: string | null } = $props();
  type Tab = "chords" | "bass" | "melody" | "drums" | "samples";
  const TABS: Tab[] = ["chords", "bass", "melody", "drums", "samples"];
  // open or not as the user left the sheet's sections (the same for every song)
  const open = $derived(sectionOpen(sid, "unstuck"));
  let tab = $state<Tab>("chords");
  let clips = $state<MidiRow[]>([]);
  let samples = $state<SampleRow[]>([]);
  let loading = $state(false);
  $effect(() => {
    const [tb, k, b, on] = [tab, songKey, bpm, open];
    // the browser preview reads its own MIDI and samples (public/dev-midi.json, dev-samples.json)
    if (!on || !(api.inApp || import.meta.env.DEV) || (!k && !b)) return;
    loading = true;
    const done = () => (loading = false);
    if (tb === "samples") {
      api
        .samples({ q: "", family: null, cat: null, kind: "loop", key: k || null, bpm: b || null, used: null, offset: 0, limit: 10 })
        .then((p) => tb === tab && (samples = p.rows))
        .catch(() => (samples = []))
        .finally(done);
    } else {
      api
        .midiPage({ q: "", kind: tb, key: tb === "drums" ? null : k || null, bpm: b || null, source: "clips", offset: 0, limit: 30 })
        // a beat has no key: by tempo only. The song's own clips are not news to it
        .then((p) => tb === tab && (clips = p.rows.filter((r) => r.song !== sid).slice(0, 10)))
        .catch(() => (clips = []))
        .finally(done);
    }
  });

  // over the render: it plays (from the start if it did not), the new part from its next bar,
  // as the render's own second (its bars counted from its start), kept there while it plays
  const barSec = $derived(((60 / (bpm || 120)) * 4));
  async function startRender(): Promise<number> {
    if (!temoin) return 0;
    if (!(player.is(temoin) && player.playing)) {
      player.play(temoin, sid, title, 0);
      await new Promise((r) => setTimeout(r, 250));
    }
    const pos = player.pos();
    let at = Math.ceil(pos / barSec) * barSec;
    // time to load it first
    if ((at - pos) / player.speed() < 0.6) at += barSec;
    return at;
  }
  // no render: a sample loop goes over the project itself, as the arrangement plays it (a MIDI
  // part cannot: the project and the part would both need the one MIDI player)
  const tr = $derived(sheetTransport.by[sid] ?? null);
  const overProject = $derived(!temoin && !!tr);
  let onProject = $state(false);
  async function startProject(): Promise<number> {
    if (!tr) return 0;
    if (!tr.sounding) {
      tr.playPause();
      for (let i = 0; i < 100 && !(midi.playing && midi.rel === tr.project); i++) await new Promise((r) => setTimeout(r, 50));
    }
    const pos = midi.t;
    let at = Math.ceil(pos / barSec) * barSec;
    if (at - pos < 0.6) at += barSec;
    return at - pos;
  }
  // the project stopped: its loop with it
  $effect(() => {
    if (onProject && tr && !midi.playing) {
      onProject = false;
      layer.stop();
    }
  });
  // one thing heard at a time from this list: a new click stops what the last one started
  let overOn = $state(false);
  function aloneSample(r: SampleRow) {
    if (player.is(r.path) && !layer.rel) return player.toggle();
    midi.stop();
    layer.stop();
    overOn = false;
    player.play(r.path, null, r.name, 0, r.bpm);
  }
  function aloneMidi(r: MidiRow) {
    if (midi.is(r.rel) && !overOn) return midi.stop();
    layer.stop();
    overOn = false;
    if (temoin && player.is(temoin)) player.pause();
    midi.play(r.rel, r.kind, r.name);
  }
  async function overMidi(r: MidiRow) {
    layer.stop();
    overOn = true;
    const at = await startRender();
    // at the render's tempo as it plays (slowed down or sped up in the player bar too)
    midi.setTarget(bpm * player.speed());
    midi.setLock(true);
    midi.loop = true;
    midi.play(r.rel, r.kind, r.name, { layer: true, sync: { at } });
  }
  async function overSample(r: SampleRow) {
    if (!temoin) {
      layer.stop();
      overOn = false;
      const startIn = await startProject();
      onProject = true;
      return layer.play(r.path, lockRate(r.bpm, bpm) ?? 1, startIn);
    }
    midi.stop();
    layer.stop();
    overOn = false;
    const at = await startRender();
    layer.play(r.path, (lockRate(r.bpm, bpm) ?? 1) * player.speed(), 0, { at });
  }
  function stopAll() {
    overOn = false;
    onProject = false;
    midi.stop();
    layer.stop();
    if (temoin && player.is(temoin)) player.pause();
  }
  function dragMidi(r: MidiRow) {
    api.midiExport(r.rel, 0, 1, bpm || null, r.kind === "drums").then((p) => api.dragOut(p)).catch((e) => app.say(String(e), { error: true }));
  }
  const busy = $derived(midi.playing || !!layer.rel);
</script>

{#if (api.inApp || import.meta.env.DEV) && open}
  <section class="card unstuck wide" data-sec="unstuck">
    <FoldHead {sid} key="unstuck" title={t("song.unstuck.title")}>
      <span class="small faint">{songKey || bpm ? t("song.unstuck.fits", { key: songKey ?? "?", bpm: bpm ? Math.round(bpm) : "?" }) : ""}</span>
      {#snippet actions()}{#if busy}<button class="btn sm" onclick={stopAll}>■ {t("song.unstuck.stop")}</button>{/if}{/snippet}
    </FoldHead>
    {#if open}
      {#if !songKey && !bpm}
        <p class="small faint">{t("song.unstuck.noKey")}</p>
      {:else}
        <p class="small muted lede">{t("song.unstuck.lede")}</p>
        <div class="tabs" role="tablist">
          {#each TABS as tb (tb)}<button role="tab" aria-selected={tab === tb} class:on={tab === tb} onclick={() => (tab = tb)}>{t(`song.unstuck.tab.${tb}`)}</button>{/each}
        </div>
        {#if loading}<p class="small faint">…</p>
        {:else if tab === "samples"}
          <ul>
            {#each samples as r (r.path)}
              <li draggable="true" title={t("song.unstuck.dragTip")} ondragstart={(e) => { e.preventDefault(); api.dragOut(r.path).catch((x) => app.say(String(x), { error: true })); }}>
                <button class="pl" title={t("song.unstuck.alone")} onclick={() => aloneSample(r)}><PlayIcon on={!layer.rel && playingFile(r.path)} size={11} /></button>
                {#if temoin || overProject}<button class="pl over" title={temoin ? t("song.unstuck.overTip") : t("song.unstuck.overProjectTip")} onclick={() => (layer.rel === r.path ? stopAll() : overSample(r))}>{#if layer.rel === r.path}<PlayIcon on size={11} />{:else}▶+{/if}</button>{/if}
                <span class="nm" title={r.path}><b>{r.name}</b><span class="faint small">{r.pack}</span></span>
                <span class="small muted">{[r.key, r.bpm ? `${Math.round(r.bpm)} bpm` : ""].filter(Boolean).join(" · ")}</span>
              </li>
            {:else}
              <li class="faint small none">{t("song.unstuck.none")}</li>
            {/each}
          </ul>
        {:else}
          <ul>
            {#each clips as r (r.rel)}
              <li draggable="true" title={t("song.unstuck.dragTip")} ondragstart={(e) => { e.preventDefault(); dragMidi(r); }}>
                <button class="pl" title={t("song.unstuck.alone")} onclick={() => aloneMidi(r)}><PlayIcon on={midi.is(r.rel) && !overOn} size={11} /></button>
                {#if temoin}<button class="pl over" title={t("song.unstuck.overTip")} onclick={() => (midi.is(r.rel) && overOn ? stopAll() : overMidi(r))}>{#if midi.is(r.rel) && overOn}<PlayIcon on size={11} />{:else}▶+{/if}</button>{/if}
                <span class="nm"><b>{r.name}</b>{#if r.song}<a class="faint small" href="#/song/{r.song}">{r.folder}</a>{:else}<span class="faint small">{r.folder}</span>{/if}</span>
                <span class="small muted">{[r.chords.slice(0, 4).join(" · "), r.key, r.bpm ? `${Math.round(r.bpm)} bpm` : "", r.bars ? t("midi.bars", { n: r.bars }) : ""].filter(Boolean).join(" · ")}</span>
              </li>
            {:else}
              <li class="faint small none">{t("song.unstuck.none")}</li>
            {/each}
          </ul>
        {/if}
      {/if}
    {/if}
  </section>
{/if}

<style>
  .lede { margin: 6px 0 8px; }
  .tabs { display: inline-flex; border: 1px solid var(--line); border-radius: 7px; overflow: hidden; }
  .tabs button { border: 0; background: none; color: var(--muted); padding: 4px 10px; cursor: pointer; font: inherit; font-size: 12.5px; }
  .tabs button + button { border-left: 1px solid var(--line); }
  .tabs button.on { background: var(--accent-bg); color: var(--ink); font-weight: 600; }
  ul { list-style: none; margin: 8px 0 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; gap: 8px; align-items: center; padding: 4px 6px; border-radius: 7px; cursor: pointer; }
  li:hover { background: var(--panel2); }
  li.none { display: block; cursor: default; }
  .pl { border: 0; background: var(--accent-bg); color: var(--ink); border-radius: 6px; padding: 2px 8px; cursor: pointer; font: inherit; font-size: 12px; }
  .pl:hover { background: var(--accent); color: var(--on-accent); }
  .pl.over { font-weight: 700; }
  .nm { display: flex; flex-direction: column; min-width: 0; }
  .nm b, .nm a, .nm span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

<script lang="ts">
  // In the window's header, at the right: the MIDI keyboard. A switch turns it off at once
  // (it is let go: Live can take it), its input, sound and volume, the notes it plays now;
  // and the history: what was played, take by take (kept 30 days), to listen to again, keep
  // as a MIDI file, drag into Live, or put on a song's or an artist's board.
  import { keyboard, KEY_SOUNDS, KEY_GROUPS, keySoundKey } from "../keyboard.svelte";
  import { noteLabel } from "../theory";
  import { i18n, t, ago, fmtNum } from "../i18n/index.svelte";
  import * as api from "../api";
  import type { Take } from "../api";
  import { app } from "../state.svelte";
  import { midi } from "../midiPlayer.svelte";
  import PianoRoll from "./PianoRoll.svelte";

  let open = $state(false);
  let tab = $state<"set" | "hist">("set");
  let root = $state<HTMLElement | null>(null);
  const bound = $derived(keyboard.live);
  // the inputs are asked for when the panel opens (the system lists them then)
  function toggle() {
    open = !open;
    if (open) void keyboard.start();
  }
  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  const notes = $derived([...keyboard.down].sort((a, b) => a - b).map((n) => noteLabel(n, i18n.lang === "fr")).join(" "));

  // ---- history
  let takes = $state<Take[]>([]);
  $effect(() => {
    void keyboard.takesVersion;
    if (open && tab === "hist") api.takes().then((x) => (takes = x)).catch(() => {});
  });
  // a take plays in the player at the foot of the window, like any MIDI: stopped there or
  // here, its sound chosen there (and kept for the next ones)
  const keyOf = (tk: Take) => "notes:take:" + tk.id;
  const playing = (tk: Take) => midi.is(keyOf(tk));
  function play(tk: Take) {
    if (playing(tk)) return midi.toggle();
    const ns = tk.notes as [number, number, number, number][];
    // seconds as beats at 120 (2 a second)
    void midi.playNotes("take:" + tk.id, tk.name ?? t("app.keyboard.hist.takeOf", { when: ago(tk.at) }), ns.map(([p, s0, l]) => [p, s0 * 2, l * 2] as [number, number, number]), Math.max(1, tk.dur * 2), 120, false, { vel: ns.map((n) => n[3]) });
    rollOf = tk.id;
  }
  // the take whose notes are drawn large (a piano roll), the playhead on them while it plays
  let rollOf = $state<number | null>(null);
  // the take being kept or put somewhere: its name and tempo
  let kOf = $state<number | null>(null);
  let kName = $state("");
  let kBpm = $state(120);
  function startKeep(tk: Take) {
    kOf = kOf === tk.id ? null : tk.id;
    kName = tk.name ?? t("app.keyboard.hist.defaultName", { date: new Date(tk.at * 1000).toLocaleDateString(i18n.lang), key: tk.key ?? "" }).trim();
  }
  async function keep(tk: Take) {
    try {
      await api.takeKeep(tk.id, kName, kBpm);
      app.say(t("app.keyboard.hist.kept", { name: kName }));
      takes = await api.takes();
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const curSong = $derived(Number(location.hash.match(/^#\/song\/(\d+)/)?.[1] ?? 0));
  const songs = $derived([...app.songs].filter((s) => !s.archived).sort((a, b) => b.mtime - a.mtime).slice(0, 200));
  const artists = $derived(app.config?.artists ?? []);
  let dest = $state("");
  async function toBoard(tk: Take) {
    if (!dest) return;
    try {
      const name = kName || t("app.keyboard.hist.take");
      const bytes = await api.takeSmf(tk.id, name, kBpm);
      const file = new File([bytes], `${name.replace(/[<>:"/\\|?*]/g, " ").trim() || "take"}.mid`, { type: "audio/midi" });
      const [kind, v] = [dest.slice(0, 1), dest.slice(2)];
      if (kind === "s") await api.boardAdd(Number(v), file);
      else await api.artistBoardAdd(v, file);
      app.say(t("app.keyboard.hist.added", { name, where: kind === "s" ? (songs.find((s) => s.id === Number(v))?.title ?? "") : v }));
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  async function remove(tk: Take) {
    try {
      await api.takeDelete(tk.id);
      takes = takes.filter((x) => x.id !== tk.id);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const secs = (s: number) => (s < 60 ? t("app.keyboard.hist.secs", { n: Math.max(1, Math.round(s)) }) : `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, "0")}`);
  // a picture of the take: its notes as short bars (pitch up, time across)
  function roll(tk: Take) {
    const ns = tk.notes as [number, number, number, number][];
    const lo = Math.min(...ns.map((n) => n[0])), hi = Math.max(...ns.map((n) => n[0]));
    const span = Math.max(1, tk.dur);
    return ns.map(([p, s, l]) => ({ x: (s / span) * 100, w: Math.max(0.6, (l / span) * 100), y: hi === lo ? 50 : 90 - ((p - lo) / (hi - lo)) * 80 }));
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="kb" bind:this={root}>
  <button class="ib" class:on={open} class:off={!keyboard.on && !!keyboard.input} aria-expanded={open} aria-haspopup="dialog" onclick={toggle}
    title={!keyboard.on && keyboard.input ? t("app.keyboard.tipOff") : bound ? t("app.keyboard.tipOn", { sound: t(keySoundKey(keyboard.sound)) }) : t("app.keyboard.tip")} aria-label={t("app.keyboard.title")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"><rect x="2.5" y="6" width="19" height="12" rx="2" /><path d="M7 6v7M12 6v7M17 6v7" /><path d="M5.5 6v4.5h3V6M10.5 6v4.5h3V6M15.5 6v4.5h3V6" fill="currentColor" stroke="none" /></svg>
    {#if bound}<i class="dot" class:hit={keyboard.down.length > 0}></i>{/if}
  </button>
  {#if open}
    <div class="pop" role="dialog" aria-label={t("app.keyboard.title")}>
      <div class="hd">
        <h3>{t("app.keyboard.title")}</h3>
        {#if keyboard.supported && keyboard.input}
          <label class="sw" title={t("app.keyboard.onTip")}>
            <input type="checkbox" checked={keyboard.on} onchange={(e) => keyboard.setOn(e.currentTarget.checked)} />
            <span>{keyboard.on ? t("app.keyboard.isOn") : t("app.keyboard.isOff")}</span>
          </label>
        {/if}
      </div>
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "set"} class:sel={tab === "set"} onclick={() => (tab = "set")}>{t("app.keyboard.tabSet")}</button>
        <button role="tab" aria-selected={tab === "hist"} class:sel={tab === "hist"} onclick={() => (tab = "hist")}>{t("app.keyboard.tabHist")}</button>
      </div>
      {#if tab === "set"}
        {#if !keyboard.supported}<p class="small warn">{t("settings.keyboard.unsupported")}</p>
        {:else}
          {#if !keyboard.on && keyboard.input}<p class="small muted">{t("app.keyboard.offText")}</p>{/if}
          {#if keyboard.error}<p class="small warn">{keyboard.error}</p>{/if}
          <label class="row"><span class="small muted">{t("settings.keyboard.input")}</span>
            <select value={keyboard.input} onchange={(e) => keyboard.setInput(e.currentTarget.value)} disabled={!keyboard.inputs.length}>
              <option value="">{t("settings.keyboard.off")}</option>
              {#each keyboard.inputs as i (i.id)}<option value={i.id}>{i.name}</option>{/each}
            </select>
          </label>
          {#if !keyboard.inputs.length}<p class="small faint">{t("settings.keyboard.none")}</p>{/if}
          <div class="small muted lab">{t("settings.keyboard.sound")}</div>
          <div class="sounds" role="radiogroup" aria-label={t("settings.keyboard.sound")}>
            {#each KEY_SOUNDS as k (k)}
              <button role="radio" aria-checked={keyboard.sound === k} class:sel={keyboard.sound === k} onclick={() => keyboard.setSound(k)}>{t(keySoundKey(k))}</button>
            {/each}
          </div>
          <!-- the orchestra, recorded: loaded the first time one is chosen -->
          {#each KEY_GROUPS as [g, list] (g)}
            <div class="small faint grp">{t(`song.session.famGroup.${g}`)}</div>
            <div class="sounds orch" role="radiogroup" aria-label={t(`song.session.famGroup.${g}`)}>
              {#each list as k (k)}
                <button role="radio" aria-checked={keyboard.sound === k} class:sel={keyboard.sound === k} onclick={() => keyboard.setSound(k)}>{t(keySoundKey(k))}</button>
              {/each}
            </div>
          {/each}
          <label class="row"><span class="small muted">{t("settings.keyboard.volume")}</span>
            <input type="range" min="0" max="1" step="0.01" value={keyboard.volume} oninput={(e) => keyboard.setVolume(+e.currentTarget.value)} />
          </label>
          {#if bound}<p class="small state">{notes ? t("settings.keyboard.playing", { notes }) : t("settings.keyboard.ready")}</p>{/if}
        {/if}
      {:else}
        <p class="small faint">{t("app.keyboard.hist.intro")}</p>
        <div class="hist">
          {#each takes as tk (tk.id)}
            <div class="take" class:kept={!!tk.name}>
              <div class="tr">
                <button class="pp" onclick={() => play(tk)} aria-label={playing(tk) ? t("app.keyboard.hist.stop") : t("app.keyboard.hist.play")}>
                  {#if playing(tk)}<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5" /></svg>
                  {:else}<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor"><path d="M7 5.5v13a1 1 0 0 0 1.5.86l11-6.5a1 1 0 0 0 0-1.72l-11-6.5A1 1 0 0 0 7 5.5z" /></svg>{/if}
                </button>
                <div class="ti">
                  <div class="tn">{tk.name ?? ago(tk.at)}</div>
                  <div class="small faint">{secs(tk.dur)} · {t("app.keyboard.hist.notes", { n: fmtNum(tk.n) })}{tk.key ? ` · ${tk.key}` : ""}{tk.name ? ` · ${ago(tk.at)}` : ""}</div>
                </div>
                <button class="rollbtn" onclick={() => (rollOf = rollOf === tk.id ? null : tk.id)} aria-expanded={rollOf === tk.id} title={t("app.keyboard.hist.rollTip")}>
                  <svg class="roll" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">{#each roll(tk) as r}<rect x={r.x} y={r.y - 4} width={r.w} height="8" rx="1" />{/each}</svg>
                </button>
                {#if tk.file}
                  <span class="drag small" draggable="true" title={t("app.keyboard.hist.dragTip")} role="button" tabindex="0"
                    ondragstart={(e) => { e.preventDefault(); api.dragOut(tk.file!).catch((x) => app.say(String(x), { error: true })); }}>{t("app.keyboard.hist.drag")}</span>
                {/if}
                <button class="small lk" onclick={() => startKeep(tk)} aria-expanded={kOf === tk.id}>{tk.name ? t("app.keyboard.hist.more") : t("app.keyboard.hist.keep")}</button>
              </div>
              {#if rollOf === tk.id}
                <PianoRoll notes={tk.notes as [number, number, number, number][]} dur={tk.dur} at={playing(tk) ? midi.t : null} />
              {/if}
              {#if kOf === tk.id}
                <div class="kp small">
                  <label>{t("app.keyboard.hist.name")} <input bind:value={kName} /></label>
                  <label>{t("app.keyboard.hist.bpm")} <input type="number" min="40" max="240" bind:value={kBpm} class="bpm" /></label>
                  <button class="btn sm primary" disabled={!kName.trim()} onclick={() => keep(tk)}>{tk.file ? t("app.keyboard.hist.keepAgain") : t("app.keyboard.hist.keepGo")}</button>
                  <div class="to">
                    <select bind:value={dest} aria-label={t("app.keyboard.hist.toBoard")}>
                      <option value="">{t("app.keyboard.hist.toBoard")}</option>
                      {#if curSong}<option value="s:{curSong}">{t("app.keyboard.hist.thisSong")}</option>{/if}
                      <optgroup label={t("app.keyboard.hist.songs")}>{#each songs as s (s.id)}<option value="s:{s.id}">{s.title ?? "?"}</option>{/each}</optgroup>
                      {#if artists.length}<optgroup label={t("app.keyboard.hist.artists")}>{#each artists as a}<option value="a:{a}">{a}</option>{/each}</optgroup>{/if}
                    </select>
                    <button class="btn sm" disabled={!dest || !kName.trim()} onclick={() => toBoard(tk)}>{t("app.keyboard.hist.add")}</button>
                  </div>
                  <button class="link del" onclick={() => remove(tk)}>{t("app.keyboard.hist.delete")}</button>
                </div>
              {/if}
            </div>
          {:else}
            <p class="small muted">{t("app.keyboard.hist.none")}</p>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* at the right end of the header, whatever is before it */
  .kb { position: relative; flex: none; margin-left: auto; }
  .ib { position: relative; width: 34px; height: 34px; border: 0; border-radius: 8px; background: none; color: var(--muted); display: grid; place-items: center; cursor: pointer; }
  .ib svg { width: 20px; height: 20px; }
  .ib:hover, .ib.on { background: var(--panel2); color: var(--ink); }
  .ib.off { opacity: .5; }
  .ib.off::after { content: ""; position: absolute; left: 7px; right: 7px; top: 50%; height: 1.6px; background: currentColor; transform: rotate(-35deg); }
  .dot { position: absolute; top: 6px; right: 5px; width: 7px; height: 7px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 0 2px var(--bg, var(--panel)); transition: transform .1s; }
  .dot.hit { transform: scale(1.4); background: var(--accent); }
  .pop { position: absolute; right: 0; top: calc(100% + 8px); z-index: 1500; width: 360px; max-height: min(640px, 80vh); overflow: auto; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); box-shadow: 0 14px 40px rgba(0, 0, 0, .28); display: flex; flex-direction: column; gap: 10px; }
  .hd { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  h3 { margin: 0; font-size: 14px; }
  .sw { display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px; cursor: pointer; color: var(--muted); }
  .tabs { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); width: fit-content; }
  .tabs button { border: 0; background: none; padding: 3px 12px; border-radius: 7px; cursor: pointer; color: var(--muted); font: inherit; font-size: 12.5px; }
  .tabs button.sel { background: var(--panel); color: var(--ink); font-weight: 600; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .row select { max-width: 200px; }
  .row input[type="range"] { flex: 1; max-width: 200px; }
  .lab { margin-bottom: -4px; }
  .sounds { display: grid; grid-template-columns: repeat(2, 1fr); gap: 6px; }
  .sounds.orch { grid-template-columns: repeat(3, 1fr); gap: 4px; }
  .sounds.orch button { padding: 4px 6px; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grp { margin: 2px 0 -6px; }
  .sounds button { border: 1px solid var(--line); border-radius: 8px; background: none; color: var(--ink); padding: 6px 8px; font: inherit; font-size: 12.5px; cursor: pointer; text-align: left; }
  .sounds button:hover { border-color: var(--faint); }
  .sounds button.sel { border-color: var(--accent); background: var(--accent-bg); font-weight: 600; }
  .state { margin: 0; color: var(--ok); }
  .warn { color: var(--fix); margin: 0; }
  p { margin: 0; }
  .hist { display: flex; flex-direction: column; gap: 6px; }
  .take { border: 1px solid var(--line); border-radius: 10px; padding: 6px 8px; }
  .take.kept { border-color: color-mix(in srgb, var(--accent) 50%, var(--line)); }
  .tr { display: grid; grid-template-columns: 26px minmax(0, 1fr) 60px auto auto; align-items: center; gap: 8px; }
  .pp { width: 26px; height: 26px; border-radius: 50%; border: 1px solid var(--line); background: var(--bg); color: var(--ink); display: grid; place-items: center; cursor: pointer; }
  .ti { min-width: 0; }
  .tn { font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .roll { width: 60px; height: 22px; fill: var(--accent); opacity: .8; display: block; }
  .rollbtn { border: 0; background: none; padding: 2px; border-radius: 6px; cursor: pointer; }
  .rollbtn:hover, .rollbtn[aria-expanded="true"] { background: var(--panel2); }
  .lk { border: 0; background: none; color: var(--accent); cursor: pointer; font: inherit; padding: 2px 4px; }
  .drag { border: 1px dashed var(--line); border-radius: 6px; padding: 1px 6px; cursor: grab; color: var(--muted); }
  .kp { display: flex; flex-direction: column; gap: 6px; margin-top: 8px; padding-top: 8px; border-top: 1px solid var(--line); }
  .kp label { display: flex; align-items: center; gap: 6px; justify-content: space-between; }
  .kp input { flex: 1; min-width: 0; border: 1px solid var(--line); background: var(--bg); color: var(--ink); border-radius: 6px; padding: 3px 7px; font: inherit; }
  .kp .bpm { flex: 0 0 70px; }
  .to { display: flex; gap: 6px; }
  .to select { flex: 1; min-width: 0; }
  .link { border: 0; background: none; padding: 0; cursor: pointer; font: inherit; text-decoration: underline; }
  .del { color: var(--muted); align-self: flex-start; }
  @media (prefers-reduced-motion: reduce) { .dot { transition: none; } }
</style>

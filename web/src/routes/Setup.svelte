<script lang="ts">
  import Logo from "../lib/components/Logo.svelte";
  // The first-run setup: nothing else of the app opens until it is finished. Projects folders,
  // sample folders, the first analysis (followed step by step), then the artists found in the
  // folder names. Everything here can be changed later in Settings.
  import * as api from "../lib/api";
  import { app } from "../lib/state.svelte";
  import FolderList from "../lib/components/FolderList.svelte";
  import ScanProgress from "../lib/components/ScanProgress.svelte";
  import RootArtists from "../lib/components/RootArtists.svelte";
  import Library from "../lib/components/Library.svelte";
  import { t, i18n, LANGS } from "../lib/i18n/index.svelte";
  let ra = $state<{ saveAll: () => Promise<void> }>();

  const STEPS = $derived((["welcome", "projects", "samples", "scan", "artists"] as const).map((k) => t(`setup.steps.${k}`)));
  let step = $state(app.config?.roots.length ? 1 : 0);
  const cfg = $derived(app.config);

  async function start() {
    // saved as "not finished": closing the app now brings the setup back
    app.config = await api.setConfig("setup_done", false);
    step = 1;
  }
  const addRoot = async (p: string) => (app.config = await api.addRoot(p));
  const removeRoot = async (p: string) => (app.config = await api.removeRoot(p));
  const addSamples = async (p: string) => {
    const dirs = cfg?.sample_dirs ?? [];
    if (dirs.some((d) => d.toLowerCase() === p.toLowerCase())) throw t("settings.already");
    app.config = await api.setConfig("sample_dirs", [...dirs, p]);
  };
  const removeSamples = async (p: string) => (app.config = await api.setConfig("sample_dirs", (cfg?.sample_dirs ?? []).filter((d) => d !== p)));
  const addMidi = async (p: string) => {
    const dirs = cfg?.midi_dirs ?? [];
    if (dirs.some((d) => d.toLowerCase() === p.toLowerCase())) throw t("settings.already");
    app.config = await api.setConfig("midi_dirs", [...dirs, p]);
  };
  const removeMidi = async (p: string) => (app.config = await api.setConfig("midi_dirs", (cfg?.midi_dirs ?? []).filter((d) => d !== p)));

  // the analysis: started on arrival, done when a pass ends after that moment
  let from = $state(0);
  let scanError = $state("");
  // the MIDI files come last and may take long: from there the setup is done, they go on in
  // the background (the sidebar shows them)
  // (the core names its steps in the interface's language: "fichiers MIDI", "MIDI files")
  const midiPhase = $derived(!!app.scan?.running && (app.scan?.stage ?? "").includes("MIDI"));
  const finished = $derived(step === 3 && from > 0 && ((!app.scan?.running && (app.scan?.last ?? 0) * 1000 >= from) || midiPhase));
  async function analyse() {
    step = 3;
    scanError = "";
    from = Date.now() - 500;
    try {
      // false: a pass was already running (the background one); its end counts the same
      await api.scan(false);
    } catch (e) {
      scanError = String(e);
    }
  }

  // artists: names found in the folders, ticked by the user, plus any typed
  let found = $state<api.ArtistGuess[]>([]);
  // the names given at the Projects step (one per projects folder): shown first, already ticked
  let given = $state<string[]>([]);
  let picked = $state<Record<string, boolean>>({});
  let typed = $state("");
  let drive = $state("");
  async function toArtists() {
    step = 4;
    drive = cfg?.old_drive ?? "";
    const low = new Set<string>();
    given = [...(cfg?.roots ?? []).map((r) => r.artist), ...(cfg?.artists ?? [])]
      .map((a) => a.trim())
      .filter((a) => a && !low.has(a.toLowerCase()) && low.add(a.toLowerCase()));
    try {
      found = (await api.artistSuggestions()).filter((g) => !low.has(g.name.toLowerCase()));
    } catch {
      found = [];
    }
    // the sure ones (in file names AND in several folders) come ticked
    picked = Object.fromEntries([...given.map((n) => [n, true]), ...found.map((g) => [g.name, g.sure])]);
  }
  let saving = $state(false);
  async function finish() {
    saving = true;
    try {
      const names = [
        ...(cfg?.artists ?? []).filter((n) => picked[n] !== false),
        ...given.filter((n) => picked[n]),
        ...found.filter((g) => picked[g.name]).map((g) => g.name),
        ...typed.split("\n").map((l) => l.trim()).filter(Boolean),
      ];
      const seen = new Set<string>();
      const artists = names.filter((n) => !seen.has(n.toLowerCase()) && seen.add(n.toLowerCase()));
      if (artists.length || cfg?.artists.length) await api.setConfig("artists", artists);
      if (drive !== (cfg?.old_drive ?? "")) await api.setConfig("old_drive", drive);
      app.config = await api.setConfig("setup_done", true);
      await app.reload();
      location.hash = "#/home";
    } catch (e) {
      app.say(String(e), { error: true });
    }
    saving = false;
  }
  const DRIVES = "DEFGHIJKLMNOPQRSTUVWXYZ".split("");
  const projects = $derived(app.stats?.projects.total ?? 0);
</script>

<div class="setup">
  <div class="box">
    <header>
      <div>
        <div class="brand"><Logo size={28} /><span>spool</span></div>
        <div class="small faint">{t("setup.configuration")}</div>
      </div>
      {#if step > 0}
        <ol class="crumbs small" aria-label={t("setup.stepsLabel")}>
          {#each STEPS.slice(1) as s, i}<li class:on={step === i + 1} class:done={step > i + 1}>{s}</li>{/each}
        </ol>
      {/if}
      <!-- the language first: someone landing in the wrong one switches before anything else -->
      <div class="lang" class:alone={step === 0} role="group" aria-label={t("common.language")}>
        {#each LANGS as [code, name]}<button class:on={i18n.lang === code} lang={code} aria-pressed={i18n.lang === code} onclick={() => app.setLang(code)}>{name}</button>{/each}
      </div>
    </header>

    {#if step === 0}
      <section>
        <h1>{t("setup.welcome.title")}</h1>
        <p class="lead">{t("setup.welcome.lead")}</p>
        <ul class="facts">
          <li><b>{t("setup.welcome.neverTouched")}</b> {t("setup.welcome.copyBeside")}</li>
          <li>{t("setup.welcome.local")}</li>
          <li>{t("setup.welcome.time")}</li>
        </ul>
        {#if cfg && !cfg.live_db}
          <p class="warn small">{t("setup.welcome.noLive")}</p>
        {/if}
        <div class="nav"><span></span><button class="btn primary" onclick={start}>{t("setup.welcome.start")}</button></div>
      </section>
    {:else if step === 1}
      <section>
        <h1>{t("setup.projects.title")}</h1>
        <p class="lead">{t("setup.projects.lead")}</p>
        <FolderList
          items={(cfg?.roots ?? []).map((r) => ({ path: r.path, note: r.label ? t("settings.folders.rootLabel", { label: r.label }) : cfg && cfg.roots.length > 1 ? t("settings.folders.mainRoot") : undefined }))}
          add={addRoot}
          remove={removeRoot}
          pick={t("settings.folders.pickProjects")}
          empty={t("setup.projects.empty")}
        />
        <RootArtists bind:this={ra} roots={cfg?.roots ?? []} />
        <div class="nav">
          <button class="btn" onclick={() => (step = 0)}>{t("setup.back")}</button>
          <button class="btn primary" disabled={!cfg?.roots.length} onclick={async () => { await ra?.saveAll(); step = 2; }}>{t("setup.continue")}</button>
        </div>
      </section>
    {:else if step === 2}
      <section>
        <h1>{t("setup.samples.title")}</h1>
        <p class="lead">{t("setup.samples.lead")}</p>
        <FolderList items={(cfg?.sample_dirs ?? []).map((path) => ({ path }))} add={addSamples} remove={removeSamples} check={api.bigFolderWarning} pick={t("settings.folders.pickSamples")} empty={t("setup.samples.empty")} />
        <h3 class="found">{t("setup.samples.midi")} <span class="faint">{t("setup.samples.optional")}</span></h3>
        <p class="small muted">{t("setup.samples.midiHint")}</p>
        <FolderList items={(cfg?.midi_dirs ?? []).map((path) => ({ path }))} add={addMidi} remove={removeMidi} check={api.bigFolderWarning} pick={t("settings.folders.pickMidi")} empty={t("setup.samples.midiEmpty")} />
        <div class="nav">
          <button class="btn" onclick={() => (step = 1)}>{t("setup.back")}</button>
          <button class="btn primary" onclick={analyse}>{cfg?.sample_dirs.length || cfg?.midi_dirs.length ? t("setup.samples.continueScan") : t("setup.samples.skipScan")}</button>
        </div>
      </section>
    {:else if step === 3}
      <section>
        <h1>{finished ? t("setup.scan.done") : t("setup.scan.running")}</h1>
        {#if !finished}
          <p class="lead">{t("setup.scan.lead")}</p>
          <ScanProgress full />
        {:else}
          {@const ok = app.stats?.projects.ok ?? 0}
          {@const fix = app.stats?.projects.fix ?? 0}
          {@const lost = app.stats?.projects.lost ?? 0}
          {@const bad = (app.stats?.projects.bad ?? 0) - lost}
          <div class="tiles">
            <div><b>{projects}</b><span>{t("setup.scan.projects", { n: projects })}</span></div>
            <div><b>{app.songs.length}</b><span>{t("setup.scan.songs", { n: app.songs.length })}</span></div>
            <div><b>{ok}</b><span>{t("setup.scan.ok", { n: ok })}</span></div>
            <div><b>{fix}</b><span>{t("setup.scan.fix", { n: fix })}</span></div>
            <div><b>{lost}</b><span>{t("setup.scan.lost", { n: lost })}</span></div>
            <div><b>{bad}</b><span>{t("setup.scan.bad", { n: bad })}</span></div>
          </div>
          {#if !projects}<p class="warn small">{t("setup.scan.none")}</p>{/if}
          {#if midiPhase}<p class="small muted">{t("setup.scan.midiPhase")}</p>{/if}
          {#if app.scan?.cancelled}<p class="small muted">{t("setup.scan.cancelled")}</p>{/if}
          <h3 class="found">{t("setup.scan.around")}</h3>
          <Library compact />
        {/if}
        {#if scanError || app.scan?.error}<p class="err small">{scanError || app.scan?.error}</p>{/if}
        <div class="nav">
          <button class="btn" disabled={!finished && !scanError} onclick={() => (step = 2)}>{t("setup.back")}</button>
          <button class="btn primary" disabled={!finished} onclick={toArtists}>{t("setup.continue")}</button>
        </div>
      </section>
    {:else}
      <section>
        <h1>{t("setup.artists.title")}</h1>
        <p class="lead">{t("setup.artists.lead")} {given.length ? t("setup.artists.leadGiven") + " " : ""}{found.length ? t("setup.artists.leadFound") : ""}</p>
        {#if given.length || found.length}
          <div class="chips">
            {#each given as n (n)}
              <label class="chip" class:on={picked[n]} title={t("setup.artists.givenTitle")}>
                <input type="checkbox" bind:checked={picked[n]} />{n}<span class="faint">{t("setup.artists.given")}</span>
              </label>
            {/each}
            {#each found as g (g.name)}
              <label class="chip" class:on={picked[g.name]} title={t("setup.artists.guessTitle", { files: g.files, folders: g.folders })}>
                <input type="checkbox" bind:checked={picked[g.name]} />{g.name}<span class="faint">{g.files || g.songs}</span>
              </label>
            {/each}
          </div>
        {:else}
          <p class="small faint">{t("setup.artists.noneFound")}</p>
        {/if}
        <label class="small muted lbl" for="more-artists">{t("setup.artists.more")}</label>
        <textarea id="more-artists" rows="3" bind:value={typed} placeholder={t("setup.artists.placeholder")}></textarea>
        <div class="drive">
          <label class="small muted" for="old-drive">{t("setup.artists.oldWindows")}</label>
          <select id="old-drive" bind:value={drive}>
            <option value="">{t("setup.artists.no")}</option>
            {#each DRIVES as d}<option value={d}>{t("setup.artists.drive", { d })}</option>{/each}
          </select>
          <span class="small faint">{t("setup.artists.oldWindowsHint")}</span>
        </div>
        <div class="nav">
          <button class="btn" onclick={() => (step = 3)}>{t("setup.back")}</button>
          <button class="btn primary" disabled={saving} onclick={finish}>{saving ? t("setup.artists.saving") : t("setup.artists.finish")}</button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .setup { position: fixed; inset: 0; background: var(--bg); display: grid; place-items: center; padding: 24px; overflow: auto; z-index: 100; }
  .box { width: min(760px, 100%); background: var(--panel); border: 1px solid var(--line); border-radius: 16px; padding: 26px 30px 24px; box-shadow: 0 30px 80px rgba(0, 0, 0, .35); }
  header { display: flex; align-items: center; gap: 12px; margin-bottom: 22px; }
  .brand { display: flex; align-items: center; gap: 10px; font-family: var(--mono); font-weight: 600; font-size: 19px; letter-spacing: -0.03em; }
  .crumbs { list-style: none; display: flex; gap: 6px; margin: 0 0 0 auto; padding: 0; flex-wrap: wrap; justify-content: flex-end; }
  .crumbs li { padding: 2px 9px; border-radius: 99px; color: var(--faint); border: 1px solid var(--line); }
  .crumbs li.on { color: var(--ink); border-color: var(--accent); background: var(--accent-bg); }
  .crumbs li.done { color: var(--muted); }
  .lang { display: inline-flex; padding: 2px; border-radius: 8px; border: 1px solid var(--line); background: var(--panel2); flex: none; }
  .lang.alone { margin-left: auto; }
  .lang button { border: 0; background: none; padding: 2px 9px; border-radius: 6px; cursor: pointer; color: var(--muted); font: inherit; font-size: 12px; }
  .lang button.on { background: var(--panel); color: var(--ink); font-weight: 600; }
  h1 { font-size: 22px; margin-bottom: 8px; }
  .lead { color: var(--muted); max-width: 62ch; margin: 0 0 18px; }
  .facts { margin: 0 0 18px; padding-left: 18px; display: flex; flex-direction: column; gap: 6px; }
  .nav { display: flex; justify-content: space-between; margin-top: 24px; padding-top: 16px; border-top: 1px solid var(--line); }
  .btn:disabled { opacity: .5; cursor: default; }
  .warn { color: var(--lost); background: color-mix(in srgb, var(--lost) 10%, transparent); padding: 9px 12px; border-radius: 9px; }
  .err { color: var(--bad); }
  .found { margin: 18px 0 8px; font-size: 13px; color: var(--muted); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 10px; }
  .tiles div { border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; display: flex; flex-direction: column; background: var(--panel2); }
  .tiles b { font-size: 24px; font-variant-numeric: tabular-nums; }
  .tiles span { color: var(--muted); font-size: 12px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 16px; max-height: 220px; overflow: auto; }
  .chip { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; border: 1px solid var(--line); border-radius: 99px; cursor: pointer; user-select: none; }
  .chip.on { border-color: var(--accent); background: var(--accent-bg); }
  .chip input { margin: 0; accent-color: var(--accent); }
  .chip .faint { font-size: 11px; font-variant-numeric: tabular-nums; }
  .lbl { display: block; margin-bottom: 4px; }
  textarea { width: 100%; background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; font: inherit; color: inherit; resize: vertical; }
  .drive { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 10px; margin-top: 16px; }
  .drive label { flex-basis: 100%; }
</style>

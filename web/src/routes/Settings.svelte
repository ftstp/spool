<script lang="ts">
  // Settings, in four parts: the folders the app reads, how it arranges (in the app only, or
  // on disk too), the data in and out, and starting over set apart. A list of settings, each
  // with what it does; the parts listed on the left follow the scroll.
  import * as api from "../lib/api";
  import { app } from "../lib/state.svelte";
  import FolderList from "../lib/components/FolderList.svelte";
  import ScanProgress from "../lib/components/ScanProgress.svelte";
  import { ago } from "../lib/format";
  import NotionImport from "../lib/components/NotionImport.svelte";
  import RootArtists from "../lib/components/RootArtists.svelte";
  import { exportSongs } from "../lib/csv";
  import FatstepLogo from "../lib/components/FatstepLogo.svelte";
  import { theme, THEMES } from "../lib/theme.svelte";
  import { updates } from "../lib/update.svelte";
  import { t, fmtTime, i18n, LANGS, type Lang } from "../lib/i18n/index.svelte";
  // the export: every song, or with the archived ones too
  let withArchived = $state(false);
  const toExport = $derived(app.songs.filter((s) => withArchived || !s.archived));

  const cfg = $derived(app.config);
  let msg = $state("");
  let busy = $state(false);
  let confirming = $state(false);

  async function set<K extends string>(key: K, value: unknown, ok?: string) {
    try {
      app.config = await api.setConfig(key, value);
      if (ok) app.say(ok);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  const addRoot = async (p: string) => {
    app.config = await api.addRoot(p);
    app.say(t("settings.folderAdded"));
    api.scan(false);
  };
  const removeRoot = async (p: string) => {
    app.config = await api.removeRoot(p);
    app.say(t("settings.folderRemoved"));
    api.scan(false);
  };
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

  // artists and working words: edited as text, saved on demand (a change regroups the songs)
  const lines = (v: string) => v.split("\n").map((l) => l.trim()).filter(Boolean);
  let artists = $state((app.config?.artists ?? []).join("\n"));
  let words = $state((app.config?.work_words ?? []).join("\n"));
  const artistsDirty = $derived(lines(artists).join("\n") !== (cfg?.artists ?? []).join("\n"));
  const wordsDirty = $derived(lines(words).join("\n") !== (cfg?.work_words ?? []).join("\n"));
  let found = $state<api.ArtistGuess[] | null>(null);
  async function suggest() {
    found = await api.artistSuggestions();
  }
  function addArtist(n: string) {
    artists = [...lines(artists), n].join("\n");
    found = found?.filter((g) => g.name !== n) ?? null;
  }

  async function reset() {
    busy = true;
    try {
      await api.resetBase();
      confirming = false;
      app.undo = await api.undoState();
      // the base is empty: the setup comes back in front
      await app.reload();
    } catch (e) {
      msg = String(e);
    }
    busy = false;
  }
  const DRIVES = "DEFGHIJKLMNOPQRSTUVWXYZ".split("");

  // the part on screen, for the list on the left
  const PARTS = $derived((["dossiers", "rangement", "donnees", "spool", "zero"] as const).map((k) => [k, t(`settings.parts.${k}`)] as const));
  let part = $state<string>("dossiers");
  function spy(node: HTMLElement) {
    const io = new IntersectionObserver(
      (es) => {
        for (const e of es) if (e.isIntersecting) part = (e.target as HTMLElement).dataset.part ?? part;
      },
      { root: node.closest("main"), rootMargin: "0px 0px -65% 0px" },
    );
    node.querySelectorAll<HTMLElement>("[data-part]").forEach((x) => io.observe(x));
    return { destroy: () => io.disconnect() };
  }
  const goTo = (k: string) => {
    part = k;
    document.getElementById("part-" + k)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  let notionFile = $state<string | null>(null);
  async function pickNotion() {
    try {
      notionFile = await api.pickFile(t("settings.data.notionDialog"), ["csv"]);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
</script>

<div class="page">
  <header class="head">
    <h1>{t("settings.title")}</h1>
    <p class="muted sub">{t("settings.sub")}</p>
  </header>
  {#if cfg}
    <div class="layout">
      <nav class="toc" aria-label={t("settings.parts.label")}>
        {#each PARTS as [k, l]}<button class:on={part === k} class:z={k === "zero"} onclick={() => goTo(k)}>{l}</button>{/each}
      </nav>

      <div class="body" use:spy>
        <section class="group" id="part-dossiers" data-part="dossiers">
          <h2>{t("settings.parts.dossiers")}</h2>
          <p class="gsub small muted">{t("settings.folders.sub")}</p>

          <div class="item stack">
            <div class="it"><h3>{t("settings.folders.projects")}</h3><p class="small muted">{t("settings.folders.projectsHint")}</p></div>
            <FolderList
              items={cfg.roots.map((r) => ({ path: r.path, note: r.label ? t("settings.folders.rootLabel", { label: r.label }) : cfg.roots.length > 1 ? t("settings.folders.mainRoot") : undefined }))}
              add={addRoot}
              remove={removeRoot}
              pick={t("settings.folders.pickProjects")}
            />
            <RootArtists roots={cfg.roots} />
          </div>

          <div class="item stack">
            <div class="it"><h3>{t("settings.folders.samples")}</h3><p class="small muted">{t("settings.folders.samplesHint")}</p></div>
            <FolderList items={cfg.sample_dirs.map((path) => ({ path }))} add={addSamples} remove={removeSamples} check={api.bigFolderWarning} pick={t("settings.folders.pickSamples")} empty={t("settings.folders.samplesEmpty")} />
          </div>

          <div class="item stack">
            <div class="it"><h3>{t("settings.folders.midi")} <span class="opt">{t("settings.folders.optional")}</span></h3><p class="small muted">{t("settings.folders.midiHint")}</p></div>
            <FolderList items={cfg.midi_dirs.map((path) => ({ path }))} add={addMidi} remove={removeMidi} check={api.bigFolderWarning} pick={t("settings.folders.pickMidi")} empty={t("settings.folders.midiEmpty")} />
          </div>

          <div class="item">
            <div class="it"><h3>{t("settings.folders.oldWindows")}</h3><p class="small muted">{t("settings.folders.oldWindowsHint")}</p></div>
            <div class="ctl">
              <select value={cfg.old_drive} aria-label={t("settings.folders.oldDrive")} onchange={(e) => set("old_drive", (e.currentTarget as HTMLSelectElement).value, t("settings.folders.oldDriveSaved"))}>
                <option value="">{t("settings.folders.none")}</option>
                {#each DRIVES as d}<option value={d}>{t("settings.folders.drive", { d })}</option>{/each}
              </select>
            </div>
          </div>
        </section>

        <section class="group" id="part-rangement" data-part="rangement">
          <h2>{t("settings.parts.rangement")}</h2>
          <p class="gsub small muted">{t("settings.arrange.sub")}</p>

          <div class="item" class:live={cfg.physical}>
            <div class="it">
              <h3>{t("settings.arrange.physical")}</h3>
              <p class="small muted">{t("settings.arrange.physicalHint")}</p>
              <p class="small state">{cfg.physical ? t("settings.arrange.physicalOn") : t("settings.arrange.physicalOff")}</p>
            </div>
            <label class="switch" title={cfg.physical ? t("settings.arrange.disable") : t("settings.arrange.enable")}>
              <input type="checkbox" role="switch" aria-label={t("settings.arrange.physical")} checked={!!cfg.physical} onchange={(e) => set("physical", e.currentTarget.checked, e.currentTarget.checked ? t("settings.arrange.physicalOnSaid") : t("settings.arrange.physicalOffSaid"))} />
              <i></i>
            </label>
          </div>

          <div class="item stack">
            <div class="it"><h3>{t("settings.arrange.artists")}</h3><p class="small muted">{t("settings.arrange.artistsHint")}</p></div>
            <textarea rows="6" bind:value={artists} placeholder={t("settings.arrange.artistPlaceholder")}></textarea>
            <div class="row">
              <button class="btn primary" disabled={!artistsDirty} onclick={() => set("artists", lines(artists), t("settings.arrange.artistsSaved"))}>{t("common.save")}</button>
              <button class="btn" onclick={suggest}>{t("settings.arrange.suggest")}</button>
            </div>
            {#if found}
              {#if found.length}
                <div class="chips">
                  {#each found as g (g.name)}<button class="chip" class:sure={g.sure} onclick={() => addArtist(g.name)} title={t(g.sure ? "settings.arrange.guessTitleSure" : "settings.arrange.guessTitle", { files: g.files, folders: g.folders })}>+ {g.name} <span class="faint">{g.files || g.songs}</span></button>{/each}
                </div>
              {:else}<p class="small faint">{t("settings.arrange.nothingMore")}</p>{/if}
            {/if}
          </div>

          <div class="item stack">
            <div class="it"><h3>{t("settings.arrange.words")}</h3><p class="small muted">{t("settings.arrange.wordsHint")}</p></div>
            <textarea rows="4" bind:value={words}></textarea>
            <div class="row"><button class="btn primary" disabled={!wordsDirty} onclick={() => set("work_words", lines(words), t("settings.arrange.wordsSaved"))}>{t("common.save")}</button></div>
          </div>
        </section>

        <section class="group" id="part-donnees" data-part="donnees">
          <h2>{t("settings.parts.donnees")}</h2>
          <p class="gsub small muted">{t("settings.data.sub")}</p>

          <div class="item" class:stack={app.scan?.running}>
            <div class="it">
              <h3>{t("settings.data.scan")}</h3>
              <p class="small muted">{t("settings.data.scanHint")}</p>
              {#if !app.scan?.running}<p class="small state">{app.scan?.last ? t("settings.data.lastScan", { ago: ago(app.scan.last) }) : t("settings.data.notScanned")}{cfg.live_db ? "" : t("settings.data.noLive")}</p>{/if}
            </div>
            {#if app.scan?.running}<ScanProgress full />
            {:else}<div class="ctl"><button class="btn" onclick={() => api.scan(false)}>{t("settings.data.scanNow")}</button><button class="btn" onclick={() => api.scan(true)}>{t("settings.data.rescan")}</button></div>{/if}
          </div>

          <div class="item">
            <div class="it">
              <h3>{t("settings.data.export")}</h3>
              <p class="small muted">{t("settings.data.exportHint")}</p>
              <label class="small chk"><input type="checkbox" bind:checked={withArchived} /> {t("settings.data.withArchived")}</label>
            </div>
            <div class="ctl"><button class="btn" disabled={!toExport.length} onclick={() => exportSongs(toExport)}>{t("settings.data.exportN", { n: toExport.length })}</button></div>
          </div>

          <div class="item">
            <div class="it"><h3>{t("settings.data.notion")}</h3><p class="small muted">{t("settings.data.notionHint")}</p></div>
            <div class="ctl"><button class="btn" onclick={pickNotion}>{t("settings.data.notionPick")}</button></div>
          </div>

        </section>

        <section class="group" id="part-spool" data-part="spool">
          <h2>{t("settings.parts.spool")}</h2>
          <p class="gsub small muted">{t("settings.spool.sub")}</p>
          <div class="item">
            <div class="it">
              <h3>{t("settings.spool.version", { v: updates.current || "..." })}</h3>
              <p class="small muted">{t("settings.spool.updatesHint")}</p>
              <p class="small state">{#if updates.info}{t("settings.spool.available", { version: updates.info.version })}{:else if updates.at}{t("settings.spool.checkedAt", { time: fmtTime(new Date(updates.at)) })}{:else}{t("settings.spool.notChecked")}{/if}</p>
            </div>
            <div class="ctl">
              {#if updates.info}<button class="btn primary" disabled={updates.installing} onclick={() => updates.install()}>{updates.installing ? t("settings.spool.downloading") : t("settings.spool.install", { version: updates.info.version })}</button>
              {:else}<button class="btn" disabled={updates.checking} onclick={() => updates.check(true)}>{updates.checking ? t("settings.spool.checking") : t("settings.spool.check")}</button>{/if}
            </div>
          </div>
          <div class="item">
            <div class="it"><h3>{t("settings.theme.title")}</h3><p class="small muted">{t("settings.theme.hint")}</p></div>
            <div class="ctl">
              <div class="seg" role="radiogroup" aria-label={t("settings.theme.title")}>
                {#each THEMES as k}<button role="radio" aria-checked={theme.value === k} class:on={theme.value === k} onclick={() => theme.set(k)}>{t(`settings.theme.${k}`)}</button>{/each}
              </div>
            </div>
          </div>
          <div class="item">
            <div class="it"><h3>{t("common.language")}</h3><p class="small muted">{t("common.languageHint")}</p></div>
            <div class="ctl">
              <select value={i18n.lang} aria-label={t("common.language")} onchange={(e) => app.setLang(e.currentTarget.value as Lang)}>
                {#each LANGS as [code, name]}<option value={code} lang={code}>{name}</option>{/each}
              </select>
            </div>
          </div>
          <div class="item">
            <div class="it">
              <h3>{t("settings.fatstep.title")}</h3>
              <div class="publisher"><FatstepLogo size={26} /></div>
              <p class="small muted">{t("settings.fatstep.body")}</p>
            </div>
            <div class="ctl"><button class="btn" onclick={() => api.openFatstep("/")}>{t("settings.fatstep.site")} ↗</button></div>
          </div>
        </section>

        <section class="group danger-zone" id="part-zero" data-part="zero">
          <h2>{t("settings.parts.zero")}</h2>
          <div class="item" class:stack={confirming}>
            <div class="it">
              <p class="small muted">{t("settings.zero.hint")}</p>
              {#if confirming}<p class="small warn">{t("settings.zero.confirm")}</p>{/if}
            </div>
            {#if confirming}
              <div class="row">
                <button class="btn danger" disabled={busy} onclick={reset}>{t("settings.zero.yes")}</button>
                <button class="btn" disabled={busy} onclick={() => (confirming = false)}>{t("common.cancel")}</button>
              </div>
            {:else}
              <div class="ctl"><button class="btn danger" onclick={() => (confirming = true)}>{t("settings.zero.start")}</button></div>
            {/if}
          </div>
        </section>
      </div>
    </div>
  {/if}
</div>

{#if notionFile}<NotionImport path={notionFile} close={() => (notionFile = null)} />{/if}

<style>
  .page { padding: 24px 28px 80px; }
  .head { margin-bottom: 22px; }
  .head h1 { margin: 0 0 4px; }
  .sub { margin: 0; max-width: 720px; }
  .layout { display: grid; grid-template-columns: 160px minmax(0, 820px); gap: 36px; align-items: start; }

  /* the parts, following the scroll */
  .toc { position: sticky; top: 0; display: flex; flex-direction: column; gap: 2px; padding-top: 2px; }
  .toc button { border: 0; background: none; text-align: left; padding: 6px 10px; border-radius: 7px; color: var(--muted); cursor: pointer; font: inherit; font-size: 13px; position: relative; }
  .toc button:hover { color: var(--ink); background: color-mix(in srgb, var(--line) 45%, transparent); }
  .toc button.on { color: var(--ink); font-weight: 600; }
  .toc button.on::before { content: ""; position: absolute; left: -2px; top: 8px; bottom: 8px; width: 3px; border-radius: 2px; background: var(--rec); }
  .toc button.z { margin-top: 12px; }
  .toc button.z.on::before { background: var(--bad); }
  /* narrow: one column, the parts list goes */
  @media (max-width: 900px) { .layout { grid-template-columns: minmax(0, 1fr); } .toc { display: none; } }

  .body { display: flex; flex-direction: column; gap: 38px; min-width: 0; }
  .group { scroll-margin-top: 16px; }
  .group h2 { margin: 0; font-size: 16px; }
  .gsub { margin: 3px 0 10px; }
  .group > .item:first-of-type { border-top: 1px solid var(--line); }

  /* a setting: what it does on the left, its control on the right (or below, for lists) */
  .item { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 10px 28px; align-items: start; padding: 16px 0; border-bottom: 1px solid var(--line); }
  .item.stack { grid-template-columns: minmax(0, 1fr); }
  .it h3 { margin: 0 0 3px; font-size: 13.5px; font-weight: 600; }
  .it p { margin: 0; max-width: 64ch; }
  .it .state { margin-top: 6px; color: var(--ink); }
  .item.live .state { color: var(--rec); }
  .seg { display: inline-flex; padding: 2px; border-radius: 9px; background: var(--panel2); border: 1px solid var(--line); }
  .seg button { border: 0; background: none; padding: 4px 11px; border-radius: 7px; cursor: pointer; color: var(--muted); font-size: 12.5px; transition: background-color .15s, color .15s; }
  .seg button.on { background: var(--panel); color: var(--ink); font-weight: 600; box-shadow: 0 1px 2px rgba(0, 0, 0, .12); }
  .publisher { margin: 8px 0 8px; color: var(--ink); }
  .opt { font-weight: 400; font-size: 11px; color: var(--faint); margin-left: 4px; }
  .ctl { display: flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end; padding-top: 2px; }
  .chk { display: inline-flex; align-items: center; gap: 6px; margin-top: 8px; cursor: pointer; }

  /* an on/off switch */
  .switch { position: relative; width: 40px; height: 22px; flex: none; cursor: pointer; margin-top: 2px; }
  .switch input { position: absolute; inset: 0; opacity: 0; margin: 0; cursor: pointer; z-index: 1; }
  .switch i { position: absolute; inset: 0; border-radius: 99px; background: var(--line); transition: background .2s; pointer-events: none; }
  .switch i::after { content: ""; position: absolute; top: 3px; left: 3px; width: 16px; height: 16px; border-radius: 50%; background: var(--panel); box-shadow: 0 1px 2px rgba(0, 0, 0, .25); transition: transform .2s; }
  .switch input:checked + i { background: var(--rec); }
  .switch input:checked + i::after { transform: translateX(18px); }
  .switch input:focus-visible + i { outline: 2px solid var(--accent); outline-offset: 2px; }

  .row { display: flex; flex-wrap: wrap; gap: 8px; }
  .row input { flex: 1; min-width: 200px; }
  textarea { width: 100%; background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; font: inherit; color: inherit; resize: vertical; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { border: 1px solid var(--line); background: var(--panel2); border-radius: 99px; padding: 3px 10px; cursor: pointer; }
  .chip:hover { border-color: var(--accent); }
  .chip.sure { border-color: color-mix(in srgb, var(--accent) 55%, var(--line)); }
  .chip .faint { font-size: 11px; }
  .it .warn { color: var(--lost); margin-top: 8px; }
  .btn:disabled { opacity: .5; cursor: default; }
  .btn.danger { border-color: color-mix(in srgb, var(--bad) 60%, var(--line)); color: var(--bad); }
  .btn.danger:hover { background: color-mix(in srgb, var(--bad) 12%, transparent); }

  /* starting over, set apart */
  .danger-zone { border: 1px solid color-mix(in srgb, var(--bad) 35%, var(--line)); border-radius: var(--radius); padding: 14px 18px 2px; }
  .danger-zone h2 { color: var(--bad); }
  .danger-zone .item { border: 0 !important; padding-top: 8px; }
  @media (prefers-reduced-motion: reduce) { .switch i, .switch i::after { transition: none; } }
</style>

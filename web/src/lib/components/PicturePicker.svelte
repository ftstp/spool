<script lang="ts" module>
  export type Candidate = { path: string; label: string };
</script>

<script lang="ts">
  // Choose a picture for something (an artist's photo, a release's cover): a file from the
  // disk (picked or dropped), or one the app already shows: the songs' covers, their mood
  // boards, the pictures in their folders. Nothing on disk is moved: a file from the disk is
  // copied into Spool's own data folder.
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let {
    title,
    current = null,
    candidates = [],
    songs = [],
    pick,
    upload,
    reset = null,
    close,
  }: {
    title: string;
    current?: string | null;
    /** Pictures offered first (the songs' covers, the board's pictures). */
    candidates?: Candidate[];
    /** Songs whose folder pictures are offered too (read from the Media list). */
    songs?: number[];
    pick: (path: string) => Promise<unknown>;
    upload: (f: File) => Promise<unknown>;
    reset?: { label: string; run: () => Promise<unknown> } | null;
    close: () => void;
  } = $props();

  let more = $state<Candidate[]>([]);
  let loading = $state(false);
  $effect(() => {
    if (!songs.length) return;
    const ids = new Set(songs);
    loading = true;
    api
      .mediaList()
      .then((l) => {
        more = l
          .filter((m) => ids.has(m.id))
          .flatMap((m) => m.items.filter((i) => i.kind === "image" || i.kind === "gif").map((i) => ({ path: i.rel, label: `${m.title ?? ""} · ${i.name}` })));
      })
      .catch(() => (more = []))
      .finally(() => (loading = false));
  });
  // each picture once, the offered ones first
  const all = $derived.by(() => {
    const seen = new Set<string>();
    return [...candidates, ...more].filter((c) => c.path && !seen.has(c.path) && seen.add(c.path)).slice(0, 120);
  });

  // a picture the webview cannot show (a .bmp, a .tiff) is not offered
  let failed = $state(new Set<string>());
  let busy = $state(false);
  let over = $state(false);
  let input = $state<HTMLInputElement>();
  async function run(p: Promise<unknown>) {
    if (busy) return;
    busy = true;
    try {
      await p;
      close();
    } catch (e) {
      app.say(String(e), { error: true });
    } finally {
      busy = false;
    }
  }
  function files(l: FileList | null | undefined) {
    const f = [...(l ?? [])].find((x) => x.type.startsWith("image/"));
    if (f) run(upload(f));
    else if (l?.length) app.say(t("picker.notImage"), { error: true });
  }
  const focus = (n: HTMLElement) => n.focus();
</script>

<div class="scrim" role="presentation" onclick={close}></div>
<div class="dlg card" role="dialog" aria-label={title} tabindex="-1" use:focus onkeydown={(e) => e.key === "Escape" && close()}
  ondragover={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); over = true; } }}
  ondragleave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) over = false; }}
  ondrop={(e) => { e.preventDefault(); over = false; files(e.dataTransfer?.files); }}>
  <h2>{title}</h2>
  <div class="drop" class:over>
    <button class="btn primary" disabled={busy} onclick={() => input?.click()}>{t("picker.fromDisk")}</button>
    <span class="small muted">{t("picker.dropHint")}</span>
  </div>
  <input bind:this={input} type="file" accept="image/*" hidden onchange={(e) => { files(e.currentTarget.files); e.currentTarget.value = ""; }} />
  {#if all.length}
    <h3 class="small muted">{t("picker.fromApp")}</h3>
    <div class="grid">
      {#each all.filter((c) => !failed.has(c.path)) as c (c.path)}
        <button class="pic" class:on={current === c.path} disabled={busy} title={c.label} onclick={() => run(pick(c.path))}>
          <img src={api.thumbUrl(c.path, 240)} alt={c.label} loading="lazy" onerror={() => (failed = new Set(failed).add(c.path))} />
        </button>
      {/each}
    </div>
  {:else if loading}<p class="small faint">{t("picker.loading")}</p>
  {:else}<p class="small faint">{t("picker.none")}</p>{/if}
  <div class="acts">
    {#if reset}<button class="btn" disabled={busy} onclick={() => reset && run(reset.run())}>{reset.label}</button>{/if}
    <span class="grow"></span>
    <button class="btn" onclick={close}>{t("common.cancel")}</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .45); z-index: 900; }
  .dlg { position: fixed; z-index: 901; left: 50%; top: 50%; transform: translate(-50%, -50%); width: min(720px, calc(100vw - 32px)); max-height: calc(100vh - 80px);
    display: flex; flex-direction: column; gap: 10px; overflow: hidden; outline: none; }
  h2 { margin: 0; font-size: 16px; }
  h3 { margin: 4px 0 0; font-weight: 500; }
  .drop { display: flex; gap: 12px; align-items: center; padding: 14px; border: 1px dashed var(--line); border-radius: 10px; transition: background-color .15s, border-color .15s; }
  .drop.over { border-color: var(--accent); background: var(--accent-bg); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); grid-auto-rows: 104px; gap: 8px; overflow-y: auto; min-height: 0; padding: 2px; align-content: start; }
  .pic { height: 100%; padding: 0; border: 2px solid transparent; border-radius: 8px; overflow: hidden; cursor: pointer; background: var(--panel2); }
  .pic:hover { border-color: var(--faint); }
  .pic.on { border-color: var(--accent); }
  .pic img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .acts { display: flex; gap: 8px; align-items: center; }
  .grow { flex: 1; }
</style>

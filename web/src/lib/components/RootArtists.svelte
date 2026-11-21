<script lang="ts">
  // The artist of each projects folder: every song of the folder whose folder names give no
  // artist gets this one. A folder named "Projets" / "Projects"... is guessed from the folder
  // above it ("C:\Users\NOVA\Kito Vale\Projets" -> "Kito Vale").
  import { untrack } from "svelte";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";

  let { roots }: { roots: { path: string; label: string; artist: string }[] } = $props();
  const GENERIC = ["projets", "projects", "projet", "project", "ableton", "musique", "music", "prod", "productions", "sons", "live", "sessions"];
  export function guess(path: string): string {
    const parts = path.split(/[\\/]+/).filter(Boolean);
    const leaf = parts.at(-1) ?? "";
    const norm = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();
    if (GENERIC.includes(norm(leaf)) && parts.length >= 2) {
      const up = parts.at(-2) ?? "";
      if (!/^[a-z]:$/i.test(up) && norm(up) !== "users" && norm(up) !== "documents") return up;
    }
    return "";
  }
  let values = $state<Record<string, string>>({});
  // follows the folders; what was typed is kept (read untracked: this effect writes it)
  $effect(() => {
    const v: Record<string, string> = {};
    const typed = untrack(() => values);
    for (const r of roots) v[r.path] = typed[r.path] ?? (r.artist || guess(r.path));
    values = v;
  });
  async function save(path: string) {
    const r = roots.find((x) => x.path === path);
    const want = (values[path] ?? "").trim();
    if (!r || want === r.artist) return;
    try {
      app.config = await api.setRootArtist(path, want);
    } catch (e) {
      app.say(String(e), { error: true });
    }
  }
  /** Saves every folder whose artist was typed or guessed but not saved yet. */
  export async function saveAll() {
    for (const r of roots) await save(r.path);
  }
</script>

{#if roots.length}
  <div class="ra">
    <div class="small muted">{t("app.rootArtists.intro")}</div>
    {#each roots as r (r.path)}
      <label class="row">
        <span class="p ell" title={r.path}>{r.path.split(/[\\/]/).filter(Boolean).slice(-2).join(" / ")}</span>
        <input type="text" placeholder={t("app.rootArtists.placeholder")} bind:value={values[r.path]} onblur={() => save(r.path)} onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()} />
        {#if r.artist && (values[r.path] ?? "") === r.artist}<span class="ok small">✓</span>{:else if guess(r.path) && !r.artist && values[r.path] === guess(r.path)}<span class="small faint">{t("app.rootArtists.guessed")}</span>{/if}
      </label>
    {/each}
  </div>
{/if}

<style>
  .ra { display: flex; flex-direction: column; gap: 6px; margin-top: 12px; }
  .row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(140px, 220px) 50px; gap: 10px; align-items: center; }
  .p { color: var(--muted); font-size: 12.5px; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ok { color: var(--ok); }
</style>

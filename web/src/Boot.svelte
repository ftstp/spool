<script lang="ts">
  // The start: shown at once, while the core opens the base on its own thread. Most starts
  // take a blink and show nothing here; an old catalogue being converted says so, with the
  // time going by, instead of a still window.
  import { onMount } from "svelte";
  import { bootStatus, type BootStatus } from "./lib/api";
  import { t } from "./lib/i18n/index.svelte";
  import Logo from "./lib/components/Logo.svelte";

  let { onready }: { onready: () => void } = $props();
  let s = $state<BootStatus | null>(null);

  onMount(() => {
    let stop = false;
    const tick = async () => {
      try {
        s = await bootStatus();
      } catch {
        /* the shell is not answering yet */
      }
      if (s?.stage === "ready") return onready();
      if (!stop) setTimeout(tick, s?.stage === "error" ? 1000 : 150);
    };
    tick();
    return () => (stop = true);
  });

  // a quick open shows nothing (no flash of a screen nobody can read)
  const shown = $derived(!!s && (s.stage === "upgrading" || s.stage === "error" || s.secs > 0.8));
  const secs = $derived(Math.floor(s?.secs ?? 0));
</script>

<main class="boot" aria-busy={s?.stage !== "error"}>
  {#if shown && s}
    <div class="card" role="status" aria-live="polite">
      <Logo size={40} />
      {#if s.stage === "upgrading"}
        <h1>{t("app.boot.upgradeTitle")}</h1>
        <p>{t("app.boot.upgrade", { mb: s.mb })}</p>
        <p class="muted small">{t("app.boot.safe")}</p>
      {:else if s.stage === "error"}
        <h1>{t("app.boot.errorTitle")}</h1>
        <p class="warn small">{s.error}</p>
      {:else}
        <h1>{t("app.boot.openTitle")}</h1>
        <p>{s.mb ? t("app.boot.open", { mb: s.mb }) : t("app.boot.create")}</p>
        {#if s.secs > 5}<p class="muted small">{t("app.boot.slow")}</p>{/if}
      {/if}
      {#if s.stage !== "error"}
        <div class="bar" aria-hidden="true"><i></i></div>
        <p class="faint small mono">{t("app.boot.elapsed", { s: secs })}</p>
      {/if}
    </div>
  {/if}
</main>

<style>
  .boot { position: fixed; inset: 0; display: grid; place-items: center; background: var(--bg); padding: 24px; }
  .card { width: min(460px, 100%); display: grid; justify-items: start; gap: 10px; }
  h1 { margin: 10px 0 0; font-size: 20px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 0; max-width: 60ch; text-wrap: pretty; }
  .warn { color: var(--bad); }
  .mono { font-family: var(--mono, monospace); font-variant-numeric: tabular-nums; }
  .bar { width: 100%; height: 3px; border-radius: 2px; background: var(--line); overflow: hidden; margin-top: 8px; }
  .bar i { display: block; width: 30%; height: 100%; background: var(--ink); border-radius: 2px; animation: slide 1.4s ease-in-out infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(340%); } }
  @media (prefers-reduced-motion: reduce) { .bar i { animation: none; width: 100%; opacity: 0.35; } }
</style>

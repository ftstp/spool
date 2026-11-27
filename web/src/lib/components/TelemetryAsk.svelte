<script lang="ts">
  // Asked once, at the start, to a user who finished the setup before this question existed
  // (new users answer it as the setup's last step). Two answers of the same weight, none
  // chosen in advance; closing (the cross, Escape) is a no. Nothing is sent before an answer.
  import { onMount } from "svelte";
  import { app } from "../state.svelte";
  import { t } from "../i18n/index.svelte";
  import Logo from "./Logo.svelte";

  let dlg = $state<HTMLDialogElement>();
  let done = false;
  onMount(() => {
    dlg?.showModal();
    // the window itself takes the focus, not one of the answers (Enter must not pick one)
    dlg?.focus();
  });
  async function choose(on: boolean) {
    if (done) return;
    done = true;
    try {
      await app.setTelemetry(on);
    } catch {
      /* not saved: asked again at the next start, nothing sent meanwhile */
    }
    dlg?.close();
  }
</script>

<dialog bind:this={dlg} class="ask" tabindex="-1" aria-labelledby="tel-title" aria-describedby="tel-body"
  oncancel={(e) => { e.preventDefault(); choose(false); }}>
  <button class="x" aria-label={t("app.telemetry.close")} title={t("app.telemetry.close")} onclick={() => choose(false)}>✕</button>
  <Logo size={28} />
  <h2 id="tel-title">{t("app.telemetry.title")}</h2>
  <p id="tel-body">{t("app.telemetry.body")}</p>
  <div class="two">
    <button class="btn" onclick={() => choose(true)}>{t("app.telemetry.share")}</button>
    <button class="btn" onclick={() => choose(false)}>{t("app.telemetry.dontShare")}</button>
  </div>
</dialog>

<style>
  .ask { width: min(480px, calc(100vw - 32px)); padding: 24px 26px 22px; border: 1px solid var(--line); border-radius: 14px; background: var(--panel); color: var(--ink);
    box-shadow: 0 30px 80px rgba(0, 0, 0, .35); outline: none; }
  .ask::backdrop { background: rgba(0, 0, 0, .45); }
  h2 { margin: 14px 0 8px; font-size: 19px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 0; color: var(--muted); max-width: 60ch; text-wrap: pretty; line-height: 1.5; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }
  .two .btn { padding: 9px 12px; font-weight: 600; }
  .x { position: absolute; top: 10px; right: 10px; border: 0; background: none; color: var(--faint); cursor: pointer; padding: 4px 7px; border-radius: 6px; }
  .x:hover { color: var(--ink); background: var(--panel2); }
</style>

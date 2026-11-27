<script lang="ts">
  // What a repair of one project would do, opened from the Repair page's list (the project's
  // name, or one of its counts: then at that part): plugins renamed, samples copied in,
  // plugins removed or replaced, and what stays missing. The page stays where it is.
  import * as api from "../api";
  import type { RepairPlan } from "../api";
  import { t } from "../i18n/index.svelte";

  let { rel, song, title, at, close }: { rel: string; song: number; title: string; at: string; close: () => void } = $props();

  type Full = RepairPlan & {
    renames: { from: string; to: string; file: string }[];
    copies: { path: string; found_at?: string; size?: number }[];
    removes: { name: string; format: string }[];
    manual?: { name: string; format: string }[];
    trials: { from: string; name?: string; to?: string; format: string }[];
    vst3?: { from: string; name?: string }[];
    unfixable?: { name: string; status: string }[];
  };
  let plan = $state<Full | null>(null);
  let failed = $state("");
  $effect(() => {
    plan = null;
    failed = "";
    api.repairPlan(rel).then((p) => (p ? (plan = p as Full) : (failed = t("repair.plan.none")))).catch((e) => (failed = String(e)));
  });
  // opened at the part asked ("renames", "copies"...), once it is drawn
  let body = $state<HTMLElement>();
  $effect(() => {
    if (plan && body && at) body.querySelector(`[data-part="${at}"]`)?.scrollIntoView({ block: "start" });
  });
  const file = (p: string) => p.replace(/\\/g, "/").split("/").pop() ?? p;
  const dir = (p: string) => p.replace(/\\/g, "/").split("/").slice(0, -1).join("/");
  const name = (r: string) => r.split("/").pop() ?? r;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<aside class="sheet" aria-label={t("repair.plan.aria", { title })}>
  <header>
    <div class="grow">
      <b>{title}</b>
      <div class="small faint ell" title={rel}>{name(rel)}</div>
    </div>
    <a class="btn sm" href="#/song/{song}">{t("repair.plan.openSong")}</a>
    <button class="x" onclick={close} aria-label={t("repair.plan.close")}>×</button>
  </header>
  <div class="body" bind:this={body}>
    {#if failed}<p class="small muted">{failed}</p>
    {:else if !plan}<p class="small muted">{t("repair.plan.loading")}</p>
    {:else}
      <p class="small" class:okc={plan.complete} class:muted={!plan.complete}>{plan.complete ? t("repair.plan.complete") : t("repair.plan.incomplete")}</p>
      {#if plan.renames.length}
        <section data-part="renames" class:hl={at === "renames"}>
          <h3>{t("repair.plan.renames", { n: plan.renames.length })}</h3>
          <p class="small faint">{t("repair.plan.renamesText")}</p>
          <ul>{#each plan.renames as r}<li><span>{r.from}</span> <span class="faint">→</span> <b>{r.to}</b> <span class="small faint">{r.file}</span></li>{/each}</ul>
        </section>
      {/if}
      {#if plan.copies.length}
        <section data-part="copies" class:hl={at === "copies"}>
          <h3>{t("repair.plan.copies", { n: plan.copies.length })}</h3>
          <p class="small faint">{t("repair.plan.copiesText")}</p>
          <ul>{#each plan.copies as c}<li><b>{file(c.path)}</b> <span class="small faint ell" title={c.found_at}>{t("repair.plan.foundIn", { dir: dir(c.found_at ?? "") })}</span></li>{/each}</ul>
        </section>
      {/if}
      {#if plan.removes.length}
        <section data-part="removes" class:hl={at === "removes"}>
          <h3>{t("repair.plan.removes", { n: plan.removes.length })}</h3>
          <p class="small faint">{t("repair.plan.removesText")}</p>
          <ul>{#each plan.removes as r}<li><b>{r.name}</b> <span class="fmt">{r.format.toUpperCase()}</span></li>{/each}</ul>
        </section>
      {/if}
      {#if plan.trials.length || plan.vst3?.length}
        <section data-part="trials" class:hl={at === "trials"}>
          <h3>{t("repair.plan.replaced", { n: plan.trials.length + (plan.vst3?.length ?? 0) })}</h3>
          <ul>
            {#each plan.trials as r}<li><span>{r.from}</span> <span class="faint">→</span> <b>{r.to ?? r.name ?? ""}</b> <span class="fmt">{r.format.toUpperCase()}</span></li>{/each}
            {#each plan.vst3 ?? [] as r}<li><span>{r.from}</span> <span class="faint">→</span> <b>{r.name ?? r.from}</b> <span class="fmt">VST3</span></li>{/each}
          </ul>
        </section>
      {/if}
      {#if plan.unfixable?.length || plan.manual?.length || plan.lost.length}
        <section data-part="left" class="left">
          <h3>{t("repair.plan.left")}</h3>
          <ul>
            {#each plan.unfixable ?? [] as u}<li><b>{u.name}</b> <span class="small faint">{t("repair.plan.notInstalled")}</span></li>{/each}
            {#each plan.manual ?? [] as u}<li><b>{u.name}</b> <span class="small faint">{t("repair.plan.byHand")}</span></li>{/each}
            {#each plan.lost.slice(0, 50) as l}<li><b>{file(l)}</b> <span class="small faint">{t("repair.plan.sampleLost")}</span></li>{/each}
          </ul>
          {#if plan.lost.length > 50}<p class="small faint">{t("repair.plan.more", { n: plan.lost.length - 50 })}</p>{/if}
        </section>
      {/if}
    {/if}
  </div>
</aside>

<style>
  .sheet { position: fixed; top: 0; right: 0; bottom: 0; width: min(460px, 100vw); z-index: 50; background: var(--panel); border-left: 1px solid var(--line); box-shadow: -12px 0 32px rgb(0 0 0 / .18); display: flex; flex-direction: column; animation: in .2s ease-out; }
  @keyframes in { from { transform: translateX(24px); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .sheet { animation: none; } }
  header { display: flex; align-items: center; gap: 10px; padding: 16px 16px 12px; border-bottom: 1px solid var(--line); }
  .grow { flex: 1; min-width: 0; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .x { border: 0; background: none; color: var(--muted); font-size: 20px; line-height: 1; padding: 2px 8px; border-radius: 6px; cursor: pointer; }
  .x:hover { color: var(--ink); background: var(--panel2); }
  .body { flex: 1; overflow: auto; padding: 12px 16px 24px; display: flex; flex-direction: column; gap: 14px; }
  section { border: 1px solid var(--line); border-radius: var(--radius); padding: 10px 12px; scroll-margin-top: 8px; transition: border-color .3s; }
  section.hl { border-color: var(--accent); }
  section.left { border-style: dashed; }
  h3 { margin: 0 0 4px; font-size: 14px; }
  p { margin: 0 0 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  li { display: flex; flex-wrap: wrap; gap: 4px 6px; align-items: baseline; min-width: 0; }
  li .ell { max-width: 100%; }
  .fmt { font-size: 10.5px; padding: 0 6px; border-radius: 5px; border: 1px solid var(--line); color: var(--muted); }
  .okc { color: var(--ok); }
</style>

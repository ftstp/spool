<script lang="ts">
  // A track's devices as Live's device view lays them out: left to right, each rack opened on
  // its chains, a Drum Rack on its pads. Read from the project, only shown.
  import type { DevView } from "../api";
  import Devices from "./Devices.svelte";
  import { t } from "../i18n/index.svelte";

  let { devices, nested = false }: { devices: DevView[]; nested?: boolean } = $props();
  const KINDS = ["instrument", "effect", "midi", "rack", "drums"];
  const kind = (k: string) => (KINDS.includes(k) ? t(`repair.devices.kind.${k}`) : undefined);
  // a Drum Rack's pads, lowest note first (as on Live's pad grid, read bottom up)
  const pads = (d: DevView) => [...d.chains];
</script>

<div class="row" class:nested>
  {#each devices as d, i (i)}
    {#if i > 0}<span class="arrow" aria-hidden="true">›</span>{/if}
    {#if d.kind === "drums" && d.chains.length}
      <div class="dev drums" class:off={!d.on}>
        <div class="dh"><b>{d.name}</b><span class="k">{t("repair.devices.pads", { n: d.chains.length })}</span></div>
        <div class="pads">
          {#each pads(d) as c, j (j)}
            {@const inst = c.devices.find((x) => x.kind === "instrument") ?? c.devices[0]}
            <div class="pad" title="{c.name}{c.devices.length ? ' : ' + c.devices.map((x) => x.name + (x.sample ? ' (' + x.sample + ')' : '')).join(' › ') : ''}">
              <span class="pn">{c.note ?? ""}</span>
              <span class="pt">{c.name || inst?.name || t("repair.devices.pad")}</span>
              {#if inst?.sample}<span class="ps">{inst.sample}</span>{:else if inst && inst.name !== c.name}<span class="ps">{inst.name}</span>{/if}
              {#if c.devices.length > 1}<span class="pfx">{t("repair.devices.moreFx", { n: c.devices.length - 1 })}</span>{/if}
            </div>
          {/each}
        </div>
      </div>
    {:else if d.chains.length}
      <div class="dev rack" class:off={!d.on}>
        <div class="dh"><b>{d.name}</b><span class="k">{t("repair.devices.chains", { n: d.chains.length })}</span></div>
        <div class="chains">
          {#each d.chains as c, j (j)}
            <div class="chain">
              <span class="cn">{c.name || t("repair.devices.chain", { n: j + 1 })}</span>
              {#if c.devices.length}<Devices devices={c.devices} nested />{:else}<span class="faint small">{t("repair.devices.empty")}</span>{/if}
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="dev {d.kind}" class:off={!d.on} class:dropped={d.dropped}
        title={d.dropped ? (d.on ? t("repair.devices.droppedMixTitle") : t("repair.devices.droppedOffTitle")) : d.plugin ? t("repair.devices.titlePlugin", { kind: kind(d.kind) ?? d.kind, plugin: d.plugin }) : t("repair.devices.titleLive", { kind: kind(d.kind) ?? d.kind })}>
        <b>{d.name}</b>
        <span class="k">{d.dropped ? (d.on ? t("repair.devices.droppedMix") : t("repair.devices.droppedOff")) : (d.plugin ?? kind(d.kind) ?? "")}</span>
        {#if d.sample}<span class="smp">{d.sample}</span>{/if}
      </div>
    {/if}
  {:else}
    <span class="faint small">{t("repair.devices.none")}</span>
  {/each}
</div>

<style>
  .row { display: flex; align-items: flex-start; gap: 4px; flex-wrap: wrap; }
  .row.nested { gap: 3px; }
  .arrow { align-self: flex-start; margin-top: 14px; color: var(--faint); font-size: 14px; }
  .dev { --c: var(--muted); display: flex; flex-direction: column; gap: 2px; min-width: 92px; max-width: 220px; padding: 6px 9px 7px; border-radius: 7px;
    background: var(--panel2); border: 1px solid var(--line); border-top: 3px solid var(--c); font-size: 12px; }
  .dev b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .k { font-size: 10.5px; color: var(--faint); }
  .smp { font-size: 10.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .instrument { --c: var(--accent); }
  .effect { --c: var(--ok); }
  .midi { --c: var(--lost); }
  .rack, .drums { --c: var(--fix); max-width: none; flex: 1 1 260px; }
  .off { opacity: .45; }
  .dropped { opacity: .5; border-style: dashed; border-top-style: dashed; --c: var(--faint); }
  .dropped b { text-decoration: line-through; text-decoration-color: var(--faint); }
  .off b { text-decoration: line-through; }
  .dh { display: flex; justify-content: space-between; gap: 10px; align-items: baseline; }
  .chains { display: flex; flex-direction: column; gap: 4px; margin-top: 4px; }
  .chain { display: grid; grid-template-columns: minmax(70px, 120px) minmax(0, 1fr); gap: 8px; align-items: center; padding: 4px 6px; border-radius: 6px; background: var(--bg); }
  .cn { font-size: 11.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nested .dev { min-width: 70px; padding: 3px 7px 4px; border-top-width: 2px; font-size: 11.5px; }
  .pads { display: grid; grid-template-columns: repeat(auto-fill, minmax(96px, 1fr)); gap: 4px; margin-top: 5px; }
  .pad { display: flex; flex-direction: column; gap: 1px; padding: 5px 7px; border-radius: 6px; background: var(--bg); border: 1px solid var(--line); min-width: 0; }
  .pn { font-size: 10px; color: var(--fix); font-weight: 600; }
  .pt { font-size: 11.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ps, .pfx { font-size: 10.5px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

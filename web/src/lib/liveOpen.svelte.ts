// "Open in Live", everywhere in the app: when Spool holds changes for the project that the set
// does not have yet (a repair ready: plugins renamed or replaced, samples found again; markers
// changed in the arrangement), it asks first which ones to carry. The ticked ones go into a copy
// written next to the project ("Name [RM].als", the original is never touched), which opens in
// Live. Nothing pending: the project opens at once, as before.
import * as api from "./api";
import { app } from "./state.svelte";
import { t } from "./i18n/index.svelte";

export type LiveChoice = { code: "R" | "M" | "P"; label: string; detail: string; on: boolean };

class LiveOpen {
  /** The project asked about, and what could go into its copy (null: no question open). */
  ask = $state<{ rel: string; name: string; choices: LiveChoice[]; marks: [number, string][] | null } | null>(null);
  busy = $state(false);

  async open(rel: string) {
    if (!api.inApp) return api.openInLive(rel).catch((e) => app.say(String(e), { error: true }));
    const [plan, marks] = await Promise.all([api.repairPlan(rel).catch(() => null), api.markersGet(rel).catch(() => null)]);
    const choices: LiveChoice[] = [];
    if (plan?.useful) {
      const parts = [
        plan.renames.length && t("liveOpen.renames", { n: plan.renames.length }),
        plan.copies.length && t("liveOpen.samples", { n: plan.copies.length }),
        plan.trials.length && t("liveOpen.trials", { n: plan.trials.length }),
        plan.removes.length && t("liveOpen.removes", { n: plan.removes.length }),
      ].filter(Boolean);
      choices.push({ code: "R", label: t("liveOpen.repair"), detail: parts.join(", ") || t("liveOpen.repairPlugins"), on: true });
    }
    if (marks) choices.push({ code: "M", label: t("liveOpen.markers"), detail: t("liveOpen.markersN", { n: marks.length }), on: true });
    if (!choices.length) return api.openInLive(rel).catch((e) => app.say(String(e), { error: true }));
    // offered with the rest, never alone: a cleanup nobody asked for is no reason to stop
    choices.push({ code: "P", label: t("liveOpen.cleanup"), detail: t("liveOpen.cleanupDetail"), on: false });
    const name = rel.split("/").pop()?.replace(/\.als$/i, "") ?? rel;
    this.ask = { rel, name, choices, marks };
  }

  /** The original, as it is. */
  original() {
    const q = this.ask;
    this.ask = null;
    if (q) api.openInLive(q.rel).catch((e) => app.say(String(e), { error: true }));
  }

  /** A copy with what is ticked, opened in Live. */
  async withChanges() {
    const q = this.ask;
    if (!q || this.busy) return;
    const on = (c: string) => q.choices.some((x) => x.code === c && x.on);
    if (!on("R") && !on("M") && !on("P")) return this.original();
    const preset: Record<string, unknown> = { name: t("liveOpen.presetName") };
    if (on("R")) preset.R = true;
    if (on("M") && q.marks) preset.M = { on: true, list: q.marks };
    if (on("P")) preset.P = { on: true, empty_tracks: true, disabled_devices: true, unused_returns: true, simplify_groups: false };
    this.busy = true;
    try {
      const out = await api.liveCopy(q.rel, preset);
      this.ask = null;
      app.say(t("liveOpen.written", { name: out.split("/").pop() ?? out }));
      await api.openInLive(out);
    } catch (e) {
      app.say(String(e), { error: true });
    } finally {
      this.busy = false;
    }
  }
}

export const liveOpen = new LiveOpen();

// The scan step in progress, in the user's words. The core names its steps in French (they
// double as ids, compared on both sides): matched here against the French names, shown in
// the interface's language.
import { t } from "./i18n/index.svelte";
import frApp from "./i18n/locales/fr/app.json";

const STEPS = frApp.scan.steps as Record<string, string>;
const OTHER = frApp.scan.other as Record<string, string>;

/** The step's key ("find", "read"...), or undefined for a stage the list does not know. */
export function scanStep(stage: string | undefined | null): string | undefined {
  return stage ? Object.keys(STEPS).find((k) => STEPS[k] === stage) : undefined;
}

export function scanStageText(stage: string | undefined | null): string {
  if (!stage) return "";
  const k = scanStep(stage);
  if (k) return t(`app.scan.steps.${k}`);
  const o = Object.keys(OTHER).find((x) => OTHER[x] === stage);
  return o ? t(`app.scan.other.${o}`) : stage;
}

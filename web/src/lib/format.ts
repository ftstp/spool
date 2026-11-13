// Small display helpers shared by every page. Words come from the locale files (lib/i18n).
import { t, ago } from "./i18n/index.svelte";

export const STAGES = ["draft", "arrangement", "mix", "master", "done", "pause"] as const;
/** A stage's name in the user's language ("none": not sorted yet). */
export const stageLabel = (s?: string | null) => t(`common.stage.${s || "none"}`);
export const bpm = (n?: number | null) => (n ? t("common.bpm", { n: Math.round(n * 10) / 10 }) : "");
export const dur = (s?: number | null) => (s ? `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}` : "");
export { ago };
/** A stable hue per title, for the colour of songs without a picture. */
export const hue = (s: string) => [...String(s)].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 360, 7);
/** @deprecated French only: use t() with a {"one", "other"} entry in the locale files. */
export const plural = (n: number, one: string, many?: string) => `${n} ${n > 1 ? many ?? one + "s" : one}`;

// Spool's words in each language. Every text the user reads comes from the locale files,
// never from a component: locales/<lang>/<namespace>.json (fr and en). The Rust side reads
// the SAME files (namespace "core", crates/i18n), so a translator edits JSON only.
//
//   t("repair.title")                          a text
//   t("repair.copies", { n: 3 })               {"one": "{n} copie", "other": "{n} copies"}
//   t("song.moved", { name, into })            "{name}" replaced
//   fmtDate(ts, { day: "numeric", month: "short" }), fmtNum(n), ago(ts): in the user's language
//
// A key missing in English falls back to French, then shows the key itself (never a crash).
// The language: Settings > Language, else the system's (French if it is French, else English).

type Dict = Record<string, unknown>;
export type Lang = "fr" | "en";
export const LANGS: [Lang, string][] = [["fr", "Français"], ["en", "English"]];

const files = import.meta.glob("./locales/*/*.json", { eager: true, import: "default" }) as Record<string, Dict>;
const dicts: Record<string, Dict> = { fr: {}, en: {} };
for (const [path, data] of Object.entries(files)) {
  const m = path.match(/\.\/locales\/(\w+)\/(\w+)\.json$/);
  if (m) (dicts[m[1]] ??= {})[m[2]] = data;
}

function detect(): Lang {
  try {
    const saved = localStorage.getItem("spool:lang");
    if (saved === "fr" || saved === "en") return saved;
  } catch {
    /* not remembered */
  }
  return (navigator.language || "").toLowerCase().startsWith("fr") ? "fr" : "en";
}

class I18n {
  lang = $state<Lang>(detect());
  /** For Intl (dates, numbers, sorting). */
  get tag() {
    return this.lang === "fr" ? "fr-FR" : "en-GB";
  }
  set(l: Lang) {
    this.lang = l;
    try {
      localStorage.setItem("spool:lang", l);
    } catch {
      /* not remembered */
    }
    document.documentElement.lang = l;
  }
}
export const i18n = new I18n();

function find(d: Dict, key: string): unknown {
  return key.split(".").reduce<unknown>((v, k) => (v && typeof v === "object" ? (v as Dict)[k] : undefined), d);
}

const rules: Record<string, Intl.PluralRules> = {};

/** The text of `key` in the current language, `{name}` replaced by `vars`. */
export function t(key: string, vars?: Record<string, string | number | null | undefined>): string {
  const lang = i18n.lang; // read first: a template using t() follows a change of language
  let v = find(dicts[lang], key);
  if (v === undefined && lang !== "fr") v = find(dicts.fr, key);
  if (v && typeof v === "object") {
    const n = Number(vars?.n ?? 0);
    const cat = (rules[lang] ??= new Intl.PluralRules(lang)).select(n);
    const o = v as Record<string, string>;
    v = o[cat] ?? o.other;
  }
  if (typeof v !== "string") return key;
  return vars ? v.replace(/\{(\w+)\}/g, (m, k) => (vars[k] === undefined || vars[k] === null ? m : String(vars[k]))) : v;
}

/** A date in the user's language. `t` in seconds (as the core sends them) or a Date. */
export function fmtDate(t: number | Date, opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short", year: "numeric" }) {
  return (t instanceof Date ? t : new Date(t * 1000)).toLocaleDateString(i18n.tag, opts);
}

export function fmtTime(t: number | Date, opts: Intl.DateTimeFormatOptions = { hour: "2-digit", minute: "2-digit" }) {
  return (t instanceof Date ? t : new Date(t * 1000)).toLocaleTimeString(i18n.tag, opts);
}

export function fmtNum(n: number, opts?: Intl.NumberFormatOptions) {
  return n.toLocaleString(i18n.tag, opts);
}

/** Comparing names the way the user's language sorts them. */
export function compare(a: string, b: string) {
  return a.localeCompare(b, i18n.lang, { numeric: true });
}

/** "today", "3 days ago"... `t` in seconds. */
export function ago(ts?: number | null): string {
  if (!ts) return "";
  const d = (Date.now() / 1000 - ts) / 86400;
  if (d < 1) return t("common.ago.today");
  if (d < 2) return t("common.ago.yesterday");
  if (d < 31) return t("common.ago.days", { n: Math.floor(d) });
  if (d < 365) return t("common.ago.months", { n: Math.floor(d / 30) });
  return t("common.ago.years", { n: Math.floor(d / 365) });
}

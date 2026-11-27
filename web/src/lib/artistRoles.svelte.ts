// The Artists page's one question about a name: who is it? Every name the folders carry is
// one of four things, and nothing else decides how songs are filed under artists:
// - one of the user's own names (its songs are its own);
// - the user under another name (read as one of their names: a merge);
// - a guest (featured: the song stays the user's, the name goes into its featurings);
// - not an artist (a word of the folder names: left in the title).
// Asked from the Artists page (cards, names to sort) and from an artist's own page; the
// answer changes at once how every song is filed, and one Ctrl+Z puts it back.
import * as api from "./api";
import type { ArtistRole } from "./api";
import type { Config } from "./api/bindings/Config";
import { app } from "./state.svelte";
import { openMenu, type MenuItem } from "./menu.svelte";
import { t } from "./i18n/index.svelte";

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** The user's own names: the ones said to be theirs, and each projects folder's artist. */
export function myNames(cfg: Config | null): string[] {
  const out: string[] = [];
  for (const a of [...(cfg?.artists ?? []), ...(cfg?.roots ?? []).map((r) => r.artist)]) {
    if (a.trim() && !out.some((x) => same(x, a))) out.push(a.trim());
  }
  return out;
}

/** What a name is now ("unset": never answered: a guest when it is only featured, else a
 *  name typed on a song by hand). `of`: the name it is read as, for an alias. */
export function roleOf(cfg: Config | null, name: string): { role: ArtistRole | "unset"; of?: string } {
  if (myNames(cfg).some((x) => same(x, name))) return { role: "me" };
  const al = Object.entries(cfg?.artist_aliases ?? {}).find(([k]) => same(k, name));
  if (al) return { role: "alias", of: al[1] };
  if (Object.keys(cfg?.artist_feats ?? {}).some((k) => same(k, name))) return { role: "guest" };
  if ((cfg?.artist_ignored ?? []).some((k) => same(k, name))) return { role: "none" };
  return { role: "unset" };
}

/** Other names read as this one (its aliases). */
export const aliasesOf = (cfg: Config | null, name: string) =>
  Object.entries(cfg?.artist_aliases ?? {})
    .filter(([, v]) => same(v, name))
    .map(([k]) => k);

/** The answer given: every song filed again, said with an undo. */
export async function setRole(name: string, role: ArtistRole, target = "") {
  try {
    app.undo = await api.artistRole(name, role, target);
    app.say(t(`artists.who.done.${role}`, { name, target }), { undo: true });
    await app.reload();
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

/** The question as a menu (a card's button or right-click, an artist's page). */
export function whoItems(name: string): MenuItem[] {
  const cfg = app.config;
  const cur = roleOf(cfg, name);
  const mine = myNames(cfg).filter((x) => !same(x, name));
  return [
    { label: t("artists.who.me"), checked: cur.role === "me", run: () => setRole(name, "me") },
    mine.length
      ? { label: t("artists.who.alias"), checked: cur.role === "alias", sub: mine.map((m) => ({ label: m, checked: cur.role === "alias" && !!cur.of && same(cur.of, m), run: () => setRole(name, "alias", m) })) }
      : { label: t("artists.who.alias"), disabled: true },
    { label: t("artists.who.guest"), checked: cur.role === "guest", run: () => setRole(name, "guest") },
    { sep: true },
    { label: t("artists.who.none"), checked: cur.role === "none", run: () => setRole(name, "none") },
  ];
}

export function whoMenu(e: MouseEvent, name: string, anchor = false) {
  if (anchor) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu({ preventDefault() {}, stopPropagation() {}, clientX: r.left, clientY: r.bottom + 4 } as MouseEvent, whoItems(name), t("artists.who.title", { name }));
  } else openMenu(e, whoItems(name), t("artists.who.title", { name }));
}

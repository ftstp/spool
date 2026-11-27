// The favourites (crates/core favorites.rs): read once, kept here, starred and unstarred from
// any page (FavStar), listed on the Home page and filtered on the lists.
import * as api from "./api";
import type { Favorite } from "./api";
import { focus } from "./focus.svelte";

export const favs = $state<{ list: Favorite[]; loaded: boolean }>({ list: [], loaded: false });
let loading: Promise<void> | null = null;

export function loadFavs() {
  loading ??= api.favorites().then((l) => {
    favs.list = l;
    favs.loaded = true;
  }).catch(() => { loading = null; });
  return loading;
}

const key = (kind: string, id: string) => `${kind}\u0000${id}`;
const index = $derived(new Set(favs.list.map((f) => key(f.kind, f.id))));
export const isFav = (kind: string, id: string) => {
  void loadFavs();
  return index.has(key(kind, id));
};
export const favsOf = (kind: string) => favs.list.filter((f) => f.kind === kind);

export async function toggleFav(f: Omit<Favorite, "at">) {
  const on = !isFav(f.kind, f.id);
  favs.list = on ? [{ ...f, at: Date.now() / 1000 }, ...favs.list] : favs.list.filter((x) => !(x.kind === f.kind && x.id === f.id));
  try {
    favs.list = await api.setFavorite({ ...f, at: Date.now() / 1000 }, on);
  } catch {
    loading = null;
    await loadFavs();
  }
}

/** Where a favourite opens. */
export function openFav(f: Favorite) {
  switch (f.kind) {
    case "song":
      location.hash = `#/song/${f.id}`;
      break;
    case "artist":
      location.hash = `#/artist/${encodeURIComponent(f.id)}`;
      break;
    case "release":
      location.hash = `#/release/${f.id}`;
      break;
    case "sample":
      location.hash = "#/samples?fav=1";
      break;
    case "midi":
      // a project's clip: its song; a file: the MIDI page
      location.hash = f.song != null ? `#/song/${f.song}` : "#/midi";
      break;
    case "board":
    case "project":
      if (f.song != null) location.hash = `#/song/${f.song}`;
      break;
    case "plugin":
      focus.plugin = f.id;
      location.hash = "#/repair";
      break;
  }
}

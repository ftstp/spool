// App-wide state: the data shown, reloaded when the core says something changed, and what
// the user can undo.
import { untrack } from "svelte";
import * as api from "./api";
import { i18n, t, type Lang } from "./i18n/index.svelte";
import type { Config, Overview, Release, ScanState, SongItem, Stats, UndoState } from "./api";

class AppState {
  songs = $state<SongItem[]>([]);
  /** EP, LP... (the next to come out first). */
  releases = $state<Release[]>([]);
  overview = $state<Overview | null>(null);
  stats = $state<Stats | null>(null);
  scan = $state<ScanState | null>(null);
  root = $state<string | null>(null);
  /** Folders and preferences; `setup_done` false keeps the first-run setup in front. */
  config = $state<Config | null>(null);
  query = $state("");
  loaded = $state(false);
  /** Bumped after every reload: an open song sheet fetches itself again. */
  version = $state(0);
  undo = $state<UndoState>({ label: null, left: 0, files: false });
  /** When a Ctrl+Z that moves files back was asked once (a second press within 8 s confirms). */
  #undoArmed = 0;
  /** Files may move on disk (Settings > Physical arranging). Off: everything stays in place. */
  get physical() {
    return !!this.config?.physical;
  }
  /** A short message at the bottom of the window (what was done, or why it was refused). */
  toast = $state<{ text: string; undo: boolean; error: boolean } | null>(null);
  #toastTimer = 0;
  /** Work under way whose result changes the numbers on screen (a reload, a recount): the
   * sidebar shows a spinner meanwhile. */
  busy = $state(0);

  /** Counts `p` as work under way until it settles. */
  async track<T>(p: Promise<T>): Promise<T> {
    // untracked: an effect that starts work must not rerun because the counter moved
    untrack(() => this.busy++);
    try {
      return await p;
    } finally {
      untrack(() => this.busy--);
    }
  }

  #reloading: Promise<void> | null = null;
  #again = false;
  /** One reload at a time: changes that come in during one ask for a single more, not one
   * each (a burst of changes used to start as many full reloads, the older able to land
   * after the newer). Resolves once what was asked is on screen. */
  reload() {
    if (this.#reloading) {
      this.#again = true;
      return this.#reloading;
    }
    this.#reloading = this.track(
      (async () => {
        try {
          do {
            this.#again = false;
            await this.#reload();
          } while (this.#again);
        } finally {
          this.#reloading = null;
        }
      })(),
    );
    return this.#reloading;
  }

  async #reload() {
    const [songs, overview, stats, root, config, releases] = await Promise.all([
      api.songs(), api.overview(), api.stats(), api.root(), api.config(), api.releases().catch(() => [] as Release[]),
    ]);
    this.releases = releases;
    this.config = config;
    this.songs = songs;
    this.overview = overview;
    this.stats = stats;
    this.root = root;
    this.loaded = true;
    this.version++;
  }

  async start() {
    this.scan = await api.scanState();
    await api.onChanged((s) => {
      const changed = !this.scan || s.version !== this.scan.version;
      this.scan = s;
      if (changed) this.reload();
    });
    this.undo = await api.undoState();
    await this.reload();
    // the core writes its messages in the interface's language: tell it which, once
    if (this.config && this.config.lang !== i18n.lang) this.setLang(i18n.lang);
  }

  /** Settings > Language: the interface at once, the core's messages from the next one. */
  async setLang(l: Lang) {
    i18n.set(l);
    try {
      this.config = await api.setConfig("lang", l);
    } catch {
      /* the browser preview: the interface alone */
    }
  }

  say(text: string, opts: { undo?: boolean; error?: boolean } = {}) {
    this.toast = { text, undo: !!opts.undo, error: !!opts.error };
    clearTimeout(this.#toastTimer);
    this.#toastTimer = window.setTimeout(() => (this.toast = null), opts.error ? 6000 : 4500);
  }

  /** Runs a change; the base tells everyone through the "changed" event. */
  async act(p: Promise<UndoState>) {
    try {
      this.undo = await p;
      if (this.undo.label) this.say(this.undo.label, { undo: true });
    } catch (e) {
      this.say(String(e), { error: true });
    }
  }

  /** A file operation: its result, then what Ctrl+Z would undo, said with an Annuler button. */
  async fileAct<T>(p: Promise<T>): Promise<T | null> {
    try {
      const r = await p;
      this.undo = await api.undoState();
      if (this.undo.label) this.say(this.undo.label, { undo: true });
      return r;
    } catch (e) {
      this.say(String(e), { error: true });
      return null;
    }
  }

  async undoLast() {
    try {
      // a change that moved files: its undo moves them back on disk, so it is asked twice
      this.undo = await api.undoState();
      if (this.undo.files && Date.now() - this.#undoArmed > 8000) {
        this.#undoArmed = Date.now();
        this.say(t("app.undo.confirmFiles", { label: this.undo.label }), { undo: true });
        return;
      }
      this.#undoArmed = 0;
      const [label, st] = await api.undo();
      this.undo = st;
      this.say(label ? t("app.undo.done", { label }) : t("app.top.nothingToUndo"));
    } catch (e) {
      this.say(String(e), { error: true });
    }
  }
}

export const app = new AppState();

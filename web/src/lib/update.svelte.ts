// Updates: Spool asks its download server (artists.fatstep.com/spool/download/latest.json)
// a little after it starts, then every 6 hours. A newer version is only OFFERED: nothing is
// installed until the user clicks, and never during a scan or a repair. The download is
// checked against Spool's signing key before anything runs.
import * as api from "./api";
import type { UpdateInfo } from "./api";
import { app } from "./state.svelte";
import { t } from "./i18n/index.svelte";

class Updates {
  /** The newer version on offer (null: up to date, or not asked yet). */
  info = $state<UpdateInfo | null>(null);
  current = $state("");
  checking = $state(false);
  installing = $state(false);
  /** "Plus tard": hidden until the next launch. */
  later = $state(false);
  /** When it was last asked (for Settings), and why it failed if it did. */
  at = $state<number | null>(null);
  error = $state("");
  #timer = 0;

  start() {
    api.appVersion().then((v) => (this.current = v)).catch(() => {});
    // not at once: the first seconds are the scan's
    window.setTimeout(() => this.check(), 20_000);
    this.#timer = window.setInterval(() => this.check(), 6 * 3600_000);
  }

  stop() {
    clearInterval(this.#timer);
  }

  /** `loud`: asked by hand (Settings), so "up to date" and failures are said. */
  async check(loud = false) {
    if (this.checking || this.installing) return;
    this.checking = true;
    try {
      this.info = await api.updateCheck();
      this.error = "";
      if (loud) app.say(this.info ? t("app.update.available", { version: this.info.version }) : t("app.update.upToDate"));
    } catch (e) {
      // offline, server down: silent unless asked
      this.error = String(e);
      if (loud) app.say(t("app.update.checkFailed", { error: String(e) }), { error: true });
    } finally {
      this.at = Date.now();
      this.checking = false;
    }
  }

  async install() {
    if (app.scan?.running) return app.say(t("app.update.scanRunning"), { error: true });
    this.installing = true;
    try {
      // downloads, checks the signature, runs the installer: Spool closes and comes back
      await api.updateInstall();
    } catch (e) {
      this.installing = false;
      app.say(String(e), { error: true });
    }
  }
}

export const updates = new Updates();

// The diagnostic report, written when the user asks: the file shown in the explorer, for
// them to send (nothing is sent by the app).
import * as api from "./api";
import { app } from "./state.svelte";
import { t } from "./i18n/index.svelte";

export async function makeReport() {
  try {
    const p = await api.diagnosticReport();
    app.say(t("app.report.done"));
    await api.reveal(p);
  } catch (e) {
    app.say(String(e), { error: true });
  }
}

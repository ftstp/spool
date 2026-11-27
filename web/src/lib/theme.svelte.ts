// Light or dark: Settings > Spool > Appearance. "system" follows Windows; "light" and "dark"
// force one, through data-theme on <html> (app.css holds both sets of colours). The window's
// title bar follows too.
export type Theme = "system" | "light" | "dark";
export const THEMES: Theme[] = ["system", "light", "dark"];

function saved(): Theme {
  try {
    const v = localStorage.getItem("spool:theme");
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* not remembered */
  }
  return "system";
}

function apply(t: Theme) {
  const root = document.documentElement;
  if (t === "system") delete root.dataset.theme;
  else root.dataset.theme = t;
  // the native title bar (only in the app; the browser preview has none)
  if ("__TAURI_INTERNALS__" in window) {
    import("@tauri-apps/api/window")
      .then((m) => m.getCurrentWindow().setTheme(t === "system" ? null : t))
      .catch(() => {});
  }
}

class ThemeState {
  value = $state<Theme>(saved());
  set(t: Theme) {
    this.value = t;
    try {
      localStorage.setItem("spool:theme", t);
    } catch {
      /* not remembered */
    }
    apply(t);
  }
  /** At start, before the first paint. */
  init() {
    apply(this.value);
  }
}
export const theme = new ThemeState();

// A folding section's title bar: a click anywhere on it folds or unfolds the section, except
// on the buttons, menus and fields it holds (they do their own thing).
export function onFoldHead(e: MouseEvent, toggle: () => void) {
  const t = e.target as HTMLElement | null;
  if (t?.closest("button, select, input, textarea, a, label, [role=radiogroup], [role=button]")) return;
  toggle();
}

// Files dropped from Explorer onto a part of the page. The window takes the system's drops
// itself (the page's own `drop` event never gets the files), with the pointer's position:
// the part under it takes them.
import * as api from "./api";
import type { BoardDrop } from "./api/bindings/BoardDrop";
import { t } from "./i18n/index.svelte";

/** `el()` takes the files dropped on it; `over` says whether files are held over it.
 *  Returns what stops listening. */
export function dropZone(el: () => HTMLElement | null | undefined, over: (on: boolean) => void, drop: (paths: string[]) => void): () => void {
  let off: (() => void) | undefined;
  let gone = false;
  const inside = (x: number, y: number) => {
    const e = el();
    const r = e?.offsetParent ? e.getBoundingClientRect() : null;
    return !!r && r.width > 0 && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  };
  api
    .onFileDrop((k, paths, x, y) => {
      const on = k !== "leave" && inside(x, y);
      over(k === "over" && on);
      if (k === "drop" && on && paths.length) drop(paths);
    })
    .then((f) => (gone ? f() : (off = f)));
  return () => {
    gone = true;
    off?.();
  };
}

/** What became of the files dropped on a board, said in one line. */
export function dropMessage(r: BoardDrop): { text: string; error: boolean } {
  const parts = [
    r.linked.length && t("song.board.linkedN", { n: r.linked.length }),
    r.copied.length && t("song.board.copiedN", { n: r.copied.length }),
    r.refused.length && t("song.board.refusedN", { n: r.refused.length, names: r.refused.slice(0, 3).join(", ") }),
  ].filter(Boolean);
  return { text: parts.join(" · "), error: !r.linked.length && !r.copied.length };
}

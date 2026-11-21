// Drag and drop inside the window, by pointer events.
// Spool keeps Tauri's native drop handler, so files dropped from Explorer reach the app with
// their paths (a song's board, the reference list). On Windows that handler also swallows the
// webview's HTML5 drag events: a song dragged onto a release, a card onto a stage, a track
// onto another never dropped. Every drag that stays inside the window goes through here.
// A source: `use:dragSource={item}`; a target: `use:dropTarget={{ accept, over, leave, drop }}`.
// A drag starts past a few pixels (a plain click stays a click), Escape cancels it, and a
// list under the pointer scrolls when the pointer nears its top or bottom edge.

export type DragItem = { kind: string; data: unknown; label: string };
export type DropTarget = {
  accept: (it: DragItem) => boolean;
  /** Called on every move over the target (the pointer position decides before/after). */
  over?: (it: DragItem, e: PointerEvent, el: HTMLElement) => void;
  leave?: () => void;
  drop: (it: DragItem, e: PointerEvent, el: HTMLElement) => void;
};

/** What is being dragged (null: nothing), for the styles of sources and targets. */
export const pdrag = $state<{ item: DragItem | null }>({ item: null });

const targets = new WeakMap<Element, () => DropTarget>();

export function dropTarget(node: HTMLElement, t: DropTarget) {
  let cur = t;
  targets.set(node, () => cur);
  return {
    update(n: DropTarget) {
      cur = n;
    },
    destroy() {
      targets.delete(node);
    },
  };
}

function targetAt(x: number, y: number, it: DragItem): { el: HTMLElement; t: DropTarget } | null {
  let el = document.elementFromPoint(x, y) as HTMLElement | null;
  while (el) {
    const g = targets.get(el);
    if (g) {
      const t = g();
      if (t.accept(it)) return { el, t };
    }
    el = el.parentElement;
  }
  return null;
}

function scroller(x: number, y: number): HTMLElement | null {
  let el = document.elementFromPoint(x, y) as HTMLElement | null;
  while (el && el !== document.body) {
    const s = getComputedStyle(el);
    if (/(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 2) return el;
    el = el.parentElement;
  }
  return null;
}

/** The click a drag ends on (the browser still sends it) must not open the link. */
function eatClick() {
  const stop = (e: Event) => {
    e.preventDefault();
    e.stopPropagation();
  };
  window.addEventListener("click", stop, { capture: true, once: true });
  setTimeout(() => window.removeEventListener("click", stop, { capture: true }), 0);
}

export function dragSource(node: HTMLElement, item: DragItem | null) {
  let it = item;
  // no native drag of the link or picture: it would take the pointer away from us
  const native = (e: DragEvent) => e.preventDefault();
  node.addEventListener("dragstart", native);
  if (node instanceof HTMLAnchorElement || node instanceof HTMLImageElement) node.draggable = false;

  function down(e: PointerEvent) {
    if (e.button !== 0 || !it || e.ctrlKey || e.metaKey || e.shiftKey) return;
    // a button inside the source (play, close) stays a button
    if ((e.target as Element).closest("button, input, textarea, select, [contenteditable]")) return;
    const cur = it;
    const sx = e.clientX, sy = e.clientY;
    let started = false;
    let ghost: HTMLDivElement | null = null;
    let at: { el: HTMLElement; t: DropTarget } | null = null;
    let px = sx, py = sy, raf = 0;
    const noSelect = (ev: Event) => ev.preventDefault();
    window.addEventListener("selectstart", noSelect);

    const tick = () => {
      const s = scroller(px, py);
      if (s) {
        const r = s.getBoundingClientRect();
        const edge = Math.min(48, r.height / 4);
        if (py < r.top + edge) s.scrollTop -= Math.ceil((r.top + edge - py) / 4);
        else if (py > r.bottom - edge) s.scrollTop += Math.ceil((py - (r.bottom - edge)) / 4);
      }
      raf = requestAnimationFrame(tick);
    };
    const move = (ev: PointerEvent) => {
      px = ev.clientX;
      py = ev.clientY;
      if (!started) {
        if (Math.hypot(px - sx, py - sy) < 6) return;
        started = true;
        pdrag.item = cur;
        window.getSelection()?.removeAllRanges();
        ghost = document.createElement("div");
        ghost.className = "pdrag-ghost";
        ghost.textContent = cur.label;
        document.body.appendChild(ghost);
        document.documentElement.classList.add("pdragging");
        raf = requestAnimationFrame(tick);
      }
      ghost!.style.transform = `translate(${px + 14}px, ${py + 10}px)`;
      const hit = targetAt(px, py, cur);
      if (hit?.el !== at?.el) at?.t.leave?.();
      at = hit;
      at?.t.over?.(cur, ev, at.el);
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("selectstart", noSelect);
      cancelAnimationFrame(raf);
      ghost?.remove();
      document.documentElement.classList.remove("pdragging");
      pdrag.item = null;
    };
    const up = (ev: PointerEvent) => {
      end();
      if (!started) return;
      eatClick();
      if (at) {
        at.t.leave?.();
        at.t.drop(cur, ev, at.el);
      }
    };
    const cancel = () => {
      at?.t.leave?.();
      end();
    };
    const key = (ev: KeyboardEvent) => {
      if (ev.key !== "Escape") return;
      ev.preventDefault();
      ev.stopPropagation();
      cancel();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", key, true);
  }
  node.addEventListener("pointerdown", down);
  return {
    update(n: DragItem | null) {
      it = n;
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("dragstart", native);
    },
  };
}

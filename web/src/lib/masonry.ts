// A grid laid out as a wall (masonry): its rows are a few pixels high and each child spans as
// many as its own height (and one gap), so with `grid-auto-flow: dense` a card goes up into
// the first place it fits, under the shortest column, instead of waiting for the row's tallest.
// Children are measured as they change (opened, folded, loaded) and as they come and go.
// The grid's column count is written on it (`data-cols`), for cards spanning two.
/** `min`: the columns' least width, as the grid's CSS says it. */
export function masonry(node: HTMLElement, min = 380) {
  const fit = (el: Element) => {
    const pos = getComputedStyle(el).position;
    // out of the flow (a lightbox, a drop zone): no rows
    if (pos === "fixed" || pos === "absolute") return;
    const st = getComputedStyle(node);
    const row = parseFloat(st.gridAutoRows) || 2;
    const gap = parseFloat(st.columnGap) || 14;
    const h = el.getBoundingClientRect().height;
    (el as HTMLElement).style.gridRowEnd = `span ${Math.max(1, Math.ceil((h + gap) / row))}`;
  };
  // how many columns it has now, for the cards spanning two (none when there is one)
  // (from its width, as `repeat(auto-fill, minmax(<min>, 1fr))` makes them: the tracks it
  // reports count the extra one a spanning card adds, and kept it at two in one column's room)
  const cols = () => {
    const gap = parseFloat(getComputedStyle(node).columnGap) || 14;
    const n = String(Math.max(1, Math.floor((node.clientWidth + gap) / (min + gap))));
    if (node.dataset.cols !== n) node.dataset.cols = n;
  };
  const ro = new ResizeObserver((es) => {
    for (const e of es) {
      if (e.target === node) cols();
      else fit(e.target);
    }
  });
  ro.observe(node);
  cols();
  const watch = () => {
    for (const c of node.children) ro.observe(c);
  };
  const mo = new MutationObserver(watch);
  mo.observe(node, { childList: true });
  watch();
  return {
    destroy() {
      ro.disconnect();
      mo.disconnect();
    },
  };
}

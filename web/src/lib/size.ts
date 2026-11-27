// An element's size, watched by its own observer let go with the element. Svelte's
// bind:clientWidth / bind:offsetHeight go through observers shared by the whole page, whose
// record of the last sizes (a WeakMap whose entries point back at their element) kept every
// song sheet left alive in memory, its data with it (40 MB and 20 000 elements).
function watch(node: HTMLElement, read: (n: HTMLElement) => number, set: (v: number) => void) {
  let last = -1;
  const tell = () => {
    const v = read(node);
    if (v !== last) set((last = v));
  };
  const ro = new ResizeObserver(tell);
  ro.observe(node);
  tell();
  return {
    update: (s: (v: number) => void) => (set = s),
    destroy: () => ro.disconnect(),
  };
}
/** The element's clientWidth, as bind:clientWidth. */
export const widthOf = (node: HTMLElement, set: (w: number) => void) => watch(node, (n) => n.clientWidth, set);
/** The element's clientHeight, as bind:clientHeight. */
export const heightOf = (node: HTMLElement, set: (h: number) => void) => watch(node, (n) => n.clientHeight, set);
/** The element's offsetHeight (borders included), as bind:offsetHeight. */
export const outerHeightOf = (node: HTMLElement, set: (h: number) => void) => watch(node, (n) => n.offsetHeight, set);

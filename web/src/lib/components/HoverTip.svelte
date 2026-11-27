<script module lang="ts">
  // `title` never reaches the page: written, it becomes `data-tip`; removed (a value gone to
  // nothing), `data-tip` goes too. Done where the page writes it (the attribute and the
  // `title` property, which Svelte uses), from the start: the watcher below alone missed a
  // title removed after it was turned, and left the old text showing.
  if (typeof Element !== "undefined" && !(Element.prototype as unknown as { __tip?: boolean }).__tip) {
    const set = Element.prototype.setAttribute;
    const remove = Element.prototype.removeAttribute;
    (Element.prototype as unknown as { __tip?: boolean }).__tip = true;
    Element.prototype.setAttribute = function (name: string, value: string) {
      if (name.toLowerCase() !== "title") return set.call(this, name, value);
      if (value === "" || value == null) remove.call(this, "data-tip");
      else set.call(this, "data-tip", String(value));
    };
    Element.prototype.removeAttribute = function (name: string) {
      remove.call(this, name.toLowerCase() === "title" ? "data-tip" : name);
    };
    Object.defineProperty(HTMLElement.prototype, "title", {
      configurable: true,
      get(this: HTMLElement) {
        return this.getAttribute("data-tip") ?? "";
      },
      set(this: HTMLElement, v: unknown) {
        this.setAttribute("title", v == null ? "" : String(v));
      },
    });
  }
</script>

<script lang="ts">
  // The app's one tooltip, drawn like the app: a rounded bubble with a small arrow at the
  // element, kept inside the window.
  //
  // Every `title` is turned into `data-tip` as soon as it is set (a watcher on the whole
  // page): the system's own tooltip then never shows. Taking the title off only while hovered
  // was not enough: a page redrawn under a still pointer put it back, and the system's
  // tooltip came a second later, over the app's one.
  //
  // A first tooltip waits a moment (as the system's do); moving on to another element while one
  // is shown shows the next at once. A wide element (a waveform, the arrangement's canvas)
  // has it follow the pointer; a text changed while hovered (a canvas naming the clip under
  // the pointer) is picked up at once.
  import { onMount, tick } from "svelte";

  const WAIT = 450;
  const WARM = 600;
  const GAP = 8;
  const EDGE = 8;

  let text = $state("");
  let left = $state(0);
  let top = $state(0);
  let arrow = $state(0);
  let below = $state(true);
  let box = $state<HTMLDivElement | null>(null);
  let el: Element | null = null;
  let lastPointer = { x: 0, y: 0 };
  let timer = 0;
  let hiddenAt = 0;

  /** A `title` that reached the page anyway (written in the HTML, or another way than the two
   *  above): `data-tip` too. */
  function adopt(e: Element) {
    const t = Element.prototype.getAttribute.call(e, "title");
    if (t === null) return;
    e.setAttribute("title", t);
    Element.prototype.removeAttributeNS.call(e, null, "title");
  }
  // A name cut short by its room (an ellipsis, a clamped line: a release on its page, a song in
  // a list) shows whole in the tooltip, when nothing else names it there. Looked for only when
  // the pointer enters an element, a few parents up.
  const cut = new WeakSet<Element>();
  function truncated(from: Element | null): Element | null {
    let x: Element | null = from;
    for (let d = 0; x && d < 4; d++, x = x.parentElement) {
      if (!(x instanceof HTMLElement) || x === document.body) return null;
      const wide = x.scrollWidth > x.clientWidth + 1;
      const tall = x.scrollHeight > x.clientHeight + 1;
      if (!wide && !tall) continue;
      const cs = getComputedStyle(x);
      if ((wide && cs.textOverflow === "ellipsis") || (tall && cs.webkitLineClamp && cs.webkitLineClamp !== "none")) {
        const txt = x.innerText.trim();
        return txt ? x : null;
      }
    }
    return null;
  }
  /** The tooltip of the element around a name cut short (a card's), said under the name. */
  const under = new WeakMap<Element, Element>();
  function tipOf(e: Element | null) {
    if (!e) return "";
    const own = e.getAttribute("data-tip");
    if (own || !(e instanceof HTMLElement) || !cut.has(e)) return own ?? "";
    const full = e.innerText.replace(/[ \t]+/g, " ").trim();
    const around = under.get(e)?.getAttribute("data-tip");
    return around ? `${full}\n${around}` : full;
  }

  function hide() {
    clearTimeout(timer);
    if (text) hiddenAt = performance.now();
    text = "";
  }
  function leave() {
    el = null;
    hide();
  }

  async function place() {
    if (!el || !text) return;
    if (!el.isConnected) return leave();
    await tick();
    if (!el || !box) return;
    const r = el.getBoundingClientRect();
    const wide = r.width > 260 || r.height > 80;
    const ax = wide ? lastPointer.x : r.left + r.width / 2;
    const w = box.offsetWidth;
    const h = box.offsetHeight;
    const anchorBottom = wide ? lastPointer.y + 14 : r.bottom;
    const anchorTop = wide ? lastPointer.y - 10 : r.top;
    below = anchorBottom + GAP + h <= innerHeight - EDGE || anchorTop - GAP - h < EDGE;
    left = Math.max(EDGE, Math.min(innerWidth - EDGE - w, ax - w / 2));
    top = below ? anchorBottom + GAP : anchorTop - GAP - h;
    // the arrow at the element, even when the bubble was pushed in from an edge
    arrow = Math.max(12, Math.min(w - 12, ax - left));
  }

  function show() {
    const t = tipOf(el);
    if (!t) return hide();
    // an icon with nothing else to name it: its tooltip names it (for a screen reader)
    if (el && !el.hasAttribute("aria-label") && !el.textContent?.trim()) el.setAttribute("aria-label", t);
    text = t;
    void place();
  }

  onMount(() => {
    for (const e of document.querySelectorAll("[title]")) adopt(e);
    const watch = new MutationObserver((list) => {
      let changed = false;
      for (const m of list) {
        if (m.type === "attributes") {
          if (m.attributeName === "title") adopt(m.target as Element);
          if (m.target === el) changed = true;
        } else
          for (const n of m.addedNodes) {
            if (n instanceof Element) {
              adopt(n);
              for (const c of n.querySelectorAll("[title]")) adopt(c);
            }
          }
      }
      // the element hovered went away (a list redrawn, a dialog closed): its tooltip too
      if (el && !el.isConnected) return leave();
      if (changed && text) show();
    });
    watch.observe(document.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ["title", "data-tip"] });

    const over = (ev: PointerEvent) => {
      lastPointer = { x: ev.clientX, y: ev.clientY };
      let t = (ev.target as Element | null)?.closest?.("[data-tip]") ?? null;
      // a name cut short inside it (or with no tooltip at all) shows whole first
      const short = truncated(ev.target as Element | null);
      if (short && !short.hasAttribute("data-tip") && (!t || t.contains(short))) {
        cut.add(short);
        if (t) under.set(short, t);
        else under.delete(short);
        t = short;
      }
      if (t === el) return;
      const warm = !!text || performance.now() - hiddenAt < WARM;
      leave();
      el = t;
      if (!t || !tipOf(t)) return;
      if (warm) show();
      else timer = window.setTimeout(show, WAIT);
    };
    const move = (ev: PointerEvent) => {
      lastPointer = { x: ev.clientX, y: ev.clientY };
      if (el && text) void place();
    };
    const out = (ev: PointerEvent) => {
      if (el && !(ev.relatedTarget instanceof Node && el.contains(ev.relatedTarget))) leave();
    };
    // a click, the wheel or a key hides it until the pointer moves to another element
    const off = () => {
      clearTimeout(timer);
      text = "";
    };
    addEventListener("pointerover", over, true);
    addEventListener("pointermove", move, true);
    addEventListener("pointerout", out, true);
    addEventListener("pointerdown", off, true);
    addEventListener("wheel", off, { capture: true, passive: true });
    addEventListener("keydown", off, true);
    addEventListener("blur", leave);
    return () => {
      watch.disconnect();
      clearTimeout(timer);
      removeEventListener("pointerover", over, true);
      removeEventListener("pointermove", move, true);
      removeEventListener("pointerout", out, true);
      removeEventListener("pointerdown", off, true);
      removeEventListener("wheel", off, true);
      removeEventListener("keydown", off, true);
      removeEventListener("blur", leave);
    };
  });
</script>

{#if text}
  <div class="htip" class:up={!below} bind:this={box} role="tooltip" style:left="{left}px" style:top="{top}px" style:--ax="{arrow}px">{text}</div>
{/if}

<style>
  .htip {
    position: fixed; z-index: 3500; max-width: min(320px, calc(100vw - 16px)); pointer-events: none;
    padding: 7px 11px; border-radius: 10px; border: 1px solid var(--line);
    background: var(--panel); color: var(--ink); font-size: 12px; line-height: 1.45; white-space: pre-line; text-wrap: pretty;
    box-shadow: 0 10px 28px rgba(0, 0, 0, .32), 0 1px 3px rgba(0, 0, 0, .18);
    animation: tipin .12s ease-out;
  }
  /* the arrow: a small square turned, carrying the bubble's border on its two outer sides */
  .htip::before {
    content: ""; position: absolute; left: var(--ax); top: -5px; width: 9px; height: 9px; margin-left: -5px;
    background: var(--panel); border: 1px solid var(--line); border-right: 0; border-bottom: 0; transform: rotate(45deg);
  }
  .htip.up::before { top: auto; bottom: -5px; border: 1px solid var(--line); border-left: 0; border-top: 0; }
  @keyframes tipin { from { opacity: 0; transform: translateY(-2px); } }
  .htip.up { animation-name: tipup; }
  @keyframes tipup { from { opacity: 0; transform: translateY(2px); } }
  @media (prefers-reduced-motion: reduce) { .htip { animation: none; } }
</style>

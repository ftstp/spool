// The samples' shapes for their rows, asked of the app in batches (a page of rows at once,
// not a request a row) and kept for the session. `null`: none (silent, unreadable).
import { SvelteMap } from "svelte/reactivity";
import { sampleWaves } from "./api";

const shapes = new SvelteMap<string, number[] | null>();
const queue = new Set<string>();
let timer = 0;

export function waveOf(path: string): number[] | null | undefined {
  if (!shapes.has(path) && !queue.has(path)) {
    queue.add(path);
    clearTimeout(timer);
    timer = window.setTimeout(flush, 30);
  }
  return shapes.get(path);
}

async function flush() {
  const all = [...queue];
  queue.clear();
  // the first rows first: a page appears from the top
  for (let i = 0; i < all.length; i += 40) {
    const part = all.slice(i, i + 40);
    let got: Record<string, number[]> = {};
    try {
      got = await sampleWaves(part);
    } catch {
      /* drawn as none */
    }
    for (const p of part) shapes.set(p, got[p] ?? null);
  }
}

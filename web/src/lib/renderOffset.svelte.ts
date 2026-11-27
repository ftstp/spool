// Where a render starts in its project's arrangement. A render exported from Live often starts
// at the start marker, the loop brace or the first bar that sounds, not at bar 1: heard against
// the arrangement, its playhead and its chords were then early by those bars. Per render file,
// in beats from the arrangement's start: found by listening (the render's loudness bar by bar
// against what the arrangement plays bar by bar), or set by hand; kept.
import type { Arrangement } from "./api/bindings/Arrangement";

const KEEP = "spool.render.offset";
type Kept = { beats: number; auto: boolean };
const key = (rel: string) => rel.replace(/\\/g, "/").toLowerCase();

class Offsets {
  by = $state.raw<Record<string, Kept>>(
    (() => {
      try {
        return JSON.parse(localStorage.getItem(KEEP) || "{}");
      } catch {
        return {};
      }
    })(),
  );
  /** The render's start in beats from the arrangement's start (0: it starts with it). */
  get(rel: string | null | undefined): number {
    return rel ? (this.by[key(rel)]?.beats ?? 0) : 0;
  }
  /** Set by hand (or found: `auto`). */
  set(rel: string, beats: number, auto = false) {
    this.by = { ...this.by, [key(rel)]: { beats: Math.max(0, beats), auto } };
    this.#keep();
  }
  /** Known at all (by hand or found): nothing to look for. */
  known(rel: string): boolean {
    return key(rel) in this.by;
  }
  isAuto(rel: string): boolean {
    return !!this.by[key(rel)]?.auto;
  }
  #keep() {
    try {
      localStorage.setItem(KEEP, JSON.stringify(this.by));
    } catch {
      /* not kept */
    }
  }
}
export const renderOffset = new Offsets();

/** Where the render most likely starts, in whole bars (in beats), from its loudness bar by
 *  bar (`peaks` over `duration` seconds) against what the arrangement plays bar by bar (the
 *  clips Live plays). Null when nothing says it clearly (then it starts with the project). */
export function detectOffset(a: Arrangement, peaks: number[], duration: number): number | null {
  const bpm = a.tempo ?? 120;
  const bpb = a.beats_per_bar || 4;
  const barSecs = (bpb * 60) / bpm;
  const nR = Math.floor(duration / barSecs);
  if (!peaks.length || nR < 4) return null;
  // the render's level bar by bar (the loudest slice of each)
  const r: number[] = [];
  for (let i = 0; i < nR; i++) {
    const from = Math.floor(((i * barSecs) / duration) * peaks.length);
    const to = Math.max(from + 1, Math.floor((((i + 1) * barSecs) / duration) * peaks.length));
    r.push(Math.max(...peaks.slice(from, to)));
  }
  // what the arrangement plays bar by bar: how many heard clips sound in it
  const nA = Math.ceil(a.len / bpb) + 1;
  const act = new Array(nA).fill(0);
  for (const t of a.tracks) {
    if (t.kind === "group" || t.muted) continue;
    for (const c of t.clips) {
      if (c.muted) continue;
      for (let b = Math.floor(c.start / bpb); b < Math.min(nA, Math.ceil(c.end / bpb)); b++) act[b]++;
    }
  }
  const corr = (o: number) => {
    const n = Math.min(nR, nA - o);
    if (n < 4) return -1;
    let sr = 0, sa = 0;
    for (let i = 0; i < n; i++) { sr += r[i]; sa += act[o + i]; }
    const mr = sr / n, ma = sa / n;
    let num = 0, dr = 0, da = 0;
    for (let i = 0; i < n; i++) {
      const x = r[i] - mr, y = act[o + i] - ma;
      num += x * y; dr += x * x; da += y * y;
    }
    return dr && da ? num / Math.sqrt(dr * da) : -1;
  };
  // a loud bar of the render laid on a bar where the arrangement plays (almost) nothing is
  // the surest sign of a wrong place: a mastered render's level is nearly flat, its loudness
  // alone says little, its silences and the arrangement's say a lot
  const rMax = Math.max(...r);
  const aMax = Math.max(1, ...act);
  // the shape matters as much as the render's level varies: a flat master, hardly at all
  const mean = r.reduce((x, y) => x + y, 0) / nR;
  const cv = Math.sqrt(r.reduce((x, y) => x + (y - mean) ** 2, 0) / nR) / Math.max(1e-6, mean);
  const wShape = Math.min(1, cv / 0.3);
  const score = (o: number) => {
    let miss = 0;
    for (let i = 0; i < nR; i++) if (r[i] > 0.25 * rMax && (act[o + i] ?? 0) < 0.15 * aMax) miss++;
    return wShape * corr(o) - 1.5 * (miss / nR);
  };
  let best = 0, bestS = score(0);
  for (let o = 1; o < nA; o++) {
    const sc = score(o);
    if (sc > bestS + 1e-9) { best = o; bestS = sc; }
  }
  if (best > 0 && bestS - score(0) > 0.12) return best * bpb;
  return null;
}

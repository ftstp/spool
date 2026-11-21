// Whole progressions, not chord after chord: the song's own loop read from its chords, the
// well-known progressions of its mode written in its key, and variations of its own loop.
// Each says what it is in one line (song.prog.<id> in the locales).
import type { BarChord } from "./api/bindings/BarChord";
import { diatonic, type Chord } from "./theory";

export type Prog = { id: string; chords: Chord[]; styles: string[]; mine?: boolean; from?: string };

// degrees as written in roman numerals: "bVI", "iv", "V7", "ii°", "iiø7", "Imaj7"
const STEPS: Record<string, number> = { I: 0, II: 2, III: 4, IV: 5, V: 7, VI: 9, VII: 11 };
function parse(deg: string, tonic: number): Chord {
  const m = /^([b#]?)([IViv]+)(°|ø)?(.*)$/.exec(deg);
  if (!m) return { root: tonic, q: "" };
  const shift = m[1] === "b" ? -1 : m[1] === "#" ? 1 : 0;
  const num = m[2];
  const minor = num === num.toLowerCase();
  const root = (tonic + STEPS[num.toUpperCase()] + shift + 12) % 12;
  const ext = m[4];
  let q: string;
  if (m[3] === "°") q = ext === "7" ? "dim7" : "dim";
  else if (m[3] === "ø") q = "m7b5";
  else if (minor) q = "m" + ext;
  else q = ext;
  return { root, q };
}

// the classics, by mode: degrees (from the key's tonic), what styles they belong to
const MAJOR: [string, string, string[]][] = [
  ["axis", "I V vi IV", ["pop", "rock"]],
  ["axisMinorStart", "vi IV I V", ["pop", "edm"]],
  ["doowop", "I vi IV V", ["soul", "pop"]],
  ["royal", "IV V iii vi", ["pop", "jpop"]],
  ["canon", "I V vi iii IV I IV V", ["pop", "orchestral"]],
  ["twoFiveOne", "ii7 V7 Imaj7 Imaj7", ["jazz"]],
  ["turnaround", "Imaj7 vi7 ii7 V7", ["jazz", "soul"]],
  ["mixo", "I bVII IV I", ["rock", "funk"]],
  ["borrowedIv", "I iii IV iv", ["soul", "pop"]],
  ["plagalRock", "I IV I V", ["rock", "reggae"]],
  ["sadPop", "IV I V vi", ["pop", "edm"]],
  ["neoSoul", "IVmaj7 iii7 ii7 Imaj7", ["soul", "jazz"]],
];
const MINOR: [string, string, string[]][] = [
  ["epic", "i bVI bIII bVII", ["pop", "edm", "orchestral"]],
  ["andalusian", "i bVII bVI V", ["flamenco", "rock"]],
  ["aeolianLoop", "i bVII bVI bVII", ["rock", "edm"]],
  ["minorCadence", "i iv V i", ["classical", "orchestral"]],
  ["harmonicDrama", "i bVI iv V", ["orchestral", "pop"]],
  ["circle", "i iv bVII bIII", ["pop", "jazz"]],
  ["minorTwoFive", "iiø7 V7 i7 i7", ["jazz"]],
  ["dorian", "i7 IV7 i7 IV7", ["soul", "funk"]],
  ["phrygian", "i bII i bVII", ["trap", "drill"]],
  ["darkPop", "i v bVI iv", ["pop", "hiphop"]],
  ["lament", "i bVII bVI bVI", ["hiphop", "rock"]],
  ["lofi", "iv7 bVII7 bIIImaj7 bVImaj7", ["hiphop", "lofi"]],
];

/** The progressions of the key's mode, written in it. */
export function classics(tonic: number, minor: boolean): Prog[] {
  return (minor ? MINOR : MAJOR).map(([id, degs, styles]) => ({ id, styles, chords: degs.split(" ").map((d) => parse(d, tonic)) }));
}

const key = (c: Chord) => `${c.root}${c.q}`;
const rotEq = (a: Chord[], b: Chord[]) => {
  if (a.length !== b.length) return false;
  const s = b.map(key).join(",");
  return a.some((_, i) => [...a.slice(i), ...a.slice(0, i)].map(key).join(",") === s);
};

/** The song's loop: the four bars (a bar's first chord each) that come back the most; its
 *  chords with sevenths and the like simplified to triads, as the classics are written. */
export function songLoop(bars: BarChord[]): Chord[] | null {
  const first = new Map<number, Chord>();
  for (const b of bars) if (!first.has(b.bar)) first.set(b.bar, { root: b.root, q: triad(b.quality) });
  const count = new Map<string, { n: number; cs: Chord[] }>();
  for (const [bar] of first) {
    const cs = [0, 1, 2, 3].map((k) => first.get(bar + k));
    if (cs.some((c) => !c)) continue;
    const k = (cs as Chord[]).map(key).join(",");
    const e = count.get(k);
    if (e) e.n++;
    else count.set(k, { n: 1, cs: cs as Chord[] });
  }
  const best = [...count.values()].sort((a, b) => b.n - a.n)[0];
  return best && best.n >= 2 ? best.cs : null;
}
const triad = (q: string) => (q.startsWith("m7b5") || q.startsWith("dim") ? "dim" : q.startsWith("m") && !q.startsWith("maj") ? "m" : q.startsWith("sus") ? q : "");

/** Variations of the song's loop: one chord swapped for another of the same function (it
 *  plays the same role, another colour), the IV borrowed from the minor, a dominant before
 *  the return, sevenths throughout. */
export function variations(loop: Chord[], tonic: number, minor: boolean): Prog[] {
  const out: Prog[] = [];
  const dia = diatonic(tonic, minor);
  const fnOf = (c: Chord) => dia.find((d) => d.root === c.root)?.fn;
  const add = (id: string, chords: Chord[], from: string) => {
    if (!rotEq(chords, loop) && !out.some((p) => rotEq(p.chords, chords))) out.push({ id, chords, styles: [], from });
  };
  loop.forEach((c, i) => {
    const fn = fnOf(c);
    if (!fn) return;
    for (const d of dia) {
      if (d.fn !== fn || d.root === c.root || d.q === "dim") continue;
      const next = loop.slice();
      next[i] = { root: d.root, q: d.q };
      add("swap", next, `${i}`);
      break;
    }
  });
  if (!minor) {
    const iv = loop.findIndex((c) => c.root === (tonic + 5) % 12 && c.q === "");
    if (iv >= 0) {
      const next = loop.slice();
      next[iv] = { root: (tonic + 5) % 12, q: "m" };
      add("borrowIv", next, `${iv}`);
    }
  }
  // a dominant seventh just before the bar that comes back to the tonic
  const home = loop.findIndex((c, i) => i > 0 && c.root === tonic);
  const at = home > 0 ? home - 1 : loop.length - 1;
  if (loop[at] && loop[at].root !== (tonic + 7) % 12) {
    const next = loop.slice();
    next[at] = { root: (tonic + 7) % 12, q: "7" };
    add("dominant", next, `${at}`);
  }
  const sev: Record<string, string> = { "": "maj7", m: "m7", dim: "m7b5" };
  add("sevenths", loop.map((c) => ({ root: c.root, q: c.root === (tonic + 7) % 12 && c.q === "" ? "7" : sev[c.q] ?? c.q })), "");
  return out.slice(0, 6);
}

/** How many of a progression's chords the song already plays (to show what is close). */
export function shared(p: Chord[], song: Chord[]): number {
  const s = new Set(song.map(key));
  return p.filter((c) => s.has(key({ root: c.root, q: triad(c.q) }))).length;
}
export const sameLoop = rotEq;

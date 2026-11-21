// A little music theory for the song sheet: chords as notes, their degree in a key, and what
// could come after a chord (the suggestions), by level and by style. Each suggestion says why
// in one line (song.harmony.why.<id> in the locales).

const SHARPS = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLATS = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const NAMES = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/** The note names a key is written with (as crates/midi spells them). */
export function spelling(tonic: number, minor: boolean): string[] {
  const major = minor ? (tonic + 3) % 12 : tonic;
  return [5, 10, 3, 8, 1].includes(major) ? FLATS : major === 0 ? NAMES : SHARPS;
}

/** Intervals of each chord quality, from the root. */
export const QUALITY: Record<string, number[]> = {
  "": [0, 4, 7], m: [0, 3, 7], "7": [0, 4, 7, 10], maj7: [0, 4, 7, 11], m7: [0, 3, 7, 10], dim: [0, 3, 6], dim7: [0, 3, 6, 9],
  m7b5: [0, 3, 6, 10], sus4: [0, 5, 7], sus2: [0, 2, 7], "6": [0, 4, 7, 9], m6: [0, 3, 7, 9], "9": [0, 4, 7, 10, 14],
  m9: [0, 3, 7, 10, 14], maj9: [0, 4, 7, 11, 14], "7#9": [0, 4, 7, 10, 15], add9: [0, 4, 7, 14],
};
const MINORISH = (q: string) => (q.startsWith("m") && !q.startsWith("maj")) || q.startsWith("dim");

export type Chord = { root: number; q: string };
export const chordName = (c: Chord, names: string[]) => names[((c.root % 12) + 12) % 12] + c.q;

/** A chord's notes: its root in the bass (octave 3) and its tones above middle C. */
export function chordNotes(c: Chord): number[] {
  const iv = QUALITY[c.q] ?? QUALITY[""];
  const r = ((c.root % 12) + 12) % 12;
  const base = 60 + (r > 6 ? r - 12 : r);
  return [36 + r + (r < 4 ? 12 : 0), ...iv.map((i) => base + i)];
}

const R = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"];
/** "ii", "bVII", "V"... (lower case for a minor or diminished chord). */
export function degree(c: Chord, tonic: number): string {
  const r = R[(((c.root - tonic) % 12) + 12) % 12];
  return MINORISH(c.q) ? r.toLowerCase() : r;
}

export type Level = "base" | "color" | "advanced";
export type Style = "jazz" | "soul" | "reggae" | "fanfare" | "orchestral";
export type Suggestion = Chord & { why: string; to?: string; level: Level; styles: Style[] };

// the diatonic chords of a key: (step from the tonic, triad quality, seventh quality)
const MAJOR: [number, string, string][] = [[0, "", "maj7"], [2, "m", "m7"], [4, "m", "m7"], [5, "", "maj7"], [7, "", "7"], [9, "m", "m7"], [11, "dim", "m7b5"]];
// natural minor with the harmonic minor's major V (its leading tone)
const MINOR: [number, string, string][] = [[0, "m", "m7"], [2, "dim", "m7b5"], [3, "", "maj7"], [5, "m", "m7"], [7, "", "7"], [8, "", "maj7"], [10, "", "7"]];

/** What could come after `cur` in the key (tonic, minor): the diatonic moves first, then the
 *  colours, then the advanced ones, and the styles' own. */
export function suggest(tonic: number, minor: boolean, cur: Chord): Suggestion[] {
  const names = spelling(tonic, minor);
  const scale = minor ? MINOR : MAJOR;
  const at = (step: number, seventh = false): Chord => {
    const d = scale.find((x) => x[0] === step)!;
    return { root: (tonic + step) % 12, q: seventh ? d[2] : d[1] };
  };
  const step = (((cur.root - tonic) % 12) + 12) % 12;
  const out: Suggestion[] = [];
  const add = (c: Chord, why: string, level: Level, styles: Style[] = [], to?: Chord) => {
    if (c.root % 12 === cur.root % 12 && c.q === cur.q) return;
    if (out.some((o) => o.root === c.root % 12 && o.q === c.q)) return;
    out.push({ root: c.root % 12, q: c.q, why, level, styles, to: to ? chordName(to, names) : undefined });
  };
  const I = at(0), IV = at(5), V = at(7), rel = minor ? at(3) : at(9), ii = at(2), vi = minor ? at(8) : at(9);

  // the bases: by the current chord's function
  if (step === 7 || step === 11) {
    add(I, "perfect", "base");
    add(vi, "deceptive", "base");
    add(IV, "plagalBack", "base");
  } else if (step === 5 || step === 2) {
    add(V, "toDominant", "base");
    add(I, step === 5 ? "plagal" : "home", "base");
    if (step === 5) add(ii, "preDominant", "base");
  } else if (step === 0) {
    add(IV, "subdominant", "base");
    add(V, "dominant", "base");
    add(rel, "relative", "base");
    add(ii, "preDominant", "base");
  } else {
    add(IV, "subdominant", "base");
    add(V, "dominant", "base");
    add(I, "home", "base");
  }
  add(rel, "relative", "base");

  // colours
  for (const target of minor ? [at(5), at(8)] : [at(2), at(9), at(7)]) {
    add({ root: (target.root + 7) % 12, q: "7" }, "secondaryDominant", "color", [], target);
  }
  if (!minor) {
    add({ root: (tonic + 5) % 12, q: "m" }, "borrowedIv", "color");
    add({ root: (tonic + 8) % 12, q: "" }, "borrowed", "color", ["orchestral"]);
    add({ root: (tonic + 10) % 12, q: "" }, "borrowed", "color");
  } else {
    add({ root: (tonic + 5) % 12, q: "" }, "dorianIV", "color", ["soul"]);
    if (step === 7) add({ root: tonic, q: "" }, "picardy", "color", ["orchestral", "fanfare"]);
  }
  add({ root: (tonic + 7) % 12, q: "sus4" }, "suspension", "color", [], V);

  // advanced
  add({ root: (tonic + 1) % 12, q: "7" }, "tritoneSub", "advanced", ["jazz"], I);
  add({ root: (tonic + 1) % 12, q: "" }, "neapolitan", "advanced", ["orchestral"], V);
  add({ root: (cur.root + 1) % 12, q: "dim7" }, "passingDim", "advanced", ["jazz"]);
  add({ root: (tonic + 10) % 12, q: "7" }, "backdoor", "advanced", ["jazz", "soul"], I);

  // styles
  add(at(2, true), "jazzIIV", "color", ["jazz"]);
  add(at(7, true), "jazzIIV", "color", ["jazz"]);
  add({ root: cur.root, q: MINORISH(cur.q) ? "m9" : cur.q === "7" ? "9" : "maj9" }, "extend", "color", ["jazz", "soul"]);
  add({ root: cur.root, q: MINORISH(cur.q) ? "m9" : "9" }, "soulNinth", "color", ["soul"]);
  add({ root: tonic, q: minor ? "m7" : "7#9" }, "funkSharp9", "color", ["soul"]);
  add(IV, "reggaeSkank", "base", ["reggae"]);
  add(V, "reggaeSkank", "base", ["reggae"]);
  add({ root: (tonic + 7) % 12, q: "7" }, "fanfareCadence", "base", ["fanfare"], I);
  add({ root: (tonic + 8) % 12, q: "" }, "epicCadence", "color", ["orchestral"]);
  add({ root: (tonic + 10) % 12, q: "" }, "epicCadence", "color", ["orchestral"]);
  add({ root: cur.root, q: "sus2" }, "sus2Pad", "color", ["orchestral"]);
  return out;
}

/** The seven chords of the key, with their function: tonic (rest), subdominant (moving away),
 *  dominant (tension). */
export function diatonic(tonic: number, minor: boolean, sevenths = false): (Chord & { fn: "T" | "SD" | "D" })[] {
  const FN: Record<number, "T" | "SD" | "D"> = minor ? { 0: "T", 2: "SD", 3: "T", 5: "SD", 7: "D", 8: "SD", 10: "D" } : { 0: "T", 2: "SD", 4: "T", 5: "SD", 7: "D", 9: "T", 11: "D" };
  return (minor ? MINOR : MAJOR).map(([st, tri, sev]) => ({ root: (tonic + st) % 12, q: sevenths ? sev : tri, fn: FN[st] }));
}

/** The notes of the key's scale (pitch classes). */
export function scaleOf(tonic: number, minor: boolean): number[] {
  return (minor ? [0, 2, 3, 5, 7, 8, 10] : [0, 2, 4, 5, 7, 9, 11]).map((i) => (tonic + i) % 12);
}

/** Diatonic or not: whether a chord's notes all belong to the key. */
export function inKey(c: Chord, tonic: number, minor: boolean): boolean {
  const sc = minor ? [0, 2, 3, 5, 7, 8, 10, 11] : [0, 2, 4, 5, 7, 9, 11];
  return (QUALITY[c.q] ?? QUALITY[""]).every((i) => sc.includes((((c.root + i - tonic) % 12) + 12) % 12));
}

// ---- chords read from notes (as crates/midi reads a song's): what a clip plays, bar by bar

const TEMPLATES: [string, number[]][] = [
  ["", [0, 4, 7]], ["m", [0, 3, 7]], ["7", [0, 4, 7, 10]], ["maj7", [0, 4, 7, 11]], ["m7", [0, 3, 7, 10]], ["dim", [0, 3, 6]], ["sus4", [0, 5, 7]], ["sus2", [0, 2, 7]],
];
/** The chord a pitch-class weight vector sounds like (the bass note as a hint), or null when
 *  there are fewer than two pitches. */
export function chordOf(w: number[], bass: number | null): Chord | null {
  const total = w.reduce((a, b) => a + b, 0);
  if (total <= 0 || w.filter((x) => x > 0).length < 2) return null;
  let best: { c: Chord; s: number } | null = null;
  for (let root = 0; root < 12; root++) {
    for (const [q, iv] of TEMPLATES) {
      const inside = iv.reduce((a, i) => a + w[(root + i) % 12], 0);
      const missing = iv.filter((i) => w[(root + i) % 12] === 0).length;
      let s = inside - (total - inside) * 0.8 - missing * total * 0.15 - (iv.length - 3) * total * 0.05;
      if (bass === root) s += total * 0.1;
      if (!best || s > best.s) best = { c: { root, q }, s };
    }
  }
  return best && best.s > 0 ? best.c : null;
}

/** A clip's chords, one per bar of the song it covers (notes: pitch, start, length in beats
 *  from the clip's start; `start`: the clip's place in the song), the same chord running on
 *  merged: [{ from, to (song beats), chord }]. */
export function clipChords(notes: [number, number, number][], start: number, end: number, bpb: number): { from: number; to: number; c: Chord }[] {
  const out: { from: number; to: number; c: Chord }[] = [];
  for (let b = Math.floor(start / bpb) * bpb; b < end; b += bpb) {
    const s = Math.max(b, start);
    const e = Math.min(b + bpb, end);
    const w = new Array(12).fill(0);
    let low: number | null = null;
    for (const [p, ns, l] of notes) {
      const a0 = start + ns;
      const a1 = a0 + l;
      if (a1 <= s || a0 >= e) continue;
      w[p % 12] += Math.min(e, a1) - Math.max(s, a0);
      if (low === null || p < low) low = p;
    }
    const c = chordOf(w, low === null ? null : low % 12);
    if (!c) continue;
    const last = out[out.length - 1];
    if (last && last.c.root === c.root && last.c.q === c.q && Math.abs(last.to - s) < 1e-6) last.to = e;
    else out.push({ from: s, to: e, c });
  }
  return out;
}

/** A run of chord names written short: a loop that comes back said once ("Am F C G ×4"). */
export function shortRun(names: string[]): string {
  for (let p = 1; p <= Math.min(8, names.length / 2); p++) {
    if (names.length % p) continue;
    if (names.every((n, i) => n === names[i % p])) return `${names.slice(0, p).join(" ")} ×${names.length / p}`;
  }
  return names.join(" ");
}

const LATIN = ["Do", "Do#", "Ré", "Mib", "Mi", "Fa", "Fa#", "Sol", "Lab", "La", "Sib", "Si"];
/** A MIDI note named as Live names it (C3 = 60), in Latin names when asked. */
export function noteLabel(p: number, latin: boolean, names: string[] = NAMES): string {
  const oct = Math.floor(p / 12) - 2;
  return (latin ? LATIN : names)[((p % 12) + 12) % 12] + oct;
}

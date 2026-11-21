// One search for every list of songs (the field at the top, the Songs page): each word
// typed must be found somewhere in the song, any order. A word is looked for as text (title,
// artist, style, folder, project file, instruments, stage, catalogue number, the releases it
// is on), and also as what it may be: a tempo ("174", "174bpm", "170-175") or a key ("Fm",
// "F#m" or "Gbm", "A min"). Accents and case do not count.
import type { SongItem } from "./api/bindings/SongItem";
import type { Release } from "./api/bindings/Release";

export const norm = (s: string | null | undefined) => (s ?? "").normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const FLAT_TO_SHARP: Record<string, string> = { db: "c#", eb: "d#", gb: "f#", ab: "g#", bb: "a#", cb: "b", fb: "e" };

/** "F#m", "Gb minor", "gbm", "A min", "C" -> "f#m" / "f#m" / "f#m" / "am" / "c"; null when it is not a key. */
export function keyOf(s: string | null | undefined): string | null {
  const m = norm(s)
    .replace(/♯/g, "#")
    .replace(/♭/g, "b")
    .replace(/\s+/g, "")
    .match(/^([a-g])(#|b|sharp|flat)?(m|min|minor|maj|major)?$/);
  if (!m) return null;
  const acc = m[2] === "sharp" ? "#" : m[2] === "flat" ? "b" : (m[2] ?? "");
  let root = m[1] + acc;
  root = FLAT_TO_SHARP[root] ?? root;
  return root + (m[3] && !m[3].startsWith("maj") ? "m" : "");
}

type Word = { text: string; key: string | null; tempo: [number, number] | null };

/** The words of a query, each with the key and tempo it may stand for. "a min" or "f# minor"
 *  typed with a space count as one key. */
export function parse(q: string): Word[] {
  const raw = norm(q).split(/\s+/).filter(Boolean);
  const out: Word[] = [];
  for (let i = 0; i < raw.length; i++) {
    let w = raw[i];
    const next = raw[i + 1];
    if (next && /^(m|min|minor|maj|major)$/.test(next) && keyOf(w + next)) {
      w += next;
      i++;
    }
    const range = w.match(/^(\d{2,3})-(\d{2,3})(?:bpm)?$/);
    const one = w.match(/^(\d{2,3}(?:[.,]\d+)?)(?:bpm)?$/);
    const tempo: [number, number] | null = range
      ? [Math.min(+range[1], +range[2]), Math.max(+range[1], +range[2])]
      : one
        ? [parseFloat(one[1].replace(",", ".")) - 1, parseFloat(one[1].replace(",", ".")) + 1]
        : null;
    out.push({ text: w.replace(/bpm$/, "") || w, key: keyOf(w), tempo: tempo && tempo[1] >= 40 && tempo[0] <= 300 ? tempo : null });
  }
  return out;
}

/** The releases each song is on (their titles and artists), for the search. */
export function releasesBySong(releases: Release[]): Map<number, string> {
  const m = new Map<number, string>();
  for (const r of releases) for (const id of r.songs) if (id != null) m.set(id, (m.get(id) ?? "") + " " + norm(r.title) + " " + norm(r.artist));
  return m;
}

/** Whether every word is found in the song. */
export function matches(s: SongItem, words: Word[], rel: Map<number, string> | null = null): boolean {
  if (!words.length) return true;
  let hay: string | null = null;
  const key = keyOf(s.key);
  for (const w of words) {
    if (w.key && key === w.key) continue;
    if (w.tempo && s.tempo && s.tempo >= w.tempo[0] && s.tempo <= w.tempo[1]) continue;
    hay ??= [s.title, s.artist, s.style, s.dir, s.rel, s.stage, s.catalog, s.key, s.kinds.join(" "), ...s.instr].map(norm).join(" \u0001 ") + (rel?.get(s.id) ?? "");
    if (!hay.includes(w.text)) return false;
  }
  return true;
}

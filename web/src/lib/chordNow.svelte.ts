// The chords of what plays, for the player bar (the chord heard and the next one). A song's
// open arrangement sets them for its project playback; a song's reference render gets them
// from the song itself, wherever it was started from (`songChords`).
import * as api from "./api";
import { renderOffset } from "./renderOffset.svelte";
import type { Harmony } from "./api/bindings/Harmony";

export type ChordRun = { from: number; to: number; root: number; q: string; bass: number | null };
export type ChordSource = {
  /** The render whose chords these are (its first bar is the arrangement's first bar). */
  temoin: string | null;
  /** The MIDI player's name for the project playing. */
  midiRel: string;
  bpm: number;
  tonic: number;
  minor: boolean;
  /** In beats from the arrangement's start. */
  runs: ChordRun[];
  /** Where the render starts in the arrangement, in beats (renderOffset). */
  offset: number;
};

// Held as it is, never proxied: a deep $state stored a proxied copy, so the arrangement's
// "is it still mine?" check on leaving never matched and a closed sheet's chords stayed.
class Current {
  v = $state.raw<ChordSource | null>(null);
  // what `v` holds, read by `release`: a teardown is handed the values from before the update
  #now: ChordSource | null = null;
  set(s: ChordSource | null) {
    this.#now = s;
    this.v = s;
  }
  /** Clears `s` if it is still the one shown. */
  release(s: ChordSource) {
    if (this.#now === s) this.set(null);
  }
}
export const chordSource = new Current();

const PC: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
const pcOf = (n: string | null) => (n && n[0] in PC ? (PC[n[0]] + (n[1] === "#" ? 1 : n[1] === "b" ? 11 : 0)) % 12 : null);

/** The runs of the same chord, from a project's harmony (in beats from its start). */
export function chordSourceOf(h: Harmony, beatsPerBar: number, bpm: number, temoin: string | null, midiRel: string, offset = 0): ChordSource | null {
  if (!h.bars.length) return null;
  const runs: ChordRun[] = [];
  for (const b of h.bars) {
    const from = b.bar * beatsPerBar + b.at;
    const bass = pcOf(b.bass);
    const root = ((b.root % 12) + 12) % 12;
    const last = runs[runs.length - 1];
    if (last && last.root === root && last.q === b.quality && last.bass === bass && Math.abs(last.to - from) < 1e-6) last.to = from + b.len;
    else runs.push({ from, to: from + b.len, root, q: b.quality, bass });
  }
  return { temoin, midiRel, bpm, tonic: h.tonic ?? 0, minor: h.minor, runs, offset };
}

// read once per project file as it is on disk (the song's sheet is asked each time: cheap)
const byFile = new Map<string, Promise<ChordSource | null>>();
/** A song's chords, for its reference render: from its current project (as its sheet shows it). */
export async function songChords(sid: number): Promise<ChordSource | null> {
  if (!api.inApp) return null;
  const d = await api.song(sid);
  const temoin = d?.song.temoin;
  if (!d || !temoin) return null;
  const f = [...d.files].sort((a, b) => Number(!!b.latest) - Number(!!a.latest) || b.mtime - a.mtime)[0];
  if (!f) return null;
  const key = `${f.rel}|${f.mtime}|${renderOffset.get(temoin)}`;
  let p = byFile.get(key);
  if (!p) {
    p = Promise.all([api.harmony(f.rel), api.arrangement(f.rel)]).then(([h, a]) =>
      chordSourceOf(h, a.beats_per_bar, a.tempo ?? 120, temoin, "notes:session|" + f.rel, renderOffset.get(temoin)),
    );
    byFile.set(key, p);
    p.catch(() => byFile.delete(key));
  }
  const s = await p;
  // the render may have changed since (another one chosen)
  return s && { ...s, temoin };
}

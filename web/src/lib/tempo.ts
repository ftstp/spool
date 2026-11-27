// Playing faster or slower without changing the key. A rate is a speed (1 = as recorded),
// kept between half and double speed. "Lock to" a tempo (a song's, the BPM typed on the
// Samples or MIDI page) picks the rate per file from its own tempo, folded by octaves of
// tempo: an 87 BPM loop locked to 174 plays as is (half-time), not twice as fast.

export const RMIN = 0.5;
export const RMAX = 2;
export const clampRate = (r: number) => Math.min(RMAX, Math.max(RMIN, Number.isFinite(r) && r > 0 ? r : 1));

/** The rate that brings `bpm` to `target`, the closest to 1 among x1/2, x1, x2. */
export function lockRate(bpm: number | null | undefined, target: number | null | undefined): number | null {
  if (!bpm || !target || bpm <= 0 || target <= 0) return null;
  const r = target / bpm;
  const best = [r, r / 2, r * 2].reduce((a, c) => (Math.abs(Math.log(c)) < Math.abs(Math.log(a)) ? c : a));
  return clampRate(best);
}

/** One step faster or slower: one BPM when the tempo is known, 5 % otherwise. */
export function stepRate(rate: number, bpm: number | null | undefined, dir: 1 | -1): number {
  if (bpm && bpm > 0) return clampRate((Math.round(bpm * rate) + dir) / bpm);
  return clampRate(Math.round((rate + dir * 0.05) * 100) / 100);
}

/** A tempo state shared by a player and its control. */
export interface Tempo {
  /** The speed chosen by hand. */
  rate: number;
  /** Locked to `target`: the speed follows each file's tempo. */
  lock: boolean;
  target: number | null;
  /** The tempo of what plays (null: unknown). */
  readonly base: number | null;
  /** The speed in effect. */
  readonly eff: number;
  setRate(r: number): void;
  setLock(on: boolean): void;
}

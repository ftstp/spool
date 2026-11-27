// Transposing what plays, by semitones (-12 to +12). MIDI notes are moved themselves; audio
// (a reference render, a project's samples) goes through the "pitch-shift" worklet
// (public/pitch-worklet.js), loaded once per audio context.

export const SMIN = -12;
export const SMAX = 12;
export const clampSemis = (n: number) => Math.max(SMIN, Math.min(SMAX, Math.round(Number.isFinite(n) ? n : 0)));

/** A player whose key can be moved (its control in the player bar). */
export interface Pitch {
  semis: number;
  setSemis(n: number): void;
}

const loaded = new WeakMap<BaseAudioContext, Promise<boolean>>();
/** The worklet ready in this context (false: it could not load, the sound goes on as is). */
export function pitchReady(ctx: BaseAudioContext): Promise<boolean> {
  let p = loaded.get(ctx);
  if (!p) {
    p = ctx.audioWorklet
      .addModule("/pitch-worklet.js")
      .then(() => true)
      .catch(() => false);
    loaded.set(ctx, p);
  }
  return p;
}

/** A transposing node (or null when the worklet is not there). */
export async function pitchNode(ctx: AudioContext, semis: number): Promise<AudioWorkletNode | null> {
  if (!(await pitchReady(ctx))) return null;
  const n = new AudioWorkletNode(ctx, "pitch-shift", { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2] });
  setPitch(n, ctx, semis);
  return n;
}

export function setPitch(n: AudioWorkletNode | null, ctx: AudioContext, semis: number) {
  n?.parameters.get("semis")?.setValueAtTime(semis, ctx.currentTime);
}

const SHARP = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLAT = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const PC: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
/** A key as written in the catalogue ("Fm", "C#m", "Eb", "G minor") moved by semitones, in
 *  the same spelling (flats stay flats), or null when it cannot be read. */
export function transposeKey(key: string, semis: number): string | null {
  const m = /^\s*([A-Ga-g])(#|b|♯|♭)?(.*)$/.exec(key);
  if (!m) return null;
  const acc = m[2] === "#" || m[2] === "♯" ? 1 : m[2] === "b" || m[2] === "♭" ? -1 : 0;
  const pc = (((PC[m[1].toUpperCase()] + acc + semis) % 12) + 12) % 12;
  // the rest as written: "m", " minor", " dorien", nothing
  return (acc < 0 ? FLAT : SHARP)[pc] + m[3];
}

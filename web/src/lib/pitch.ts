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

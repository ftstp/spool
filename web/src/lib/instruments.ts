// Recorded instruments for hearing MIDI: a piano, the orchestra (strings by section, pizzicato,
// harp, woodwinds, brass, timpani, glockenspiel). A few notes of each, a few semitones apart,
// short and compressed (public/instruments, from VSCO 2 Community Edition, CC0: see
// public/instruments/LICENSE.txt, tools/make_orchestra.py); a note between two is the nearest
// one played a little faster or slower. A held instrument (bowed, blown) loops the steady part
// of its notes, so a long note goes on as long as it is held. Loaded the first time an
// instrument is asked for; until then (a few hundred ms) the synth plays.

export type Inst =
  | "piano" | "strings" | "brass"
  | "viola" | "cello" | "contrabass" | "pizz" | "harp"
  | "flute" | "oboe" | "clarinet" | "bassoon"
  | "horn" | "trumpet" | "trombone" | "tuba"
  | "timpani" | "glock";
export const INSTS: Inst[] = ["piano", "strings", "brass", "viola", "cello", "contrabass", "pizz", "harp", "flute", "oboe", "clarinet", "bassoon", "horn", "trumpet", "trombone", "tuba", "timpani", "glock"];

/** `loop`: the sustain loop of a held note, in seconds of its file. */
type Zone = { note: number; file: string; loop?: [number, number] };
type Def = { zones: Zone[]; /** seconds the recorded notes last */ len: number; release: number; gain: number; /** a held instrument: its notes loop */ held?: boolean };

// filled by tools/instruments.py: the notes recorded for each instrument
import manifest from "./instruments.json";
const DEFS = manifest as unknown as Record<Inst, Def>;

const buffers = new Map<string, AudioBuffer>();
const loading = new Map<string, Promise<void>>();

// decoded once, at 48 kHz, for every player (a buffer plays in any context)
let decoder: OfflineAudioContext | null = null;

/** Starts loading an instrument (all its notes); resolves when ready. */
export function loadInst(inst: Inst): Promise<void> {
  const k = inst;
  if (ready(inst)) return Promise.resolve();
  let p = loading.get(k);
  if (!p) {
    const def = DEFS[inst];
    p = Promise.all(
      (def?.zones ?? []).map(async (z) => {
        if (buffers.has(z.file)) return;
        const r = await fetch(`/instruments/${z.file}`);
        if (!r.ok) throw new Error(z.file);
        decoder ??= new OfflineAudioContext(1, 1, 48000);
        buffers.set(z.file, await decoder.decodeAudioData(await r.arrayBuffer()));
      }),
    ).then(() => {});
    p.catch(() => loading.delete(k));
    loading.set(k, p);
  }
  return p;
}

/** All of an instrument's notes are decoded. */
export function ready(inst: Inst): boolean {
  const def = DEFS[inst];
  return !!def && def.zones.length > 0 && def.zones.every((z) => buffers.has(z.file));
}

/** One note of a recorded instrument, from `at` for `dur` seconds (Infinity: held until its
 *  stop is called, as a key is), into `out`. Null when the instrument is not loaded yet (the
 *  caller plays its synth meanwhile). */
export function playInst(ctx: BaseAudioContext, out: AudioNode, inst: Inst, n: number, vel: number, at: number, dur: number): { stop: (t: number) => void } | null {
  if (!ready(inst)) {
    void loadInst(inst).catch(() => {});
    return null;
  }
  const def = DEFS[inst];
  const z = def.zones.reduce((a, b) => (Math.abs(b.note - n) < Math.abs(a.note - n) ? b : a));
  const buf = buffers.get(z.file)!;
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = 2 ** ((n - z.note) / 12);
  // held: round its steady part as long as the note lasts
  const looped = !!(def.held && z.loop);
  if (looped) {
    src.loop = true;
    src.loopStart = z.loop![0];
    src.loopEnd = z.loop![1];
  }
  const amp = ctx.createGain();
  // softer notes darker too, as a player plays them
  const lp = ctx.createBiquadFilter();
  lp.type = "lowpass";
  lp.frequency.value = 2500 + 15000 * vel * vel;
  src.connect(lp);
  lp.connect(amp);
  amp.connect(out);
  const level = def.gain * (0.25 + 0.75 * vel);
  amp.gain.setValueAtTime(level, at);
  const natural = looped ? Infinity : at + buf.duration / src.playbackRate.value;
  const release = (t: number) => {
    const end = Math.min(t, natural);
    amp.gain.cancelScheduledValues(end);
    amp.gain.setTargetAtTime(0, end, def.release / 3);
    try {
      src.stop(end + def.release * 2 + 0.05);
    } catch {
      /* already over */
    }
  };
  src.start(at);
  if (Number.isFinite(dur)) release(at + dur);
  else if (Number.isFinite(natural)) src.stop(natural + 0.05);
  // let go after it ends (a held note with no end: when its stop is called)
  if (Number.isFinite(dur) || Number.isFinite(natural))
    setTimeout(() => amp.disconnect(), (Math.min(natural, at + (Number.isFinite(dur) ? dur : 1e9)) - ctx.currentTime + def.release * 2 + 1) * 1000);
  else src.onended = () => amp.disconnect();
  return { stop: release };
}

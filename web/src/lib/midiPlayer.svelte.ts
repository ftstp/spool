// Hearing a MIDI file: read here (a Standard MIDI File, fetched through the app's media
// protocol, never written), played by a small Web Audio synth with four sounds: piano, synth,
// bass and drums. "Auto" picks the sound from what the file is (its kind on the MIDI page,
// then per channel: channel 10 and drum files are drums, a GM bass program or a low line is
// bass, a GM piano program or stacked notes is piano, the rest a synth).
import * as api from "./api";
import { t } from "./i18n/index.svelte";
import { player, exclusive, queueHooks } from "./player.svelte";
import { clampRate, lockRate, type Tempo } from "./tempo";
import { clampSemis, pitchNode, pitchReady, setPitch, type Pitch } from "./pitch";
import { vol, onVolume } from "./volume.svelte";
import type { Patch } from "./patch";

export type Sound = "piano" | "synth" | "bass" | "drums";
export type Choice = "auto" | Sound;
export const SOUNDS: Choice[] = ["auto", "piano", "synth", "bass", "drums"];

/** `dn`: for a drum, the General MIDI piece it plays (a kit written an octave or two higher,
 *  or laid out otherwise, is brought back to it: see `mapDrums`). */
/** `pt`: a project track's own sound (see patch.ts), unless a sound was chosen by hand
 *  (`man`); `b`: its pitch bend, [seconds from its start, semitones]. */
type Note = { t: number; d: number; n: number; v: number; ch: number; prog: number; trk: number; snd: Sound; dn?: number; pt?: Patch; man?: boolean; b?: [number, number][] };
/** A piece of a sample heard in a project: at `t` seconds for `d`, read from `off` seconds
 *  into `url` at `rate` times its speed. */
export type AudioSeg = { t: number; d: number; url: string; off: number; rate: number; trk: number };
/** `bpm`: the file's own tempo at its start (null: SMPTE time, no tempo). `audio`: a whole
 *  project, its audio clips beside its notes, sorted by `t`. */
type Song = { notes: Note[]; len: number; bpm: number | null; audio?: AudioSeg[] };

// samples decoded for "Play the project", the last few kept (a project's stems can be big)
const DECODED = new Map<string, AudioBuffer | null>();

// Long recordings (a tape digitised, an hour of a live set) are not decoded: a decoded file
// takes about twice its size on disk in memory, and a project of 15 GB of them made the window
// run out of memory and close. Past this size a file is read as it plays, the way the player
// plays a render (a stream, a little at a time), and starts where its clip starts.
const STREAM_BYTES = 48e6;
const SIZES = new Map<string, number>();
/** A file's size, asked for its first byte (0: unknown). */
async function sizeOf(u: string): Promise<number> {
  const known = SIZES.get(u);
  if (known !== undefined) return known;
  let n = 0;
  try {
    const res = await fetch(api.mediaUrl(u), { headers: { Range: "bytes=0-0" } });
    n = Number(res.headers.get("content-range")?.split("/")[1]) || 0;
    if (!n && res.status === 200) n = Number(res.headers.get("content-length")) || 0;
    void res.body?.cancel().catch(() => {});
  } catch {
    /* unknown: decoded as before */
  }
  SIZES.set(u, n);
  return n;
}
const streamed = (u: string) => (SIZES.get(u) ?? 0) > STREAM_BYTES;

// ---------- the file ----------

/** A Standard MIDI File (also inside a RIFF RMID): its notes in seconds, and its length
 *  rounded up to whole bars (a clip loops on its bars). */
export function parseMidi(buf: ArrayBuffer): Song {
  const b = new Uint8Array(buf);
  const tag = (i: number) => String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3]);
  const u32 = (i: number) => ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0;
  const u16 = (i: number) => (b[i] << 8) | b[i + 1];
  let h = 0;
  while (h + 14 <= b.length && tag(h) !== "MThd") h++;
  if (h + 14 > b.length) throw new Error("not a MIDI file");
  const ntr = u16(h + 10);
  const div = u16(h + 12);
  let p = h + 8 + u32(h + 4);

  type On = { tick: number; v: number; prog: number };
  const raw: { tick: number; end: number; n: number; v: number; ch: number; prog: number; trk: number }[] = [];
  const tempos: { tick: number; us: number }[] = [];
  let sig: [number, number] | null = null;
  let last = 0;
  for (let trk = 0; trk < ntr && p + 8 <= b.length; trk++) {
    while (p + 8 <= b.length && tag(p) !== "MTrk") p += 8 + u32(p + 4);
    if (p + 8 > b.length) break;
    const end = Math.min(b.length, p + 8 + u32(p + 4));
    let q = p + 8;
    let tick = 0;
    let status = 0;
    const prog = new Array(16).fill(0);
    const open = new Map<number, On[]>();
    const vlq = () => {
      let v = 0;
      for (let k = 0; k < 4 && q < end; k++) {
        const c = b[q++];
        v = (v << 7) | (c & 0x7f);
        if (!(c & 0x80)) break;
      }
      return v;
    };
    while (q < end) {
      tick += vlq();
      let st = b[q];
      if (st & 0x80) {
        q++;
        if (st < 0xf0) status = st;
      } else st = status;
      if (st === 0xff) {
        const type = b[q++];
        const len = vlq();
        if (type === 0x51 && len >= 3) tempos.push({ tick, us: (b[q] << 16) | (b[q + 1] << 8) | b[q + 2] });
        if (type === 0x58 && len >= 2 && !sig) sig = [b[q], 2 ** b[q + 1]];
        q += len;
        if (type === 0x2f) break;
      } else if (st === 0xf0 || st === 0xf7) {
        q += vlq();
      } else if (st >= 0x80) {
        const hi = st & 0xf0;
        const ch = st & 15;
        if (hi === 0xc0 || hi === 0xd0) {
          const d1 = b[q++];
          if (hi === 0xc0) prog[ch] = d1;
        } else {
          const n = b[q++];
          const v = b[q++];
          const key = ch * 128 + n;
          if (hi === 0x90 && v > 0) {
            const l = open.get(key) ?? [];
            l.push({ tick, v, prog: prog[ch] });
            open.set(key, l);
          } else if (hi === 0x80 || hi === 0x90) {
            const on = open.get(key)?.shift();
            if (on) raw.push({ tick: on.tick, end: tick, n, v: on.v, ch, prog: on.prog, trk });
          }
        }
      } else break; // a broken track: what was read is kept
      last = Math.max(last, tick);
    }
    // notes never released end with their track
    for (const [key, l] of open) for (const on of l) raw.push({ tick: on.tick, end: Math.max(tick, on.tick + 1), n: key % 128, v: on.v, ch: key >> 7, prog: on.prog, trk });
    p = end;
  }

  // ticks to seconds, through the tempo changes (120 bpm until the first)
  tempos.sort((a, c) => a.tick - c.tick);
  const smpte = div & 0x8000;
  const tpq = smpte ? 0 : div || 96;
  const sec = (tick: number) => {
    if (smpte) return tick / ((256 - (div >> 8)) * (div & 0xff) || 1000);
    let s = 0;
    let at = 0;
    let us = 500000;
    for (const x of tempos) {
      if (x.tick >= tick) break;
      s += ((x.tick - at) * us) / tpq / 1e6;
      at = x.tick;
      us = x.us;
    }
    return s + ((tick - at) * us) / tpq / 1e6;
  };
  const notes: Note[] = raw
    .map((r) => ({ t: sec(r.tick), d: Math.max(0.03, sec(r.end) - sec(r.tick)), n: r.n, v: r.v, ch: r.ch, prog: r.prog, trk: r.trk, snd: "synth" as Sound }))
    .sort((a, c) => a.t - c.t);
  const endAt = notes.reduce((m, x) => Math.max(m, x.t + x.d), sec(last));
  const [num, den] = sig ?? [4, 4];
  const bar = smpte ? 2 : (((num * 4) / den) * (tempos[0]?.tick === 0 ? tempos[0].us : 500000)) / 1e6;
  const len = Math.max(bar, Math.ceil(endAt / bar - 0.02) * bar);
  const bpm = smpte ? null : Math.round((60e6 / (tempos[0]?.tick === 0 ? tempos[0].us : 500000)) * 100) / 100;
  return { notes, len, bpm };
}

/** Which sound plays each note. */
function assign(song: Song, choice: Choice, kind: string) {
  if (choice !== "auto") {
    for (const x of song.notes) {
      x.snd = choice;
      x.man = true;
    }
    // a kit chosen by hand: its notes brought back to the pieces, track by track
    if (choice === "drums") {
      const byTrk = new Map<number, Note[]>();
      for (const x of song.notes) byTrk.set(x.trk, [...(byTrk.get(x.trk) ?? []), x]);
      for (const l of byTrk.values()) mapDrums(l);
    }
    return;
  }
  const byKind: Record<string, Sound> = { chords: "piano", bass: "bass", melody: "synth", drums: "drums" };
  // per track and channel: what its notes look like
  const parts = new Map<string, Note[]>();
  for (const x of song.notes) {
    const k = `${x.trk}:${x.ch}`;
    (parts.get(k) ?? parts.set(k, []).get(k)!).push(x);
  }
  for (const x of song.notes) x.man = false;
  for (const l of parts.values()) {
    // a project's track: its own sound says (its drums mapped only without their own samples)
    const pt = l[0].pt;
    if (pt) {
      const snd: Sound = pt.fam === "drums" ? "drums" : pt.fam === "piano" ? "piano" : ["bass", "sub", "reese", "bassgtr"].includes(pt.fam) ? "bass" : "synth";
      for (const x of l) x.snd = snd;
      if (snd === "drums" && !pt.zones.length) mapDrums(l);
      continue;
    }
    const ch = l[0].ch;
    const prog = l[0].prog;
    let snd: Sound;
    if (kind === "drums" || ch === 9) snd = "drums";
    else if (byKind[kind]) snd = byKind[kind];
    else if (prog >= 32 && prog <= 39) snd = "bass";
    else if (prog <= 7 && prog > 0) snd = "piano";
    else {
      const pitches = l.map((x) => x.n).sort((a, c) => a - c);
      const median = pitches[pitches.length >> 1];
      // stacked notes (chords): notes starting together
      let stacked = 0;
      for (let i = 1; i < l.length; i++) if (Math.abs(l[i].t - l[i - 1].t) < 0.02) stacked++;
      snd = median < 48 ? "bass" : stacked > l.length / 3 ? "piano" : "synth";
    }
    for (const x of l) x.snd = snd;
    if (snd === "drums") mapDrums(l);
  }
}

// the pieces of a kit as General MIDI numbers them (kick, rim, snare, clap, hats, crash, ride)
const GM_CORE = new Set([35, 36, 37, 38, 39, 40, 42, 44, 46, 49, 51, 57]);
/** A drum part brought back to General MIDI: shifted by whole octaves when that lands most of
 *  its notes on the usual pieces (a Drum Rack written from C3, a kit an octave up); else read
 *  by its shape: the lowest note is the kick, the most played the hats, the next the snare,
 *  the others toms. Without this, a kick written high sounded like a small high tom. */
function mapDrums(l: Note[]) {
  if (!l.length) return;
  const fit = (k: number) => l.filter((x) => GM_CORE.has(x.n + k)).length / l.length;
  let best = 0;
  for (const k of [0, -12, -24, -36, 12, 24]) if (fit(k) > fit(best) + 0.05) best = k;
  if (fit(best) >= 0.6) {
    for (const x of l) x.dn = x.n + best;
    return;
  }
  const count = new Map<number, number>();
  for (const x of l) count.set(x.n, (count.get(x.n) ?? 0) + 1);
  const pitches = [...count.keys()].sort((a, b) => a - b);
  const map = new Map<number, number>();
  map.set(pitches[0], 36);
  const rest = pitches.slice(1).sort((a, b) => (count.get(b) ?? 0) - (count.get(a) ?? 0));
  if (rest[0] !== undefined) map.set(rest[0], 42);
  if (rest[1] !== undefined) map.set(rest[1], 38);
  const toms = [41, 45, 48, 50, 46, 49];
  rest.slice(2).sort((a, b) => a - b).forEach((p, i) => map.set(p, toms[i % toms.length]));
  for (const x of l) x.dn = map.get(x.n) ?? x.n;
}

// ---------- the synth ----------

let noiseBuf: AudioBuffer | null = null;
function noise(ctx: BaseAudioContext) {
  if (!noiseBuf || noiseBuf.sampleRate !== ctx.sampleRate) {
    noiseBuf = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const d = noiseBuf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  }
  const s = ctx.createBufferSource();
  s.buffer = noiseBuf;
  s.loop = true;
  return s;
}
const hz = (n: number) => 440 * 2 ** ((n - 69) / 12);

function osc(ctx: BaseAudioContext, type: OscillatorType, f: number, to: AudioNode, at: number, stop: number, detune = 0) {
  const o = ctx.createOscillator();
  o.type = type;
  o.frequency.value = f;
  o.detune.value = detune;
  o.connect(to);
  o.start(at);
  o.stop(stop);
  return o;
}

/** One note, scheduled at `at` (AudioContext time), into `out`. */
export function voice(ctx: BaseAudioContext, out: AudioNode, x: Note, at: number) {
  if (x.pt && !x.man) return patchVoice(ctx, out, x, at);
  const vel = x.v / 127;
  const f = hz(x.n);
  const end = at + x.d;
  if (x.snd === "drums") return drum(ctx, out, x.dn ?? x.n, vel, at);
  const amp = ctx.createGain();
  amp.gain.value = 0;
  amp.connect(out);
  const lp = ctx.createBiquadFilter();
  lp.type = "lowpass";
  lp.connect(amp);
  if (x.snd === "piano") {
    // a struck string: bright then darker, dying away even while held
    const ring = 0.4 + 1.6 * Math.max(0, 1 - (x.n - 21) / 88);
    lp.frequency.setValueAtTime(Math.min(16000, f * 8 + 1200), at);
    lp.frequency.setTargetAtTime(f * 2.5 + 300, at + 0.01, ring / 2);
    amp.gain.setValueAtTime(0, at);
    amp.gain.linearRampToValueAtTime(0.22 * vel, at + 0.004);
    amp.gain.setTargetAtTime(0, at + 0.004, ring);
    amp.gain.setTargetAtTime(0, end, 0.09);
    const stop = end + 0.6;
    osc(ctx, "triangle", f, lp, at, stop);
    const h = ctx.createGain();
    h.gain.value = 0.35;
    h.connect(lp);
    osc(ctx, "sine", f * 2, h, at, stop);
    osc(ctx, "sawtooth", f, h, at, stop, 4);
  } else if (x.snd === "bass") {
    lp.Q.value = 3;
    lp.frequency.setValueAtTime(Math.min(4000, f * 10 + 200), at);
    lp.frequency.setTargetAtTime(f * 2 + 120, at + 0.005, 0.09);
    amp.gain.setValueAtTime(0, at);
    amp.gain.linearRampToValueAtTime(0.32 * vel, at + 0.006);
    amp.gain.setTargetAtTime(0.24 * vel, at + 0.006, 0.12);
    amp.gain.setTargetAtTime(0, end, 0.04);
    const stop = end + 0.3;
    osc(ctx, "sawtooth", f, lp, at, stop);
    const sub = ctx.createGain();
    sub.gain.value = 0.9;
    sub.connect(amp);
    osc(ctx, "sine", f, sub, at, stop);
  } else {
    // synth: two detuned saws, the filter opening then settling
    lp.Q.value = 2;
    lp.frequency.setValueAtTime(Math.min(12000, f * 7 + 600), at);
    lp.frequency.setTargetAtTime(f * 3 + 400, at + 0.01, 0.2);
    amp.gain.setValueAtTime(0, at);
    amp.gain.linearRampToValueAtTime(0.18 * vel, at + 0.015);
    amp.gain.setTargetAtTime(0.13 * vel, at + 0.015, 0.15);
    amp.gain.setTargetAtTime(0, end, 0.14);
    const stop = end + 0.9;
    osc(ctx, "sawtooth", f, lp, at, stop, -8);
    osc(ctx, "sawtooth", f, lp, at, stop, 8);
  }
}

// ---------- a track's own sound (a project's MIDI track, see patch.ts) ----------

/** The voice still sounding on each mono track (a bass, a lead), on this output: the next
 *  note ends it, or slides from it when the track glides. */
const MONO = new Map<string, { out: AudioNode; oscs: (OscillatorNode | AudioBufferSourceNode)[]; amp: GainNode; end: number; f: number; level: number }>();

function shaper(ctx: BaseAudioContext, k: number) {
  const ws = ctx.createWaveShaper();
  const c = new Float32Array(1024);
  for (let i = 0; i < c.length; i++) {
    const x = (i / (c.length - 1)) * 2 - 1;
    c[i] = Math.tanh(x * (1 + k * 6)) / Math.tanh(1 + k * 6);
  }
  ws.curve = c;
  return ws;
}

/** The pitch bend of a note on these sources' detune (cents), from its start. */
function bendOn(params: AudioParam[], b: [number, number][] | undefined, at: number) {
  if (!b) return;
  for (const p of params) {
    for (const [t, v] of b) {
      if (t <= 0) p.setValueAtTime(v * 100, at);
      else p.linearRampToValueAtTime(v * 100, at + t);
    }
  }
}

function patchVoice(ctx: BaseAudioContext, out: AudioNode, x: Note, at: number) {
  const pt = x.pt!;
  const vel = Math.max(0.05, x.v / 127);
  // a little life: no two notes quite alike (a repeated bass note stops sounding stamped)
  const hum = 1 + (Math.random() - 0.5) * 0.12;
  const f = hz(x.n);
  const end = at + x.d;
  // ---- the real samples (Simpler, Sampler, Drum Rack)
  if (pt.zones.length) {
    const pad = pt.zones.find((z) => z.note === x.n);
    const z = pad ?? pt.zones.find((z) => z.note < 0 && x.n >= z.lo && x.n <= z.hi) ?? (pt.fam === "drums" ? undefined : pt.zones.find((z) => z.note < 0));
    const buf = z ? DECODED.get(z.path) : null;
    if (z && buf) {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      // a pad plays as recorded; a sampler's zone follows the keyboard from its root
      src.playbackRate.value = pad ? 1 : 2 ** ((x.n - z.root) / 12);
      const g = ctx.createGain();
      const peak = 0.55 * vel;
      g.gain.setValueAtTime(0, at);
      g.gain.linearRampToValueAtTime(peak, at + 0.002);
      src.connect(g);
      g.connect(out);
      bendOn([src.detune], x.b, at);
      // its part of the file only (a sliced loop's pad: its slice, not the whole loop)
      const from = Math.min(Math.max(0, z.start || 0), Math.max(0, buf.duration - 0.001));
      const to = z.end > from ? Math.min(z.end, buf.duration) : buf.duration;
      const rate = src.playbackRate.value;
      if (pad || pt.fam === "drums") {
        // a hit rings out to its part's end (at most four seconds), with a short fade
        const stop = at + Math.min(4, (to - from) / rate);
        g.gain.setValueAtTime(peak, Math.max(at + 0.002, stop - 0.01));
        g.gain.linearRampToValueAtTime(0, stop);
        src.start(at, from);
        src.stop(stop);
      } else {
        g.gain.setTargetAtTime(0, Math.min(end, at + (to - from) / rate), 0.08 * pt.tail);
        src.start(at, from);
        src.stop(Math.min(end + 0.6 * pt.tail, at + (to - from) / rate));
      }
      return;
    }
    if (pt.fam === "drums") return drum(ctx, out, x.dn ?? x.n, vel, at);
  }
  if (pt.fam === "drums") return drum(ctx, out, x.dn ?? x.n, vel, at);
  // ---- a mono line: the next note ends the last one, or slides from it
  const key = `${x.trk}`;
  const prev = pt.mono ? MONO.get(key) : undefined;
  if (prev && prev.out === out && prev.end > at - 0.004) {
    if (pt.glide) {
      // the same voice goes on: its pitch slides, its end moves
      const glide = 0.07;
      for (const o of prev.oscs) {
        if (o instanceof OscillatorNode) {
          const ratio = o.frequency.value / prev.f;
          o.frequency.cancelScheduledValues(at);
          o.frequency.setValueAtTime(o.frequency.value, at);
          o.frequency.exponentialRampToValueAtTime(Math.max(1, f * ratio), at + glide);
          o.frequency.value = f * ratio;
        }
      }
      prev.amp.gain.cancelScheduledValues(at);
      prev.amp.gain.setTargetAtTime(prev.level * (0.75 + 0.25 * vel), at, 0.02);
      prev.amp.gain.setTargetAtTime(0, end, 0.05 * pt.tail);
      for (const o of prev.oscs) o.stop(end + 0.5 * pt.tail);
      prev.end = end;
      prev.f = f;
      return;
    }
    prev.amp.gain.cancelScheduledValues(at);
    prev.amp.gain.setTargetAtTime(0, at, 0.006);
  }
  // ---- the voice: oscillators, filter, drive, amp
  const amp = ctx.createGain();
  amp.gain.value = 0;
  let dest: AudioNode = out;
  if (pt.drive > 0) {
    const ws = shaper(ctx, pt.drive);
    const trim = ctx.createGain();
    trim.gain.value = 1 / (1 + pt.drive);
    ws.connect(trim);
    trim.connect(out);
    dest = ws;
  }
  amp.connect(dest);
  const lp = ctx.createBiquadFilter();
  lp.type = "lowpass";
  lp.connect(amp);
  const oscs: OscillatorNode[] = [];
  const o = (type: OscillatorType, mult: number, gain: number, det = 0, to: AudioNode = lp) => {
    const g = ctx.createGain();
    g.gain.value = gain;
    g.connect(to);
    const n = ctx.createOscillator();
    n.type = type;
    n.frequency.value = f * mult;
    n.detune.value = det + (Math.random() - 0.5) * 4;
    n.connect(g);
    oscs.push(n);
    return n;
  };
  // envelope: attack, the level held, decay toward it, release
  let A = 0.005, peak = 0.22, hold = 0.16, dec = 0.15, rel = 0.06;
  // filter: from `cut` (times the pitch) down to `cut2`, at `fdec`
  let cut = 6, cut2 = 3, fdec = 0.2, q = 1.5;
  switch (pt.fam) {
    case "bass":
      o("sawtooth", 1, 0.8); o("sine", 1, 0.9, 0, amp); o("square", 0.5, 0.25);
      A = 0.004; peak = 0.3; hold = 0.22; dec = 0.12; rel = 0.04; cut = 10; cut2 = 2.2; fdec = 0.09; q = 3;
      break;
    case "sub":
      o("sine", 1, 1); o("triangle", 2, 0.12);
      A = 0.003; peak = 0.38; hold = 0.32; dec = 0.3; rel = 0.08; cut = 20; cut2 = 12; fdec = 0.3; q = 0.7;
      break;
    case "reese":
      o("sawtooth", 1, 0.6, -14); o("sawtooth", 1, 0.6, 14); o("sawtooth", 0.5, 0.35, 7); o("sine", 1, 0.7, 0, amp);
      A = 0.01; peak = 0.24; hold = 0.2; dec = 0.3; rel = 0.08; cut = 5; cut2 = 3; fdec = 0.4; q = 2.5;
      break;
    case "bassgtr":
      o("triangle", 1, 0.9); o("sawtooth", 1, 0.35); o("sine", 1, 0.6, 0, amp);
      A = 0.003; peak = 0.34; hold = 0.1; dec = 0.35; rel = 0.05; cut = 14; cut2 = 3; fdec = 0.07; q = 1.2;
      break;
    case "lead":
      o("sawtooth", 1, 0.6, -7); o("square", 1, 0.35, 7);
      A = 0.01; peak = 0.17; hold = 0.14; dec = 0.2; rel = 0.12; cut = 8; cut2 = 4.5; fdec = 0.25; q = 2;
      break;
    case "pluck":
      o("sawtooth", 1, 0.6); o("triangle", 2, 0.25);
      A = 0.002; peak = 0.2; hold = 0.0; dec = 0.25; rel = 0.15; cut = 14; cut2 = 1.5; fdec = 0.08; q = 2;
      break;
    case "pad":
      o("sawtooth", 1, 0.4, -10); o("sawtooth", 1, 0.4, 10); o("triangle", 2, 0.2, 3);
      A = 0.35; peak = 0.1; hold = 0.09; dec = 0.5; rel = 0.6; cut = 4; cut2 = 3; fdec = 1; q = 0.8;
      break;
    case "organ":
      // drawbars: the fundamental and its harmonics, steady
      for (const [m, g] of [[0.5, 0.3], [1, 0.6], [2, 0.4], [3, 0.25], [4, 0.2], [8, 0.08]]) o("sine", m, g);
      A = 0.008; peak = 0.13; hold = 0.13; dec = 0.1; rel = 0.05; cut = 40; cut2 = 40; fdec = 1; q = 0.5;
      break;
    case "epiano": {
      // a tine: a sine struck by a decaying modulator (FM), then a softer body
      const car = o("sine", 1, 0.9);
      const mod = ctx.createOscillator();
      mod.frequency.value = f * 1;
      const mg = ctx.createGain();
      mg.gain.setValueAtTime(f * 2.5 * vel, at);
      mg.gain.setTargetAtTime(f * 0.2, at, 0.15);
      mod.connect(mg);
      mg.connect(car.frequency);
      oscs.push(mod);
      o("sine", 2, 0.12);
      A = 0.003; peak = 0.24; hold = 0.06; dec = 0.9; rel = 0.25; cut = 30; cut2 = 12; fdec = 0.5; q = 0.5;
      break;
    }
    case "brass":
      o("sawtooth", 1, 0.6, -4); o("sawtooth", 1, 0.5, 4);
      A = 0.05; peak = 0.17; hold = 0.15; dec = 0.2; rel = 0.12; cut = 2.5; cut2 = 6; fdec = 0.08; q = 1;
      break;
    case "piano":
      o("triangle", 1, 0.9); o("sine", 2, 0.3); o("sawtooth", 1, 0.25, 4);
      A = 0.004; peak = 0.22; hold = 0.0; dec = 0.9; rel = 0.12; cut = 8; cut2 = 2.5; fdec = 0.5; q = 0.6;
      break;
    default:
      o("sawtooth", 1, 0.5, -8); o("sawtooth", 1, 0.5, 8);
      A = 0.015; peak = 0.18; hold = 0.13; dec = 0.15; rel = 0.14; cut = 7; cut2 = 3; fdec = 0.2; q = 2;
  }
  if (pt.chorus && pt.fam !== "organ") o("sawtooth", 1, 0.2, 18);
  // velocity opens the filter and plays louder; a little life on the cut
  const vf = (0.55 + 0.6 * vel) * hum;
  lp.Q.value = q;
  lp.frequency.setValueAtTime(Math.min(18000, f * cut * vf + 150), at);
  lp.frequency.setTargetAtTime(Math.min(18000, f * cut2 * vf + 120), at + A, fdec);
  if (pt.wobble > 0) {
    // the filter moving (an auto filter, an LFO): about a quarter note at 120
    const lfo = ctx.createOscillator();
    lfo.frequency.value = 2;
    const lg = ctx.createGain();
    lg.gain.value = f * cut2 * pt.wobble * 2;
    lfo.connect(lg);
    lg.connect(lp.frequency);
    oscs.push(lfo);
  }
  if (pt.fam === "lead" || pt.fam === "brass") {
    // a slow vibrato after the attack
    const vib = ctx.createOscillator();
    vib.frequency.value = 5.2;
    const vg = ctx.createGain();
    vg.gain.setValueAtTime(0, at);
    vg.gain.linearRampToValueAtTime(f * 0.006, at + 0.4);
    vib.connect(vg);
    for (const n of oscs) if (n.type !== "sine" || pt.fam === "lead") vg.connect(n.frequency);
    oscs.push(vib);
  }
  const level = peak * vel;
  amp.gain.setValueAtTime(0, at);
  amp.gain.linearRampToValueAtTime(level, at + A);
  amp.gain.setTargetAtTime(Math.max(0.0001, hold * vel), at + A, dec);
  amp.gain.setTargetAtTime(0, end, rel * pt.tail);
  if (pt.fam === "sub") {
    // an 808: the pitch falls into the note
    for (const n of oscs) {
      n.frequency.setValueAtTime(n.frequency.value * 1.6, at);
      n.frequency.exponentialRampToValueAtTime(Math.max(1, n.frequency.value), at + 0.05);
    }
  }
  bendOn(oscs.filter((n) => n !== oscs[oscs.length - 1] || !(pt.wobble > 0)).map((n) => n.detune), x.b, at);
  const stop = end + rel * pt.tail * 7 + 0.05;
  for (const n of oscs) {
    n.start(at);
    n.stop(stop);
  }
  if (pt.mono) MONO.set(key, { out, oscs, amp, end, f, level: Math.max(0.0001, hold * vel) });
}

/** The General MIDI drum map, roughly: kick, snare, clap, hats, toms, cymbals, the rest a tick. */
function drum(ctx: BaseAudioContext, out: AudioNode, n: number, vel: number, at: number) {
  // each piece: its own envelope into the output
  const env = (peak: number, decay: number, attack = 0.001) => {
    const g = ctx.createGain();
    g.gain.setValueAtTime(0, at);
    g.gain.linearRampToValueAtTime(peak * vel, at + attack);
    g.gain.setTargetAtTime(0, at + attack, decay);
    g.connect(out);
    return { g, stop: at + attack + decay * 7 };
  };
  const tone = (from: number, to: number, sweep: number, peak: number, decay: number, type: OscillatorType = "sine") => {
    const e = env(peak, decay);
    const o = osc(ctx, type, from, e.g, at, e.stop);
    o.frequency.setValueAtTime(from, at);
    o.frequency.exponentialRampToValueAtTime(to, at + sweep);
  };
  const hiss = (type: BiquadFilterType, freq: number, peak: number, decay: number, q = 0.8, attack = 0.001) => {
    const e = env(peak, decay, attack);
    const s = noise(ctx);
    const f = ctx.createBiquadFilter();
    f.type = type;
    f.frequency.value = freq;
    f.Q.value = q;
    s.connect(f);
    f.connect(e.g);
    s.start(at, Math.random() * 0.5);
    s.stop(e.stop);
  };
  // the metal of hats and cymbals: six square waves at inharmonic ratios, filtered high
  const metal = (peak: number, decay: number, bright = 7000) => {
    const e = env(peak, decay);
    const hp = ctx.createBiquadFilter();
    hp.type = "highpass";
    hp.frequency.value = bright;
    const bp = ctx.createBiquadFilter();
    bp.type = "bandpass";
    bp.frequency.value = 10000;
    bp.Q.value = 0.6;
    hp.connect(bp);
    bp.connect(e.g);
    for (const r of [2, 3, 4.16, 5.43, 6.79, 8.21]) osc(ctx, "square", 40 * r * 1.7, hp, at, e.stop);
  };
  if (n === 35 || n === 36) {
    // kick: a pitch drop with a body, and the beater's click
    tone(150, 46, 0.09, 1, 0.12);
    tone(60, 44, 0.2, 0.5, 0.16);
    hiss("highpass", 3500, 0.25, 0.004);
  } else if (n === 38 || n === 40) {
    // snare: the drum's tone, and the wires
    tone(240, 180, 0.04, 0.45, 0.05, "triangle");
    hiss("highpass", 1800, 0.55, 0.07);
    hiss("bandpass", 4500, 0.3, 0.05, 0.6);
  } else if (n === 37) {
    // rim / side stick
    tone(1700, 1500, 0.01, 0.35, 0.012, "triangle");
    hiss("bandpass", 2500, 0.25, 0.01, 2);
  } else if (n === 39) {
    // clap: a few quick bursts, then the room
    for (const k of [0, 0.01, 0.021]) {
      const e = env(0.45, 0.006);
      const s = noise(ctx);
      const f = ctx.createBiquadFilter();
      f.type = "bandpass";
      f.frequency.value = 1300;
      f.Q.value = 1.1;
      s.connect(f);
      f.connect(e.g);
      s.start(at + k, Math.random() * 0.5);
      s.stop(at + k + 0.05);
    }
    hiss("bandpass", 1300, 0.35, 0.09, 1.1, 0.03);
  } else if (n === 42 || n === 44) {
    metal(0.18, 0.025);
    hiss("highpass", 9000, 0.12, 0.02);
  } else if (n === 46) {
    metal(0.16, 0.16);
    hiss("highpass", 8500, 0.1, 0.14);
  } else if (n === 49 || n === 52 || n === 55 || n === 57) {
    // crash / china / splash
    metal(0.13, 0.55, 5000);
    hiss("highpass", 6000, 0.14, 0.6, 0.5);
  } else if (n === 51 || n === 59) {
    metal(0.1, 0.35, 6000);
    tone(3100, 3000, 0.01, 0.05, 0.25, "triangle");
  } else if (n === 53) {
    // ride bell
    tone(2200, 2150, 0.01, 0.16, 0.3, "triangle");
    metal(0.06, 0.25, 6000);
  } else if ([41, 43, 45, 47, 48, 50].includes(n)) {
    // toms, low to high
    const f = 70 * 2 ** ((n - 41) / 9);
    tone(f * 1.5, f, 0.12, 0.7, 0.16);
    hiss("lowpass", 1200, 0.08, 0.04);
  } else if (n === 54 || n === 69 || n === 70 || n === 82) {
    // tambourine, cabasa, maracas, shaker
    hiss("highpass", 7000, 0.18, 0.04, 0.8, 0.008);
  } else if (n === 56) {
    // cowbell
    const e = env(0.18, 0.09);
    const bp = ctx.createBiquadFilter();
    bp.type = "bandpass";
    bp.frequency.value = 800;
    bp.connect(e.g);
    osc(ctx, "square", 540, bp, at, e.stop);
    osc(ctx, "square", 800, bp, at, e.stop);
  } else if (n >= 60 && n <= 68) {
    // bongos, congas, timbales: small skins
    const f = 180 * 2 ** ((n - 60) / 12);
    tone(f * 1.3, f, 0.04, 0.4, 0.08);
  } else {
    // anything else: a short noisy tick, not a pitched tom
    hiss("bandpass", 3000, 0.2, 0.02, 1.2);
  }
}

// ---------- the player ----------

const CACHE = new Map<string, Song>();
const KEEP = "spool.midi.sound";

class MidiPlayer implements Tempo, Pitch {
  /** The file playing (as the MIDI page names it), "" for none. */
  rel = $state("");
  playing = $state(false);
  /** Where it is in the clip, and the clip's length (seconds). */
  t = $state(0);
  len = $state(0);
  error = $state("");
  /** Samples of a project being read before it plays. */
  loading = $state(false);
  loop = $state(true);
  sound = $state<Choice>(load());
  /** Faster or slower (a MIDI file changes tempo without changing key), and transposed. */
  rate = $state(1);
  lock = $state(false);
  target = $state<number | null>(null);
  base = $state<number | null>(null);
  semis = $state(0);
  eff = $derived((this.lock ? lockRate(this.base, this.target) : null) ?? this.rate);
  /** In the player bar until closed: the file's name, and where its notes are (0..1 a slot,
   *  drawn like a waveform). */
  shown = $state(false);
  name = $state("");
  peaks = $state<number[] | null>(null);
  #used = 1;
  #kind = "";
  #song: Song | null = null;
  #ctx: AudioContext | null = null;
  #bus: GainNode | null = null;
  #start = 0;
  #idx = 0;
  #aidx = 0;
  #timer: ReturnType<typeof setInterval> | null = null;
  #raf = 0;

  /** Plays a MIDI file (a second click on the one playing stops it). `kind`: the MIDI page's
   *  kind of the file (chords, bass, melody, drums, multi), for the Auto sound. */
  /** `layer`: over the audio playing (the reference render is not paused), the first note
   *  `startIn` seconds from now (on the render's next bar). */
  async play(rel: string, kind = "", name = "", opts: { layer?: boolean; startIn?: number; sync?: { at: number }; muted?: number[] } = {}) {
    if (this.rel === rel && this.playing) return this.stop();
    this.stop();
    // a project: each track through its own gain (muted ones silent); anything else as one
    this.muted = opts.muted ?? [];
    this.#project = !!opts.muted;
    this.#tracks = new Map();
    this.#sync = opts.sync ?? null;
    if (this.rel !== rel) this.peaks = null;
    this.rel = rel;
    this.#kind = kind;
    this.error = "";
    this.shown = true;
    this.name = name || (rel.replace(/\\/g, "/").split("/").pop()?.replace(/\.midi?$/i, "") ?? rel);
    const clip = rel.startsWith("clip:");
    // notes handed over by the page (a clip of an arrangement): nothing to fetch
    const given = rel.startsWith("notes:");
    const url = clip || given ? (api.inApp || given ? rel : "") : api.mediaUrl(rel);
    if (!url) {
      this.error = t("midi.player.inApp");
      return;
    }
    try {
      let song = CACHE.get(rel);
      if (!song) {
        // a clip of a project: its notes from the base; a file: through the media protocol
        let buf: ArrayBuffer;
        if (clip) buf = await api.clipMidi(rel.slice(5));
        else {
          const res = await fetch(url);
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
          buf = await res.arrayBuffer();
        }
        song = parseMidi(buf);
        if (CACHE.size > 60) CACHE.delete(CACHE.keys().next().value!);
        CACHE.set(rel, song);
      }
      if (this.rel !== rel) return; // another file was clicked meanwhile
      // a project of audio only (recordings, stems) has no notes and plays all the same
      if (!song.notes.length && !song.audio?.length) {
        this.error = t("midi.player.empty");
        return;
      }
      assign(song, this.sound, kind);
      this.#song = song;
      this.base = song.bpm;
      this.peaks = notePeaks(song);
      // one sound at a time in the app
      if (!opts.layer) {
        exclusive.midi = () => this.stop();
        player.pause();
      }
      const ctx = (this.#ctx ??= new AudioContext());
      await ctx.resume();
      this.#bus = this.#newBus(ctx);
      this.#start = ctx.currentTime + 0.05 + Math.max(0, opts.startIn ?? 0);
      this.#used = this.eff;
      // over the render: its start where the render will be at `sync.at` (worked out once
      // loaded, the load taking what it takes), then kept there by the pump
      if (this.#sync) this.#start = this.#syncStart(ctx);
      this.#idx = 0;
      this.#aidx = 0;
      this.len = song.len;
      this.t = 0;
      this.playing = true;
      this.#pump();
      this.#timer = setInterval(() => this.#pump(), 40);
      const tick = () => {
        if (!this.playing || !this.#ctx) return;
        this.t = Math.max(0, Math.min(this.len, (this.#ctx.currentTime - this.#start) * this.#used));
        this.#raf = requestAnimationFrame(tick);
      };
      this.#raf = requestAnimationFrame(tick);
    } catch (e) {
      if (this.rel === rel) this.error = t("midi.player.unreadable") + (e instanceof Error && e.message ? ` (${e.message})` : "");
    }
  }

  // notes scheduled a quarter of a second ahead (a stop is heard at once)
  #pump() {
    const ctx = this.#ctx;
    const song = this.#song;
    const bus = this.#bus;
    if (!ctx || !song || !bus || !this.playing) return;
    const ahead = ctx.currentTime + 0.25;
    // the speed changed: the clip goes on from where it is, at the new speed
    if (this.eff !== this.#used) {
      const at = (ctx.currentTime - this.#start) * this.#used;
      this.#used = this.eff;
      this.#start = ctx.currentTime - at / this.#used;
    }
    const r = this.#used;
    // over the render: the clip's loop kept on it (two clocks drift apart; a jump in the
    // render, a click on its waveform, moves the clip with it)
    if (this.#sync && player.playing) {
      const L = song.len / r;
      let d = (this.#start - this.#syncStart(ctx)) % L;
      if (d > L / 2) d -= L;
      if (d < -L / 2) d += L;
      if (Math.abs(d) > 0.012) {
        this.#start -= d;
        if (d > 0) {
          // earlier now: the notes already scheduled are skipped back over
          const at = (ctx.currentTime - this.#start) * r;
          const i = song.notes.findIndex((n) => n.t >= at);
          if (at >= 0) this.#idx = i < 0 ? song.notes.length : i;
        }
      }
    }
    for (let guard = 0; guard < 64; guard++) {
      this.#pumpAudio(ctx, bus, song, ahead, r);
      const ns = song.notes;
      while (this.#idx < ns.length && this.#start + ns[this.#idx].t / r < ahead) {
        const x = ns[this.#idx];
        const n = x.snd === "drums" ? x.n : Math.max(0, Math.min(127, x.n + this.semis));
        voice(ctx, this.#trackOut(ctx, bus, x.trk), { ...x, n, d: x.d / r }, Math.max(ctx.currentTime, this.#start + x.t / r));
        this.#idx++;
      }
      if (this.#idx < ns.length || this.#aidx < (song.audio?.length ?? 0)) return;
      const endAt = this.#start + song.len / r;
      if (this.loop) {
        if (endAt > ahead) return;
        this.#start = endAt;
        this.#idx = 0;
        this.#aidx = 0;
      } else {
        if (ctx.currentTime >= endAt + 0.2) {
          const rel = this.rel;
          this.stop();
          // a list playing through: the next song
          player.midiEnded(rel);
        }
        return;
      }
    }
  }

  // the project's audio: each piece started when it comes within reach, from where the
  // playhead is in it (a seek lands in the middle of a clip)
  #pumpAudio(ctx: AudioContext, bus: GainNode, song: Song, ahead: number, r: number) {
    const segs = song.audio;
    if (!segs) return;
    while (this.#aidx < segs.length && this.#start + segs[this.#aidx].t / r < ahead) {
      this.#startPiece(ctx, bus, segs[this.#aidx++], r);
    }
  }
  /** One piece of audio started (from where the playhead is in it); nothing when its sample
   *  is not decoded (yet) or it is over. */
  #startPiece(ctx: AudioContext, bus: GainNode, x: NonNullable<Song["audio"]>[number], r: number) {
    if (streamed(x.url)) return this.#streamPiece(ctx, bus, x, r);
    const buf = DECODED.get(x.url);
    const now = ctx.currentTime;
    let begin = this.#start + x.t / r;
    const end = this.#start + (x.t + x.d) / r;
    if (!buf || end <= now + 0.01) return;
    const speed = x.rate * r;
    let off = x.off;
    // read from before the sample's start: silence until it starts
    if (off < 0) {
      begin += -off / speed;
      off = 0;
    }
    const from = Math.max(begin, now);
    off += (from - begin) * speed;
    if (off >= buf.duration || end <= from) return;
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.playbackRate.value = speed;
    src.connect(this.#trackOut(ctx, this.#audioIn(ctx, bus), x.trk));
    src.start(from, off, Math.min(buf.duration - off, (end - from) * speed));
  }
  // the long recordings playing (see STREAM_BYTES): stopped with the rest
  #streams = new Set<{ a: HTMLAudioElement; timers: ReturnType<typeof setTimeout>[]; node: MediaElementAudioSourceNode | null }>();
  #streamPiece(ctx: AudioContext, bus: GainNode, x: NonNullable<Song["audio"]>[number], r: number) {
    const now = ctx.currentTime;
    let begin = this.#start + x.t / r;
    const end = this.#start + (x.t + x.d) / r;
    if (end <= now + 0.01) return;
    const speed = x.rate * r;
    let off = x.off;
    if (off < 0) {
      begin += -off / speed;
      off = 0;
    }
    const from = Math.max(begin, now);
    off += (from - begin) * speed;
    const a = new Audio();
    a.crossOrigin = "anonymous";
    a.preload = "auto";
    a.preservesPitch = false;
    a.src = api.mediaUrl(x.url);
    const s = { a, timers: [] as ReturnType<typeof setTimeout>[], node: null as MediaElementAudioSourceNode | null };
    this.#streams.add(s);
    try {
      s.node = ctx.createMediaElementSource(a);
      s.node.connect(this.#trackOut(ctx, this.#audioIn(ctx, bus), x.trk));
    } catch {
      this.#streams.delete(s);
      return;
    }
    const stop = () => {
      s.timers.forEach(clearTimeout);
      a.pause();
      s.node?.disconnect();
      a.removeAttribute("src");
      a.load();
      this.#streams.delete(s);
    };
    // placed once the file says how long it is (the clip may start past its end)
    a.addEventListener(
      "loadedmetadata",
      () => {
        if (!this.#streams.has(s)) return;
        if (off >= a.duration) return stop();
        a.currentTime = off;
        a.playbackRate = speed;
        const wait = Math.max(0, from - ctx.currentTime);
        s.timers.push(setTimeout(() => this.#streams.has(s) && a.play().catch(stop), wait * 1000));
        s.timers.push(setTimeout(stop, Math.max(0, end - ctx.currentTime) * 1000 + 50));
      },
      { once: true },
    );
    a.addEventListener("ended", stop, { once: true });
  }
  #stopStreams() {
    for (const s of [...this.#streams]) {
      s.timers.forEach(clearTimeout);
      s.a.pause();
      s.node?.disconnect();
      s.a.removeAttribute("src");
      s.a.load();
    }
    this.#streams.clear();
  }
  #audioBus: { bus: GainNode; g: GainNode; p: AudioWorkletNode | null; pending?: boolean } | null = null;
  // the samples a little under the synth's notes, on the same output (faded with it)
  #audioIn(ctx: AudioContext, bus: GainNode) {
    if (this.#audioBus?.bus !== bus) {
      const g = ctx.createGain();
      g.gain.value = 0.7;
      g.connect(bus);
      this.#audioBus = { bus, g, p: null };
      void pitchReady(ctx);
      if (this.semis) void this.#pitchAudio(ctx, this.#audioBus);
    }
    return this.#audioBus.g;
  }

  #sync: { at: number } | null = null;
  // the AudioContext time at which the clip's start meets the render's second `at` (what
  // the context schedules is heard `outputLatency` later; the render's clock is what is heard)
  #syncStart(ctx: AudioContext) {
    const s = this.#sync!;
    return ctx.currentTime + (s.at - player.pos()) / player.speed() - (ctx.outputLatency || 0);
  }

  /** Tracks of a project not heard (their index in the arrangement): cut at once. */
  muted = $state<number[]>([]);
  // each track's gain, per output it feeds: the MIDI notes go to the bus, the audio clips to
  // the audio input in front of it (two outputs, one gain each per track; one map per output,
  // or the audio clips and the notes kept replacing each other's gains and a muted audio
  // track went on sounding)
  #tracks = new Map<AudioNode, Map<number, GainNode>>();
  #project = false;
  // each track of a project through its own gain, so a track is cut while it plays
  #trackOut(ctx: AudioContext, bus: AudioNode, trk: number): AudioNode {
    if (!this.#project) return bus;
    let outs = this.#tracks.get(bus);
    if (!outs) this.#tracks.set(bus, (outs = new Map()));
    let g = outs.get(trk);
    if (!g) {
      g = ctx.createGain();
      g.gain.value = this.muted.includes(trk) ? 0 : 1;
      g.connect(bus);
      outs.set(trk, g);
    }
    return g;
  }
  setMuted(trks: number[]) {
    this.muted = trks;
    const ctx = this.#ctx;
    if (!ctx) return;
    for (const outs of this.#tracks.values()) for (const [trk, g] of outs) g.gain.setTargetAtTime(trks.includes(trk) ? 0 : 1, ctx.currentTime, 0.01);
  }

  #master: DynamicsCompressorNode | null = null;
  /** A fresh output: the old one is faded out with the notes already scheduled on it. */
  #newBus(ctx: AudioContext) {
    if (!this.#master) {
      const m = ctx.createDynamicsCompressor();
      m.threshold.value = -14;
      m.ratio.value = 4;
      // a whole project (twenty tracks, real samples) adds up past full scale: a limiter
      // (fast, hard) then a soft clip that never lets a peak through, so it never crackles
      const lim = ctx.createDynamicsCompressor();
      lim.threshold.value = -6;
      lim.knee.value = 0;
      lim.ratio.value = 20;
      lim.attack.value = 0.002;
      lim.release.value = 0.12;
      const clip = ctx.createWaveShaper();
      const curve = new Float32Array(2049);
      for (let i = 0; i < curve.length; i++) {
        const x = (i / (curve.length - 1)) * 2 - 1;
        curve[i] = Math.tanh(x * 1.2) / Math.tanh(1.2) * 0.89;
      }
      clip.curve = curve;
      clip.oversample = "2x";
      // the app's volume knob, after everything
      const out = ctx.createGain();
      out.gain.value = vol.v;
      m.connect(lim);
      lim.connect(clip);
      clip.connect(out);
      out.connect(ctx.destination);
      onVolume((v) => out.gain.setTargetAtTime(v, ctx.currentTime, 0.02));
      this.#master = m;
    }
    const bus = ctx.createGain();
    bus.gain.value = 0.8;
    bus.connect(this.#master);
    return bus;
  }
  #fade(bus: GainNode | null) {
    this.#stopStreams();
    if (!bus || !this.#ctx) return;
    bus.gain.setTargetAtTime(0, this.#ctx.currentTime, 0.015);
    setTimeout(() => bus.disconnect(), 120);
  }

  /** Plays notes given in beats ((pitch, start, length)), as a clip of `len` beats at `bpm`:
   *  a clip of a song's arrangement, heard from its sheet. */
  async playNotes(
    key: string,
    name: string,
    notes: ([number, number, number] | [number, number, number, number, number])[],
    len: number,
    bpm: number,
    drums: boolean,
    extra: { vel?: number[]; bend?: ([number, number][] | null)[]; patch?: Patch } = {},
  ) {
    const s = 60 / (bpm > 0 ? bpm : 120);
    // a track's own samples (a Simpler, a Drum Rack): read before it plays
    const zones = [...new Set(extra.patch?.zones.map((z) => z.path) ?? [])].filter((u) => !DECODED.has(u));
    if (zones.length) {
      const ctx = (this.#ctx ??= new AudioContext());
      await Promise.all(
        zones.slice(0, 64).map(async (u) => {
          try {
            const res = await fetch(api.mediaUrl(u));
            if (!res.ok) throw new Error(String(res.status));
            DECODED.set(u, await ctx.decodeAudioData(await res.arrayBuffer()));
          } catch {
            DECODED.set(u, null);
          }
        }),
      );
    }
    // a 4th and 5th value: the track the note is on, and drums (1) or not (0): a whole
    // arrangement, each track given its own sound by Auto
    const song: Song = {
      notes: notes
        .map(([n, t, d, trk, dr], i) => ({
          t: t * s,
          d: Math.max(0.03, d * s),
          n,
          v: extra.vel?.[i] || 100,
          ch: (dr ?? (drums ? 1 : 0)) ? 9 : 0,
          prog: 0,
          trk: trk ?? 0,
          snd: "synth" as Sound,
          pt: extra.patch,
          b: extra.bend?.[i]?.map(([bt, v]) => [bt * s, v] as [number, number]),
        }))
        .sort((a, b) => a.t - b.t),
      len: Math.max(s, len * s),
      bpm,
    };
    CACHE.set("notes:" + key, song);
    return this.play("notes:" + key, drums ? "drums" : "", name);
  }

  /** A whole project: its MIDI notes as `playNotes`, and its audio clips (`audio` in beats).
   *  It starts as soon as the samples of its first seconds (from `extra.from`, in beats) are
   *  decoded; the others are decoded while it plays, each heard from where the playhead is
   *  once ready. It used to decode every sample first, muted tracks' too: a few long stems
   *  kept it silent for seconds. `missing`: how many samples could not be read, once known. */
  async playProject(
    key: string,
    name: string,
    notes: [number, number, number, number, number][],
    audio: { t: number; d: number; path: string; off: number; secs: number; trk: number }[],
    len: number,
    bpm: number,
    muted: number[] = [],
    extra: { vel?: number[]; bend?: ([number, number][] | null)[]; patches?: Record<number, Patch>; from?: number } = {},
  ): Promise<{ missing: Promise<number> }> {
    const s = 60 / (bpm > 0 ? bpm : 120);
    const from = extra.from ?? 0;
    const off = new Set(muted);
    // when each file is first heard, in beats after `from`: the soonest decoded first, the
    // muted tracks' and what is already past after the rest
    const need = new Map<string, number>();
    const put = (u: string, at: number) => need.set(u, Math.min(need.get(u) ?? Infinity, at));
    for (const a of audio) {
      const at = a.t + a.d <= from ? 1e9 + a.t : Math.max(0, a.t - from);
      put(a.path, off.has(a.trk) ? at + 1e6 : at);
    }
    for (const [trk, p] of Object.entries(extra.patches ?? {})) {
      const first = notes.find((n) => n[3] === Number(trk) && n[1] + n[2] > from);
      const at = first ? Math.max(0, first[1] - from) : 1e9;
      for (const z of p.zones) put(z.path, off.has(Number(trk)) ? at + 1e6 : at);
    }
    const urls = [...need.keys()].sort((a, b) => need.get(a)! - need.get(b)!);
    this.stop();
    const rel = "notes:" + key;
    this.rel = rel;
    this.name = name;
    this.shown = true;
    this.error = "";
    this.loading = true;
    const ctx = (this.#ctx ??= new AudioContext());
    const load = async (u: string) => {
      try {
        const res = await fetch(api.mediaUrl(u));
        if (!res.ok) throw new Error(String(res.status));
        DECODED.set(u, await ctx.decodeAudioData(await res.arrayBuffer()));
      } catch {
        DECODED.set(u, null);
      }
    };
    // the audio clips' sizes first (one byte asked of each): the long recordings are played
    // as they are read, not decoded (see STREAM_BYTES)
    const clipUrls = [...new Set(audio.map((a) => a.path))];
    for (let i = 0; i < clipUrls.length; i += 8) {
      await Promise.all(clipUrls.slice(i, i + 8).map(sizeOf));
      if (this.rel !== rel) return { missing: Promise.resolve(0) };
    }
    // what must be there before the first note: the files heard in the first 4 seconds
    const soon = 4 / s;
    const todo = urls.filter((u) => !DECODED.has(u) && !streamed(u));
    const first = todo.filter((u) => need.get(u)! < soon);
    const later = todo.filter((u) => need.get(u)! >= soon);
    try {
      // four at a time: a project of forty stems should not read them all at once
      for (let i = 0; i < first.length; i += 4) {
        await Promise.all(first.slice(i, i + 4).map(load));
        if (this.rel !== rel) return { missing: Promise.resolve(0) };
      }
    } finally {
      this.loading = false;
    }
    const song: Song = {
      notes: notes
        .map(([n, t, d, trk, dr], i) => ({
          t: t * s,
          d: Math.max(0.03, d * s),
          n,
          v: extra.vel?.[i] || 100,
          ch: dr ? 9 : 0,
          prog: 0,
          trk,
          snd: "synth" as Sound,
          pt: extra.patches?.[trk],
          b: extra.bend?.[i]?.map(([bt, v]) => [bt * s, v] as [number, number]),
        }))
        .sort((a, b) => a.t - b.t),
      // every piece: one whose sample is not decoded yet is skipped, then started late
      audio: audio.map((a) => ({ t: a.t * s, d: a.d * s, url: a.path, off: a.off, rate: a.secs / s, trk: a.trk })).sort((a, b) => a.t - b.t),
      len: Math.max(s, len * s),
      bpm,
    };
    CACHE.set(rel, song);
    this.rel = "";
    await this.play(rel, "", name, { muted });
    // the rest, while it plays
    const missing = (async () => {
      for (let i = 0; i < later.length; i += 4) {
        if (this.rel !== rel) return 0;
        const part = later.slice(i, i + 4);
        await Promise.all(part.map(load));
        for (const u of part) this.#late(rel, u);
      }
      // the samples of the last projects only
      const keep = new Set(urls);
      for (const u of [...DECODED.keys()]) if (!keep.has(u) && DECODED.size > urls.length + 40) DECODED.delete(u);
      return [...new Set(audio.map((a) => a.path))].filter((u) => !DECODED.get(u) && !streamed(u)).length;
    })();
    return { missing };
  }

  /** A sample decoded after the start: its pieces already passed over by the scheduler, still
   *  sounding now, start from where the playhead is in them. */
  #late(rel: string, url: string) {
    const ctx = this.#ctx;
    const song = this.#song;
    const bus = this.#bus;
    if (!ctx || !song?.audio || !bus || !this.playing || this.rel !== rel) return;
    for (let i = 0; i < this.#aidx && i < song.audio.length; i++) {
      if (song.audio[i].url === url) this.#startPiece(ctx, bus, song.audio[i], this.#used);
    }
  }

  /** Jumps to a place in the clip (0..1); started again first when it was stopped. */
  async seek(frac: number) {
    if (!this.playing) {
      if (!this.rel) return;
      await this.play(this.rel, this.#kind, this.name, this.#project ? { muted: this.muted } : {});
    }
    const ctx = this.#ctx;
    const song = this.#song;
    if (!ctx || !song || !this.playing) return;
    const at = Math.max(0, Math.min(0.999, frac)) * song.len;
    this.#fade(this.#bus);
    this.#bus = this.#newBus(ctx);
    this.#start = ctx.currentTime + 0.03 - at / this.#used;
    this.#idx = song.notes.findIndex((n) => n.t >= at);
    if (this.#idx < 0) this.#idx = song.notes.length;
    // the audio pieces still sounding at `at` start again from there
    this.#aidx = song.audio ? song.audio.findIndex((x) => x.t + x.d > at) : 0;
    if (this.#aidx < 0) this.#aidx = song.audio?.length ?? 0;
    this.#pump();
  }
  /** The bar's play button: the same file again, or stopped. */
  toggle() {
    if (this.playing) this.stop();
    else if (this.rel) this.play(this.rel, this.#kind, this.name, this.#project ? { muted: this.muted } : {});
  }
  /** Out of the player bar. */
  close() {
    this.stop();
    this.shown = false;
  }

  setRate(r: number) {
    this.lock = false;
    this.rate = clampRate(r);
  }
  setLock(on: boolean) {
    this.lock = on;
  }
  setTarget(bpm: number | null) {
    this.target = bpm && bpm > 0 ? bpm : null;
  }
  /** Transposed by semitones (drums stay as they are), -24 to +24. */
  setSemis(n: number) {
    this.semis = clampSemis(n);
    // the project's samples: through the transposing worklet (its notes move themselves)
    if (this.#ctx && this.#audioBus?.p) setPitch(this.#audioBus.p, this.#ctx, this.semis);
    else if (this.#ctx && this.#audioBus && this.semis) void this.#pitchAudio(this.#ctx, this.#audioBus);
  }
  async #pitchAudio(ctx: AudioContext, ab: { bus: GainNode; g: GainNode; p: AudioWorkletNode | null }) {
    if (ab.p || ab.pending) return;
    ab.pending = true;
    const p = await pitchNode(ctx, this.semis);
    ab.pending = false;
    if (!p || this.#audioBus !== ab) return;
    ab.g.disconnect();
    ab.g.connect(p);
    p.connect(ab.bus);
    ab.p = p;
    setPitch(p, ctx, this.semis);
  }

  /** The sound changed: the next notes play with it. */
  setSound(c: Choice) {
    this.sound = c;
    try {
      localStorage.setItem(KEEP, c);
    } catch {
      /* not kept: Auto next time */
    }
    if (this.#song) assign(this.#song, c, this.#kind);
  }

  stop() {
    this.#sync = null;
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
    cancelAnimationFrame(this.#raf);
    // a quick fade, then the notes already scheduled are cut off
    this.#fade(this.#bus);
    this.#bus = null;
    this.#song = null;
    this.playing = false;
    this.t = 0;
    if (exclusive.midi) exclusive.midi = null;
  }

  is(rel: string) {
    return this.rel === rel && this.playing;
  }
}

/** Where the notes are, as a waveform would show it: 200 slots, the louder and denser the
 *  higher. */
function notePeaks(song: Song): number[] {
  const n = 200;
  const p = new Array(n).fill(0);
  for (const x of song.notes) {
    const a = Math.floor((x.t / song.len) * n);
    const b = Math.min(n - 1, Math.floor(((x.t + x.d) / song.len) * n));
    for (let i = Math.max(0, a); i <= b; i++) p[i] += (x.v / 127) * (i === a ? 1 : 0.35);
  }
  // the audio clips too (a project of recordings only has no notes)
  for (const x of song.audio ?? []) {
    const a = Math.floor((x.t / song.len) * n);
    const b = Math.min(n - 1, Math.floor(((x.t + x.d) / song.len) * n));
    for (let i = Math.max(0, a); i <= b; i++) p[i] += 0.5;
  }
  const max = Math.max(...p, 0.01);
  return p.map((v) => Math.round((0.08 + 0.92 * Math.sqrt(v / max)) * 100) / 100);
}

function load(): Choice {
  try {
    const v = localStorage.getItem(KEEP) as Choice | null;
    return v && SOUNDS.includes(v) ? v : "auto";
  } catch {
    return "auto";
  }
}

export const midi = new MidiPlayer();
queueHooks.midi = midi;

// A MIDI keyboard plugged into the computer, played through a small instrument of its own
// (piano, organ, electric piano, synths, a bass: chosen from the player bar) with its own volume, to try things over a song while it
// plays. The input is chosen in Settings and kept; nothing is recorded or sent anywhere.
// Web MIDI is read only: no system-exclusive messages, so the system asks nothing.

export type KeySound = "piano" | "epiano" | "organ" | "synth" | "pad" | "pluck" | "lead" | "bass";
export const KEY_SOUNDS: KeySound[] = ["piano", "epiano", "organ", "synth", "pad", "pluck", "lead", "bass"];

const KEEP = "spool.keyboard";
type Kept = { input: string; sound: KeySound; volume: number };
const kept = ((): Kept => {
  try {
    const v = JSON.parse(localStorage.getItem(KEEP) || "{}");
    return { input: typeof v.input === "string" ? v.input : "", sound: KEY_SOUNDS.includes(v.sound) ? v.sound : "piano", volume: typeof v.volume === "number" ? v.volume : 0.7 };
  } catch {
    return { input: "", sound: "piano", volume: 0.7 };
  }
})();

type Voice = { stop: (at: number) => void; held: boolean };
const hz = (n: number) => 440 * Math.pow(2, (n - 69) / 12);

class Keyboard {
  supported = typeof navigator !== "undefined" && "requestMIDIAccess" in navigator;
  inputs = $state<{ id: string; name: string }[]>([]);
  input = $state(kept.input);
  sound = $state<KeySound>(kept.sound);
  volume = $state(kept.volume);
  error = $state("");
  /** Notes held now (the Settings page lights them). */
  down = $state<number[]>([]);
  #access: MIDIAccess | null = null;
  #bound: MIDIInput | null = null;
  #ctx: AudioContext | null = null;
  #out: GainNode | null = null;
  #voices = new Map<number, Voice>();
  #pedal = false;

  #save() {
    try {
      localStorage.setItem(KEEP, JSON.stringify({ input: this.input, sound: this.sound, volume: this.volume }));
    } catch {
      /* not kept */
    }
  }
  /** Asks for the MIDI inputs (once), binds the one kept. Called when the app opens. */
  async start() {
    if (!this.supported || this.#access) return;
    try {
      this.#access = await navigator.requestMIDIAccess({ sysex: false });
      this.#access.onstatechange = () => this.#list();
      this.#list();
    } catch (e) {
      this.error = String(e);
    }
  }
  #list() {
    const a = this.#access;
    if (!a) return;
    this.inputs = [...a.inputs.values()].map((i) => ({ id: i.id, name: i.name || i.id }));
    this.#bind();
  }
  #bind() {
    const want = this.input && this.#access?.inputs.get(this.input);
    if (this.#bound && this.#bound !== want) this.#bound.onmidimessage = null;
    this.#bound = want || null;
    if (this.#bound) this.#bound.onmidimessage = (m) => this.#message(m.data);
  }
  setInput(id: string) {
    this.input = id;
    this.#save();
    this.#bind();
    if (!id) this.allOff();
  }
  setSound(s: KeySound) {
    this.sound = s;
    this.#save();
  }
  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(1, v));
    this.#save();
    if (this.#out && this.#ctx) this.#out.gain.setTargetAtTime(this.volume * 0.5, this.#ctx.currentTime, 0.02);
  }

  #message(d: Uint8Array | null) {
    if (!d || d.length < 2) return;
    const kind = d[0] & 0xf0;
    const n = d[1];
    const v = d[2] ?? 0;
    if (kind === 0x90 && v > 0) this.noteOn(n, v);
    else if (kind === 0x80 || (kind === 0x90 && v === 0)) this.noteOff(n);
    else if (kind === 0xb0 && n === 64) {
      // the sustain pedal: notes let go of are kept until it is lifted
      this.#pedal = v >= 64;
      if (!this.#pedal) for (const [k, x] of this.#voices) if (!x.held) this.#release(k);
    } else if (kind === 0xb0 && (n === 123 || n === 120)) this.allOff();
  }
  #audio() {
    if (!this.#ctx) {
      this.#ctx = new AudioContext({ latencyHint: "interactive" });
      this.#out = this.#ctx.createGain();
      this.#out.gain.value = this.volume * 0.5;
      const comp = this.#ctx.createDynamicsCompressor();
      comp.threshold.value = -10;
      this.#out.connect(comp);
      comp.connect(this.#ctx.destination);
    }
    if (this.#ctx.state === "suspended") void this.#ctx.resume();
    return { ctx: this.#ctx, out: this.#out! };
  }
  noteOn(n: number, vel = 100) {
    const { ctx, out } = this.#audio();
    this.#release(n);
    const v = this.#voice(ctx, out, n, vel / 127);
    this.#voices.set(n, v);
    this.down = [...this.down.filter((x) => x !== n), n];
  }
  noteOff(n: number) {
    this.down = this.down.filter((x) => x !== n);
    const v = this.#voices.get(n);
    if (!v) return;
    v.held = false;
    if (!this.#pedal) this.#release(n);
  }
  #release(n: number) {
    const v = this.#voices.get(n);
    if (!v || !this.#ctx) return;
    v.stop(this.#ctx.currentTime);
    this.#voices.delete(n);
  }
  allOff() {
    for (const k of [...this.#voices.keys()]) this.#release(k);
    this.down = [];
  }

  #voice(ctx: AudioContext, out: AudioNode, n: number, vel: number): Voice {
    const f = hz(n);
    const now = ctx.currentTime;
    const amp = ctx.createGain();
    amp.gain.value = 0;
    amp.connect(out);
    const oscs: OscillatorNode[] = [];
    const osc = (type: OscillatorType, freq: number, gain: number, dest: AudioNode = amp, detune = 0) => {
      const o = ctx.createOscillator();
      o.type = type;
      o.frequency.value = freq;
      o.detune.value = detune;
      const g = ctx.createGain();
      g.gain.value = gain;
      o.connect(g);
      g.connect(dest);
      o.start(now);
      oscs.push(o);
      return o;
    };
    let release = 0.3;
    // a filter for the synths: opens with the velocity
    const filt = (freq: number, q = 0.7) => {
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.setValueAtTime(Math.min(18000, freq), now);
      lp.Q.value = q;
      lp.connect(amp);
      return lp;
    };
    if (this.sound === "synth") {
      // two saws a little apart, held
      const lp = filt(f * (3 + 6 * vel) + 300, 1.2);
      osc("sawtooth", f, 0.28, lp, -7);
      osc("sawtooth", f, 0.28, lp, 7);
      amp.gain.setTargetAtTime(0.36 * (0.5 + 0.5 * vel), now, 0.004);
      amp.gain.setTargetAtTime(0.26 * (0.5 + 0.5 * vel), now + 0.08, 0.3);
      release = 0.22;
    } else if (this.sound === "pad") {
      // three saws spread wide, a slow swell, a long tail
      const lp = filt(1400 + 1600 * vel, 0.5);
      for (const d of [-14, 0, 13]) osc("sawtooth", f, 0.16, lp, d);
      osc("sine", f / 2, 0.12, lp);
      amp.gain.setTargetAtTime(0.32 * (0.6 + 0.4 * vel), now, 0.18);
      release = 0.9;
    } else if (this.sound === "pluck") {
      // a saw through a filter closing at once: a short, bright pick
      const lp = filt(f * (8 + 10 * vel), 2);
      lp.frequency.setTargetAtTime(Math.max(200, f * 1.2), now + 0.005, 0.07);
      osc("sawtooth", f, 0.35, lp);
      osc("square", f * 2, 0.08, lp, 5);
      amp.gain.setTargetAtTime(0.5 * vel + 0.1, now, 0.002);
      amp.gain.setTargetAtTime(0, now + 0.01, 0.35);
      release = 0.12;
    } else if (this.sound === "lead") {
      // a square and a saw an octave apart, bright and held
      const lp = filt(f * 7 + 600, 1.5);
      osc("square", f, 0.2, lp);
      osc("sawtooth", f * 2, 0.08, lp, 4);
      amp.gain.setTargetAtTime(0.34 * (0.6 + 0.4 * vel), now, 0.003);
      release = 0.1;
    } else if (this.sound === "bass") {
      // a saw and a sine under it, the filter closing a little after the attack
      const lp = filt(f * (4 + 6 * vel) + 150, 1.4);
      lp.frequency.setTargetAtTime(f * 2.5 + 120, now + 0.02, 0.18);
      osc("sawtooth", f, 0.32, lp);
      osc("sine", f, 0.4, amp);
      amp.gain.setTargetAtTime(0.5 * (0.6 + 0.4 * vel), now, 0.003);
      release = 0.1;
    } else if (this.sound === "organ") {
      // drawbars 8', 4', 2 2/3', 2': held as long as the key, a slight vibrato
      const lfo = ctx.createOscillator();
      lfo.frequency.value = 6.2;
      const depth = ctx.createGain();
      depth.gain.value = 4;
      lfo.connect(depth);
      lfo.start(now);
      oscs.push(lfo);
      for (const [m, g] of [[1, 0.5], [2, 0.32], [3, 0.18], [4, 0.14], [0.5, 0.22]] as const) depth.connect(osc("sine", f * m, g).detune);
      amp.gain.setTargetAtTime(0.32 * (0.6 + 0.4 * vel), now, 0.006);
      release = 0.07;
    } else if (this.sound === "epiano") {
      // a bell on a sine: the tine fades quicker than the body
      const body = osc("sine", f, 0.55);
      void body;
      const mod = ctx.createOscillator();
      mod.frequency.value = f * 7;
      const mg = ctx.createGain();
      mg.gain.setValueAtTime(f * 2.2 * vel, now);
      mg.gain.setTargetAtTime(0, now, 0.25);
      mod.connect(mg);
      mod.start(now);
      oscs.push(mod);
      mg.connect(oscs[0].frequency);
      osc("sine", f * 2, 0.12);
      amp.gain.setTargetAtTime(0.42 * vel, now, 0.004);
      amp.gain.setTargetAtTime(0.1 * vel, now + 0.05, 1.4);
      release = 0.35;
    } else {
      // piano: a bright attack that dies away, through a filter closing with it
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.setValueAtTime(Math.min(16000, f * (6 + 10 * vel)), now);
      lp.frequency.setTargetAtTime(Math.max(400, f * 2.5), now, 0.6);
      lp.connect(amp);
      osc("triangle", f, 0.6, lp, -3);
      osc("triangle", f, 0.4, lp, 4);
      osc("sine", f * 2, 0.16, lp);
      osc("sine", f * 3, 0.06, lp);
      amp.gain.setTargetAtTime(0.45 * vel, now, 0.002);
      amp.gain.setTargetAtTime(0, now + 0.02, n < 60 ? 2.6 : 1.6);
      release = 0.25;
    }
    return {
      held: true,
      stop: (at) => {
        amp.gain.cancelScheduledValues(at);
        amp.gain.setValueAtTime(amp.gain.value, at);
        amp.gain.setTargetAtTime(0, at, release / 3);
        for (const o of oscs) o.stop(at + release * 2 + 0.05);
        setTimeout(() => amp.disconnect(), (release * 2 + 0.2) * 1000);
      },
    };
  }
}

export const keyboard = new Keyboard();

// A MIDI keyboard plugged into the computer, played through a small instrument of its own
// (piano, organ, electric piano, synths, a bass) with its own volume, to try things over a
// song while it plays. In the app the keyboard is read by the app itself (api.midiOpen) and
// let go when it is turned off, so Live can take it; in the browser preview, Web MIDI.
// Its input (by name: a number changes when another keyboard is plugged), on or off, sound
// and volume are kept. What is played is kept as takes (the history, 30 days, on this
// computer only): a pause of a few seconds ends a take.
import * as api from "./api";
import { playInst, loadInst, INSTS, type Inst } from "./instruments";
import { FAM_GROUPS } from "./patch";

export type KeySound = "piano" | "epiano" | "organ" | "synth" | "pad" | "pluck" | "lead" | "bass" | Inst;
/** The keyboard's own sounds (the synth's, the piano), then the orchestra in groups (the
 *  recorded instruments, as a project's track can play them). */
export const KEY_SOUNDS: KeySound[] = ["piano", "epiano", "organ", "synth", "pad", "pluck", "lead", "bass"];
export const KEY_GROUPS: [string, KeySound[]][] = FAM_GROUPS.map(([g, fams]) => [g, fams.filter((f): f is Inst => (INSTS as string[]).includes(f))]);
const ALL_SOUNDS: KeySound[] = [...KEY_SOUNDS, ...KEY_GROUPS.flatMap(([, x]) => x)];
/** A sound's name: the keyboard's own, or the orchestra's (as a track's sound menu says). */
export const keySoundKey = (k: KeySound) => ((KEY_SOUNDS as string[]).includes(k) ? `settings.keyboard.sounds.${k}` : `song.session.fam.${k}`);
const recorded = (s: KeySound): s is KeySound & Inst => (INSTS as string[]).includes(s);

const KEEP = "spool.keyboard";
type Kept = { input: string; name: string; on: boolean; sound: KeySound; volume: number };
const kept = ((): Kept => {
  try {
    const v = JSON.parse(localStorage.getItem(KEEP) || "{}");
    return {
      input: typeof v.input === "string" ? v.input : "",
      name: typeof v.name === "string" ? v.name : "",
      on: v.on !== false,
      sound: ALL_SOUNDS.includes(v.sound) ? v.sound : "piano",
      volume: typeof v.volume === "number" ? v.volume : 0.7,
    };
  } catch {
    return { input: "", name: "", on: true, sound: "piano", volume: 0.7 };
  }
})();

type Voice = { stop: (at: number) => void; held: boolean };
const hz = (n: number) => 440 * Math.pow(2, (n - 69) / 12);
/** A pause this long (no key held) ends a take. */
const TAKE_GAP_MS = 6000;

class Keyboard {
  supported = api.nativeMidi || (typeof navigator !== "undefined" && "requestMIDIAccess" in navigator);
  inputs = $state<{ id: string; name: string }[]>([]);
  input = $state(kept.input || kept.name);
  /** Turned off: the keyboard is let go (Live can use it); the choice is kept. */
  on = $state(kept.on);
  sound = $state<KeySound>(kept.sound);
  volume = $state(kept.volume);
  error = $state("");
  /** Notes held now (the Settings page lights them). */
  down = $state<number[]>([]);
  /** Goes up when a take is saved (the history lists them again). */
  takesVersion = $state(0);
  #access: MIDIAccess | null = null;
  #bound: MIDIInput | null = null;
  #listening = false;
  #opened = "";
  #ctx: AudioContext | null = null;
  #out: GainNode | null = null;
  #voices = new Map<number, Voice>();
  #pedal = false;
  #take: { t0: number; at: number; notes: [number, number, number, number][]; open: Map<number, number> } | null = null;
  #takeTimer = 0;
  #watch = 0;

  /** On, with an input chosen and plugged in. */
  get live() {
    return this.on && !!this.input && this.inputs.some((i) => i.id === this.input) && !this.error;
  }
  #save() {
    const name = this.inputs.find((i) => i.id === this.input)?.name ?? kept.name;
    const v = { input: this.input, name, on: this.on, sound: this.sound, volume: this.volume };
    kept.name = name;
    try {
      localStorage.setItem(KEEP, JSON.stringify(v));
    } catch {
      /* in the base below */
    }
    // in the base too: the web view's storage lost the choice (written, then gone after the
    // app was closed by an update)
    void api.setUiPref("ui.keyboard", v).catch(() => {});
  }
  /** The choice kept in the base, when the window's storage has none (or another one). */
  async #restore() {
    if (this.#restored) return;
    this.#restored = true;
    const v = await api.uiPref<Partial<Kept>>("ui.keyboard").catch(() => null);
    if (!v || typeof v.input !== "string") {
      // only in the window's storage (before 0.1.68): put in the base as well
      if (this.input) this.#save();
      return;
    }
    if (v.input === this.input && v.on === this.on) return;
    this.input = v.input;
    if (typeof v.name === "string") kept.name = v.name;
    if (typeof v.on === "boolean") this.on = v.on;
    if (v.sound && ALL_SOUNDS.includes(v.sound)) this.sound = v.sound;
    if (typeof v.volume === "number") this.volume = v.volume;
  }
  #restored = false;
  /** Lists the MIDI inputs (again: one plugged since shows) and binds the one kept. Called
   *  when the app opens and when the keyboard's panel opens. */
  async start() {
    if (!this.supported) return;
    await this.#restore();
    if (recorded(this.sound)) void loadInst(this.sound).catch(() => {});
    try {
      if (api.nativeMidi) {
        if (!this.#listening) {
          this.#listening = true;
          await api.onMidiIn((d) => this.#message(Uint8Array.from(d)));
        }
        await this.#refresh();
        this.#watchPorts();
      } else {
        if (!this.#access) {
          this.#access = await navigator.requestMIDIAccess({ sysex: false });
          this.#access.onstatechange = () => this.#list();
        }
        this.#list();
      }
    } catch (e) {
      this.error = String(e);
    }
  }
  /** The inputs plugged in now (native): a keyboard switched off leaves the list, and one
   *  switched back on comes back under the same name, to be opened again (the handle held
   *  while it was off reads nothing any more). */
  async #refresh() {
    const names = await api.midiInputs();
    const same = names.length === this.inputs.length && names.every((n, i) => this.inputs[i]?.id === n);
    if (!same) this.inputs = names.map((n) => ({ id: n, name: n }));
    // kept from an earlier version by its Web MIDI number: found again by its name
    if (this.input && !names.includes(this.input) && kept.name && names.includes(kept.name)) this.input = kept.name;
    // gone: let go of the dead handle, so it is opened anew when it comes back
    if (this.#opened && !names.includes(this.#opened)) {
      this.#opened = "";
      this.allOff();
      await api.midiClose().catch(() => {});
    }
    await this.#bind();
  }
  /** Windows says nothing when a keyboard is switched off or on: the list is read again
   *  every 1.5 s, only while the keyboard is on with an input chosen (one call, no repaint
   *  unless the list changed). */
  #watchPorts() {
    if (!api.nativeMidi) return;
    const want = this.on && !!this.input;
    if (want && !this.#watch) {
      this.#watch = window.setInterval(() => void this.#refresh().catch(() => {}), 1500);
    } else if (!want && this.#watch) {
      clearInterval(this.#watch);
      this.#watch = 0;
    }
  }
  #list() {
    const a = this.#access;
    if (!a) return;
    this.inputs = [...a.inputs.values()].map((i) => ({ id: i.id, name: i.name || i.id }));
    // the number of an input can change from one launch to the next: found again by name
    if (this.input && !this.inputs.some((i) => i.id === this.input) && kept.name) {
      const same = this.inputs.find((i) => i.name === kept.name);
      if (same) this.input = same.id;
    }
    void this.#bind();
  }
  async #bind() {
    const want = this.on && this.input ? this.input : "";
    if (api.nativeMidi) {
      if (want === this.#opened) return;
      this.#opened = "";
      if (!want || !this.inputs.some((i) => i.id === want)) {
        if (this.error) this.error = "";
        await api.midiClose().catch(() => {});
        return;
      }
      try {
        await api.midiOpen(want);
        this.#opened = want;
        if (this.error) this.error = "";
      } catch (e) {
        // tried again at each look at the list: free once Live lets go of it
        const msg = String(e);
        if (this.error !== msg) this.error = msg;
      }
      return;
    }
    const port = want ? this.#access?.inputs.get(want) : undefined;
    if (this.#bound && this.#bound !== port) {
      this.#bound.onmidimessage = null;
      void this.#bound.close?.();
    }
    this.#bound = port || null;
    if (this.#bound) this.#bound.onmidimessage = (m) => this.#message(m.data);
  }
  setInput(id: string) {
    this.input = id;
    if (id) this.on = true;
    this.#save();
    void this.#bind();
    this.#watchPorts();
    if (!id) this.allOff();
  }
  /** On or off at once (off: let go, for Live). */
  setOn(on: boolean) {
    this.on = on;
    this.#save();
    if (!on) {
      this.allOff();
      this.#endTake();
    }
    void this.#bind();
    this.#watchPorts();
  }
  setSound(s: KeySound) {
    this.sound = s;
    this.#save();
    if (recorded(s)) void loadInst(s).catch(() => {});
  }
  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(1, v));
    this.#save();
    if (this.#out && this.#ctx) this.#out.gain.setTargetAtTime(this.volume * 0.5, this.#ctx.currentTime, 0.02);
  }

  #message(d: Uint8Array | null) {
    if (!d || d.length < 2 || !this.on) return;
    const kind = d[0] & 0xf0;
    const n = d[1];
    const v = d[2] ?? 0;
    if (kind === 0x90 && v > 0) {
      this.noteOn(n, v);
      this.#record(n, v);
    } else if (kind === 0x80 || (kind === 0x90 && v === 0)) {
      this.noteOff(n);
      this.#recordOff(n);
    } else if (kind === 0xb0 && n === 64) {
      // the sustain pedal: notes let go of are kept until it is lifted
      this.#pedal = v >= 64;
      if (!this.#pedal) for (const [k, x] of this.#voices) if (!x.held) this.#release(k);
    } else if (kind === 0xb0 && (n === 123 || n === 120)) this.allOff();
  }

  // ---- the history: what the keyboard plays, as takes
  #record(n: number, vel: number) {
    clearTimeout(this.#takeTimer);
    const now = performance.now();
    if (!this.#take) this.#take = { t0: now, at: Date.now() / 1000, notes: [], open: new Map() };
    const t = this.#take;
    const prev = t.open.get(n);
    if (prev !== undefined) t.notes[prev][2] = Math.max(0.02, (now - t.t0) / 1000 - t.notes[prev][1]);
    t.open.set(n, t.notes.push([n, (now - t.t0) / 1000, 0, vel]) - 1);
  }
  #recordOff(n: number) {
    const t = this.#take;
    if (!t) return;
    const i = t.open.get(n);
    if (i !== undefined) {
      t.notes[i][2] = Math.max(0.02, (performance.now() - t.t0) / 1000 - t.notes[i][1]);
      t.open.delete(n);
    }
    if (!t.open.size) {
      clearTimeout(this.#takeTimer);
      this.#takeTimer = window.setTimeout(() => this.#endTake(), TAKE_GAP_MS);
    }
  }
  #endTake() {
    clearTimeout(this.#takeTimer);
    const t = this.#take;
    this.#take = null;
    if (!t) return;
    const end = (performance.now() - t.t0) / 1000;
    for (const i of t.open.values()) t.notes[i][2] = Math.max(0.02, end - t.notes[i][1]);
    // one note alone is a key touched, not a take
    if (t.notes.length < 2) return;
    const notes = t.notes.map(([p, s, l, v]) => [p, Math.round(s * 1000) / 1000, Math.round(l * 1000) / 1000, v] as [number, number, number, number]);
    api.takeSave(t.at, notes).then(() => this.takesVersion++).catch(() => {});
  }

  /** A take played back through the keyboard's sound (not recorded). Returns a stop. */
  playTake(notes: [number, number, number, number][], done?: () => void): () => void {
    const timers: number[] = [];
    const held = new Set<number>();
    for (const [p, s, l, v] of notes) {
      timers.push(window.setTimeout(() => { this.noteOn(p, v); held.add(p); }, s * 1000));
      timers.push(window.setTimeout(() => { this.noteOff(p); held.delete(p); }, (s + l) * 1000));
    }
    const end = notes.reduce((m, n) => Math.max(m, n[1] + n[2]), 0);
    timers.push(window.setTimeout(() => done?.(), end * 1000 + 300));
    return () => {
      for (const x of timers) clearTimeout(x);
      for (const p of held) this.noteOff(p);
      done?.();
    };
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
    // a recorded instrument, held as long as the key (the synth meanwhile it loads)
    if (recorded(this.sound)) {
      const h = playInst(ctx, out, this.sound, n, vel, ctx.currentTime, Infinity);
      if (h) return { held: true, stop: (at) => h.stop(at) };
    }
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

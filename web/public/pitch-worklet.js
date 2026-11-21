// Transposing audio without changing its speed: two read heads slide through a short delay
// line at the new pitch's rate, each faded in and out (a raised-cosine window) while the
// other one is heard, so the jump back of a head is never heard. At 0 semitones the sound
// goes straight through (no delay, nothing changed).
class PitchShift extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [{ name: "semis", defaultValue: 0, minValue: -24, maxValue: 24, automationRate: "k-rate" }];
  }
  constructor() {
    super();
    // ~60 ms windows: short enough to keep the attacks, long enough for low notes
    this.W = Math.round(sampleRate * 0.06);
    this.size = this.W * 2 + 8;
    this.buf = [new Float32Array(this.size), new Float32Array(this.size)];
    this.w = 0;
    this.phase = 0;
  }
  process(inputs, outputs, params) {
    const input = inputs[0];
    const output = outputs[0];
    if (!output.length) return true;
    const semis = params.semis[0];
    const n = output[0].length;
    if (!input.length) {
      for (const ch of output) ch.fill(0);
      return true;
    }
    if (Math.abs(semis) < 0.01) {
      for (let c = 0; c < output.length; c++) output[c].set(input[Math.min(c, input.length - 1)]);
      this.phase = 0;
      return true;
    }
    const ratio = Math.pow(2, semis / 12);
    const step = (1 - ratio) / this.W; // the delay grows (lower) or shrinks (higher)
    const { W, size } = this;
    const chans = Math.min(2, output.length);
    for (let i = 0; i < n; i++) {
      for (let c = 0; c < chans; c++) this.buf[c][this.w] = input[Math.min(c, input.length - 1)][i];
      let p1 = this.phase;
      let p2 = p1 + 0.5;
      if (p2 >= 1) p2 -= 1;
      const g1 = Math.sin(Math.PI * p1) ** 2;
      const g2 = Math.sin(Math.PI * p2) ** 2;
      const d1 = p1 * W + 2;
      const d2 = p2 * W + 2;
      for (let c = 0; c < chans; c++) {
        const b = this.buf[c];
        output[c][i] = g1 * this.tap(b, d1, size) + g2 * this.tap(b, d2, size);
      }
      this.w = (this.w + 1) % size;
      this.phase += step;
      if (this.phase >= 1) this.phase -= 1;
      if (this.phase < 0) this.phase += 1;
    }
    for (let c = chans; c < output.length; c++) output[c].set(output[0]);
    return true;
  }
  // the sample `d` samples back, between two samples read linearly
  tap(b, d, size) {
    let r = this.w - d;
    while (r < 0) r += size;
    const i = Math.floor(r);
    const f = r - i;
    return b[i] * (1 - f) + b[(i + 1) % size] * f;
  }
}
registerProcessor("pitch-shift", PitchShift);

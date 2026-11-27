// A file's tonal balance: its average spectrum, read here from the decoded audio (the file is
// only read). Up to 160 slices of 8192 samples spread over the whole file, each windowed
// (Hann) and measured by an FFT, averaged in power, then gathered in sixth-of-an-octave bands
// from 30 Hz to 18 kHz. Each curve is shifted so its loudest band sits at 0 dB: two files are
// compared by their balance, not by how loud they are.
import * as api from "./api";

export type Spectrum = { f: number[]; db: number[] };
/** The six ranges a mix is talked about in, for the summary under the curves. */
export const RANGES: [string, number, number][] = [
  ["sub", 20, 60],
  ["bass", 60, 250],
  ["lowmid", 250, 500],
  ["mid", 500, 2000],
  ["highmid", 2000, 6000],
  ["air", 6000, 20000],
];

const N = 8192;
const CACHE = new Map<string, Promise<Spectrum>>();

function fft(re: Float64Array, im: Float64Array) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j], re[i]];
      [im[i], im[j]] = [im[j], im[i]];
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const a = (-2 * Math.PI) / len;
    const wr = Math.cos(a), wi = Math.sin(a);
    for (let i = 0; i < n; i += len) {
      let cr = 1, ci = 0;
      for (let k = 0; k < len / 2; k++) {
        const ur = re[i + k], ui = im[i + k];
        const vr = re[i + k + len / 2] * cr - im[i + k + len / 2] * ci;
        const vi = re[i + k + len / 2] * ci + im[i + k + len / 2] * cr;
        re[i + k] = ur + vr;
        im[i + k] = ui + vi;
        re[i + k + len / 2] = ur - vr;
        im[i + k + len / 2] = ui - vi;
        const nr = cr * wr - ci * wi;
        ci = cr * wi + ci * wr;
        cr = nr;
      }
    }
  }
}

async function compute(path: string): Promise<Spectrum> {
  const res = await fetch(api.mediaUrl(path));
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const ctx = new OfflineAudioContext(1, 1, 44100);
  const buf = await ctx.decodeAudioData(await res.arrayBuffer());
  const sr = buf.sampleRate;
  const chans = Array.from({ length: buf.numberOfChannels }, (_, c) => buf.getChannelData(c));
  const len = buf.length;
  const slices = Math.max(1, Math.min(160, Math.floor(len / N)));
  const step = slices > 1 ? Math.floor((len - N) / (slices - 1)) : 0;
  const win = new Float64Array(N).map((_, i) => 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (N - 1)));
  const power = new Float64Array(N / 2);
  const re = new Float64Array(N), im = new Float64Array(N);
  let used = 0;
  for (let s = 0; s < slices; s++) {
    const at = s * step;
    let energy = 0;
    for (let i = 0; i < N; i++) {
      let x = 0;
      for (const ch of chans) x += ch[at + i] ?? 0;
      x /= chans.length;
      energy += x * x;
      re[i] = x * win[i];
      im[i] = 0;
    }
    // silence (an intro, a gap) says nothing of the balance
    if (energy / N < 1e-7) continue;
    fft(re, im);
    for (let k = 0; k < N / 2; k++) power[k] += re[k] * re[k] + im[k] * im[k];
    used++;
    // let the page breathe on long files
    if (s % 20 === 19) await new Promise((r) => setTimeout(r, 0));
  }
  const f: number[] = [], db: number[] = [];
  for (let lo = 30; lo < 18000; lo *= 2 ** (1 / 6)) {
    const hi = lo * 2 ** (1 / 6);
    const a = Math.max(1, Math.floor((lo * N) / sr)), b = Math.min(N / 2 - 1, Math.ceil((hi * N) / sr));
    let p = 0, n = 0;
    for (let k = a; k <= b; k++) {
      p += power[k];
      n++;
    }
    f.push(Math.sqrt(lo * hi));
    db.push(10 * Math.log10(Math.max(1e-20, p / Math.max(1, n) / Math.max(1, used))));
  }
  const top = Math.max(...db);
  return { f, db: db.map((x) => x - top) };
}

/** The average spectrum of a file (an absolute path or a name under the projects), kept. */
export function spectrum(path: string): Promise<Spectrum> {
  let p = CACHE.get(path);
  if (!p) {
    p = compute(path);
    p.catch(() => CACHE.delete(path));
    if (CACHE.size > 30) CACHE.delete(CACHE.keys().next().value!);
    CACHE.set(path, p);
  }
  return p;
}

/** How much more (dB) `a` has than `b` in each range, both curves aligned on their mean. */
export function rangeDiff(a: Spectrum, b: Spectrum): { range: string; db: number }[] {
  const mean = (s: Spectrum) => s.db.reduce((x, y) => x + y, 0) / s.db.length;
  const [ma, mb] = [mean(a), mean(b)];
  return RANGES.map(([range, lo, hi]) => {
    const idx = a.f.map((f, i) => [f, i] as const).filter(([f]) => f >= lo && f < hi).map(([, i]) => i);
    if (!idx.length) return { range, db: 0 };
    const avg = (s: Spectrum, m: number) => idx.reduce((x, i) => x + (s.db[i] - m), 0) / idx.length;
    return { range, db: Math.round((avg(a, ma) - avg(b, mb)) * 10) / 10 };
  });
}

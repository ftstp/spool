// One player for the whole app: whatever is playing keeps playing while you move between
// pages. Any card, list row or sheet can start a song's reference render (or another file).
import { vol, onVolume } from "./volume.svelte";
import * as api from "./api";
import { t, fmtNum } from "./i18n/index.svelte";
import { clampRate, lockRate, type Tempo } from "./tempo";
import { clampSemis, pitchNode, setPitch, type Pitch } from "./pitch";

const PEAKS = new Map<string, number[]>();
const INFO = new Map<string, Promise<api.AudioInfo | null>>();

/** A file's waveform and loudness: measured by the app (kept on disk), in the browser preview
 *  only the waveform, decoded here. */
export function infoFor(rel: string): Promise<api.AudioInfo | null> {
  if (!rel) return Promise.resolve(null);
  let p = INFO.get(rel);
  if (!p) {
    p = api.inApp
      ? api.audioInfo(rel).catch(() => null)
      : api.devAudioInfo(rel).then(
          (i) =>
            i ??
            peaksFor(api.mediaUrl(rel)).then((peaks) =>
              peaks ? { peaks, duration: 0, lufs: null, peak_db: 0, rms_db: 0, lra: null, short: [], sample_rate: 0, channels: 0 } : null,
            ),
        );
    INFO.set(rel, p);
  }
  return p;
}

/** Where the sound starts in a file, in seconds: past the silence it opens with (an export
 *  from bar 1 of a set whose music starts later). 0 when it starts at once, or not known. */
export function soundStart(i: api.AudioInfo | null): number {
  if (!i?.duration || !i.peaks.length) return 0;
  const k = i.peaks.findIndex((p) => p > 0.02);
  if (k <= 0) return 0;
  // a little before the first slice that sounds: the attack is not cut
  const at = (k / i.peaks.length) * i.duration - 0.15;
  return at > 0.5 && at < i.duration - 1 ? at : 0;
}

/** "-14.2 LUFS · peak -0.2 dB · RMS -9.9 dB · LRA 6 LU" (in English) */
export function loudness(i: api.AudioInfo | null): string {
  if (!i || !i.sample_rate) return "";
  const f = (x: number) => fmtNum(Math.round(x * 10) / 10);
  return [i.lufs != null && `${f(i.lufs)} LUFS`, t("song.player.peak", { v: f(i.peak_db) }), `RMS ${f(i.rms_db)} dB`, i.lra != null && `LRA ${f(i.lra)} LU`].filter(Boolean).join(" · ");
}

/** The waveform of a file: decoded once in the browser, then kept (memory + localStorage). */
export async function peaksFor(url: string): Promise<number[] | null> {
  if (!url) return null;
  const hit = PEAKS.get(url);
  if (hit) return hit;
  try {
    const kept = localStorage.getItem("pk:" + url);
    if (kept) {
      const p = JSON.parse(kept) as number[];
      PEAKS.set(url, p);
      return p;
    }
  } catch {
    /* no storage: decoded again next time */
  }
  try {
    const buf = await (await fetch(url)).arrayBuffer();
    const ctx = new AudioContext();
    const audio = await ctx.decodeAudioData(buf);
    ctx.close();
    const ch = audio.getChannelData(0);
    const n = 400;
    const step = Math.max(1, Math.floor(ch.length / n));
    const p: number[] = [];
    for (let i = 0; i < n; i++) {
      let m = 0;
      for (let j = i * step, e = Math.min(ch.length, j + step); j < e; j += 16) m = Math.max(m, Math.abs(ch[j]));
      p.push(Math.round(m * 100) / 100);
    }
    PEAKS.set(url, p);
    try {
      localStorage.setItem("pk:" + url, JSON.stringify(p));
    } catch {
      /* storage full: kept in memory only */
    }
    return p;
  } catch {
    return null;
  }
}

/** A second sound over the one playing: a loop heard with the reference render, at its
 *  tempo (the key kept), from the render's next bar. Stopped with the player. */
class Layer {
  rel = $state("");
  #a: HTMLAudioElement | null = null;
  constructor() {
    // the app's volume knob, on what is heard (a silent seek stays silent)
    onVolume((v) => {
      if (this.#a && this.#a.volume > 0) this.#a.volume = v;
      if (this.#gain && this.#ctx) this.#gain.gain.setTargetAtTime(v, this.#ctx.currentTime, 0.02);
    });
  }
  #timer = 0;
  #check = 0;
  /** A loop over the reference render. `sync.at`: the render's second where the loop's start
   *  falls; the loop starts silent, finds its place, is heard from the bar, then is kept on
   *  the render by its speed alone (see below). Without it, it starts `startIn` seconds from now. */
  async play(rel: string, rate: number, startIn: number, sync: { at: number } | null = null) {
    this.stop();
    const url = api.mediaUrl(rel);
    if (!url) return;
    const a = new Audio(url);
    a.loop = true;
    a.preload = "auto";
    a.preservesPitch = true;
    a.playbackRate = Math.max(0.5, Math.min(2, rate || 1));
    this.#a = a;
    this.rel = rel;
    if (!sync) {
      this.#timer = window.setTimeout(() => a.play().catch(() => {}), Math.max(0, startIn) * 1000);
      return;
    }
    // at the song's tempo already (within 1.5 %: a pitch change of a quarter of a semitone at
    // most, not heard): played by the audio engine, sample-accurate and looping without a gap
    if (Math.abs(a.playbackRate - 1) <= 0.015 && (await this.#engine(rel, url, a.playbackRate, sync))) return;
    if (this.#a !== a) return;
    // loaded first: the time it takes must not shift the start
    await new Promise<void>((r) => {
      a.oncanplaythrough = () => r();
      setTimeout(r, 4000);
      a.load();
    });
    if (this.#a !== a) return;
    const base = a.playbackRate;
    // where the loop must be now for its start to fall on the render's `at` (before `at`:
    // the end of the loop, as if it had already been going round)
    const want = () => {
      const p = ((player.pos() - sync.at) / player.speed()) * base;
      return a.duration ? ((p % a.duration) + a.duration) % a.duration : Math.max(0, p);
    };
    const gap = () => {
      let err = a.currentTime - want();
      if (a.duration) err = ((err + a.duration * 1.5) % a.duration) - a.duration / 2;
      return err;
    };
    // it starts silent, already turning; while silent it may jump to its place (nothing is
    // heard), and is heard from the bar once in place. Then only its speed moves, by a few
    // percent at most (the pitch kept): no jump, so no crackle. A large gap (the render was
    // moved by hand) makes it silent again while it finds its place.
    let heard = false;
    let unmuteAt = 0;
    // a media element's clock is only so precise: in place within 35 ms. A jump lands late
    // by about the same each time: learned from each (aimed that much ahead the next time).
    // After six it is taken as placed (its speed does the rest)
    let jumps = 0;
    let lead = 0;
    // right after a jump the element reports where it was asked to go, not where it plays:
    // measured again only once the jump is done, and a moment after
    let settledAt = 0;
    a.onseeked = () => (settledAt = performance.now() + 160);
    a.volume = 0;
    a.currentTime = want();
    await a.play().catch(() => {});
    // stopped (another one clicked) while it was starting
    if (this.#a !== a) return void a.pause();
    const tick = () => {
      if (this.#a !== a) return;
      if (!player.playing) {
        if (!a.paused) a.pause();
        heard = false;
        a.volume = 0;
        return;
      }
      if (a.paused) {
        a.currentTime = want();
        a.play().catch(() => {});
        return;
      }
      if (!heard && (a.seeking || performance.now() < settledAt)) return;
      const err = gap();
      if (!heard) {
        a.playbackRate = base;
        if (Math.abs(err) > 0.035 && jumps < 6) {
          if (jumps) lead = Math.max(0, Math.min(0.5, lead - err));
          jumps++;
          settledAt = performance.now() + 1000;
          a.currentTime = want() + lead;
          return;
        }
        const wait = (sync.at - player.pos()) / player.speed();
        if (wait <= 0.01) {
          heard = true;
          a.volume = vol.v;
        } else if (!unmuteAt && wait < 0.25) {
          // heard exactly on the bar
          unmuteAt = window.setTimeout(() => {
            unmuteAt = 0;
            if (this.#a === a) {
              heard = true;
              a.volume = vol.v;
            }
          }, wait * 1000);
        }
        return;
      }
      if (Math.abs(err) > 0.25) {
        heard = false;
        jumps = 0;
        a.volume = 0;
        a.currentTime = want();
        return;
      }
      a.playbackRate = base * (1 - Math.max(-0.04, Math.min(0.04, err * 2)));
    };
    this.#check = window.setInterval(tick, 80);
    tick();
  }
  // ---- the audio engine's way (Web Audio): the loop decoded once, started on the context's
  // clock where the render will be at `at`, then kept on the render by tiny speed changes
  // (a few thousandths: no pitch change heard, no jump)
  #ctx: AudioContext | null = null;
  #src: AudioBufferSourceNode | null = null;
  #gain: GainNode | null = null;
  #bufs = new Map<string, AudioBuffer>();
  async #engine(rel: string, url: string, rate: number, sync: { at: number }): Promise<boolean> {
    try {
      const ctx = (this.#ctx ??= new AudioContext());
      await ctx.resume();
      let buf = this.#bufs.get(url);
      if (!buf) {
        const res = await fetch(url);
        if (!res.ok) return false;
        buf = await ctx.decodeAudioData(await res.arrayBuffer());
        if (this.#bufs.size > 4) this.#bufs.delete(this.#bufs.keys().next().value!);
        this.#bufs.set(url, buf);
      }
      if (this.rel !== rel) return true;
      // the HTML element made for it is not used
      this.#a?.pause();
      this.#a = null;
      const dur = buf.duration;
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      src.playbackRate.value = rate;
      const g = ctx.createGain();
      g.gain.value = vol.v;
      src.connect(g);
      g.connect(ctx.destination);
      // where the render is heard now, on the context's clock (what the context plays is heard
      // `outputLatency` later than it is scheduled)
      const lat = ctx.outputLatency || 0;
      const renderAt = (t: number) => player.pos() + (t - ctx.currentTime + lat) * player.speed();
      // the loop's phase (seconds into the sample) the render asks for at context time `t`
      const wantAt = (t: number) => {
        const p = ((renderAt(t) - sync.at) / player.speed()) * rate;
        return ((p % dur) + dur) % dur;
      };
      const t0 = ctx.currentTime + 0.05;
      const wait = (sync.at - player.pos()) / player.speed();
      // before `at`: started then, from the top; after (a click mid-bar): from where it would be
      if (wait > 0.05) src.start(ctx.currentTime + wait - lat, 0);
      else src.start(t0, wantAt(t0));
      // its phase, followed through the speed changes
      let ref = wait > 0.05 ? { t: ctx.currentTime + wait - lat, ph: 0 } : { t: t0, ph: wantAt(t0) };
      let cur = rate;
      const phaseAt = (t: number) => (((ref.ph + Math.max(0, t - ref.t) * cur) % dur) + dur) % dur;
      this.#src = src;
      this.#gain = g;
      this.#check = window.setInterval(() => {
        if (this.#src !== src) return;
        if (!player.playing) return void g.gain.setTargetAtTime(0, ctx.currentTime, 0.01);
        g.gain.setTargetAtTime(vol.v, ctx.currentTime, 0.01);
        const t = ctx.currentTime;
        if (t < ref.t) return;
        let err = phaseAt(t) - wantAt(t);
        err = ((err + dur * 1.5) % dur) - dur / 2;
        // a large gap: the render was moved by hand; the loop starts again in its place
        if (Math.abs(err) > 0.2) {
          this.#restart(rel, url, rate, sync);
          return;
        }
        ref = { t, ph: phaseAt(t) };
        cur = rate * (1 - Math.max(-0.004, Math.min(0.004, err * 0.8)));
        src.playbackRate.setValueAtTime(cur, t);
      }, 200);
      return true;
    } catch {
      return false;
    }
  }
  #restart(rel: string, url: string, rate: number, sync: { at: number }) {
    this.#stopEngine();
    void this.#engine(rel, url, rate, sync);
  }
  #stopEngine() {
    clearInterval(this.#check);
    const g = this.#gain;
    if (g && this.#ctx) {
      g.gain.setTargetAtTime(0, this.#ctx.currentTime, 0.01);
      const s = this.#src;
      setTimeout(() => {
        try {
          s?.stop();
        } catch {
          /* already stopped */
        }
        g.disconnect();
      }, 80);
    }
    this.#src = null;
    this.#gain = null;
  }

  stop() {
    clearTimeout(this.#timer);
    clearInterval(this.#check);
    this.#stopEngine();
    this.#a?.pause();
    this.#a = null;
    this.rel = "";
  }
}
export const layer = new Layer();

/** A file the player plays now (for a play button showing pause). */
export const playingFile = (rel: string) => player.is(rel) && player.playing;
/** A play button's click: the same file paused / played on, another one started. */
export function playOrPause(rel: string, start: () => void) {
  if (player.is(rel)) player.toggle();
  else start();
}

/** A song of a list played through: its reference render when it has one (known by the
 *  list, or read from its sheet), else its newest project (`rel`). */
export type QItem = { id: number; temoin?: string | boolean | null; rel?: string | null; title?: string | null };
/** A list of songs a play button sits in (see PlayBtn): its songs in order, and its name. */
export type SongQueue = { songs: QItem[]; name: string };

/** What the queue needs from the other modules, set by them (they import this one):
 *  `project` plays a song's project with the app's synth and says the MIDI player's name for
 *  it (projectPlay.ts), `midi` is the MIDI player (midiPlayer.svelte.ts). */
export const queueHooks: {
  project: ((rel: string, title: string) => Promise<string>) | null;
  midi: { rel: string; t: number; seek: (f: number) => void } | null;
} = { project: null, midi: null };

/** A list being listened through (the Samples page, the MIDI page): the file playing, where
 *  it is in the list, and how to play the one before or after. The player bar shows
 *  previous / next while that file is the one playing. */
class Browse {
  cur = $state("");
  i = $state(0);
  n = $state(0);
  #step: ((d: number) => void) | null = null;
  set(cur: string, i: number, n: number, step: (d: number) => void) {
    this.cur = cur;
    this.i = i;
    this.n = n;
    this.#step = step;
  }
  /** Left by its page. */
  clear(step?: unknown) {
    if (step === undefined || step === this.#step) {
      this.cur = "";
      this.#step = null;
    }
  }
  step(d: number) {
    this.#step?.(d);
  }
}
export const browse = new Browse();

/** A song's tempo, from the catalogue (set by the app: this module does not load it). */
let songTempo: ((sid: number) => number | null) | null = null;
export const setSongTempo = (f: (sid: number) => number | null) => (songTempo = f);

/** Another player (the MIDI one) stopped when this one plays: one sound at a time. */
export const exclusive: { midi: (() => void) | null } = { midi: null };

class Player implements Tempo, Pitch {
  constructor() {
    // the app's volume knob: applied at once
    onVolume(() => this.#apply());
  }
  src = $state("");
  /** The file playing, as the app names it (for its waveform). */
  rel = $state("");
  /** The song it belongs to (for the link back to its sheet). */
  sid = $state<number | null>(null);
  title = $state("");
  name = $state("");
  playing = $state(false);
  t = $state(0);
  len = $state(0);
  error = $state("");
  /** Faster or slower, the key kept (the browser's own time-stretch): see `tempo.ts`. */
  rate = $state(1);
  lock = $state(false);
  target = $state<number | null>(null);
  /** The tempo of the file playing, when known (a sample's, a song's). */
  base = $state<number | null>(null);
  loop = $state(false);
  /** 0..1: lowered to compare two files at the same loudness. */
  volume = $state(1);
  eff = $derived((this.lock ? lockRate(this.base, this.target) : null) ?? this.rate);
  #audio: HTMLAudioElement | null = null;

  setRate(r: number) {
    this.lock = false;
    this.rate = clampRate(r);
    this.#apply();
  }
  setLock(on: boolean) {
    this.lock = on;
    this.#apply();
  }
  setLoop(on: boolean) {
    this.loop = on;
    this.#apply();
  }
  /** The tempo to lock to (the Samples page's BPM, a song's), null for none. */
  setTarget(bpm: number | null) {
    this.target = bpm && bpm > 0 ? bpm : null;
    this.#apply();
  }
  #apply() {
    const a = this.#audio;
    if (!a) return;
    a.preservesPitch = true;
    a.playbackRate = this.eff;
    a.loop = this.loop;
    a.volume = Math.max(0, Math.min(1, this.volume * vol.v));
  }
  setVolume(v: number) {
    this.volume = v;
    this.#apply();
  }

  /** Higher or lower by semitones, the speed kept. The element goes through the audio
   *  engine (a transposing worklet) from the first change on; at 0 the worklet lets the
   *  sound through untouched. */
  semis = $state(0);
  #pctx: AudioContext | null = null;
  #pnode: AudioWorkletNode | null = null;
  #routing: Promise<void> | null = null;
  setSemis(n: number) {
    this.semis = clampSemis(n);
    this.#route();
  }
  #route() {
    const a = this.#audio;
    if (this.#pnode && this.#pctx) return setPitch(this.#pnode, this.#pctx, this.semis);
    if (!a || !this.semis || this.#routing) return;
    this.#routing = (async () => {
      const ctx = (this.#pctx ??= new AudioContext());
      await ctx.resume().catch(() => {});
      const node = await pitchNode(ctx, this.semis);
      if (!node) {
        this.#routing = null;
        return;
      }
      ctx.createMediaElementSource(a).connect(node);
      node.connect(ctx.destination);
      this.#pnode = node;
      setPitch(node, ctx, this.semis);
    })();
  }

  /** Where the file is, read from the element itself (`t` is updated only a few times a
   *  second): what anything played over it is put on. */
  pos() {
    return this.#audio?.currentTime ?? this.t;
  }
  /** How fast it plays (1: as recorded). */
  speed() {
    return this.#audio?.playbackRate || 1;
  }

  #el() {
    if (!this.#audio) {
      const a = new Audio();
      // read through the audio engine once transposed: the media answer allows it (CORS)
      a.crossOrigin = "anonymous";
      a.preload = "metadata";
      a.onplay = () => (this.playing = true);
      a.onpause = () => (this.playing = false);
      a.onended = () => {
        this.playing = false;
        // a list playing through: the next song
        if (this.queue && this.queue.cur === this.src && this.queue.i < this.queue.items.length - 1) this.next();
      };
      a.ontimeupdate = () => (this.t = a.currentTime);
      a.onloadedmetadata = () => (this.len = a.duration);
      a.onerror = () => {
        this.playing = false;
        this.error = t("song.player.unreadable");
      };
      this.#audio = a;
    }
    return this.#audio;
  }

  /** Plays a file (relative to the projects root, or absolute) for a song. `bpm`: its tempo
   *  when known (a sample's); a song's reference render takes the song's. */
  /** Goes up at every play asked: an answer that comes back for an older click (a song's sheet
   *  read first) plays nothing. A slow answer used to start, pages later, a song clicked
   *  long before. */
  #ask = 0;

  play(rel: string, sid: number | null, title: string, at = 0, bpm: number | null = null) {
    this.#ask++;
    const url = api.mediaUrl(rel);
    if (!url) {
      this.error = t("song.player.playsInApp");
      this.src = "";
      this.title = title;
      return;
    }
    const a = this.#el();
    if (this.src !== url) {
      this.src = url;
      this.t = 0;
      this.len = 0;
      this.error = "";
      a.src = url;
    }
    this.sid = sid;
    this.title = title;
    this.rel = rel;
    this.base = bpm ?? (sid != null ? (songTempo?.(sid) ?? null) : null);
    // full volume: Compare lowers it again right after, for the louder side
    this.volume = 1;
    this.#apply();
    this.name = rel.replace(/\\/g, "/").split("/").pop() ?? rel;
    if (at) a.currentTime = at;
    if (!this.#queued) this.queue = null;
    exclusive.midi?.();
    a.play().catch(() => (this.error = t("song.player.unreadable")));
    // from the top: past the silence the file opens with, once its waveform is known (still
    // in that silence then; a click elsewhere meanwhile is left alone)
    if (!at) {
      const ask = this.#ask;
      infoFor(rel).then((i) => {
        const lead = soundStart(i);
        if (lead && ask === this.#ask && this.src === url && a.currentTime < lead - 0.2) a.currentTime = lead;
      });
    }
  }

  /** Songs played one after another (a list's, a release's tracks), the one at `i` playing:
   *  its reference render, or its project when it has none (through the MIDI player).
   *  `cur`: what plays for it (the render's address, the MIDI player's name for the project),
   *  so previous / next show only while the queue's song is the one heard. `wait`: the next
   *  one being read. Anything else played ends it. */
  queue = $state<{ items: QItem[]; i: number; name: string; cur: string; wait: boolean } | null>(null);
  #queued = false;
  async playQueue(items: (number | QItem)[], i: number, name: string) {
    this.queue = { items: items.map((x) => (typeof x === "number" ? { id: x } : x)), i, name, cur: "", wait: true };
    await this.#playAt(i, 1);
  }
  async #playAt(i: number, dir: 1 | -1) {
    const q = this.queue;
    if (!q || i < 0 || i >= q.items.length) return;
    q.i = i;
    q.wait = true;
    const it = q.items[i];
    const ask = ++this.#ask;
    const d = await api.song(it.id).catch(() => null);
    if (this.queue !== q || ask !== this.#ask) return;
    const rel = d?.song.temoin;
    const title = d?.song.title || it.title || "";
    if (rel) {
      this.#queued = true;
      this.play(rel, it.id, title);
      this.#queued = false;
      q.cur = this.src;
      q.wait = false;
      return;
    }
    if (it.rel && queueHooks.project) {
      // the project, with the app's synth: the render loaded before is paused, the bar
      // shows the project
      this.pause();
      try {
        q.cur = await queueHooks.project(it.rel, title);
      } finally {
        if (this.queue === q) q.wait = false;
      }
      return;
    }
    // neither (removed meanwhile): on to the one after (or before)
    q.wait = false;
    const j = i + dir;
    if (j >= 0 && j < q.items.length) return this.#playAt(j, dir);
  }
  /** The project the queue played ended by itself: the next song. */
  midiEnded(rel: string) {
    const q = this.queue;
    if (q && q.cur === rel && q.i < q.items.length - 1) this.next();
  }
  next() {
    if (this.queue) this.#playAt(this.queue.i + 1, 1);
  }
  /** Back to the start of the song, or to the one before when it has only just started. */
  prev() {
    const q = this.queue;
    if (!q) return;
    const m = queueHooks.midi;
    const onMidi = !!m && m.rel === q.cur;
    const at = onMidi ? m!.t : this.t;
    if (at > 3 || q.i === 0) {
      if (onMidi) m!.seek(0);
      else this.seek(0);
    } else this.#playAt(q.i - 1, -1);
  }
  /** Played outside the queue (a project from a list's button): the queue ends. */
  endQueue() {
    this.queue = null;
  }

  async playSong(sid: number, title: string) {
    if (this.sid === sid && this.src) return this.toggle();
    const ask = ++this.#ask;
    const d = await api.song(sid);
    // another play asked meanwhile: that one wins
    if (ask !== this.#ask) return;
    const rel = d?.song.temoin;
    if (rel) this.play(rel, sid, title);
  }

  toggle() {
    const a = this.#audio;
    if (!a || !this.src) return;
    if (a.paused) exclusive.midi?.();
    if (a.paused) this.#pctx?.resume().catch(() => {});
    if (a.paused) a.play().catch(() => (this.error = t("song.player.unreadable")));
    else {
      a.pause();
      layer.stop();
    }
  }

  /** Paused, whatever `playing` says yet: it follows the element's events, a moment later, and
   *  a `toggle` read from it then started the render again (switching a session from its
   *  render back to the project restarted the render and stopped the project). */
  pause() {
    const a = this.#audio;
    if (a && !a.paused) {
      a.pause();
      layer.stop();
    }
  }

  seek(frac: number) {
    const a = this.#audio;
    if (a && this.len) a.currentTime = Math.max(0, Math.min(1, frac)) * this.len;
  }

  stop() {
    layer.stop();
    this.#audio?.pause();
    this.src = "";
    this.sid = null;
    this.error = "";
  }

  /** Is this file the one loaded? */
  is(rel: string) {
    return !!this.src && this.src === api.mediaUrl(rel);
  }
}

export const player = new Player();
export const mmss = (s: number) => (isFinite(s) && s > 0 ? `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}` : "0:00");

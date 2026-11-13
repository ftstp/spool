// One player for the whole app: whatever is playing keeps playing while you move between
// pages. Any card, list row or sheet can start a song's reference render (or another file).
import * as api from "./api";
import { t, fmtNum } from "./i18n/index.svelte";

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
      : peaksFor(api.mediaUrl(rel)).then((peaks) =>
          peaks ? { peaks, duration: 0, lufs: null, peak_db: 0, rms_db: 0, lra: null, sample_rate: 0, channels: 0 } : null,
        );
    INFO.set(rel, p);
  }
  return p;
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

class Player {
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
  #audio: HTMLAudioElement | null = null;

  #el() {
    if (!this.#audio) {
      const a = new Audio();
      a.preload = "metadata";
      a.onplay = () => (this.playing = true);
      a.onpause = () => (this.playing = false);
      a.onended = () => (this.playing = false);
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

  /** Plays a file (relative to the projects root, or absolute) for a song. */
  play(rel: string, sid: number | null, title: string, at = 0) {
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
    this.name = rel.replace(/\\/g, "/").split("/").pop() ?? rel;
    if (at) a.currentTime = at;
    a.play().catch(() => (this.error = t("song.player.unreadable")));
  }

  /** A song's reference render, from its sheet (the lists only know that it has one). */
  async playSong(sid: number, title: string) {
    if (this.sid === sid && this.src) return this.toggle();
    const d = await api.song(sid);
    const rel = d?.song.temoin;
    if (rel) this.play(rel, sid, title);
  }

  toggle() {
    const a = this.#audio;
    if (!a || !this.src) return;
    if (a.paused) a.play().catch(() => (this.error = t("song.player.unreadable")));
    else a.pause();
  }

  seek(frac: number) {
    const a = this.#audio;
    if (a && this.len) a.currentTime = Math.max(0, Math.min(1, frac)) * this.len;
  }

  stop() {
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

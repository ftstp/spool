// A project played with the app's own synth and its audio clips' samples: what a song's sheet
// plays under "Project", and what a list's play button plays for a song with no reference
// render. Both go through here, so they sound the same and share their mutes.
import * as api from "./api";
import type { Arrangement } from "./api/bindings/Arrangement";
import type { ArrTrack } from "./api/bindings/ArrTrack";
import type { ArrClip } from "./api/bindings/ArrClip";
import { patchFor, noteBend, withFam, type Fam, type Patch } from "./patch";
import { midi, type PlayMix } from "./midiPlayer.svelte";
import { queueHooks } from "./player.svelte";
import { t } from "./i18n/index.svelte";
import { feature } from "./telemetry";

export type Note5 = [number, number, number, number, number];
export type AudioPiece = { t: number; d: number; path: string; off: number; secs: number; trk: number; g: number };

/** The key a project plays under (the sheet and the lists use the same one). */
export const sessionKey = (rel: string) => `session|${rel}`;

/** The project's length in beats, whole bars. */
export function totalBeats(a: Arrangement): number {
  const bpb = a.beats_per_bar || 4;
  return Math.max(1, Math.ceil(a.len / bpb)) * bpb;
}

// a track's own sound (instrument, preset, effects, samples, how it is played), kept per track
const patchCache = new WeakMap<ArrTrack, Patch>();
export function patchOf(tr: ArrTrack): Patch {
  let p = patchCache.get(tr);
  if (!p) {
    p = patchFor(tr, tr.clips.flatMap((c) => c.notes.map(([n, s, d]) => [n, c.start + s, d] as [number, number, number])));
    patchCache.set(tr, p);
  }
  return p;
}

/** Everything the project plays: its notes (with velocities, bends and each track's sound) and
 *  its audio clips. `picks`: the sounds set by hand, by track name (see storedSounds). */
export function projectParts(a: Arrangement, picks: Record<string, Fam> = {}) {
  const notes: Note5[] = [];
  const vel: number[] = [];
  const bend: ([number, number][] | null)[] = [];
  const patches: Record<number, Patch> = {};
  const audio: AudioPiece[] = [];
  a.tracks.forEach((tr, ti) => {
    const dr = tr.role === "drums" || tr.role === "perc" ? 1 : 0;
    if (tr.kind === "midi" && tr.clips.some((c) => c.notes.length)) patches[ti] = picks[tr.name] ? withFam(patchOf(tr), picks[tr.name]) : patchOf(tr);
    for (const c of tr.clips) {
      if (c.muted) continue;
      if (tr.kind === "midi")
        c.notes.forEach(([p, s, d], k) => {
          notes.push([p, c.start + s, d, ti, dr]);
          vel.push(c.vels[k] ?? 100);
          bend.push(noteBend(c.bend as [number, number][], s, d));
        });
      else if (c.sample) for (const [st, len, off, secs] of c.segs) audio.push({ t: c.start + st, d: len, path: c.sample, off, secs, trk: ti, g: c.gain ?? 1 });
    }
  });
  return { notes, vel, bend, patches, audio };
}

// ---- a track's sound set by hand (the app's guess can sound wrong), by name, kept per project
const SOUND = "spool.session.sound";
export function storedSounds(): Record<string, Record<string, Fam>> {
  try {
    return JSON.parse(localStorage.getItem(SOUND) || "{}");
  } catch {
    return {};
  }
}
export function storeSounds(m: Record<string, Record<string, Fam>>) {
  try {
    localStorage.setItem(SOUND, JSON.stringify(m));
  } catch {
    /* not kept */
  }
}

// ---- tracks not heard, by name, kept per project ("Project" on the sheet sets them)
const MUTE = "spool.session.mute2";
export function storedMutes(): Record<string, string[]> {
  try {
    return JSON.parse(localStorage.getItem(MUTE) || "{}");
  } catch {
    return {};
  }
}
export function storeMutes(m: Record<string, string[]>) {
  try {
    localStorage.setItem(MUTE, JSON.stringify(m));
  } catch {
    /* not kept */
  }
}
/** A track that plays a reference (a released song dropped in the set to compare with), or
 *  is named so. */
export const looksRef = (tr: { name: string; clips: ArrClip[] }, refPaths: string[]) =>
  /(^|[^a-z])(ref|refs|reference|référence)([^a-z]|$)/i.test(tr.name) || tr.clips.some((c) => c.sample && refPaths.includes(c.sample.toLowerCase()));
/** The tracks muted by name: what the user chose for this project, else, a first time, what
 *  Live itself does not play and the references (the user's own music is what "Project" is for). */
export function mutedNamesOf(a: Arrangement, rel: string, refPaths: string[], stored = storedMutes()): string[] {
  return stored[rel] ?? a.tracks.filter((tr) => tr.muted || looksRef(tr, refPaths)).map((tr) => tr.name);
}
/** Each track's groups, the nearest first (groups nest). */
export function groupsOf(a: Arrangement): string[][] {
  const byId = new Map(a.tracks.map((tr) => [tr.id, tr]));
  return a.tracks.map((tr) => {
    const out: string[] = [];
    let g = byId.get(tr.group);
    for (let guard = 0; g && guard < 16; guard++) {
      out.push(g.name);
      g = byId.get(g.group);
    }
    return out;
  });
}
/** The tracks not heard: muted (or in a muted group), or left out by a solo. */
export function silentTracks(a: Arrangement, muted: string[], solo: string[] = []): number[] {
  const gs = groupsOf(a);
  const isMuted = (n: string) => muted.includes(n);
  const isSolo = (n: string) => solo.includes(n);
  return a.tracks
    .map((tr, i) => {
      const g = gs[i] ?? [];
      const off = isMuted(tr.name) || g.some(isMuted);
      const heard = !solo.length || isSolo(tr.name) || g.some(isSolo);
      return off || !heard ? i : -1;
    })
    .filter((i) => i >= 0);
}

/** Where "from the start" starts, in beats: the first bar heard, or later the first locator
 *  (the marker put where the song begins: a stem often opens on bars of silence, which the
 *  clips cannot tell). A locator named after a later section (drop, break...) is not the
 *  start. Muted tracks and the references do not count: a reference dropped at bar 1, or a
 *  muted sketch, made it start on silence. */
export function songStart(a: Arrangement, notes: Note5[], audio: { t: number; trk: number }[], muted: number[]): number {
  const off = new Set(muted);
  const heard = [...notes.filter((n) => !off.has(n[3])).map((n) => n[1]), ...audio.filter((x) => !off.has(x.trk)).map((x) => x.t)];
  const first = heard.length ? Math.min(...heard) : 0;
  const bpb = a.beats_per_bar || 4;
  const bar = first >= bpb ? Math.floor(first / bpb) * bpb : 0;
  const later = /drop|break|build|chorus|refrain|couplet|verse|bridge|pont|outro|fin\b|end\b|hook|solo|interlude/i;
  const loc = a.locators.find((l) => l.at >= 0 && l.at < totalBeats(a) && !later.test(l.name));
  return loc ? Math.max(loc.at, bar) : bar;
}

/** Each track's mix as Live has it (its fader, Utilities, clear cuts and their automation),
 *  with the groups holding it: a group's fader and cuts act on every track inside. In beats. */
export function mixOf(a: Arrangement): Record<number, PlayMix> {
  const byId = new Map(a.tracks.map((t) => [t.id, t]));
  const out: Record<number, PlayMix> = {};
  a.tracks.forEach((tr, ti) => {
    if (tr.kind === "group" || !tr.mix) return;
    const m: PlayMix = { gain: 1, envs: [], filters: [] };
    let cur: ArrTrack | undefined = tr;
    for (let guard = 0; cur && guard < 16; guard++) {
      if (cur.mix) {
        m.gain *= cur.mix.gain;
        m.envs.push(...(cur.mix.envs as [number, number][][]));
        m.filters.push(...cur.mix.filters.map((f) => ({ kind: f.kind, hz: f.hz, env: f.env as [number, number][], slope: f.slope })));
      }
      cur = cur.group >= 0 ? byId.get(cur.group) : undefined;
    }
    out[ti] = m;
  });
  return out;
}

/** Where each track sits in the balance (dB against the loudest, kick, bass and voice at 0):
 *  a starting point an engineer would take, not the project's faders (the app cannot hear the
 *  plugins' gains, the groups' processing, the master that brings it all up). A drum track
 *  by what its name says it holds. */
const ROLE_DB: Record<string, number> = { drums: -1, perc: -8, bass: 0, vox: 0, lead: -2, keys: -4, pluck: -4, pad: -7, fx: -10, other: -4 };
export function balanceDb(tr: { role: string; name: string }): number {
  const n = tr.name.toLowerCase();
  if (tr.role === "drums") {
    if (/\b(kick|kik|bd|kicks)\b/.test(n)) return 0;
    if (/\b(snare|snr|sd|clap|claps|rim)\b/.test(n)) return -2;
    if (/\b(hats?|hh|hihats?|hi hat|top|tops|cym|cymbals?|ride|crash|shaker)\b/.test(n)) return -8;
  }
  return ROLE_DB[tr.role] ?? -4;
}
export function balanceOf(a: Arrangement): Record<number, number> {
  return Object.fromEntries(a.tracks.map((tr, i) => [i, balanceDb(tr)]));
}

/** Plays an arrangement from `fromBeat` (0: from where the song starts). Says what is wrong
 *  through `say` (no notes, samples missing). */
export async function playArrangement(a: Arrangement, rel: string, name: string, silent: number[], fromBeat: number, say: (s: string) => void) {
  const { notes, vel, bend, patches, audio } = projectParts(a, storedSounds()[rel] ?? {});
  if (!notes.length && !audio.length) return say(t("song.session.noMidi"));
  const key = sessionKey(rel);
  const total = totalBeats(a);
  if (fromBeat === 0) fromBeat = songStart(a, notes, audio, silent);
  midi.loop = false;
  const bpm = a.tempo ?? 120;
  const r = await midi.playProject(key, name, notes, audio, total, bpm, [...silent], { vel, bend, patches, from: fromBeat, mix: mixOf(a), bal: balanceOf(a), names: Object.fromEntries(a.tracks.map((tr, i) => [i, tr.name])) });
  feature("project_played");
  if (fromBeat > 0 && midi.rel === "notes:" + key) midi.seek(fromBeat / total);
  r.missing.then((n) => n && say(t("song.session.missing", { n })));
}

/** Is this project the one playing (or loading)? */
export const projectOn = (rel: string) => !!rel && (midi.playing || midi.loading) && midi.rel === "notes:" + sessionKey(rel);

/** A song with no reference render, from a list: its newest project, with the mutes kept for it. */
export async function playProjectRel(rel: string, title: string, say: (s: string) => void) {
  if (projectOn(rel)) return midi.stop();
  await startProjectRel(rel, title, say);
}

/** The project of a song heard from anywhere (the player bar's Render / Project switch, no
 *  sheet open): its arrangement read, played from `secs` (the render's place; both follow
 *  the same bars), with the mutes kept for it. */
export async function projectFromSecs(rel: string, title: string, secs: number, say: (s: string) => void, offsetBeats = 0) {
  const [a, refs] = await Promise.all([api.arrangement(rel), api.refTracks().catch(() => [] as string[])]);
  const refPaths = refs.map((p) => p.toLowerCase());
  const silent = silentTracks(a, mutedNamesOf(a, rel, refPaths));
  // the render starts `offsetBeats` into the arrangement (renderOffset)
  const beat = (secs * (a.tempo ?? 120)) / 60 + offsetBeats;
  await playArrangement(a, rel, t("song.session.midiName", { title }), silent, beat > 0 && beat < totalBeats(a) ? beat : 0, say);
}

/** The same, always started (a list played through): the MIDI player's name for it. */
export async function startProjectRel(rel: string, title: string, say: (s: string) => void): Promise<string> {
  const [a, refs] = await Promise.all([api.arrangement(rel), api.refTracks().catch(() => [] as string[])]);
  const refPaths = refs.map((p) => p.toLowerCase());
  const silent = silentTracks(a, mutedNamesOf(a, rel, refPaths));
  await playArrangement(a, rel, t("song.session.midiName", { title }), silent, 0, say);
  return "notes:" + sessionKey(rel);
}

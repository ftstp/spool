// What a MIDI track of a project should sound like in the small synth, from what the set
// says of it: its instrument and preset (a Farfisa is an organ, a DX7 an electric piano, a
// MODO BASS a bass guitar, "Reese" a reese...), the effects after it (a saturator drives it,
// an auto filter moves it, a chorus widens it, a reverb lengthens it), how its notes are
// played (legato lines glide), and the samples it holds (a Simpler, a Drum Rack: the real
// sound). Not the real synth, but its family: a bass line that sounds like one.
import type { ArrTrack } from "./api/bindings/ArrTrack";

export type Fam =
  | "piano" | "synth" | "bass" | "drums" | "organ" | "epiano" | "pluck" | "pad" | "reese" | "sub" | "bassgtr" | "brass" | "lead"
  // the orchestra (recorded: see instruments.ts)
  | "strings" | "viola" | "cello" | "contrabass" | "pizz" | "harp"
  | "flute" | "oboe" | "clarinet" | "bassoon"
  | "horn" | "trumpet" | "trombone" | "tuba"
  | "timpani" | "glock";
export type Zone = { note: number; lo: number; hi: number; root: number; path: string };
export type Patch = {
  fam: Fam;
  /** 0..1: a saturator, an overdrive. */
  drive: number;
  chorus: boolean;
  /** 0..1: a filter moving (an auto filter, an LFO tool). */
  wobble: number;
  /** Notes played legato slide into each other. */
  glide: boolean;
  /** One note at a time (a bass, a lead): a new one ends the last. */
  mono: boolean;
  /** Release length, times the family's (a reverb after it: longer). */
  tail: number;
  /** The samples it plays (Simpler, Sampler, Drum Rack), played instead of a synth. */
  zones: Zone[];
};

const has = (s: string, words: string[]) => words.some((w) => s.includes(w));

/** Whether a part is played legato: a note often still held when the next one starts. */
export function legato(notes: [number, number, number][]): boolean {
  if (notes.length < 6) return false;
  const s = [...notes].sort((a, b) => a[1] - b[1]);
  let over = 0;
  for (let i = 1; i < s.length; i++) if (s[i][1] > s[i - 1][1] + 1e-3 && s[i - 1][1] + s[i - 1][2] > s[i][1] + 0.01) over++;
  return over / (s.length - 1) > 0.25;
}

export function patchFor(tr: ArrTrack, notes: [number, number, number][]): Patch {
  const snd = tr.sound;
  const text = `${snd?.instrument ?? ""} ${snd?.preset ?? ""} ${tr.name}`.toLowerCase();
  const fx = (snd?.fx ?? []).join(" ").toLowerCase();
  const role = tr.role;
  const zones = (snd?.zones ?? []).map((z) => ({ note: z.note, lo: z.lo, hi: z.hi, root: z.root, path: z.path, start: z.start, end: z.end }));
  let fam: Fam;
  // the orchestra, by its instrument's name (before the drums: timpani sit on a perc track)
  const orch = orchestraOf(text);
  const inst = (snd?.instrument ?? "").toLowerCase();
  const player = !inst || has(inst, ["kontakt", "sine player", "decent sampler", "labs", "halion", "sampletank", "uvi", "engine 2", "spitfire"]);
  if (orch) fam = orch;
  else if (role === "drums" || role === "perc" || has(text, ["drum rack", "ezdrummer", "addictive drums", "superior drummer", "battery", "drum", "kit", "808 kit", "perc"])) fam = "drums";
  else if (has(text, ["organ", "farfisa", "hammond", "b3", "b-3", "vox continental", "orgue"])) fam = "organ";
  else if (has(text, ["dx7", "rhodes", "wurli", "vurli", "e-piano", "epiano", "e piano", "electric piano", "lounge", "electric", "suitcase", "clav"])) fam = "epiano";
  else if (has(text, ["piano", "grand", "upright", "keyscape", "noire", "ivory", "pianoteq", "keys"])) fam = "piano";
  else if (has(text, ["modo bass", "bass guitar", "trilian", "picked", "finger", "scarbee", "precision", "jazz bass", "upright bass", "contrebasse"])) fam = "bassgtr";
  else if (has(text, ["808", " sub", "sub ", "subbass", "sub bass"]) && role !== "pad") fam = "sub";
  else if (has(text, ["reese", "neuro", "wobble", "growl", "dubstep", "hoover"])) fam = "reese";
  else if (has(text, ["pad", "string", "choir", "atmos", "ambient", "omnisphere", "warm", "nappe"])) fam = "pad";
  else if (has(text, ["pluck", "arp", "bell", "mallet", "harp", "kalimba", "marimba", "vibra", "celesta", "glock"])) fam = "pluck";
  else if (has(text, ["brass", "horn", "trumpet", "trombone", "sax", "cuivre", "trompette"])) fam = "brass";
  else if (has(text, ["lead", "solo", "saw lead"])) fam = "lead";
  else if (role === "bass") fam = "bass";
  // a sample player (Kontakt...) whose patch the set does not name, or no instrument read:
  // most often a piano or keys, never a synth lead (a lead was mono and buzzed like a bowed
  // string: a piano part's chords came out one note at a time)
  else if (player && role !== "pad" && role !== "pluck") fam = "piano";
  else if (role === "lead") fam = "lead";
  else if (role === "pad") fam = "pad";
  else if (role === "pluck") fam = "pluck";
  else if (role === "keys") fam = "epiano";
  else fam = "synth";
  const monoFam = ["bass", "sub", "reese", "bassgtr", "lead"].includes(fam);
  const leg = monoFam && legato(notes);
  return {
    fam,
    drive: has(fx, ["saturat", "overdrive", "decapitator", "distort", "roar", "ott", "pedal", "amp", "trash", "fuzz", "tape", "clip", "drive", "devil"]) ? 0.6 : fam === "reese" ? 0.4 : 0,
    chorus: has(fx, ["chorus", "ensemble", "phaser", "flanger", "dimension", "juno"]) || fam === "pad",
    wobble: has(fx, ["auto filter", "lfo", "kickstart", "volumeshaper", "shaperbox", "filter"]) ? 0.35 : fam === "reese" ? 0.3 : 0,
    glide: !!snd?.glide || leg,
    mono: monoFam,
    tail: has(fx, ["reverb", "valhalla", "verb", "raum", "hybrid"]) ? 1.8 : 1,
    zones,
  };
}

/** The orchestra's instruments a track's name points to ("Cello Section", "Flûte", "Timpani"),
 *  the most precise first; null when it names none. */
export function orchestraOf(text: string): Fam | null {
  const t = text.toLowerCase();
  const rules: [Fam, RegExp][] = [
    ["pizz", /pizz/],
    ["timpani", /timpan|timbale/],
    ["glock", /glock|celesta|célesta|carillon/],
    ["harp", /\bharp|harpe/],
    ["contrabass", /contrabass|double ?bass|basses? section|contrabasse/],
    ["cello", /cello|violoncelle|celli/],
    ["viola", /\bviola|\baltos?\b/],
    ["flute", /flute|flûte|piccolo/],
    ["oboe", /oboe|hautbois|cor anglais|english horn/],
    ["clarinet", /clarinet|clarinette/],
    ["bassoon", /bassoon|basson/],
    ["horn", /french horn|\bhorns? section|\bcors?\b|\bf horn/],
    ["trumpet", /trumpet|trompette|flugel|cornet/],
    ["trombone", /trombone|\btbn/],
    ["tuba", /\btuba|euphonium/],
    ["strings", /violin|violon|string ensemble|\bstrings\b|cordes|orchestra|orchestre|spitfire|bbc so/],
  ];
  return rules.find(([, r]) => r.test(t))?.[0] ?? null;
}

/** The families a track's sound can be set to by hand, in the order a menu lists them: the
 *  synth's, then the orchestra in groups (sub-menus). */
export const FAMS: Fam[] = ["piano", "epiano", "organ", "synth", "pad", "pluck", "lead", "bass", "sub", "reese", "bassgtr", "drums"];
export const FAM_GROUPS: [string, Fam[]][] = [
  ["strings", ["strings", "viola", "cello", "contrabass", "pizz", "harp"]],
  ["winds", ["flute", "oboe", "clarinet", "bassoon"]],
  ["brass", ["brass", "trumpet", "horn", "trombone", "tuba"]],
  ["perc", ["timpani", "glock"]],
];

/** A track's sound set by hand to another family: the app's synth for it (the samples it
 *  held are left out, they were what sounded wrong), the effects read from the set kept. */
export function withFam(p: Patch, fam: Fam): Patch {
  if (fam === p.fam) return p;
  const mono = ["bass", "sub", "reese", "bassgtr", "lead"].includes(fam);
  return {
    ...p,
    fam,
    zones: [],
    mono,
    glide: mono && p.glide,
    chorus: p.chorus || fam === "pad",
    wobble: fam === "reese" ? Math.max(p.wobble, 0.3) : p.wobble,
    drive: fam === "reese" ? Math.max(p.drive, 0.4) : p.drive,
  };
}

/** A note's pitch bend from its clip's: the points inside the note and the one in force at
 *  its start, as [beats from the note's start, semitones]. */
export function noteBend(bend: [number, number][], s: number, d: number): [number, number][] | null {
  if (!bend.length) return null;
  let before: [number, number] | null = null;
  const inside: [number, number][] = [];
  for (const [t, v] of bend) {
    if (t <= s) before = [0, v];
    else if (t < s + d) inside.push([t - s, v]);
  }
  const out = [...(before ? [before] : []), ...inside];
  return out.some((p) => Math.abs(p[1]) > 0.01) ? out : null;
}

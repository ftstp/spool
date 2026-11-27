// The families of the Samples page and their categories (crates/index samples.rs CATS,
// FX_CATS, DRUMLOOP_CATS), in the order shown. An effect and a drum loop have their kinds ("fx"
// and "drumloop": one naming no kind); a full loop and a demo are their own category.
export const FAMILIES: [string, string[]][] = [
  ["drums", ["kick", "snare", "clap", "hihat", "cymbal", "tom", "perc"]],
  ["drumloop", ["drumloop", "toploop", "percloop", "kickloop", "fill", "break"]],
  ["bass", ["808", "sub", "bass", "growl"]],
  ["melodic", ["lead", "pad", "pluck", "keys", "guitar", "brass", "strings", "woodwind", "synth", "melody"]],
  ["vocal", ["vocal"]],
  ["fx", ["riser", "downlifter", "impact", "sweep", "noise", "glitch", "fx"]],
  ["fullloop", ["fullloop"]],
  ["demo", ["demo"]],
  // last: its key (9) leaves the others where they were
  ["ambience", ["ambience", "crowd", "foley"]],
];

/** The categories of a family worth choosing from (a family of one category: none to choose). */
export const catsOf = (f: string) => {
  const cs = FAMILIES.find((x) => x[0] === f)?.[1] ?? [];
  return cs.length > 1 ? cs : [];
};

/** The one category of a family that has only one (a drum loop, a voice), else null. */
export const onlyCat = (f: string) => {
  const cs = FAMILIES.find((x) => x[0] === f)?.[1] ?? [];
  return cs.length === 1 ? cs[0] : null;
};

/** The folder a sample is in ("/" separated). */
export const dirOf = (path: string) => path.slice(0, path.lastIndexOf("/"));

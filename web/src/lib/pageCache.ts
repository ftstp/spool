// What the big lists (MIDI, Samples) showed, kept while the app is open: going back to a page,
// a category or a filter already seen shows it at once, then it is read again only if the
// base changed since (app.version). A few dozen views at most, the oldest let go.
const MAX = 40;
const kept = new Map<string, { version: number; value: unknown }>();

/** What was shown for this view (the version it was read at, to tell whether it is current). */
export function cached<T>(key: string): { version: number; value: T } | null {
  const v = kept.get(key);
  if (!v) return null;
  // the most recent last
  kept.delete(key);
  kept.set(key, v);
  return v as { version: number; value: T };
}

export function keep<T>(key: string, version: number, value: T) {
  kept.delete(key);
  kept.set(key, { version, value });
  while (kept.size > MAX) kept.delete(kept.keys().next().value as string);
}

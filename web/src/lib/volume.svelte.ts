// The app's own volume (the player bar's knob), over everything it plays: the player, the
// sample laid over a render, the MIDI synth. Kept between launches. Apart from the player's
// `volume`, which Compare lowers to hear two files at the same loudness.
const KEY = "spool.volume";

function load(): number {
  try {
    const v = Number(localStorage.getItem(KEY));
    return localStorage.getItem(KEY) !== null && Number.isFinite(v) ? Math.max(0, Math.min(1, v)) : 1;
  } catch {
    return 1;
  }
}

export const vol = $state({ v: load(), before: 1 });
const listeners = new Set<(v: number) => void>();

/** Called now and at every change. */
export function onVolume(f: (v: number) => void) {
  listeners.add(f);
  f(vol.v);
}

export function setVolume(x: number) {
  vol.v = Math.max(0, Math.min(1, x));
  try {
    localStorage.setItem(KEY, String(vol.v));
  } catch {
    /* not kept */
  }
  for (const f of listeners) f(vol.v);
}

/** Silent, or back to where it was. */
export function toggleMute() {
  if (vol.v > 0) {
    vol.before = vol.v;
    setVolume(0);
  } else setVolume(vol.before || 1);
}

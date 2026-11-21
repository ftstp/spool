// A release played whole: its tracks one after the other (each its render, or its project
// when it has none), with previous / next in the player bar. From the Releases page's covers
// and from the sidebar's releases in progress.
import { player } from "./player.svelte";
import { midi } from "./midiPlayer.svelte";
import { app } from "./state.svelte";
import type { Release } from "./api/bindings/Release";
import type { SongItem } from "./api/bindings/SongItem";

/** Its tracks that can be heard, in order. */
export function tracksOf(r: Release): SongItem[] {
  const by = new Map(app.songs.map((s) => [s.id, s]));
  return r.songs.map((id) => (id != null ? by.get(id) : undefined)).filter((x): x is SongItem => !!x && (!!x.temoin || !!x.rel));
}

/** This release is the one heard (or its next track is being read). */
export function releasePlaying(r: Release): boolean {
  const q = player.queue;
  if (!q || q.name !== r.title || q.items.length !== tracksOf(r).length) return false;
  return (player.playing && q.cur === player.src) || ((midi.playing || midi.loading) && q.cur === midi.rel) || q.wait;
}

/** A click on its play button: paused when it plays, else played from its first track. */
export function playRelease(e: MouseEvent, r: Release) {
  e.preventDefault();
  e.stopPropagation();
  if (releasePlaying(r)) {
    if (midi.playing && player.queue?.cur === midi.rel) midi.stop();
    else player.toggle();
    return;
  }
  const l = tracksOf(r);
  if (l.length) void player.playQueue(l, 0, r.title);
}

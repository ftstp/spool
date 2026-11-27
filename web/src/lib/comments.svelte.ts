// A song's comments (the notes pinned on its waveforms and its post-its), one list per song
// shared by every place that shows them: the sheet's waveform, the player bar's, the
// Comments section, the mood board's notes. A change shows everywhere at once, before the
// base has written it; the writes of one song go one after the other, in order. A write that
// fails puts the list back as the base has it.
import * as api from "./api";
import type { Comment } from "./api/bindings/Comment";
import { app } from "./state.svelte";
import { untrack } from "svelte";

const by = $state<Record<number, Comment[]>>({});
const queue = new Map<number, Promise<unknown>>();

async function load(sid: number) {
  try {
    by[sid] = await api.comments(sid);
  } catch {
    /* kept as it is */
  }
}

/** The song's comments (empty until read: reading starts on the first ask). */
export function commentsOf(sid: number): Comment[] {
  const v = by[sid];
  if (v === undefined) untrack(() => queueMicrotask(() => by[sid] === undefined && void load(sid)));
  return v ?? [];
}

// an undo, a redo or a scan may have changed them: read again (the songs read so far)
$effect.root(() => {
  $effect(() => {
    void app.version;
    untrack(() => {
      for (const k of Object.keys(by)) if (!queue.has(+k)) void load(+k);
    });
  });
});

function write(sid: number, show: (l: Comment[]) => Comment[], send: () => Promise<typeof app.undo>) {
  by[sid] = show(by[sid] ?? []);
  const prev = queue.get(sid) ?? Promise.resolve();
  const p = prev
    .then(send)
    .then((u) => {
      app.undo = u;
    })
    .catch(async (e) => {
      app.say(String(e), { error: true });
      await load(sid);
    })
    .finally(() => {
      if (queue.get(sid) === p) queue.delete(sid);
    });
  queue.set(sid, p);
  return p;
}

/** A comment written: on a render or a project (`rel`) at `at` seconds, or a post-it (`rel` ""). */
export function addComment(sid: number, c: { rel: string; at: number; text: string }) {
  const text = c.text.trim();
  if (!text) return Promise.resolve();
  // its id given here (the base keeps it): an edit right after reaches the same comment
  const now = Date.now();
  const id = Math.max(now, ...(by[sid] ?? []).map((x) => Number(x.id) + 1));
  const made = { id, created: now / 1000, done: false, rel: c.rel.replace(/\\/g, "/"), at: Math.max(0, c.at), text } as Comment;
  return write(sid, (l) => [...l, made], () => api.commentEdit(sid, made));
}

/** A comment changed (its text, its moment, ticked). */
export function editComment(sid: number, c: Comment) {
  return write(sid, (l) => l.map((x) => (x.id === c.id ? { ...x, ...c, text: c.text.trim() || x.text } : x)), () => api.commentEdit(sid, c));
}

/** A comment removed. */
export function removeComment(sid: number, c: Comment) {
  return write(sid, (l) => l.filter((x) => x.id !== c.id), () => api.commentEdit(sid, c, true));
}

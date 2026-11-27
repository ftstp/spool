<script lang="ts">
  import { untrack } from "svelte";
  import { widthOf } from "../size";
  // A song's arrangement, drawn from its project: one row per track (coloured by its role,
  // groups as headers), the clips where they are in time (MIDI ones with their notes), the
  // locators as sections. A click on a clip plays it: its notes with the small synth, an
  // audio clip's sample with the player. Read from the set, never written.
  import * as api from "../api";
  import type { Arrangement } from "../api/bindings/Arrangement";
  import type { ArrClip } from "../api/bindings/ArrClip";
  import { app } from "../state.svelte";
  import { player } from "../player.svelte";
  import { midi } from "../midiPlayer.svelte";
  import { t } from "../i18n/index.svelte";
  import { mmss, infoFor } from "../player.svelte";
  import HarmonyPanel from "./HarmonyPanel.svelte";
  import ChordChart from "./ChordChart.svelte";
  import { openMenu, type MenuItem } from "../menu.svelte";
  import { sectionOpen } from "../tabs.svelte";
  import type { Harmony } from "../api/bindings/Harmony";
  import { chordNotes, clipChords, shortRun, noteLabel, chordName, spelling, type Chord } from "../theory";
  import { i18n } from "../i18n/index.svelte";
  import { noteBend, FAMS, FAM_GROUPS, type Fam } from "../patch";
  import { patchOf, mutedNamesOf, storedMutes, storeMutes, groupsOf as groupsOfA, silentTracks, playArrangement, sessionKey as keyOf, storedSounds, storeSounds } from "../projectPlay";
  import type { ArrTrack } from "../api/bindings/ArrTrack";
  import { chordSource, chordSourceOf } from "../chordNow.svelte";
  import { sheetTransport, type SheetTransport } from "../sheetTransport.svelte";
  import FoldHead from "./FoldHead.svelte";
  import { renderOffset, detectOffset } from "../renderOffset.svelte";

  let { files, title, sid, temoin = null }: { files: { rel: string; name: string; mtime: number; latest?: boolean; old?: boolean }[]; title: string; sid: number; temoin?: string | null } = $props();
  // open or not as the user left the sheet's sections (the same for every song)
  const open = $derived(sectionOpen(sid, "session"));
  const chordsOpen = $derived(sectionOpen(sid, "chords"));
  const sorted = $derived([...files].sort((a, b) => Number(!!b.latest) - Number(!!a.latest) || b.mtime - a.mtime));
  let rel = $state("");
  $effect(() => {
    if (!sorted.some((f) => f.rel === rel)) rel = sorted[0]?.rel ?? "";
  });
  let a = $state<Arrangement | null>(null);
  let h = $state<Harmony | null>(null);
  let cur = $state<Chord | null>(null);
  let error = $state("");
  let loading = $state(false);
  $effect(() => {
    const r = rel;
    a = null;
    h = null;
    cur = null;
    error = "";
    if (!r || !api.withArrangement) return;
    api.harmony(r).then((x) => {
      if (r !== rel) return;
      h = x;
      const first = x.bars[0];
      if (first) cur = { root: first.root, q: first.quality };
    }).catch(() => {});
    loading = true;
    api
      .arrangement(r)
      .then((x) => r === rel && (a = x))
      .catch((e) => r === rel && (error = String(e)))
      .finally(() => r === rel && (loading = false));
  });

  // the rows: tracks with clips, and the groups holding them (empty tracks only on demand)
  let all = $state(false);
  const rows = $derived.by(() => {
    if (!a) return [];
    const ts = a.tracks;
    return ts.filter((tr, i) => {
      if (all || tr.clips.length) return true;
      if (tr.kind !== "group") return false;
      // a group shows when one of the grouped tracks after it has clips
      for (let j = i + 1; j < ts.length && ts[j].grouped; j++) if (ts[j].clips.length) return true;
      return false;
    });
  });
  const ROLE: Record<string, string> = {
    drums: "#e0604f", perc: "#e8894a", bass: "#8a6cf0", keys: "#4f8fe0", pad: "#47b3c9", pluck: "#3fb38a",
    lead: "#e3b341", vox: "#d965a8", fx: "#8c96a3", other: "#7aa0a8", return: "#8c96a3",
  };
  const colorOf = (role: string) => ROLE[role] ?? ROLE.other;
  // "22-4-ACE(x64) 2": the numbers Live puts in front go
  const clean = (n: string) => n.replace(/^[\d\s.-]+(?=\S)/, "") || n;
  const bars = $derived(a ? Math.max(1, Math.ceil(a.len / a.beats_per_bar)) : 1);
  const total = $derived(bars * (a?.beats_per_bar ?? 4));
  const pct = (b: number) => `${(b / total) * 100}%`;
  // ---- the markers (Live's locators): the set's own, or as changed here. Changed, they are
  // kept per project and written into a copy when the project is next opened in Live (the
  // set itself is never written). A right-click on the timeline adds, renames or removes one;
  // a marker's name dragged along moves it, bar by bar; a double-click renames it.
  let edited = $state<[number, string][] | null>(null);
  $effect(() => {
    const r = rel;
    edited = null;
    if (!r) return;
    api.markersGet(r).then((l) => r === rel && (edited = l)).catch(() => {});
  });
  const locs = $derived(
    edited ? [...edited].sort((x, y) => x[0] - y[0]).map(([at, name]) => ({ at, name })) : [...(a?.locators ?? [])].sort((x, y) => x.at - y.at),
  );
  function setLocs(list: { at: number; name: string }[]) {
    const next = [...list].sort((x, y) => x.at - y.at).map((l) => [Math.max(0, l.at), l.name] as [number, string]);
    const own = [...(a?.locators ?? [])].sort((x, y) => x.at - y.at);
    // back to what the set has: nothing kept
    const same = next.length === own.length && next.every(([at, n], i) => Math.abs(at - own[i].at) < 1e-6 && n === own[i].name);
    edited = same ? null : next;
    api.markersSet(rel, edited).catch((e) => app.say(String(e), { error: true }));
  }
  // sections from the markers: each runs to the next one (`i`: the marker's place in `locs`)
  const sections = $derived(
    locs.map((l, i, ls) => ({ i, name: l.name, from: l.at, to: ls[i + 1]?.at ?? total })).filter((s) => s.to > s.from),
  );
  const focusSelect = (el: HTMLInputElement) => {
    el.focus();
    el.select();
  };
  const bpbOf = () => a?.beats_per_bar ?? 4;
  // the beat under the pointer on the timeline, on the nearest bar
  function barAt(e: MouseEvent, el: HTMLElement) {
    const r = el.getBoundingClientRect();
    const beat = Math.max(0, Math.min(total, ((e.clientX - r.left) / r.width) * total));
    return Math.round(beat / bpbOf()) * bpbOf();
  }
  let renaming = $state<number | null>(null);
  let rulerEl = $state<HTMLElement>();
  function addMarker(at: number) {
    const name = t("song.markers.newName", { n: locs.length + 1 });
    const list = [...locs, { at, name }];
    setLocs(list);
    // named at once: its place in the sorted list
    renaming = [...list].sort((x, y) => x.at - y.at).findIndex((l) => l.at === at && l.name === name);
  }
  function renameMarker(i: number, name: string) {
    renaming = null;
    const n = name.trim();
    if (!n || n === locs[i]?.name) return;
    setLocs(locs.map((l, j) => (j === i ? { ...l, name: n } : l)));
  }
  const removeMarker = (i: number) => setLocs(locs.filter((_, j) => j !== i));
  function rulerMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (!rulerEl) return;
    const at = barAt(e, rulerEl);
    const hit = (e.target as HTMLElement).closest<HTMLElement>("[data-loc]");
    const i = hit ? Number(hit.dataset.loc) : -1;
    const bar = Math.round(at / bpbOf()) + 1;
    const items: MenuItem[] = [];
    if (i >= 0 && locs[i]) {
      items.push(
        { label: t("song.markers.rename"), run: () => (renaming = i) },
        { label: t("song.markers.remove"), run: () => removeMarker(i) },
        { sep: true },
      );
    }
    items.push({ label: t("song.markers.add", { bar }), disabled: locs.some((l) => Math.abs(l.at - at) < 1e-6), run: () => addMarker(at) });
    if (edited) items.push({ sep: true }, { label: t("song.markers.revert"), run: () => setLocs(a?.locators ?? []) });
    openMenu(e, items, i >= 0 && locs[i] ? locs[i].name : t("song.markers.title"));
  }
  // a marker's name dragged along the timeline: moved to the bar under the pointer
  let mdrag = $state<{ i: number; x0: number; at: number; moved: boolean } | null>(null);
  let dragged = false;
  function markerDown(e: PointerEvent, i: number) {
    if (e.button !== 0 || renaming !== null) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    mdrag = { i, x0: e.clientX, at: locs[i].at, moved: false };
  }
  function markerMove(e: PointerEvent) {
    if (!mdrag || !rulerEl) return;
    if (Math.abs(e.clientX - mdrag.x0) > 4) mdrag.moved = true;
    if (mdrag.moved) mdrag.at = barAt(e, rulerEl);
  }
  function markerUp() {
    const d = mdrag;
    mdrag = null;
    if (!d?.moved) return;
    // the click that ends a drag does not move the playhead
    dragged = true;
    setTimeout(() => (dragged = false));
    if (Math.abs(d.at - locs[d.i].at) > 1e-6 && !locs.some((l, j) => j !== d.i && Math.abs(l.at - d.at) < 1e-6))
      setLocs(locs.map((l, j) => (j === d.i ? { ...l, at: d.at } : l)));
  }
  const secLeft = (s: { i: number; from: number }) => (mdrag && mdrag.i === s.i && mdrag.moved ? mdrag.at : s.from);
  // ---- zoom and travel, as in Live: Ctrl + wheel zooms where the pointer is (the wheel over
  // the timeline too), a drag on the timeline zooms (up and down) and travels (sideways), the
  // − / + buttons and the + / - keys zoom, the bar under the lanes scrolls (a mouse with no
  // sideways wheel), Shift + wheel travels. "Whole song" fits the song itself: a clip left far
  // after it (a sketch, a forgotten take) does not squeeze it into a corner; it stays there,
  // reached by scrolling. About 40 bars or the whole song, the choice is kept.
  const ZOOM = "spool.session.zoom";
  let zoomOn = $state((() => { try { return localStorage.getItem(ZOOM) !== "0"; } catch { return true; } })());
  let laneW = $state(0);
  const VIEW_BARS = 40;
  // where the song ends, clips left far after it aside: the clips Live plays in time order, the
  // song stops at a gap of 32 bars or more after which few clips remain (15 % of them at most)
  const songBars = $derived.by(() => {
    if (!a) return 1;
    const bpb = a.beats_per_bar || 4;
    // what Live plays (a track or a clip switched off there: a sketch kept aside, not the song)
    const heard = a.tracks.filter((t) => t.kind !== "group" && !t.muted).flatMap((t) => t.clips.filter((c) => !c.muted).map((c) => [c.start, c.end] as const));
    const cs = (heard.length ? heard : a.tracks.flatMap((t) => t.clips.map((c) => [c.start, c.end] as const))).sort((x, y) => x[0] - y[0]);
    let end = 0;
    for (let i = 0; i < cs.length; i++) {
      if (i > 0 && cs[i][0] - end > 32 * bpb && cs.length - i <= Math.max(2, cs.length * 0.15)) break;
      end = Math.max(end, cs[i][1]);
    }
    const lastLoc = Math.max(0, ...(a.locators ?? []).filter((l) => l.at <= end + 32 * bpb).map((l) => l.at));
    return Math.max(1, Math.min(bars, Math.ceil(Math.max(end, lastLoc) / bpb)));
  });
  /** How many bars are in sight. */
  let vis = $state(VIEW_BARS);
  // a new project (or the choice changed): about 40 bars, or the whole song
  $effect(() => {
    void rel;
    const sb = songBars;
    vis = untrack(() => zoomOn) ? Math.min(VIEW_BARS, sb) : sb;
  });
  const z = $derived(laneW ? Math.max(1, bars / Math.max(1, Math.min(bars, vis))) : 1);
  let off = $state(0);
  const clampOff = (x: number) => Math.max(0, Math.min(laneW * z - laneW, x));
  function setZoom(on: boolean) {
    zoomOn = on;
    try { localStorage.setItem(ZOOM, on ? "1" : "0"); } catch { /* not kept */ }
    zoomTo(on ? Math.min(VIEW_BARS, songBars) : songBars, 0);
    if (!on) off = 0;
  }
  // a zoom moving: the lane jumps with the pointer (no gliding behind it)
  let zooming = $state(false);
  let zoomTimer = 0;
  /** Zoomed to `n` bars in sight, the point `px` from the lane's left staying where it is. */
  function zoomTo(n: number, px: number) {
    const w = laneW;
    if (!w) return;
    const before = laneW * z;
    const at = (off + px) / before;
    vis = Math.max(1, Math.min(bars, n));
    const after = laneW * Math.max(1, bars / Math.max(1, Math.min(bars, vis)));
    off = Math.max(0, Math.min(after - w, at * after - px));
    zooming = true;
    clearTimeout(zoomTimer);
    zoomTimer = window.setTimeout(() => (zooming = false), 180);
  }
  const zoomBy = (k: number, px = laneW / 2) => zoomTo(Math.min(bars, vis) / k, px);
  // the playhead kept in sight. Playing, it runs to the middle of the lane, then the lane
  // follows it there at the speed it plays (started past the middle, the lane moves once to
  // put it there). Not playing, the start set by a click is kept in sight as before.
  let follow = $state(false);
  // following the playhead can be turned off (kept), and stops by itself while the user looks
  // elsewhere: moving the lane by hand during playback pauses it until the button or the next
  // play
  const FOLLOW = "spool.session.follow";
  let followOn = $state((() => { try { return localStorage.getItem(FOLLOW) !== "0"; } catch { return true; } })());
  let followPaused = $state(false);
  const tracking = $derived(followOn && !followPaused);
  function setFollow(on: boolean) {
    followOn = on;
    followPaused = false;
    try { localStorage.setItem(FOLLOW, on ? "1" : "0"); } catch { /* not kept */ }
  }
  /** The lane moved by hand: during playback, following waits. */
  function pan(x: number) {
    off = clampOff(x);
    if (head !== null) followPaused = true;
  }
  $effect(() => {
    const b = head ?? (cue > 0 ? cue : null);
    const zz = z, w = laneW;
    if (zz <= 1 || !w) {
      follow = false;
      return void (off = 0);
    }
    if (b === null) {
      follow = false;
      return void (off = untrack(() => clampOff(off)));
    }
    const x = (Math.min(b, total) / total) * w * zz;
    const o = untrack(() => off);
    if (head !== null) {
      if (!tracking) {
        follow = false;
        return;
      }
      const mid = o + w * 0.5;
      // out of sight, or well past the middle (a seek, a start): one move to the middle
      if (x < o || x > mid + w * 0.08) {
        follow = false;
        off = clampOff(x - w * 0.5);
      } else if (x > mid) {
        follow = true;
        off = clampOff(x - w * 0.5);
      }
      return;
    }
    follow = false;
    if (x < o + w * 0.02 || x > o + w * 0.86) off = clampOff(x - w * 0.12);
  });
  function wheelPan(node: HTMLElement) {
    const f = (e: WheelEvent) => {
      const r = node.getBoundingClientRect();
      const onRuler = (e.target as HTMLElement).closest?.(".ruler");
      if (e.ctrlKey || e.metaKey || (onRuler && !e.shiftKey && Math.abs(e.deltaY) > Math.abs(e.deltaX))) {
        e.preventDefault();
        zoomBy(Math.exp(-e.deltaY * 0.0022), e.clientX - r.left);
        return;
      }
      if (z <= 1) return;
      const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.shiftKey ? e.deltaY : 0;
      if (!d) return;
      e.preventDefault();
      pan(off + d);
    };
    node.addEventListener("wheel", f, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", f) };
  }
  const ruler = $derived.by(() => {
    const inSight = bars / z;
    const step = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096].find((x) => inSight / x <= 16) ?? 8192;
    // the labels a screen around what is seen (a long set: not thousands of them)
    const from = laneW ? Math.max(0, Math.floor(((off / (laneW * z)) * bars - inSight) / step) * step) : 0;
    const to = laneW ? Math.min(bars, (off / (laneW * z)) * bars + 2 * inSight) : bars;
    const out: number[] = [];
    for (let b = from; b < to; b += step) out.push(b);
    return out;
  });
  // ---- a drag on the timeline, as in Live: down zooms in, up out (where it started), sideways
  // travels; a click that does not move goes on seeking
  let rdrag: { x0: number; y0: number; vis0: number; off0: number; px: number; moved: boolean } | null = null;
  function rulerDown(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("[data-loc], input")) return;
    const lane = (e.currentTarget as HTMLElement).closest(".lane") as HTMLElement;
    rdrag = { x0: e.clientX, y0: e.clientY, vis0: Math.min(bars, vis), off0: off, px: e.clientX - lane.getBoundingClientRect().left, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function rulerMove(e: PointerEvent) {
    const d = rdrag;
    if (!d) return;
    const dx = e.clientX - d.x0, dy = e.clientY - d.y0;
    if (!d.moved && Math.hypot(dx, dy) < 4) return;
    d.moved = true;
    // the zoom from where the drag started, then the sideways travel on top
    const n = d.vis0 * Math.exp(-dy * 0.012);
    const before = laneW * Math.max(1, bars / Math.max(1, Math.min(bars, d.vis0)));
    const at = (d.off0 + d.px) / before;
    vis = Math.max(1, Math.min(bars, n));
    const after = laneW * z;
    off = Math.max(0, Math.min(after - laneW, at * after - d.px - dx));
    if (Math.abs(dx) >= 4 && head !== null) followPaused = true;
    zooming = true;
  }
  function rulerUp() {
    const d = rdrag;
    rdrag = null;
    zooming = false;
    if (!d?.moved) return;
    dragged = true;
    setTimeout(() => (dragged = false));
  }
  // the + / - keys while the pointer is over the arrangement
  let overGrid = false;
  function zoomKeys(e: KeyboardEvent) {
    if (!overGrid || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target instanceof HTMLElement && e.target.matches("input, textarea, select")) return;
    if (e.key === "+" || e.key === "=") zoomBy(1.5);
    else if (e.key === "-" || e.key === "_") zoomBy(1 / 1.5);
    else return;
    e.preventDefault();
  }
  // ---- the bar under the lanes: what is seen of the whole, dragged to travel
  let sdrag: { x0: number; off0: number; w: number } | null = null;
  function scrollDown(e: PointerEvent) {
    const track = e.currentTarget as HTMLElement;
    const r = track.getBoundingClientRect();
    const fw = laneW * z;
    const thumbL = (off / fw) * r.width, thumbW = (laneW / fw) * r.width;
    const x = e.clientX - r.left;
    // outside the thumb: it jumps there (centred), then drags
    if (x < thumbL || x > thumbL + thumbW) pan((x / r.width) * fw - laneW / 2);
    sdrag = { x0: e.clientX, off0: off, w: r.width };
    track.setPointerCapture(e.pointerId);
  }
  function scrollMove(e: PointerEvent) {
    if (!sdrag) return;
    pan(sdrag.off0 + ((e.clientX - sdrag.x0) / sdrag.w) * laneW * z);
  }
  const secs = $derived(a?.tempo ? (total * 60) / a.tempo : 0);
  // the chord row: one block per run of the same chord (in beats: a bar can hold several)
  const chordRuns = $derived.by(() => {
    const out: { from: number; to: number; name: string; c: Chord }[] = [];
    const bpb = a?.beats_per_bar ?? 4;
    for (const b of h?.bars ?? []) {
      const name = b.name + (b.bass ? "/" + b.bass : "");
      const from = b.bar * bpb + b.at;
      const last = out[out.length - 1];
      if (last && last.name === name && Math.abs(last.to - from) < 1e-6) last.to = from + b.len;
      else out.push({ from, to: from + b.len, name, c: { root: b.root, q: b.quality } });
    }
    return out;
  });
  function pickChord(c: Chord, name: string) {
    cur = c;
    const bar = a?.beats_per_bar ?? 4;
    midi.playNotes(`row|${name}`, name, chordNotes(c).map((p) => [p, 0, bar * 0.95] as [number, number, number]), bar, a?.tempo ?? 120, false);
  }

  // a clip's notes inside its block, scaled to its own pitch range
  function noteBoxes(c: ArrClip) {
    if (!c.notes.length) return [];
    const lo = Math.min(...c.notes.map((n) => n[0]));
    const hi = Math.max(...c.notes.map((n) => n[0]));
    const span = c.end - c.start;
    return c.notes.map(([p, s, d]) => ({
      x: (s / span) * 100,
      w: Math.max(0.6, (d / span) * 100),
      y: hi === lo ? 45 : 82 - ((p - lo) / (hi - lo)) * 70,
    }));
  }
  let on = $state("");
  function play(c: ArrClip, track: string, drums: boolean, i: number) {
    const key = `${rel}|${track}|${i}`;
    if (c.notes.length && a) {
      on = key;
      const len = c.end - c.start;
      const tr = a.tracks.find((x) => x.name === track && x.clips.includes(c));
      midi.playNotes(key, c.name || track, c.notes as [number, number, number][], len, a.tempo ?? 120, drums, {
        vel: c.vels,
        bend: c.notes.map(([, s, d]) => noteBend(c.bend as [number, number][], s, d)),
        patch: tr ? patchOf(tr) : undefined,
      });
    } else if (c.sample) {
      on = key;
      if (player.is(c.sample)) player.toggle();
      else player.play(c.sample, null, c.name || track);
    } else {
      app.say(t("song.session.cannotPlay"));
    }
  }
  // ---- the clips, drawn
  // ---- what a MIDI clip plays: its chords (harmonic parts), its range, its velocities
  const keyNames = $derived(spelling(h?.tonic ?? 0, h?.minor ?? false));
  const HARMONIC = (role: string) => !["drums", "perc", "fx", "vox", "return"].includes(role);
  const chordCache = new WeakMap<ArrClip, { from: number; to: number; name: string }[]>();
  function chordsIn(c: ArrClip, role: string) {
    if (!c.notes.length || !HARMONIC(role) || !a) return [];
    let r = chordCache.get(c);
    if (!r) {
      r = clipChords(c.notes as [number, number, number][], c.start, c.end, a.beats_per_bar).map((x) => ({ from: x.from, to: x.to, name: chordName(x.c, keyNames) }));
      chordCache.set(c, r);
    }
    return r;
  }
  const latin = $derived(i18n.lang === "fr");

  // the tune, for the chart's staff. Every MIDI clip is sorted by what it plays: a bass (one
  // note at a time, low, or on a bass track), chords (several notes at once), or a line (one
  // note at a time, sometimes two) - a melody when it sits in the upper part of the song's
  // whole range. A tune moves from clip to clip and track to track: the highest note of the
  // melody clips at each moment, on a sixteenth grid, held until the next one.
  const melody = $derived.by((): [number, number, number][] => {
    if (!a) return [];
    type Cand = { role: string; ns: [number, number, number][]; poly: number; mean: number };
    const cands: Cand[] = [];
    for (const tr of a.tracks) {
      if (tr.kind !== "midi" || tr.muted || !HARMONIC(tr.role)) continue;
      for (const c of tr.clips) {
        if (c.muted || c.notes.length < 2) continue;
        const ns = c.notes.map(([p, st, d]) => [p, c.start + st, d] as [number, number, number]);
        const onsets = new Set(c.notes.map((n) => Math.round(n[1] * 8))).size;
        cands.push({ role: tr.role, ns, poly: c.notes.length / onsets, mean: ns.reduce((t, n) => t + n[0], 0) / ns.length });
      }
    }
    if (!cands.length) return [];
    const pitches = cands.flatMap((c) => c.ns.map((n) => n[0])).sort((x, y) => x - y);
    const med = pitches[Math.floor(pitches.length / 2)];
    const isBass = (c: Cand) => c.role === "bass" || (c.poly < 1.3 && c.mean < Math.min(52, med - 7));
    const isChords = (c: Cand) => c.role === "pad" || c.poly >= 1.8;
    // a line under C3 is a bass part whatever its track says
    const lines = cands.filter((c) => !isBass(c) && !isChords(c) && c.mean >= Math.max(med - 3, 48));
    if (!lines.length) return [];
    // the highest note at each sixteenth where a note starts
    const q = (x: number) => Math.round(x * 4) / 4;
    const at = new Map<number, { p: number; end: number }>();
    for (const c of lines)
      for (const [p, st, d] of c.ns) {
        const o = q(st);
        const end = Math.max(o + 0.25, q(st + d));
        const cur = at.get(o);
        if (!cur || p > cur.p) at.set(o, { p, end });
      }
    // a note far under the tune (an octave below its middle) is accompaniment, not the tune
    const mid = [...at.values()].map((n) => n.p).sort((x, y) => x - y)[Math.floor(at.size / 2)] ?? 0;
    for (const [o, n] of at) if (n.p < mid - 12) at.delete(o);
    const on = [...at.keys()].sort((x, y) => x - y);
    return on.map((o, i) => {
      const n = at.get(o)!;
      const next = on[i + 1] ?? Infinity;
      return [n.p, o, Math.min(n.end, next) - o] as [number, number, number];
    });
  });
  function clipFacts(c: ArrClip, role: string): string[] {
    if (!c.notes.length || !a) return [];
    const ps = c.notes.map((n) => n[0]);
    const lo = Math.min(...ps);
    const hi = Math.max(...ps);
    const out = [t("song.session.menu.notesInfo", { n: c.notes.length, lo: noteLabel(lo, latin, keyNames), hi: noteLabel(hi, latin, keyNames), bars: Math.round(((c.end - c.start) / a.beats_per_bar) * 10) / 10 })];
    const vs = (c.vels ?? []).filter((v) => v > 0);
    if (vs.length) out.push(t("song.session.menu.velInfo", { lo: Math.min(...vs), hi: Math.max(...vs) }));
    const ch = chordsIn(c, role);
    if (ch.length) out.push(t("song.session.menu.chordsInfo", { chords: shortRun(ch.map((x) => x.name)) }));
    return out;
  }

  const ROW_H = 18; // a row's height (its line inside), as the names column has them
  let cv = $state<HTMLCanvasElement>();
  // the lane's whole width (zoomed), and the stretch of it the picture covers: three screens
  // around what is seen, so the lane's sliding (transform) never shows an edge, instead of
  // the whole song (15 000 px wide zoomed in: tens of MB in the graphics process each sheet)
  const fw = $derived(laneW * z);
  // moved by half screens only, so following the playhead does not draw it again each frame
  const x0 = $derived(Math.max(0, Math.min(Math.floor((off - laneW) / (laneW / 2 || 1)) * (laneW / 2), fw - 3 * laneW)));
  const x1 = $derived(Math.min(fw, x0 + 3 * laneW));
  let hover = $state<{ ti: number; ci: number } | null>(null);
  const hoverTip = $derived.by(() => {
    if (!hover) return undefined;
    const tr = rows[hover.ti];
    const c = tr?.clips[hover.ci];
    if (!c) return undefined;
    return [c.name || tr.name, ...clipFacts(c, tr.role), c.notes.length ? t("song.session.midiTip") : c.sample ? t("song.session.audioTip") : t("song.session.missing")].join("\n");
  });
  // the clip under the pointer
  function at(e: MouseEvent): { ti: number; ci: number } | null {
    if (!cv || !fw) return null;
    const r = cv.getBoundingClientRect();
    const ti = Math.floor((e.clientY - r.top) / ROW_H);
    const tr = rows[ti];
    if (!tr) return null;
    const beat = ((e.clientX - r.left + x0) / fw) * total;
    const ci = tr.clips.findIndex((c) => beat >= c.start && beat < c.end);
    return ci < 0 ? null : { ti, ci };
  }
  function move(e: MouseEvent) {
    const h = at(e);
    if (h?.ti !== hover?.ti || h?.ci !== hover?.ci) hover = h;
  }
  // a clip's right-click: where it is on disk, and what sounds like it in the library
  const KIND_OF: Record<string, string> = { drums: "drums", perc: "drums", bass: "bass", keys: "chords", pad: "chords", lead: "melody", pluck: "melody", vox: "melody" };
  function clipMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const hit = at(e);
    if (!hit || !a) return;
    const tr = rows[hit.ti];
    const c = tr.clips[hit.ci];
    const name = c.name || clean(tr.name);
    const fail = (x: unknown) => app.say(String(x), { error: true });
    const items: MenuItem[] = [
      // what the clip plays, said first (read only)
      ...clipFacts(c, tr.role).map((l) => ({ label: l, disabled: true }) as MenuItem),
      ...(c.notes.length ? [{ sep: true } as MenuItem] : []),
      { label: playing(tr.name, hit.ci) ? t("menu.media.stop") : t("menu.media.listen"), disabled: !c.notes.length && !c.sample, run: () => play(c, tr.name, tr.role === "drums" || tr.role === "perc", hit.ci) },
      { sep: true },
    ];
    if (c.notes.length) {
      const p = new URLSearchParams();
      const kind = KIND_OF[tr.role];
      if (kind) p.set("kind", kind);
      if (h?.key) p.set("key", h.key);
      if (a.tempo) p.set("bpm", String(Math.round(a.tempo * 10) / 10));
      p.set("from", title);
      items.push(
        { label: t("song.session.menu.similarMidi"), run: () => (location.hash = "#/midi?" + p.toString()) },
        { label: t("song.session.menu.saveMidi"), run: () => api.writeMidi(`${title} - ${name}`, c.notes as [number, number, number][], a?.tempo ?? 120).then((f) => api.reveal(f)).catch(fail) },
        { label: t("song.session.menu.revealProject"), run: () => api.reveal(rel).catch(fail) },
      );
    } else if (c.sample) {
      const s = c.sample;
      items.push(
        { label: t("song.session.menu.similarSamples"), run: () => (location.hash = "#/samples?alike=" + encodeURIComponent(s)) },
        { label: t("song.session.menu.revealSample"), run: () => api.reveal(s).catch(fail) },
        { label: t("menu.folder.copyPath"), run: () => api.copyPath(s).then((x) => app.say(t("menu.pathCopied", { path: x }))).catch(fail) },
      );
    } else {
      items.push({ label: t("song.session.menu.missing"), disabled: true, run: () => {} });
      items.push({ label: t("song.session.menu.revealProject"), run: () => api.reveal(rel).catch(fail) });
    }
    openMenu(e, items, name);
  }
  function clickClip(e: MouseEvent) {
    const h = at(e);
    if (!h) return;
    const tr = rows[h.ti];
    play(tr.clips[h.ci], tr.name, tr.role === "drums" || tr.role === "perc", h.ci);
  }
  // colours: the theme's, read once per drawing, mixed as the page's color-mix() did
  function rgb(v: string): [number, number, number] {
    const x = v.trim();
    if (x.startsWith("#")) {
      const h = x.length === 4 ? x.slice(1).split("").map((c) => c + c).join("") : x.slice(1, 7);
      return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
    }
    const m = x.match(/\d+(\.\d+)?/g);
    return m ? [+m[0], +m[1], +m[2]] : [128, 128, 128];
  }
  const mix = (a: [number, number, number], b: [number, number, number], k: number) => `rgb(${a.map((v, i) => Math.round(v * k + b[i] * (1 - k))).join(",")})`;
  $effect(() => {
    const c = cv;
    const w = fw;
    const from = x0, vw = x1 - x0;
    if (!c || !w || vw <= 0 || !a) return;
    // what the drawing depends on
    const rs = rows;
    const hv = hover;
    const playingKey = on && (midi.playing || player.playing) ? on : "";
    const dpr = window.devicePixelRatio || 1;
    const hgt = rs.length * ROW_H;
    // resized only when its size changes: each resize is a new picture in the graphics
    // process (redrawn on every hover and playhead move, it held hundreds of MB there)
    const W = Math.round(vw * dpr), H = Math.round(hgt * dpr);
    if (c.width !== W) c.width = W;
    if (c.height !== H) c.height = H;
    c.style.height = hgt + "px";
    c.style.width = vw + "px";
    c.style.left = from + "px";
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, W, H);
    g.setTransform(dpr, 0, 0, dpr, -from * dpr, 0);
    const css = getComputedStyle(c);
    const panel = rgb(css.getPropertyValue("--panel") || "#1c1c1c");
    const ink = rgb(css.getPropertyValue("--ink") || "#eeeeee");
    rs.forEach((tr, ti) => {
      const col = rgb(colorOf(tr.role));
      const y = ti * ROW_H + 2;
      const h = ROW_H - 4;
      tr.clips.forEach((cl, ci) => {
        const x = (cl.start / total) * w;
        const cwid = Math.max(1.5, ((cl.end - cl.start) / total) * w);
        if (x + cwid < from || x > from + vw) return;
        const isOn = playingKey === `${rel}|${tr.name}|${ci}`;
        g.globalAlpha = cl.muted || isMuted(tr.name) ? 0.35 : 1;
        g.save();
        g.beginPath();
        g.roundRect(x, y, cwid, h, 3);
        g.clip();
        if (isOn) {
          g.fillStyle = mix(col, col, 1);
          g.fillRect(x, y, cwid, h);
        } else if (!cl.notes.length) {
          // an audio clip: hatched
          g.fillStyle = mix(col, panel, 0.18);
          g.fillRect(x, y, cwid, h);
          g.strokeStyle = mix(col, panel, 0.3);
          g.lineWidth = 4;
          for (let sx = x - h; sx < x + cwid + h; sx += 8) {
            g.beginPath();
            g.moveTo(sx, y + h);
            g.lineTo(sx + h, y);
            g.stroke();
          }
        } else {
          g.fillStyle = mix(col, panel, 0.32);
          g.fillRect(x, y, cwid, h);
          g.fillStyle = mix(col, ink, 0.9);
          for (const n of noteBoxes(cl)) g.fillRect(x + (n.x / 100) * cwid, y + (n.y / 100) * h, Math.max(1, (n.w / 100) * cwid), 2);
          // the clip's chords written over it, where there is room for the name
          const chs = chordsIn(cl, tr.role);
          if (chs.length) {
            g.font = "600 10px system-ui, sans-serif";
            g.textBaseline = "middle";
            const label = (text: string, sx: number) => {
              const tw = g.measureText(text).width + 6;
              g.fillStyle = mix(panel, col, 0.82);
              g.fillRect(sx + 1, y + 2, tw, h - 4);
              g.fillStyle = mix(ink, col, 0.95);
              g.fillText(text, sx + 4, y + h / 2 + 0.5);
            };
            const fits = chs.every((k) => g.measureText(k.name).width + 6 <= ((k.to - k.from) / total) * w - 2);
            if (fits) for (const k of chs) label(k.name, (k.from / total) * w);
            else {
              // too narrow for a chord a bar: the clip's run said once, if it fits the clip
              const run = shortRun(chs.map((k) => k.name));
              if (g.measureText(run).width + 6 <= cwid - 2) label(run, x);
            }
          }
        }
        g.restore();
        g.strokeStyle = hv && hv.ti === ti && hv.ci === ci ? mix(col, col, 1) : `rgba(${col.join(",")},0.7)`;
        g.lineWidth = hv && hv.ti === ti && hv.ci === ci ? 2 : 1;
        g.beginPath();
        g.roundRect(x + 0.5, y + 0.5, cwid - 1, h - 1, 3);
        g.stroke();
        g.globalAlpha = 1;
      });
    });
  });
  const playing = (track: string, i: number) => on === `${rel}|${track}|${i}` && (midi.playing || player.playing);

  // the whole session: the reference render with a playhead over the arrangement (it is the
  // real sound, if it was rendered from the first bar), or the project itself: its MIDI
  // tracks with the synth and its audio clips from their samples, in time
  const bpm = $derived(a?.tempo ?? 120);
  const sessionKey = $derived(keyOf(rel));
  const midiOn = $derived(midi.playing && midi.rel === "notes:" + sessionKey);
  const temoinOn = $derived(!!temoin && player.is(temoin) && player.playing);
  const projLoading = $derived(midi.loading && midi.rel === "notes:" + sessionKey);
  // the song's chords played one after the other (from the chart's button): from the first one
  const chordsKey = $derived(`chords|${rel}`);
  const chordsOn = $derived(midi.playing && midi.rel === "notes:" + chordsKey);
  // the chords back to back: a stretch without any (empty bars) is skipped, and the chart's
  // playhead follows the song's bars through `jumps` (where in the song each played beat is)
  let jumps: { play: number; song: number }[] = [];
  const songAt = (p: number) => {
    let j = jumps[0];
    for (const x of jumps) if (x.play <= p + 1e-6) j = x;
    return j ? j.song + (p - j.play) : p;
  };
  function playChords() {
    if (!a || !h?.bars.length) return;
    if (chordsOn) return midi.stop();
    const bpb = a.beats_per_bar;
    jumps = [];
    let shift = 0;
    let lastEnd = -1;
    const notes: [number, number, number][] = [];
    for (const b of h.bars) {
      const songStart = b.bar * bpb + b.at;
      if (lastEnd < 0 || songStart - lastEnd > 1e-6) {
        // a gap (or the start): the play time goes on where the last chord ended
        const playStart = lastEnd < 0 ? 0 : lastEnd - shift;
        shift = songStart - playStart;
        jumps.push({ play: playStart, song: songStart });
      }
      lastEnd = songStart + b.len;
      const at = songStart - shift;
      const ps = chordNotes({ root: b.root, q: b.quality });
      // the bass under it when it is not the root
      if (b.bass) {
        const bi = keyNames.indexOf(b.bass);
        if (bi >= 0) ps[0] = 36 + bi + (bi < 4 ? 12 : 0);
      }
      for (const p of ps) notes.push([p, at, b.len * 0.95]);
    }
    midi.loop = false;
    midi.playNotes(chordsKey, t("song.chart.title"), notes, lastEnd - shift, bpm, false);
  }
  // ---- where the render starts in the arrangement (renderOffset): found by listening the first
  // time, set by hand from the button; every place the render meets the arrangement uses it
  const tOff = $derived(renderOffset.get(temoin));
  $effect(() => {
    const arr = a, r = temoin;
    if (!arr || !r || renderOffset.known(r)) return;
    infoFor(r).then((i) => {
      if (!i || renderOffset.known(r)) return;
      const o = detectOffset(arr, i.peaks, i.duration);
      renderOffset.set(r, o ?? 0, true);
      if (o) app.say(t("song.session.offsetFound", { bar: Math.round(o / (arr.beats_per_bar || 4)) + 1 }));
    }).catch(() => {});
  });
  function offsetMenu(e: MouseEvent) {
    if (!temoin || !a) return;
    const r = temoin;
    const bpbN = a.beats_per_bar || 4;
    const barNow = Math.round(tOff / bpbN) + 1;
    const at = head ?? (cue > 0 ? cue : null);
    const set = (bar: number) => {
      const was = temoinOn ? head : null;
      renderOffset.set(r, (Math.max(1, bar) - 1) * bpbN);
      // heard now: it goes on from where it is, in its new place
      if (was !== null) playTemoin(was || 0.001);
    };
    const items: MenuItem[] = [
      { label: t("song.session.offsetNow", { bar: barNow }) + (renderOffset.isAuto(r) && tOff ? ` (${t("song.session.offsetAuto")})` : ""), disabled: true },
      { sep: true },
      { label: t("song.session.offsetBar1"), checked: tOff === 0, run: () => set(1) },
      ...(at !== null && Math.floor(at / bpbN) + 1 !== barNow ? [{ label: t("song.session.offsetHere", { bar: Math.floor(at / bpbN) + 1 }), run: () => set(Math.floor(at / bpbN) + 1) }] : []),
      { label: t("song.session.offsetEarlier"), disabled: barNow <= 1, run: () => set(barNow - 1) },
      { label: t("song.session.offsetLater"), run: () => set(barNow + 1) },
      { sep: true },
      {
        label: t("song.session.offsetDetect"),
        run: () => infoFor(r).then((i) => {
          const o = i && a ? detectOffset(a, i.peaks, i.duration) : null;
          renderOffset.set(r, o ?? 0, true);
          app.say(o ? t("song.session.offsetFound", { bar: Math.round(o / bpbN) + 1 }) : t("song.session.offsetNone"));
        }),
      },
    ];
    openMenu(e, items, t("song.session.offsetTitle"));
  }
  const head = $derived(midiOn ? (midi.t * bpm) / 60 : chordsOn ? songAt((midi.t * bpm) / 60) : temoin && player.is(temoin) && player.playing ? (player.t * bpm) / 60 + tOff : null);
  // a new playback follows again
  const headOn = $derived(head !== null);
  $effect(() => {
    if (headOn) untrack(() => (followPaused = false));
  });
  const nowChord = $derived(head === null ? null : (chordRuns.find((r) => head! >= r.from && head! < r.to) ?? null));
  // what the play button plays: the render (default, when there is one) or the project
  // itself; the choice is kept. The switch always shows what is sounding: the render started
  // before the arrangement was loaded (the sheet's bar, a list) shows as the render.
  type Src = "project" | "temoin";
  const SRC = "spool.session.src";
  let srcPick = $state<Src>((() => { try { return (localStorage.getItem(SRC) as Src) || "temoin"; } catch { return "temoin"; } })());
  const src = $derived<Src>(!temoin ? "project" : temoinOn ? "temoin" : midiOn || projLoading ? "project" : srcPick);
  // where the next play starts (a click on the ruler while nothing plays only moves it)
  let cue = $state(0);
  $effect(() => {
    void rel;
    cue = 0;
  });
  const sounding = $derived(midiOn || temoinOn || projLoading);
  function playPause() {
    if (sounding) {
      if (head !== null) cue = head;
      if (midiOn || projLoading) midi.stop();
      if (temoinOn) player.pause();
      return;
    }
    // from the top (cue 0): the render past its opening silence, see player.play
    if (src === "temoin") {
      const from = cue;
      void pastRender(from).then((past) => {
        if (past) cue = 0;
        playTemoin(past ? 0 : from);
      });
    } else playMidi(cue);
  }
  // the other source, from where this one is (both follow the same arrangement). A render
  // shorter than the project (an edit, a part) has nothing past its end: switched to there (or
  // played from there), it starts from the top, and the user is told where it ends.
  const renderLen = async () => (temoin ? (player.is(temoin) && player.len) || (await infoFor(temoin))?.duration || 0 : 0);
  async function pastRender(beat: number) {
    const len = await renderLen();
    if (!len || ((beat - tOff) * 60) / bpm < len - 0.25) return false;
    app.say(t("song.session.renderEndsTop", { at: mmss(len) }));
    return true;
  }
  async function setSrc(s: Src) {
    if (s === src) return;
    const was = sounding;
    let at: number | null = head ?? (cue > 0 ? cue : null);
    if (s === "temoin" && at !== null && (await pastRender(at))) at = 0;
    if (midiOn || projLoading) midi.stop();
    if (temoinOn) player.pause();
    srcPick = s;
    try { localStorage.setItem(SRC, s); } catch { /* not kept */ }
    if (at !== null) cue = at;
    if (was) {
      if (s === "temoin") playTemoin(cue || 0.001);
      else playMidi(cue || 0.001);
    }
  }
  /** Back to the first bar: what plays goes on from there; nothing playing, the next play
   *  starts there. */
  function toStart() {
    // the render paused where it was: its own place goes back too
    if (temoin && player.is(temoin) && !player.playing) player.seek(0);
    seekBeat(0);
  }

  // the sheet's transport bar (kept in sight at its top) plays through these
  $effect(() => {
    if (!a) return;
    // the song as it is now: read again in the teardown, the prop may already name the next tab
    const id = sid;
    const mine: SheetTransport = {
      sid: id,
      src,
      setSrc,
      playPause,
      sounding,
      loading: projLoading,
      at: head !== null ? Math.min(1, head / total) : cue > 0 ? cue / total : null,
      seek: (f: number) => seekBeat(f * total),
      toStart,
      secs,
      summary: t("song.session.bars", { n: bars }),
      key: h?.key ?? null,
      temoin,
      project: "notes:" + sessionKey,
      rel,
    };
    sheetTransport.set(id, mine);
    return () => sheetTransport.drop(id, mine);
  });

  // the chords heard, for the player bar (the render's and the project's playback)
  $effect(() => {
    if (!a || !h) return;
    const mine = chordSourceOf(h, a.beats_per_bar, bpm, temoin, "notes:" + sessionKey, tOff);
    if (!mine) return;
    chordSource.set(mine);
    return () => chordSource.release(mine);
  });

  // tracks not heard in "Project", by name, kept per project (projectPlay)
  let refPaths = $state<string[]>([]);
  $effect(() => {
    void app.version;
    api.refTracks().then((r) => (refPaths = r.map((p) => p.toLowerCase()))).catch(() => {});
  });
  let muteState = $state<Record<string, string[]>>(storedMutes());
  // a first time: what Live itself does not play (a track or group switched off, see
  // `ArrTrack.muted`) and the references (projectPlay)
  const mutedNames = $derived(a ? mutedNamesOf(a, rel, refPaths, muteState) : []);
  const isMuted = (name: string) => mutedNames.includes(name);
  function toggleMute(name: string) {
    const next = isMuted(name) ? mutedNames.filter((n) => n !== name) : [...mutedNames, name];
    muteState = { ...muteState, [rel]: next };
    storeMutes(muteState);
  }
  // solo, as in Live: when a track or a group is soloed, only the soloed ones are heard (a
  // group carries its tracks). Kept for the project while the sheet is open.
  let soloState = $state<Record<string, string[]>>({});
  const soloNames = $derived(soloState[rel] ?? []);
  const isSolo = (name: string) => soloNames.includes(name);
  function toggleSolo(name: string, only: boolean) {
    const next = isSolo(name) ? soloNames.filter((n) => n !== name) : only ? [name] : [...soloNames, name];
    soloState = { ...soloState, [rel]: next };
  }
  // each track's groups, the nearest first (groups nest: a group inside a group)
  const groupsOf = $derived(a ? groupsOfA(a) : []);
  // the tracks not heard: muted (or in a muted group), or left out by a solo
  const silent = $derived(a ? silentTracks(a, mutedNames, soloNames) : []);
  const isSilent = (tr: ArrTrack) => !!a && silent.includes(a.tracks.indexOf(tr));
  // what is muted or soloed reaches the project as it plays
  $effect(() => {
    const s = silent;
    if (midiOn) midi.setMuted(s);
  });
  // ---- what each row sounds now: a small meter by its name. The project playing, each
  // track's real level (its own gain, after mute and solo; a group, the loudest of its
  // tracks). The render playing (one mix, no tracks in it), the rows with a clip under the
  // playhead are lit, without a level. Drawn a frame at a time straight on the meters, only
  // while something plays and the arrangement is open.
  let gridEl = $state<HTMLElement>();
  const feeds = $derived.by(() => {
    if (!a) return [];
    const all = a.tracks;
    const gs = groupsOf;
    return rows.map((tr) => {
      const i = all.indexOf(tr);
      if (tr.kind !== "group") return [i];
      const out: number[] = [];
      gs.forEach((g, j) => g.includes(tr.name) && out.push(j));
      return out;
    });
  });
  $effect(() => {
    const el = gridEl;
    const arr = a;
    const fs = feeds;
    const proj = midiOn;
    const rend = !proj && temoinOn;
    if (!el || !arr || !open || (!proj && !rend)) return;
    const meters = [...el.querySelectorAll<HTMLElement>(".vu b")];
    el.classList.toggle("est", rend);
    const shown = new Float32Array(meters.length);
    const tempo = bpm;
    let raf = requestAnimationFrame(function tick() {
      let lv: Map<number, number> | null = null;
      let beat = 0;
      if (proj) lv = midi.levels();
      else beat = (player.t * tempo) / 60;
      for (let r = 0; r < meters.length; r++) {
        let v = 0;
        for (const ti of fs[r] ?? []) {
          if (lv) {
            const pk = lv.get(ti) ?? 0;
            const db = pk > 0 ? (20 * Math.log10(pk) + 48) / 48 : 0;
            if (db > v) v = db;
          } else {
            const tr = arr.tracks[ti];
            if (!silent.includes(ti) && tr?.clips.some((c) => !c.muted && beat >= c.start && beat < c.end)) v = 0.6;
          }
        }
        v = Math.max(0, Math.min(1, v));
        const k = lv ? Math.max(v, shown[r] * 0.86) : v;
        if (Math.abs(k - shown[r]) > 0.004) {
          shown[r] = k;
          // a cover over the scale, drawn back from the right: the colours stay where they are
          meters[r].style.transform = `scaleX(${(1 - k).toFixed(3)})`;
        }
      }
      raf = requestAnimationFrame(tick);
    });
    return () => {
      cancelAnimationFrame(raf);
      for (const m of meters) m.style.transform = "";
      el.classList.remove("est");
    };
  });
  // each track's sound in the app's synth: guessed from the set (instrument, preset, samples),
  // or set by hand from its name's menu when the guess sounds wrong. Kept per project; a track
  // set by hand says so, and goes back to the guess from the same menu.
  let soundState = $state<Record<string, Record<string, Fam>>>(storedSounds());
  const picks = $derived(soundState[rel] ?? {});
  const guessOf = (tr: ArrTrack) => patchOf(tr).fam;
  function setSound(tr: ArrTrack, fam: Fam | null) {
    const next = { ...picks };
    if (fam && fam !== guessOf(tr)) next[tr.name] = fam;
    else delete next[tr.name];
    soundState = { ...soundState, [rel]: next };
    storeSounds(soundState);
    // heard at once: the project plays on from where it was, with the new sound
    if (midiOn && head !== null) playArrangement(a!, rel, t("song.session.midiName", { title }), silent, head || 0.001, (m) => app.say(m));
  }
  function soundMenu(e: MouseEvent, tr: ArrTrack) {
    if (tr.kind !== "midi" || !tr.clips.some((c) => c.notes.length)) return;
    const guess = guessOf(tr);
    const cur = picks[tr.name] ?? guess;
    const items: MenuItem[] = [
      ...FAMS.map((f) => ({ label: t(`song.session.fam.${f}`) + (f === guess ? ` (${t("song.session.soundGuess")})` : ""), checked: f === cur, run: () => setSound(tr, f) })),
      // the orchestra, a sub-menu a family (its name checked when the sound is in it)
      ...FAM_GROUPS.map(([g, fs]) => ({
        label: t(`song.session.famGroup.${g}`),
        checked: fs.includes(cur),
        sub: fs.map((f) => ({ label: t(`song.session.fam.${f}`) + (f === guess ? ` (${t("song.session.soundGuess")})` : ""), checked: f === cur, run: () => setSound(tr, f) })),
      })),
      { sep: true },
      { label: t("song.session.soundReset"), disabled: !picks[tr.name], run: () => setSound(tr, null) },
      { label: t("song.session.soundResetAll"), disabled: !Object.keys(picks).length, run: () => resetSounds() },
    ];
    openMenu(e, items, t("song.session.soundTitle", { track: clean(tr.name) || "?" }));
  }
  function resetSounds() {
    const next = { ...soundState };
    delete next[rel];
    soundState = next;
    storeSounds(soundState);
    if (midiOn && head !== null) playArrangement(a!, rel, t("song.session.midiName", { title }), silent, head || 0.001, (m) => app.say(m));
  }
  async function playMidi(fromBeat = 0) {
    if (!a) return;
    if ((midiOn || projLoading) && fromBeat === 0) return midi.stop();
    await playArrangement(a, rel, t("song.session.midiName", { title }), silent, fromBeat, (m) => app.say(m));
  }
  function playTemoin(fromBeat = 0) {
    if (!temoin) return;
    if (temoinOn && fromBeat === 0) return player.toggle();
    // the render's seconds: it starts `tOff` beats into the arrangement (before it: its start)
    player.play(temoin, sid, title, fromBeat === 0 ? 0 : Math.max(0.001, ((fromBeat - tOff) * 60) / bpm));
  }
  // a click on the ruler: what plays goes there; nothing playing, the next play starts there
  function seekRuler(e: MouseEvent) {
    if (dragged) return;
    const el = e.currentTarget as HTMLElement;
    const r = el.getBoundingClientRect();
    seekBeat(Math.max(0, ((e.clientX - r.left) / r.width) * total));
  }
  // a bar of the chart clicked: what plays goes there; nothing playing, it plays from there
  function jumpTo(beat: number) {
    if (chordsOn) midi.stop();
    if (sounding && !chordsOn) return seekBeat(beat);
    cue = beat;
    if (src === "temoin")
      void pastRender(beat).then((past) => {
        if (past) cue = 0;
        playTemoin(past ? 0 : beat || 0.001);
      });
    else playMidi(beat || 0.001);
  }
  function seekBeat(beat: number) {
    cue = beat;
    if (midiOn) midi.seek(beat / total);
    else if (temoinOn) {
      if (beat <= 0) return playTemoin(0.001);
      void pastRender(beat).then((past) => {
        if (past) cue = 0;
        playTemoin(past ? 0.001 : beat);
      });
    }
  }
</script>

<svelte:window onkeydown={zoomKeys} />
{#snippet versionPick()}
  {#if sorted.length > 1}
    <select bind:value={rel} aria-label={t("song.session.version")} title={t("song.session.versionTip")}>
      {#each sorted as f (f.rel)}<option value={f.rel}>{f.name}{f.old ? ` (${t("song.session.old")})` : ""}</option>{/each}
    </select>
  {/if}
{/snippet}
{#if api.withArrangement && open}
  <!-- played from the sheet's transport bar (kept in sight at its top) -->
  <section class="card session wide" data-sec="session">
    <FoldHead {sid} key="session" title={t("song.session.title")}>
      <span class="small faint">
        {#if a}{[a.tempo ? t("common.bpm", { n: Math.round(a.tempo * 10) / 10 }) : "", t("song.session.bars", { n: bars }), secs ? mmss(secs) : ""].filter(Boolean).join(" · ")}{/if}
      </span>
      {#snippet actions()}
        {@render versionPick()}
        {#if temoin}<button class="btn sm" title={t("song.session.offsetTip")} onclick={offsetMenu}>{t("song.session.offsetBtn", { bar: Math.round(tOff / (a?.beats_per_bar || 4)) + 1 })}</button>{/if}
        <span class="zoombtns">
          <button class="btn sm zb" aria-label={t("song.session.zoomOut")} title={t("song.session.zoomOutTip")} onclick={() => zoomBy(1 / 1.5)} disabled={vis >= bars}>−</button>
          <button class="btn sm zb" aria-label={t("song.session.zoomIn")} title={t("song.session.zoomInTip")} onclick={() => zoomBy(1.5)} disabled={vis <= 1}>+</button>
        </span>
        {#if z > 1.001}<button class="btn sm" class:on={tracking} aria-pressed={tracking} title={tracking ? t("song.session.followOnTip") : followOn ? t("song.session.followPausedTip") : t("song.session.followOffTip")} onclick={() => setFollow(!tracking)}>{t("song.session.follow")}</button>{/if}
        {#if bars > VIEW_BARS || songBars < bars}<button class="btn sm" aria-pressed={!zoomOn} title={zoomOn ? t("song.session.wholeTip") : t("song.session.zoomTip", { n: VIEW_BARS })} onclick={() => setZoom(!zoomOn)}>{zoomOn ? t("song.session.whole") : t("song.session.zoom")}</button>{/if}
        <label class="small muted"><input type="checkbox" bind:checked={all} /> {t("song.session.allTracks")}</label>
      {/snippet}
    </FoldHead>
    {#if loading}<p class="small faint">{t("song.session.reading")}</p>
    {:else if error}<p class="small bad">{error}</p>
    {:else if a && !rows.length}<p class="small faint">{t("song.session.empty")}</p>
    {:else if a}
      <div class="grid" style="--n:{rows.length}" bind:this={gridEl} role="presentation" onpointerenter={() => (overGrid = true)} onpointerleave={() => (overGrid = false)}>
        <div class="names">
          <div class="corner"></div>
          {#if chordRuns.length}<div class="nm chordnm">{t("song.harmony.chords")}</div>{/if}
          {#each rows as tr, i (i)}
            <div class="nm" class:group={tr.kind === "group"} class:in={tr.grouped} class:off={isSilent(tr)} title={tr.muted ? `${tr.name}\n${t("song.session.offInLive")}` : tr.name}>
              <button class="mute" class:on={isMuted(tr.name)} aria-pressed={isMuted(tr.name)} title={isMuted(tr.name) ? t(tr.kind === "group" ? "song.session.unmuteGroup" : "song.session.unmute") : t(tr.kind === "group" ? "song.session.muteGroup" : "song.session.mute")} onclick={() => toggleMute(tr.name)}>
                <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M4 9h4l5-4v14l-5-4H4z" fill="currentColor" stroke="none" />{#if isMuted(tr.name)}<path d="M16 9l5 6M21 9l-5 6" />{:else}<path d="M16.5 8.5a5 5 0 0 1 0 7" />{/if}</svg></button>
              <button class="solo" class:on={isSolo(tr.name)} aria-pressed={isSolo(tr.name)} title={isSolo(tr.name) ? t("song.session.unsolo") : t(tr.kind === "group" ? "song.session.soloGroup" : "song.session.solo")} onclick={(e) => toggleSolo(tr.name, !(e.ctrlKey || e.metaKey))}>S</button>
              {#if tr.kind === "midi" && tr.clips.some((c) => c.notes.length)}
                <button class="snd" class:set={!!picks[tr.name]} style:--c={colorOf(tr.role)}
                  title={picks[tr.name] ? t("song.session.soundSet", { fam: t(`song.session.fam.${picks[tr.name]}`) }) : t("song.session.soundTip", { fam: t(`song.session.fam.${guessOf(tr)}`) })}
                  aria-label={t("song.session.soundTitle", { track: clean(tr.name) || "?" })} onclick={(e) => soundMenu(e, tr)}></button>
              {:else}<i style:background={colorOf(tr.role)}></i>{/if}
              {#if tr.kind === "midi" && tr.clips.some((c) => c.notes.length)}
                <!-- the track's name and the sound it plays here: a click chooses another one -->
                <button class="trk" title={picks[tr.name] ? t("song.session.soundSet", { fam: t(`song.session.fam.${picks[tr.name]}`) }) : t("song.session.soundTip", { fam: t(`song.session.fam.${guessOf(tr)}`) })}
                  onclick={(e) => soundMenu(e, tr)} oncontextmenu={(e) => soundMenu(e, tr)}><span>{clean(tr.name) || "?"}</span><em class="sndname" class:set={!!picks[tr.name]}>{t(`song.session.fam.${picks[tr.name] ?? guessOf(tr)}`)}</em><svg viewBox="0 0 10 10" width="7" height="7" aria-hidden="true"><path d="M2 3.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg></button>
              {:else}<span>{clean(tr.name) || "?"}</span>{/if}
              <span class="vu" aria-hidden="true"><b></b></span>
            </div>
          {/each}
        </div>
        <div class="lane" id="lane-{sid}" use:widthOf={(w) => (laneW = w)} use:wheelPan>
          <div class="zoom" class:follow class:zooming style:width="{z * 100}%" style:transform={off ? `translateX(${-off}px)` : null}>
          {#if head !== null}<span class="playhead" style:left={pct(Math.min(head, total))}></span>
          {:else if cue > 0}<span class="playhead cue" style:left={pct(Math.min(cue, total))}></span>{/if}
          <div class="ruler" role="presentation" title={t("song.session.rulerTip")} onclick={seekRuler} oncontextmenu={rulerMenu} bind:this={rulerEl}
            onpointerdown={rulerDown} onpointermove={rulerMove} onpointerup={rulerUp}>
            {#each sections as s, k (s.i)}
              {#if renaming === s.i}
                <input class="secname" style:left={pct(s.from)} value={s.name} aria-label={t("song.markers.rename")}
                  onclick={(e) => e.stopPropagation()} use:focusSelect
                  onblur={(e) => renameMarker(s.i, e.currentTarget.value)}
                  onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); if (e.key === "Escape") renaming = null; }} />
              {:else}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="sec" class:alt={k % 2 === 1} class:moving={mdrag?.i === s.i && mdrag.moved} data-loc={s.i} style:left={pct(secLeft(s))} style:width={pct(s.to - s.from)}
                  title={s.name + "\n" + t("song.markers.tip")} ondblclick={(e) => { e.stopPropagation(); renaming = s.i; }}
                  onpointerdown={(e) => markerDown(e, s.i)} onpointermove={markerMove} onpointerup={markerUp}>{s.name}</span>
              {/if}
            {/each}
            {#each ruler as b (b)}<span class="bar" style:left={pct(b * (a.beats_per_bar))}>{b + 1}</span>{/each}
          </div>
          {#if chordRuns.length}
            <div class="chords">
              <!-- the chord heard, named at the playhead even where its block is too narrow -->
              {#if nowChord}<span class="chnow" style:left={pct(Math.min(head ?? 0, total))}>{nowChord.name}</span>{/if}
              {#each chordRuns as r, i (i)}
                <button class="ch" class:tiny={(r.to - r.from) / total < 0.03} class:on={cur && cur.root === r.c.root && cur.q === r.c.q} style:left={pct(r.from)} style:width={pct(r.to - r.from)}
                  title={r.name} onclick={() => pickChord(r.c, r.name)}>{r.name}</button>
              {/each}
            </div>
          {/if}
          <div class="tracks">
            {#each sections as s, i (i)}{#if i % 2 === 1}<span class="band" style:left={pct(s.from)} style:width={pct(s.to - s.from)}></span>{/if}{/each}
            {#each locs as l, i (i)}<span class="loc" style:left={pct(l.at)} title={l.name}></span>{/each}
            {#each rows as tr, ti (ti)}<div class="row" class:group={tr.kind === "group"}></div>{/each}
            <!-- the clips drawn in one picture: hundreds of clips and their notes as page
                 elements made every scroll of the sheet repaint thousands of them -->
            <canvas class="clips" bind:this={cv} style:cursor={hover ? "pointer" : "default"} data-tip={hoverTip}
              onmousemove={move} onmouseleave={() => (hover = null)} onclick={clickClip} oncontextmenu={clipMenu}></canvas>
          </div>
          </div>
        </div>
      </div>
      {#if z > 1.001}
        <!-- what is seen of the whole: drag it, or click where to go -->
        <div class="hscroll" role="scrollbar" tabindex="0" aria-controls="lane-{sid}" aria-orientation="horizontal" aria-label={t("song.session.scrollTip")} aria-valuenow={Math.round((off / Math.max(1, laneW * z - laneW)) * 100)}
          title={t("song.session.scrollTip")} onpointerdown={scrollDown} onpointermove={scrollMove} onpointerup={() => (sdrag = null)}
          onkeydown={(e) => { if (e.key === "ArrowLeft" || e.key === "ArrowRight") { e.preventDefault(); pan(off + (e.key === "ArrowRight" ? 1 : -1) * laneW * 0.25); } }}>
          <i style:left="{(off / (laneW * z)) * 100}%" style:width="{Math.max(2, (1 / z) * 100)}%"></i>
          {#if songBars < bars}<span class="songend" style:left="{(songBars / bars) * 100}%" title={t("song.session.songEnd")}></span>{/if}
        </div>
      {/if}
      {#if edited}
        <p class="small pending">{t("song.markers.pending")} <button class="link" onclick={() => setLocs(a?.locators ?? [])}>{t("song.markers.revert")}</button></p>
      {/if}
      <p class="small faint hint">{t("song.session.hint", { title })}</p>
    {/if}
  </section>
{/if}
{#if api.withArrangement && chordsOpen}
  <section class="card chordsec wide" data-sec="chords">
    <FoldHead {sid} key="chords" title={t("song.sections.label.chords")}>
      {#if h?.key}<span class="small faint">{h.key}</span>{/if}
      {#snippet actions()}{#if !open}{@render versionPick()}{/if}{/snippet}
    </FoldHead>
    {#if a && h}
      <ChordChart {h} {bars} bpb={a.beats_per_bar} {bpm} {sections} {head} {cur} {melody} pick={pickChord} playAll={playChords} playingAll={chordsOn} jump={jumpTo} />
      <HarmonyPanel {h} bind:cur bpm={a.tempo ?? 120} bar={a.beats_per_bar} {title} />
    {:else if loading}<p class="small faint">{t("song.session.reading")}</p>
    {:else if error}<p class="small bad">{error}</p>
    {:else if a}<p class="small faint">{t("song.sections.noChords")}</p>{/if}
  </section>
{/if}

<style>
  select { border: 1px solid var(--line); background: var(--panel); color: var(--ink); border-radius: 7px; padding: 3px 6px; font: inherit; font-size: 12.5px; max-width: 280px; }
  /* the whole arrangement in the sheet (no scroller of its own): its timeline and chords stay
     in sight under the section's title while the sheet scrolls by, ready for a click */
  .grid { --stick: calc(var(--sheet-top, 0px) + var(--fh-h, 0px)); display: grid; grid-template-columns: 210px minmax(0, 1fr); margin-top: 10px; overflow: clip; border: 1px solid var(--line); border-radius: 10px; background: var(--panel2); }
  .names { z-index: 2; background: var(--panel2); border-right: 1px solid var(--line); }
  .corner { height: 32px; border-bottom: 1px solid var(--line); position: sticky; top: var(--stick); background: var(--panel2); z-index: 3; }
  /* a track's sound: its role's colour; set by hand, a ring and the sound's name */
  .snd { flex: none; width: 8px; height: 8px; padding: 0; border: 0; border-radius: 50%; background: var(--c); cursor: pointer; }
  .snd:hover { box-shadow: 0 0 0 2px color-mix(in srgb, var(--c) 45%, transparent); }
  .snd.set { background: none; box-shadow: inset 0 0 0 2px var(--accent); }
  .trk { min-width: 0; flex: 1; display: flex; align-items: center; gap: 5px; padding: 0 2px; border: 0; border-radius: 4px; background: none; color: inherit; font: inherit; cursor: pointer; text-align: left; }
  .trk span { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .trk svg { flex: none; opacity: 0; color: var(--faint); }
  .trk:hover { background: var(--panel); color: var(--ink); }
  .trk:hover svg { opacity: 1; }
  .sndname { flex: none; max-width: 76px; overflow: hidden; text-overflow: ellipsis; font-style: normal; font-size: 10.5px; color: var(--faint); }
  .sndname.set { color: var(--accent); font-weight: 600; }
  .nm { height: 18px; display: flex; align-items: center; gap: 4px; padding: 0 8px; font-size: 11.5px; color: var(--muted); white-space: nowrap; overflow: hidden; }
  .nm span { overflow: hidden; text-overflow: ellipsis; }
  .nm i { width: 7px; height: 7px; border-radius: 2px; flex: none; }
  .nm.group { color: var(--ink); font-weight: 600; }
  .nm.in { padding-left: 18px; }
  .solo { border: 0; background: none; color: var(--faint); padding: 0 3px; cursor: pointer; font: 700 9.5px/14px system-ui, sans-serif; border-radius: 3px; flex: none; }
  .solo:hover { color: var(--ink); }
  .solo.on { color: #1a1a1a; background: var(--lead, #e3b341); }
  .lane { position: relative; min-width: 0; overflow-x: clip; }
  .zoom { position: relative; transition: transform .25s ease-out; }
  /* following the playhead: the position comes a few times a second (the render's player),
     each step glided at a steady speed so the lane runs smoothly under a still playhead */
  .zoom.follow { transition: transform .25s linear; }
  /* following the playhead: the button lit */
  .btn.on { border-color: var(--accent); color: var(--accent); }
  .zoom.zooming { transition: none; }
  .zoombtns { display: inline-flex; }
  .zb { min-width: 28px; padding-inline: 6px; font-weight: 700; }
  .zoombtns .zb:first-child { border-top-right-radius: 0; border-bottom-right-radius: 0; }
  .zoombtns .zb:last-child { border-top-left-radius: 0; border-bottom-left-radius: 0; margin-left: -1px; }
  .ruler { touch-action: none; }
  .hscroll { position: relative; height: 12px; margin: 6px 0 0 210px; border-radius: 99px; background: var(--panel2); border: 1px solid var(--line); cursor: pointer; touch-action: none; }
  .hscroll i { position: absolute; top: 1px; bottom: 1px; border-radius: 99px; background: color-mix(in srgb, var(--ink) 30%, transparent); }
  .hscroll:hover i { background: color-mix(in srgb, var(--ink) 45%, transparent); }
  .hscroll .songend { position: absolute; top: -2px; bottom: -2px; width: 2px; margin-left: -1px; background: var(--accent); border-radius: 1px; pointer-events: none; }
  @media (prefers-reduced-motion: reduce) { .zoom, .zoom.follow { transition: none; } }
  .ruler { position: sticky; top: var(--stick); z-index: 3; height: 32px; border-bottom: 1px solid var(--line); background: var(--panel2); }
  .bar { position: absolute; top: 18px; font-size: 10px; color: var(--faint); padding-left: 3px; border-left: 1px solid var(--line); line-height: 14px; }
  .sec { position: absolute; top: 0; height: 17px; font-size: 10.5px; font-weight: 600; color: var(--ink); padding: 3px 4px 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; background: color-mix(in srgb, var(--accent) 10%, transparent); border-left: 2px solid color-mix(in srgb, var(--accent) 55%, transparent); }
  .sec.alt { background: color-mix(in srgb, var(--accent) 4%, transparent); }
  .sec { cursor: grab; touch-action: none; }
  .sec.moving { cursor: grabbing; z-index: 2; background: color-mix(in srgb, var(--accent) 24%, transparent); }
  .secname { position: absolute; top: 0; z-index: 4; width: 140px; height: 17px; padding: 0 4px; border: 1px solid var(--accent); border-radius: 3px; background: var(--panel); color: var(--ink); font: 600 10.5px/15px system-ui, sans-serif; outline: 0; }
  .pending { margin: 8px 0 0; color: var(--muted); }
  .pending .link { border: 0; background: none; color: var(--accent); cursor: pointer; padding: 0; font: inherit; }
  .pending .link:hover { text-decoration: underline; }
  .tracks { position: relative; }
  .clips { position: absolute; top: 0; left: 0; display: block; z-index: 1; }
  .ruler { cursor: pointer; }
  .playhead.cue { background: color-mix(in srgb, var(--rec) 45%, transparent); }
  .playhead { position: absolute; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: var(--rec); z-index: 4; pointer-events: none; }
  .loc { position: absolute; top: 0; bottom: 0; width: 0; border-left: 1px dashed color-mix(in srgb, var(--accent) 45%, transparent); pointer-events: none; z-index: 1; }
  .chords { position: sticky; top: calc(var(--stick) + 32px); z-index: 3; height: 20px; border-bottom: 1px solid var(--line); background: var(--panel); }
  .chordnm { position: sticky; top: calc(var(--stick) + 32px); z-index: 3; height: 20px; color: var(--ink); font-weight: 600; border-bottom: 1px solid var(--line); background: var(--panel); }
  .ch { position: absolute; top: 2px; bottom: 2px; padding: 0 4px; border: 0; border-left: 1px solid var(--line); background: none; color: var(--ink); font: inherit; font-size: 11px; font-weight: 600; text-align: left; cursor: pointer; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .ch:hover { background: var(--accent-bg); }
  /* a chord too short for its name: a mark, named on hover */
  .ch.tiny { font-size: 0; padding: 0; background: color-mix(in srgb, var(--accent) 12%, transparent); }
  .ch.on { background: color-mix(in srgb, var(--rec) 18%, transparent); }
  .chnow { position: absolute; top: 1px; z-index: 5; transform: translateX(3px); padding: 0 6px; border-radius: 4px; background: var(--rec); color: #fff; font-size: 11px; font-weight: 700; line-height: 17px; white-space: nowrap; pointer-events: none; }
  .session { container-type: inline-size; }
  .band { position: absolute; top: 0; bottom: 0; background: color-mix(in srgb, var(--ink) 3%, transparent); pointer-events: none; }
  .row { position: relative; height: 18px; border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent); }
  .row.group { background: color-mix(in srgb, var(--ink) 4%, transparent); }
  .clip { position: absolute; top: 2px; bottom: 2px; padding: 0; border: 0; border-radius: 3px; cursor: pointer; overflow: hidden;
    background: color-mix(in srgb, var(--c) 32%, var(--panel)); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 70%, transparent); }
  .clip.audio { background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--c) 30%, var(--panel)) 0 4px, color-mix(in srgb, var(--c) 18%, var(--panel)) 4px 8px); }
  .clip:hover { box-shadow: inset 0 0 0 1px var(--c), 0 0 0 1px var(--c); z-index: 1; }
  .clip.on { background: var(--c); }
  .clip.muted { opacity: 0.35; }
  .clip i { position: absolute; height: 2px; background: color-mix(in srgb, var(--c) 90%, var(--ink)); border-radius: 1px; pointer-events: none; }
  .clip.on i { background: var(--panel); }
  .hint { margin: 8px 0 0; }
  .mute { border: 0; background: none; color: var(--faint); padding: 0 2px; cursor: pointer; display: inline-grid; place-items: center; flex: none; }
  .mute:hover { color: var(--ink); }
  .mute.on { color: var(--bad); }
  .nm.off span { opacity: .45; text-decoration: line-through; }
  /* a row's meter: green, amber near the top; lit without a level while the render plays */
  .vu { flex: none; position: relative; width: 22px; height: 4px; margin-left: auto; border-radius: 2px; overflow: hidden; background: linear-gradient(90deg, #3fb38a 0 70%, #e3b341 70% 90%, #e0604f 90%); }
  .nm.off .vu { opacity: 1; text-decoration: none; }
  .vu b { position: absolute; inset: 0; transform-origin: right; transform: scaleX(1); background: color-mix(in srgb, var(--ink) 9%, var(--panel2)); }
  .grid:global(.est) .vu { background: var(--accent); }
</style>

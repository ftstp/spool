<script lang="ts">
  // What plays in the player bar as a piano roll, in place of a waveform (one drawn from notes
  // said little): a MIDI file, a clip, or a whole project. Pitch upwards, time across, the part
  // played lit, the notes sounding now brighter. Several tracks: each in a colour of its own,
  // its name on hover; drums in a lane of their own under the pitched notes (their pitches are
  // pieces of a kit, not a melody); a project's audio clips as bars, a lane per track, at the
  // bottom. Muted tracks left out as they are muted. Click to seek.
  import type { Roll } from "../midiPlayer.svelte";
  import { widthOf } from "../size";
  import { t } from "../i18n/index.svelte";

  let { roll, muted = [], now = 0, seek, height = 32 }: { roll: Roll; muted?: number[]; now?: number; seek?: (frac: number) => void; height?: number } = $props();
  let canvas = $state<HTMLCanvasElement>();
  let w = $state(0);

  // a colour per track (one track: the accent), apart enough to be told at a few pixels
  const HUES = [205, 28, 140, 330, 50, 265, 175, 0, 95, 300, 225, 15];
  const many = $derived(roll.tracks.length > 1);
  const colorOf = (trk: number, accent: string) => {
    if (!many) return accent;
    const i = roll.tracks.findIndex((x) => x.trk === trk);
    return `hsl(${HUES[i % HUES.length]} 62% 62%)`;
  };

  type NoteR = Roll["notes"][number];
  type SegR = Roll["audio"][number];
  // what is drawn: the tracks not muted; pitched notes over their range, drums in their lane,
  // the audio tracks' lanes at the bottom
  const layout = $derived.by(() => {
    const off = new Set(muted);
    const notes = roll.notes.filter((x) => !off.has(x.trk));
    const audio = roll.audio.filter((x) => !off.has(x.trk));
    const pitched = notes.filter((x) => !x.dr);
    const drums = notes.filter((x) => x.dr);
    let lo = 127, hi = 0;
    for (const x of pitched) {
      lo = Math.min(lo, x.n);
      hi = Math.max(hi, x.n);
    }
    // a narrow range spread a little: three notes do not fill the bar's height
    if (pitched.length && hi - lo < 12) {
      const mid = (hi + lo) / 2;
      lo = Math.floor(mid - 6);
      hi = Math.ceil(mid + 6);
    }
    const kit = [...new Set(drums.map((x) => x.n))].sort((a, b) => a - b);
    const lanes = [...new Set(audio.map((x) => x.trk))].sort((a, b) => a - b);
    const maxD = notes.reduce((m, x) => Math.max(m, x.d), 0);
    return { notes, audio, pitched, drums, lo, hi, kit, lanes, maxD };
  });

  // heights, top to bottom (device pixels): pitched notes, drums, audio lanes
  const geo = (H: number) => {
    const L = layout;
    const sep = 2;
    const hasNotes = L.pitched.length + L.drums.length > 0;
    const audioH = L.lanes.length ? (hasNotes ? Math.round(H * 0.38) : H) : 0;
    const audioTop = H - audioH;
    const drumBottom = audioTop - (audioH && hasNotes ? sep : 0);
    const drumH = L.drums.length ? (L.pitched.length ? Math.max(5, Math.round(drumBottom * 0.28)) : drumBottom) : 0;
    const pitchH = drumBottom - drumH - (drumH && L.pitched.length ? sep : 0);
    const rowH = L.pitched.length ? pitchH / (L.hi - L.lo + 1) : 0;
    const kitH = L.kit.length ? drumH / L.kit.length : 0;
    const laneH = L.lanes.length ? audioH / L.lanes.length : 0;
    return { audioTop, drumBottom, drumH, pitchH, rowH, kitH, laneH };
  };
  function noteRect(x: NoteR, W: number, g: ReturnType<typeof geo>) {
    const L = layout;
    const x0 = (x.t / roll.len) * W;
    if (x.dr) {
      const k = L.kit.indexOf(x.n);
      return { x: x0, y: g.drumBottom - (k + 1) * g.kitH, w: Math.max(1, Math.min((x.d / roll.len) * W, 2)), h: Math.max(1, g.kitH) };
    }
    const hh = Math.max(1.5, g.rowH);
    return { x: x0, y: g.pitchH - (x.n - L.lo + 1) * g.rowH - (hh - g.rowH) / 2, w: Math.max(1.5, (x.d / roll.len) * W - 0.5), h: hh };
  }
  function segRect(x: SegR, W: number, g: ReturnType<typeof geo>) {
    const i = layout.lanes.indexOf(x.trk);
    return { x: (x.t / roll.len) * W, y: g.audioTop + i * g.laneH, w: Math.max(1.5, (x.d / roll.len) * W - 1), h: Math.max(1, g.laneH - (g.laneH > 3 ? 1 : 0)) };
  }
  function paint(c: HTMLCanvasElement, W: number, H: number, accent: string, alpha: number) {
    const g = c.getContext("2d")!;
    g.clearRect(0, 0, W, H);
    const gm = geo(H);
    for (const x of layout.audio) {
      const r = segRect(x, W, gm);
      g.globalAlpha = alpha * 0.75;
      g.fillStyle = colorOf(x.trk, accent);
      g.fillRect(r.x, r.y, r.w, r.h);
    }
    for (const x of layout.notes) {
      const r = noteRect(x, W, gm);
      g.globalAlpha = alpha * (0.55 + 0.45 * (x.v / 127));
      g.fillStyle = colorOf(x.trk, accent);
      g.fillRect(r.x, r.y, r.w, r.h);
    }
    g.globalAlpha = 1;
  }

  let colors = { accent: "#7ba6dc", line: "#e8e8ea" };
  let dim: HTMLCanvasElement | null = null;
  let lit: HTMLCanvasElement | null = null;
  // the notes drawn once per roll, mute and size (twice: dim, and lit for the played part);
  // each frame only copies them and draws what sounds now
  $effect(() => {
    const c = canvas;
    if (!c || !w) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(w * dpr), H = Math.round(height * dpr);
    void layout;
    const css = getComputedStyle(c);
    colors = { accent: css.getPropertyValue("--accent").trim() || "#7ba6dc", line: css.getPropertyValue("--ink").trim() || "#e8e8ea" };
    dim = Object.assign(document.createElement("canvas"), { width: W, height: H });
    lit = Object.assign(document.createElement("canvas"), { width: W, height: H });
    paint(dim, W, H, colors.accent, 0.38);
    paint(lit, W, H, colors.accent, 1);
    c.width = W;
    c.height = H;
    draw();
  });
  $effect(() => {
    void now;
    draw();
  });
  function draw() {
    const c = canvas;
    if (!c || !dim || !lit || dim.width !== c.width) return;
    const g = c.getContext("2d")!;
    const W = c.width, H = c.height;
    const px = Math.max(0, Math.min(W, (now / (roll.len || 1)) * W));
    g.clearRect(0, 0, W, H);
    g.drawImage(dim, 0, 0);
    if (px > 0) g.drawImage(lit, 0, 0, px, H, 0, 0, px, H);
    const gm = geo(H);
    const L = layout;
    if (now > 0) {
      // the notes sounding now, brighter, and the playhead
      g.fillStyle = colors.line;
      g.globalAlpha = 0.85;
      for (const x of L.notes) {
        if (x.t > now) break;
        if (x.t + x.d < now || x.t < now - L.maxD) continue;
        const r = noteRect(x, W, gm);
        g.fillRect(r.x, r.y, r.w, r.h);
      }
      g.globalAlpha = 0.7;
      g.fillRect(Math.round(px), 0, Math.max(1, window.devicePixelRatio || 1), H);
      g.globalAlpha = 1;
    }
  }

  // what is under the pointer: the track (several tracks) and the note
  const NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
  let tip = $state("");
  function hover(e: MouseEvent) {
    if (!canvas || !w) return;
    const dpr = window.devicePixelRatio || 1;
    const W = canvas.width, H = canvas.height;
    const mx = e.offsetX * dpr, my = e.offsetY * dpr;
    const gm = geo(H);
    const dist = (r: { x: number; y: number; w: number; h: number }) =>
      (mx < r.x ? r.x - mx : mx > r.x + r.w ? mx - r.x - r.w : 0) + 2 * (my < r.y ? r.y - my : my > r.y + r.h ? my - r.y - r.h : 0);
    let best: { trk: number; what: string } | null = null;
    let bd = Infinity;
    for (const x of layout.notes) {
      const d = dist(noteRect(x, W, gm));
      if (d < bd) { bd = d; best = { trk: x.trk, what: x.dr ? t("midi.roll.drums") : `${NAMES[x.n % 12]}${Math.floor(x.n / 12) - 2}` }; }
    }
    for (const x of layout.audio) {
      const d = dist(segRect(x, W, gm));
      if (d < bd) { bd = d; best = { trk: x.trk, what: t("midi.roll.audio") }; }
    }
    if (!best || bd > 6 * dpr) { tip = ""; return; }
    const i = roll.tracks.findIndex((x) => x.trk === best!.trk);
    const name = roll.tracks[i]?.name || t("midi.roll.track", { n: i + 1 });
    tip = many ? `${name} · ${best.what}` : best.what;
  }
</script>

<div class="roll" use:widthOf={(x) => (w = x)} style:height="{height}px">
  <canvas bind:this={canvas} style:width="{w}px" style:height="{height}px" class:seek={!!seek} aria-hidden="true" title={tip || null}
    onmousemove={hover} onmouseleave={() => (tip = "")}
    onclick={(e) => seek?.(e.offsetX / w)}></canvas>
</div>

<style>
  .roll { width: 100%; min-width: 0; border-radius: 4px; background: color-mix(in srgb, var(--ink) 4%, transparent); }
  canvas { display: block; }
  .seek { cursor: pointer; }
</style>

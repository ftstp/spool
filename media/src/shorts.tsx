// Three vertical shorts (1080x1920) for Reels / Shorts / TikTok, from the tour's material.
// Every name shown is fictional. Safe zones: nothing that matters above y 192, below y 1536,
// or right of x 918 (the platforms' buttons and captions sit there).
import React from "react";
import { AbsoluteFill, Audio, Easing, Img, Sequence, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { b } from "./beat";
import { FatstepLogo } from "./FatstepLogo";
import { MONO, Mode, SANS, SpoolMark, T, ease, easeOut, prog } from "./ui";

// story beats: scenes until beat 28, the end card from beat 28; its last line lands on beat 32,
// the downbeat of the track's final chord. 36 beats of music + 1 s of ring-out.
export const SHORT_TOTAL = b(36) + 30;
const SAFE = { left: 72, right: 918, top: 200, bottom: 1536 };
const PANEL = { left: 72, top: 560, w: 846, h: 940 };
const FADE = 8;

// ---- shared pieces ------------------------------------------------------------------------------
const Bg: React.FC<{ mode: Mode }> = ({ mode }) => {
  const c = T[mode];
  const glow = mode === "light" ? "rgba(255,255,255,.8)" : "rgba(255,255,255,.05)";
  return <AbsoluteFill style={{ background: `radial-gradient(900px 1100px at 45% 55%, ${glow}, transparent 70%), ${c.bg}` }} />;
};

const In: React.FC<{ children: React.ReactNode; first?: boolean }> = ({ children, first }) => {
  const f = useCurrentFrame();
  return <AbsoluteFill style={{ opacity: first ? 1 : prog(f, 0, FADE, Easing.inOut(Easing.quad)) }}>{children}</AbsoluteFill>;
};

/** Big caption in the top safe area: a mono kicker, then up to 2 lines. */
const Cap: React.FC<{ mode: Mode; kicker?: string; text: string; size?: number; at?: number }> = ({ mode, kicker, text, size = 66, at = 0 }) => {
  const f = useCurrentFrame();
  const c = T[mode];
  const a = prog(f, at, at + 10, easeOut);
  return (
    <div style={{ position: "absolute", left: SAFE.left, top: SAFE.top + 20, width: SAFE.right - SAFE.left, opacity: a, transform: `translateY(${(1 - a) * 24}px)` }}>
      {kicker && (
        <div style={{ fontFamily: MONO, fontSize: 26, letterSpacing: "0.12em", textTransform: "uppercase", color: c.muted, display: "flex", alignItems: "center", gap: 14, marginBottom: 18 }}>
          <span style={{ width: 14, height: 14, borderRadius: 14, background: c.rec, display: "inline-block" }} />{kicker}
        </div>
      )}
      <div style={{ fontFamily: SANS, fontWeight: 600, fontSize: size, lineHeight: 1.12, letterSpacing: "-0.02em", color: c.ink }}>{text}</div>
    </div>
  );
};

/** A region of a 1440x900 screenshot in a rounded panel. Keys: centre (x, y) in screenshot css px, zoom z. */
type CK = { f: number; x: number; y: number; z: number };
const at = (f: number, keys: CK[]) => {
  let k0 = keys[0], k1 = keys[0];
  for (let i = 0; i < keys.length; i++) if (keys[i].f <= f) { k0 = keys[i]; k1 = keys[i + 1] ?? keys[i]; }
  if (f < keys[0].f) k1 = k0;
  const t = k1 === k0 ? 0 : prog(f, k0.f, k1.f, ease);
  return { x: k0.x + (k1.x - k0.x) * t, y: k0.y + (k1.y - k0.y) * t, z: k0.z + (k1.z - k0.z) * t };
};
const Crop: React.FC<{ shot: string; mode: Mode; keys: CK[]; overlay?: React.ReactNode; layers?: { shot: string; opacity: number }[] }> = ({ shot, mode, keys, overlay, layers = [] }) => {
  const f = useCurrentFrame();
  const { x, y, z } = at(f, keys);
  const c = T[mode];
  return (
    <div style={{ position: "absolute", left: PANEL.left, top: PANEL.top, width: PANEL.w, height: PANEL.h, borderRadius: 30, overflow: "hidden", background: c.bg,
      boxShadow: mode === "light" ? "0 50px 100px -40px rgba(40,30,20,.45), 0 0 0 2px rgba(22,22,22,.10)" : "0 50px 110px -40px rgba(0,0,0,.9), 0 0 0 2px rgba(255,255,255,.10)" }}>
      {/* a fixed 1440x900 (css px) layer, moved by one unrounded translate3d + scale: no layout
          changes per frame, and the 5760 px screenshot is always drawn smaller than its pixels */}
      <div style={{ position: "absolute", left: 0, top: 0, width: 1440, height: 900, transformOrigin: "0 0",
        transform: `translate3d(${PANEL.w / 2 - x * z}px, ${PANEL.h / 2 - y * z}px, 0) scale(${z})` }}>
        <Img src={staticFile(`shots/${shot}.jpg`)} style={{ position: "absolute", left: 0, top: 0, width: 1440, height: 900 }} />
        {layers.map((l) => <Img key={l.shot} src={staticFile(`shots/${l.shot}.jpg`)} style={{ position: "absolute", left: 0, top: 0, width: 1440, height: 900, opacity: l.opacity }} />)}
        {overlay}
      </div>
    </div>
  );
};

const EndCard: React.FC = () => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const c = T.dark;
  const pop = spring({ frame: f, fps, config: { damping: 18, stiffness: 90 } });
  const line = (a: number) => ({ opacity: prog(f, a - 6, a + 6), transform: `translateY(${(1 - prog(f, a - 6, a + 12, easeOut)) * 20}px)` });
  return (
    <In>
      <AbsoluteFill style={{ background: "radial-gradient(900px 1100px at 40% 50%, rgba(255,255,255,.06), transparent 70%), #161616" }} />
      <div style={{ position: "absolute", left: SAFE.left + 8, top: 560, width: SAFE.right - SAFE.left }}>
        <div style={{ display: "flex", alignItems: "center", gap: 28, opacity: pop, transform: `scale(${0.9 + 0.1 * pop})`, transformOrigin: "0 50%" }}>
          <SpoolMark size={130} mode="light" />
          <div style={{ fontFamily: MONO, fontWeight: 600, fontSize: 128, letterSpacing: "-0.04em", color: c.ink, lineHeight: 1 }}>spool</div>
        </div>
        <div style={{ marginTop: 60, fontFamily: SANS, fontWeight: 600, fontSize: 64, lineHeight: 1.12, color: c.ink, ...line(b(1)) }}>Spool, free for<br />Ableton Live</div>
        <div style={{ marginTop: 36, fontFamily: MONO, fontSize: 42, color: "#ff3b2f", ...line(b(2)) }}>artists.fatstep.com/spool</div>
        <div style={{ marginTop: 90, display: "flex", alignItems: "center", gap: 20, fontFamily: SANS, fontSize: 34, color: c.muted, ...line(b(4)) }}>
          by <FatstepLogo height={54} color={c.ink} markColor={c.mark} />
        </div>
      </div>
    </In>
  );
};

/** Lays scenes out on beats; the end card from beat 28 to the end. */
const Short: React.FC<{ music: string; scenes: [number, React.FC][] }> = ({ music, scenes }) => {
  const all: [number, React.FC][] = [...scenes, [28, EndCard]];
  return (
    <AbsoluteFill style={{ background: "#161616" }}>
      {all.map(([beat, C], i) => {
        const from = b(beat);
        const to = i + 1 < all.length ? b(all[i + 1][0]) + FADE : SHORT_TOTAL;
        return <Sequence key={i} from={from} durationInFrames={to - from}><C /></Sequence>;
      })}
      <Audio src={staticFile(music)} />
    </AbsoluteFill>
  );
};

// ---- mockup bits ------------------------------------------------------------------------------
const FileCard: React.FC<{ name: string; dir?: string; c: typeof T.light; style?: React.CSSProperties; badge?: React.ReactNode }> = ({ name, dir, c, style, badge }) => (
  <div style={{ position: "absolute", padding: "22px 28px", borderRadius: 18, background: c.panel, boxShadow: "0 16px 36px -18px rgba(40,30,20,.4), 0 0 0 1px rgba(22,22,22,.08)",
    fontFamily: MONO, whiteSpace: "nowrap", ...style }}>
    <div style={{ fontSize: 34, color: c.ink, display: "flex", alignItems: "center", gap: 16 }}>
      <svg width="28" height="34" viewBox="0 0 22 26"><path d="M2 1h12l6 6v18H2z" fill="none" stroke={c.muted} strokeWidth="1.6" /><path d="M14 1v6h6" fill="none" stroke={c.muted} strokeWidth="1.6" /></svg>
      {name}{badge}
    </div>
    {dir && <div style={{ fontSize: 22, color: c.faint, marginTop: 6, paddingLeft: 44 }}>{dir}</div>}
  </div>
);

// =====================================================================================
// Short 1: "Years of .als files, one catalogue"
// =====================================================================================
const FILES1 = [
  ["Ivory Current v2.als", "Sketches/2019", 0], ["Gentle Cascade.als", "Projects", 1], ["Ivory Current final.als", "Desktop/old", 0],
  ["Hazy Carousel 2018.als", "Backup", 2], ["Gentle Cascade v2.1.als", "Projects", 1], ["Copper Quarry.als", "Collabs", 3],
  ["Ivory Current [old].als", "Backup", 0], ["Hazy Carousel 2.als", "Downloads", 2], ["Copper Quarry 07-01.als", "Collabs", 3],
] as const;
const SONGS1 = [
  { t: "Ivory Current", m: "Kaito Brenner · 85 bpm", s: "Draft", col: "#eda100", art: "#9b5a4f", n: 3 },
  { t: "Gentle Cascade", m: "Nova Lindqvist · 80 bpm", s: "Mix", col: "#2a78d6", art: "#7a4f8f", n: 2 },
  { t: "Hazy Carousel", m: "Nova Lindqvist · 84 bpm", s: "Draft", col: "#eda100", art: "#5f8a45", n: 2 },
  { t: "Copper Quarry", m: "Nova Lindqvist · 77 bpm", s: "Arrangement", col: "#e87ba4", art: "#4f5f94", n: 2 },
];
const S1Files: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  return (
    <In first>
      <Bg mode="light" />
      {FILES1.map(([name, dir, song], i) => {
        // they fly in from below on the first beats (the hook), then gather into their song on beat 7
        const sx = 80 + (i % 2) * 190 + ((i * 37) % 60), sy = 600 + i * 98;
        const enter = prog(f, i * 2, i * 2 + 16, easeOut);
        const g = prog(f, b(5) + i, b(7), ease);
        const ty = 640 + (song as number) * 210;
        return (
          <FileCard key={i} name={name as string} dir={dir as string} c={c}
            style={{ left: 0, top: 0, opacity: enter * (1 - prog(f, b(7) - 4, b(7) + 4)), transformOrigin: "0 0",
              transform: `translate3d(${sx + (90 - sx) * g}px, ${sy + (1 - enter) * 500 + (ty - sy) * g}px, 0) rotate(${(((i * 37) % 9) - 4) * 1.4 * (1 - g)}deg) scale(${1 - 0.3 * g})` }} />
        );
      })}
      {SONGS1.map((s, i) => {
        const a = prog(f, b(6) + i * 3, b(7) + i * 3, easeOut);
        return (
          <div key={s.t} style={{ position: "absolute", left: 72, top: 600 + i * 210, width: 846, height: 180, opacity: a, transform: `translateY(${(1 - a) * 30}px)`, borderRadius: 24,
            background: c.panel, boxShadow: "0 20px 44px -24px rgba(40,30,20,.45), 0 0 0 1px rgba(22,22,22,.08)", display: "flex", alignItems: "center", gap: 28, padding: "0 32px", fontFamily: SANS }}>
            <div style={{ width: 112, height: 112, borderRadius: 18, background: s.art, color: "#fff", display: "grid", placeItems: "center", fontSize: 48, fontWeight: 600 }}>{s.t[0]}</div>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 44, fontWeight: 600, color: c.ink }}>{s.t}</div>
              <div style={{ fontSize: 28, color: c.muted, marginTop: 4 }}>{s.m}</div>
              <div style={{ fontSize: 28, color: c.ink, marginTop: 6, display: "flex", alignItems: "center", gap: 10 }}>
                <i style={{ width: 14, height: 14, borderRadius: 14, background: s.col, display: "inline-block" }} />{s.s}
                <span style={{ fontFamily: MONO, color: c.faint, marginLeft: 14 }}>{s.n} versions</span>
              </div>
            </div>
          </div>
        );
      })}
      <Cap mode="light" text={f < b(5) ? "Years of .als files." : "One catalogue of songs."} size={88} at={f < b(5) ? 0 : b(5)} />
    </In>
  );
};
const S1List: React.FC = () => (
  <In><Bg mode="light" />
    {/* tilt down the list column */}
    <Crop shot="songs-light" mode="light" keys={[{ f: 0, x: 420, y: 410, z: 2.0 }, { f: b(6) + FADE, x: 420, y: 640, z: 2.0 }]} />
    <Cap mode="light" kicker="Songs" text="Every song, its stage, key and tempo" />
  </In>
);
const S1Sheet: React.FC = () => (
  <In><Bg mode="light" />
    {/* diagonal: from the header down-left to the reference render */}
    <Crop shot="song-light" mode="light" keys={[{ f: 0, x: 1010, y: 420, z: 1.55 }, { f: b(2), x: 1010, y: 420, z: 1.55 }, { f: b(6) + FADE, x: 880, y: 600, z: 1.55 }]} />
    <Cap mode="light" kicker="A song" text="A sheet with its reference render" />
  </In>
);
const S1Versions: React.FC = () => (
  <In><Bg mode="light" />
    {/* pull back from the versions count to the whole projects card */}
    <Crop shot="songv-light" mode="light" keys={[{ f: 0, x: 860, y: 290, z: 2.6 }, { f: b(5), x: 835, y: 400, z: 1.55 }, { f: b(6) + FADE, x: 835, y: 405, z: 1.5 }]} />
    <Cap mode="light" kicker="Versions" text="Every version of an idea, together" />
  </In>
);
export const Short1: React.FC = () => (
  <Short music="music-short-1.wav" scenes={[[0, S1Files], [10, S1List], [16, S1Sheet], [22, S1Versions]]} />
);

// =====================================================================================
// Short 2: "Your files are never modified"
// =====================================================================================
const S2Hook: React.FC = () => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const c = T.light;
  const pop = spring({ frame: f - 2, fps, config: { damping: 12, stiffness: 120 } });
  return (
    <In first><Bg mode="light" />
      <FileCard name="Ivory Current.als" dir="Music/Projects/Ivory Current Project" c={c}
        style={{ left: 72, top: 820, transform: `translateY(${(1 - prog(f, 0, 14, easeOut)) * 300}px)`, opacity: prog(f, 0, 8) }}
        badge={<span style={{ marginLeft: 14, fontSize: 22, color: "#fff", background: c.ink, padding: "4px 14px", borderRadius: 30, transform: `scale(${pop})`, display: "inline-block" }}>untouched</span>} />
      <Cap mode="light" text="Spool never modifies your files." size={92} />
    </In>
  );
};
const S2Copy: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const appear = prog(f, b(3), b(4), easeOut);
  const rows: [string, "orig" | "copy" | "dir"][] = [["Ivory Current.als", "orig"], ["Ivory Current [R].als", "copy"], ["Samples", "dir"]];
  return (
    <In><Bg mode="light" />
      <div style={{ position: "absolute", left: 72, top: 640, width: 846, borderRadius: 24, overflow: "hidden", background: "#fff", fontFamily: '"Segoe UI", ' + SANS,
        boxShadow: "0 50px 100px -40px rgba(40,30,20,.45), 0 0 0 2px rgba(22,22,22,.10)" }}>
        <div style={{ height: 76, background: "#f3f3f3", display: "flex", alignItems: "center", padding: "0 28px", gap: 16, fontSize: 28, color: "#333", borderBottom: "1px solid #e5e5e5" }}>
          <svg width="34" height="28" viewBox="0 0 26 22"><path d="M1 3h9l2 3h13v15H1z" fill="#f2c55c" /></svg>Ivory Current Project
        </div>
        {rows.map(([name, kind]) => {
          const isCopy = kind === "copy";
          return (
            <div key={name} style={{ height: 120, overflow: "hidden", opacity: isCopy ? appear : 1,
              transform: `translate3d(0, ${kind === "dir" ? -(1 - appear) * 120 : 0}px, 0)`, clipPath: isCopy ? `inset(0 0 ${(1 - appear) * 100}% 0)` : undefined,
              background: isCopy ? `rgba(229,53,43,${0.12 * (1 - prog(f, b(5), b(6))) + 0.04})` : "transparent" }}>
              <div style={{ height: 120, display: "flex", alignItems: "center", gap: 22, padding: "0 32px", fontSize: 38, color: "#1a1a1a" }}>
                {kind === "dir"
                  ? <svg width="44" height="38" viewBox="0 0 26 22"><path d="M1 3h9l2 3h13v15H1z" fill="#f2c55c" /></svg>
                  : <svg width="44" height="44" viewBox="0 0 30 30"><rect x="3" y="2" width="24" height="26" rx="4" fill="#161616" /><text x="15" y="20" fontSize="10" fontFamily="Arial" fontWeight="700" textAnchor="middle" fill="#fff">ALS</text></svg>}
                <div>
                  <div>{name}</div>
                  {isCopy && <div style={{ fontFamily: MONO, fontSize: 24, color: "#e5352b", marginTop: 4 }}>repaired copy</div>}
                  {kind === "orig" && <div style={{ fontFamily: MONO, fontSize: 24, color: "#666", marginTop: 4 }}>the original, as it was</div>}
                </div>
              </div>
            </div>
          );
        })}
      </div>
      <Cap mode="light" kicker="Repair" text="Repairs go to a copy, next to the original" />
    </In>
  );
};
const S2Arrange: React.FC = () => (
  <In><Bg mode="light" />
    {/* pan right along the "Physical arranging" row to its switch, off */}
    <Crop shot="arranging-light" mode="light" keys={[{ f: 0, x: 668, y: 200, z: 2.0 }, { f: b(1), x: 668, y: 200, z: 2.0 }, { f: b(6), x: 1050, y: 190, z: 2.0 }, { f: b(8) + FADE, x: 1060, y: 190, z: 2.0 }]} />
    <Cap mode="light" kicker="Settings" text="Physical arranging is off: nothing moves on disk" />
  </In>
);
const S2Board: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const pop = (i: number): React.CSSProperties => { const a = b(0.5 + i); return { opacity: prog(f, a - 4, a + 8), transform: `translateY(${(1 - prog(f, a - 4, a + 14, easeOut)) * 30}px)` }; };
  const linked = (where: string) => (
    <div style={{ position: "absolute", left: 18, bottom: 16, fontFamily: MONO, fontSize: 22, color: "#fff", background: "rgba(0,0,0,.45)", padding: "6px 14px", borderRadius: 30 }}>linked · stays in {where}</div>
  );
  return (
    <In><Bg mode="light" />
      <div style={{ position: "absolute", left: 72, top: 600, width: 846, height: 900 }}>
        <div style={{ position: "absolute", left: 0, top: 0, width: 846, height: 380, borderRadius: 22, background: "linear-gradient(160deg, #f3c27a 0%, #e0795a 45%, #6b3b5e 100%)", ...pop(0) }}>{linked("Pictures/Refs")}</div>
        <div style={{ position: "absolute", left: 0, top: 410, width: 408, height: 300, borderRadius: 22, background: "linear-gradient(200deg, #9fc4cf 0%, #3f6f86 55%, #1d3445 100%)", ...pop(1) }}>{linked("Desktop")}</div>
        <div style={{ position: "absolute", left: 438, top: 410, width: 408, height: 300, borderRadius: 22, background: c.panel, border: `2px solid ${c.line}`, padding: 28, fontFamily: SANS, ...pop(2) }}>
          <div style={{ fontFamily: MONO, fontSize: 22, letterSpacing: ".12em", color: c.muted }}>IDEA</div>
          <div style={{ fontSize: 34, color: c.ink, marginTop: 12, lineHeight: 1.3 }}>Half-time drop after the second chorus</div>
        </div>
      </div>
      <Cap mode="light" kicker="Mood boards" text="Link files from anywhere. They stay put." />
    </In>
  );
};
export const Short2: React.FC = () => (
  <Short music="music-short-2.wav" scenes={[[0, S2Hook], [4, S2Copy], [12, S2Arrange], [20, S2Board]]} />
);

// =====================================================================================
// Short 3: "Light or dark, your way"
// =====================================================================================
const S3Wipe: React.FC = () => {
  const f = useCurrentFrame();
  const w = prog(f, 4, b(3), Easing.bezier(0.65, 0, 0.35, 1));
  const layer = (mode: Mode) => (
    <AbsoluteFill><Bg mode={mode} />
      <Crop shot={`stats-${mode}`} mode={mode} keys={[{ f: 0, x: 760, y: 470, z: 1.05 }, { f: b(4) + FADE, x: 760, y: 470, z: 1.1 }]} />
      <Cap mode={mode} text="Light or dark? Your call." size={92} />
    </AbsoluteFill>
  );
  return (
    <In first>
      {layer("light")}
      <AbsoluteFill style={{ clipPath: `inset(0 ${1080 * (1 - w)}px 0 0)` }}>{layer("dark")}</AbsoluteFill>
      {w > 0 && w < 1 && <div style={{ position: "absolute", left: 0, top: 0, width: 6, height: 1920, transform: `translate3d(${1080 * w - 3}px, 0, 0)`, background: "#e5352b", boxShadow: "0 0 40px rgba(229,53,43,.6)" }} />}
    </In>
  );
};
const S3Stats: React.FC = () => (
  <In><Bg mode="dark" />
    {/* push in on the tempo chart */}
    <Crop shot="stats-dark" mode="dark" keys={[{ f: 0, x: 1220, y: 330, z: 1.4 }, { f: b(6) + FADE, x: 1222, y: 300, z: 2.0 }]} />
    <Cap mode="dark" kicker="Statistics" text="Your whole catalogue at a glance" />
  </In>
);
const S3Release: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const step = f < b(2) ? 0 : f < b(3) ? 1 : 2;
  const picker = (
    <div style={{ position: "absolute", left: 464, top: 88, display: "flex", gap: 6, padding: 5, borderRadius: 10, background: c.panel, border: `1px solid ${c.line}`,
      boxShadow: "0 10px 30px -12px rgba(40,30,20,.35)", fontFamily: SANS, fontSize: 15, opacity: prog(f, b(0.5), b(1)) }}>
      {["Single", "EP", "Album"].map((t, i) => (
        <div key={t} style={{ padding: "5px 14px", borderRadius: 7, background: i === step ? c.ink : "transparent", color: i === step ? c.bg : c.muted, fontWeight: i === step ? 600 : 400 }}>{t}</div>
      ))}
    </div>
  );
  return (
    <In><Bg mode="light" />
      {/* tilt down from the release header to its tracks */}
      <Crop shot="release-light" mode="light" overlay={picker} keys={[{ f: 0, x: 640, y: 230, z: 1.5 }, { f: b(3), x: 640, y: 230, z: 1.5 }, { f: b(6) + FADE, x: 640, y: 480, z: 1.5 }]} />
      <Cap mode="light" kicker="Releases" text="Singles, EPs, albums, track by track" />
    </In>
  );
};
const S3Lang: React.FC = () => {
  const f = useCurrentFrame();
  const fr = prog(f, b(3) - 6, b(3) + 4, Easing.inOut(Easing.quad));
  return (
    <In><Bg mode="dark" />
      {/* lateral drift across the home page; French on the beat */}
      <Crop shot="home-dark" mode="dark" layers={[{ shot: "frhome-dark", opacity: fr }]} keys={[{ f: 0, x: 500, y: 330, z: 1.6 }, { f: b(6) + FADE, x: 820, y: 330, z: 1.6 }]} />
      <Cap mode="dark" kicker="Languages" text={f < b(3) ? "In English…" : "…or in French"} at={f < b(3) ? 0 : b(3) - 4} />
    </In>
  );
};
const S3Setting: React.FC = () => {
  const f = useCurrentFrame();
  const d = prog(f, b(2) - 4, b(2) + 6, Easing.inOut(Easing.quad));
  const mode: Mode = d > 0.5 ? "dark" : "light";
  return (
    <In>
      <Bg mode="light" /><AbsoluteFill style={{ opacity: d }}><Bg mode="dark" /></AbsoluteFill>
      {/* pull back from the Light / Dark switch */}
      <Crop shot="settings-light" mode={mode} layers={[{ shot: "settings-dark", opacity: d }]} keys={[{ f: 0, x: 1180, y: 390, z: 2.3 }, { f: b(6) + FADE, x: 1000, y: 380, z: 1.45 }]} />
      <Cap mode={mode} kicker="Settings" text="Or let it follow Windows" />
    </In>
  );
};
export const Short3: React.FC = () => (
  <Short music="music-short-3.wav" scenes={[[0, S3Wipe], [4, S3Stats], [10, S3Release], [16, S3Lang], [22, S3Setting]]} />
);

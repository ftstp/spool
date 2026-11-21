// Shared pieces of the tour: theme tokens (Spool's app.css), the desktop window mockup,
// the camera, captions and the scene shell.
import React from "react";
import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame } from "remotion";

export const FPS = 30;
export const FADE = 10; // frames a scene keeps playing under the next one, which fades in from the beat

export type Mode = "light" | "dark";
export const T = {
  light: { bg: "#f4f1ec", panel: "#fbf9f5", panel2: "#ebe7df", ink: "#161616", muted: "#5f5b54", faint: "#9c968c", line: "#ddd7cc", rec: "#e5352b", fix: "#1c9a86", mark: "#2a8f86" },
  dark: { bg: "#141312", panel: "#1c1b19", panel2: "#171615", ink: "#f1ede6", muted: "#a39e95", faint: "#6e6a63", line: "#2c2a27", rec: "#ff3b2f", fix: "#4fc9b0", mark: "#5fd0c5" },
};
export const SANS = '"IBM Plex Sans", "Segoe UI", sans-serif';
export const MONO = '"IBM Plex Mono", ui-monospace, monospace';
export const ease = Easing.bezier(0.45, 0, 0.2, 1); // slow in, long soft landing
export const easeOut = Easing.bezier(0.16, 1, 0.3, 1);

/** 0..1 between frames a and b, eased. */
export const prog = (f: number, a: number, b: number, e = ease) =>
  interpolate(f, [a, b], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: e });

// ---- window geometry (stage = 1920x1080) -------------------------------------------------------
export const WIN = { x: 304, y: 30, w: 1312, h: 820, bar: 40 };
const CX = 960, CY = WIN.y + (WIN.h + WIN.bar) / 2;

/** Camera keyframe: at frame f, scale s, centred on (u, v) = a point of the window's content
 *  (0..1 of its width / visible height). */
export type Key = { f: number; s: number; u: number; v: number };
export const cameraStyle = (frame: number, keys: Key[]): React.CSSProperties => {
  let k0 = keys[0], k1 = keys[0];
  for (let i = 0; i < keys.length; i++) {
    if (keys[i].f <= frame) { k0 = keys[i]; k1 = keys[i + 1] ?? keys[i]; }
  }
  if (frame < keys[0].f) k1 = k0;
  const t = k1 === k0 ? 0 : prog(frame, k0.f, k1.f);
  const s = k0.s + (k1.s - k0.s) * t, u = k0.u + (k1.u - k0.u) * t, v = k0.v + (k1.v - k0.v) * t;
  const fx = WIN.x + u * WIN.w, fy = WIN.y + WIN.bar + v * WIN.h;
  // where the focus point lands on screen: where it is at s = 1 (the window does not move),
  // drifting to the middle of the window area as the zoom grows
  const w = Math.min(1, Math.max(0, (s - 1) / 0.35));
  const lx = fx + (CX - fx) * w, ly = fy + (CY - fy) * w;
  // floats, never rounded; one fixed origin; translate3d so the layer is composited and resampled
  // from the full-size screenshot every frame (no whole-pixel layout steps)
  return { transformOrigin: "0 0", transform: `translate3d(${lx - fx * s}px, ${ly - fy * s}px, 0) scale(${s})` };
};

// ---- the Spool mark ---------------------------------------------------------------------------
export const SpoolMark: React.FC<{ size: number; mode?: Mode; rec?: number }> = ({ size, mode = "dark", rec = 1 }) => {
  const box = mode === "dark" ? "#161616" : "#f4f1ec", fg = mode === "dark" ? "#f4f1ec" : "#161616";
  return (
    <svg viewBox="0 0 54 54" width={size} height={size} style={{ display: "block" }}>
      <rect width="54" height="54" rx="12" fill={box} />
      <circle cx="18" cy="27" r="8" fill="none" stroke={fg} strokeWidth="3" />
      <circle cx="36" cy="27" r="8" fill="none" stroke={fg} strokeWidth="3" />
      <circle cx="18" cy="27" r="2" fill={fg} />
      <circle cx="36" cy="27" r="2" fill={fg} />
      <path d="M18 35h18" stroke={fg} strokeWidth="3" />
      <circle cx="44" cy="11" r="4.5" fill="#ff3b2f" opacity={rec} />
    </svg>
  );
};

// ---- the desktop window -----------------------------------------------------------------------
export const WindowFrame: React.FC<{ mode: Mode; title?: string; children: React.ReactNode; style?: React.CSSProperties; w?: number; h?: number }> = ({ mode, title = "Spool", children, style, w = WIN.w, h = WIN.h }) => {
  const c = T[mode];
  return (
    <div style={{ position: "absolute", left: WIN.x + (WIN.w - w) / 2, top: WIN.y, width: w, height: h + WIN.bar, borderRadius: 12, overflow: "hidden",
      background: c.bg, boxShadow: mode === "light" ? "0 40px 90px -30px rgba(40,30,20,.35), 0 0 0 1px rgba(22,22,22,.10)" : "0 40px 100px -30px rgba(0,0,0,.8), 0 0 0 1px rgba(255,255,255,.08)", ...style }}>
      <div style={{ height: WIN.bar, display: "flex", alignItems: "center", padding: "0 0 0 14px", gap: 10, background: c.panel2, borderBottom: `1px solid ${c.line}`, fontFamily: SANS, fontSize: 14, color: c.muted }}>
        <SpoolMark size={18} mode={mode === "light" ? "dark" : "light"} />
        <span>{title}</span>
        <div style={{ flex: 1 }} />
        {["min", "max", "close"].map((k) => (
          <div key={k} style={{ width: 52, height: WIN.bar, display: "grid", placeItems: "center" }}>
            <svg width="11" height="11" viewBox="0 0 11 11" stroke={c.muted} strokeWidth="1" fill="none">
              {k === "min" && <path d="M0.5 5.5h10" />}
              {k === "max" && <rect x="0.5" y="0.5" width="10" height="10" rx="1.5" />}
              {k === "close" && <path d="M0.5 0.5l10 10M10.5 0.5l-10 10" />}
            </svg>
          </div>
        ))}
      </div>
      <div style={{ position: "relative", width: w, height: h, overflow: "hidden" }}>{children}</div>
    </div>
  );
};

/** A screenshot filling the window's width. The files are 5760 px wide (captured at 4x), so even
 *  at the closest zoom the image is drawn from at least ~1.6 source pixels per screen pixel. */
export const Shot: React.FC<{ name: string; style?: React.CSSProperties }> = ({ name, style }) => (
  <Img src={staticFile(`shots/${name}.jpg`)} style={{ position: "absolute", left: 0, top: 0, width: "100%", display: "block", ...style }} />
);

// ---- backdrop + caption -------------------------------------------------------------------------
export const Backdrop: React.FC<{ mode: Mode }> = ({ mode }) => {
  const c = T[mode];
  const glow = mode === "light" ? "rgba(255,255,255,.75)" : "rgba(255,255,255,.045)";
  return <AbsoluteFill style={{ background: `radial-gradient(1200px 700px at 50% 38%, ${glow}, transparent 70%), ${c.bg}` }} />;
};

export type Cap = { from: number; to: number; text: string };
/** One line at a time under the window, a mono kicker above it. */
export const Captions: React.FC<{ mode: Mode; kicker: string; caps: Cap[] }> = ({ mode, kicker, caps }) => {
  const f = useCurrentFrame();
  const c = T[mode];
  return (
    <>
    {/* a band of backdrop colour, so a zoomed window fades out under the caption */}
    <div style={{ position: "absolute", left: 0, right: 0, top: 800, bottom: 0, background: `linear-gradient(to bottom, ${c.bg}00 0px, ${c.bg}eb 80px, ${c.bg} 120px)` }} />
    <div style={{ position: "absolute", left: 0, right: 0, top: 900, height: 170, display: "flex", flexDirection: "column", alignItems: "center" }}>
      <div style={{ fontFamily: MONO, fontSize: 20, letterSpacing: "0.14em", textTransform: "uppercase", color: c.muted, display: "flex", alignItems: "center", gap: 10,
        opacity: prog(f, 4, 24) }}>
        <span style={{ width: 9, height: 9, borderRadius: 9, background: c.rec, display: "inline-block" }} />{kicker}
      </div>
      <div style={{ position: "relative", width: "100%", height: 90, marginTop: 12 }}>
        {caps.map((cap, i) => {
          const a = prog(f, cap.from, cap.from + 18, easeOut) * (1 - prog(f, cap.to - 14, cap.to));
          const y = (1 - prog(f, cap.from, cap.from + 22, easeOut)) * 14 - prog(f, cap.to - 14, cap.to) * 8;
          return (
            <div key={i} style={{ position: "absolute", inset: 0, textAlign: "center", fontFamily: SANS, fontWeight: 600, fontSize: 54, letterSpacing: "-0.02em",
              color: c.ink, opacity: a, transform: `translateY(${y}px)`, whiteSpace: "nowrap" }}>{cap.text}</div>
          );
        })}
      </div>
    </div>
    </>
  );
};

/** Points one thing out in a screenshot: the rest dims, a red outline around it. Each spot is a
 *  rectangle of the window's content (0..1), shown from `from` to `to`; from one spot to the
 *  next it glides, so two things side by side are shown one after the other in the same shot. */
export type Spot = { from: number; to: number; x: number; y: number; w: number; h: number };
export const Spotlight: React.FC<{ spots: Spot[]; mode?: Mode }> = ({ spots, mode = "light" }) => {
  const f = useCurrentFrame();
  const on = (s: Spot) => prog(f, s.from, s.from + 10) * (1 - prog(f, s.to - 10, s.to));
  const alpha = Math.max(0, ...spots.map(on));
  if (alpha <= 0.001) return null;
  let i = 0;
  for (let k = 0; k < spots.length; k++) if (spots[k].from <= f) i = k;
  const cur = spots[i], prev = spots[i - 1];
  const t = prev && cur.from - prev.to < 6 ? prog(f, cur.from - 4, cur.from + 12) : 1;
  const r = (k: "x" | "y" | "w" | "h") => (prev ? prev[k] + (cur[k] - prev[k]) * t : cur[k]);
  const dim = mode === "light" ? `rgba(30,24,18,${0.5 * alpha})` : `rgba(0,0,0,${0.62 * alpha})`;
  return (
    <div style={{ position: "absolute", left: `${r("x") * 100}%`, top: `${r("y") * 100}%`, width: `${r("w") * 100}%`, height: `${r("h") * 100}%`, borderRadius: 14,
      boxShadow: `0 0 0 3px rgba(229,53,43,${alpha}), 0 0 0 4000px ${dim}`, pointerEvents: "none" }} />
  );
};

/** A scene fades (and settles) in over FADE frames, on top of the one before it. */
export const SceneIn: React.FC<{ children: React.ReactNode; first?: boolean }> = ({ children, first }) => {
  const f = useCurrentFrame();
  const o = first ? 1 : prog(f, 0, FADE, Easing.inOut(Easing.quad));
  return <AbsoluteFill style={{ opacity: o }}>{children}</AbsoluteFill>;
};

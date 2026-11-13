// The tour's scenes. Every name shown is fictional (the screenshots come from the browser
// preview fed with invented data); keep it that way.
import React from "react";
import { AbsoluteFill, Easing, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { b, OUTRO_LEN } from "./beat";
import { FatstepLogo } from "./FatstepLogo";
import {
  Backdrop, Captions, Key, MONO, Mode, SANS, SceneIn, Shot, SpoolMark, T, WIN, WindowFrame, cameraStyle, ease, easeOut, prog,
} from "./ui";

/** A screenshot in the window, with a camera and captions. */
const ShotScene: React.FC<{ mode: Mode; shot: string; kicker: string; caps: { from: number; to: number; text: string }[]; keys: Key[]; scroll?: (f: number) => number; overlay?: React.ReactNode; stage?: React.ReactNode }> = ({ mode, shot, kicker, caps, keys, scroll, overlay, stage }) => {
  const f = useCurrentFrame();
  return (
    <SceneIn>
      <Backdrop mode={mode} />
      <AbsoluteFill style={cameraStyle(f, keys)}>
        <WindowFrame mode={mode}>
          <Shot name={shot} scroll={scroll ? scroll(f) : 0} />
          {overlay}
        </WindowFrame>
      </AbsoluteFill>
      {stage}
      <Captions mode={mode} kicker={kicker} caps={caps} />
    </SceneIn>
  );
};

// ---- 1. intro ---------------------------------------------------------------------------------
export const Intro: React.FC = () => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const c = T.light;
  const pop = spring({ frame: f - 2, fps, config: { damping: 18, mass: 0.9, stiffness: 80 } });
  const rec = 0.35 + 0.65 * (0.5 + 0.5 * Math.cos((f / fps) * Math.PI * 1.2));
  return (
    <AbsoluteFill>
      <Backdrop mode="light" />
      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", flexDirection: "column" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 30, transform: `scale(${0.85 + 0.15 * pop})`, opacity: pop }}>
          <SpoolMark size={132} rec={rec} />
          <div style={{ fontFamily: MONO, fontWeight: 600, fontSize: 132, letterSpacing: "-0.04em", color: c.ink, lineHeight: 1 }}>spool</div>
        </div>
        <div style={{ marginTop: 44, fontFamily: SANS, fontSize: 46, color: c.ink, letterSpacing: "-0.01em",
          opacity: prog(f, b(4) - 10, b(4) + 6), transform: `translateY(${(1 - prog(f, b(4) - 10, b(4) + 12, easeOut)) * 16}px)` }}>
          Your Ableton Live catalogue, sorted and repaired.
        </div>
        <div style={{ marginTop: 22, fontFamily: MONO, fontSize: 20, letterSpacing: "0.14em", textTransform: "uppercase", color: c.muted, opacity: prog(f, b(5) - 6, b(5) + 10) }}>
          Free · Open source · Windows
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

// ---- 2. scattered .als files become songs -----------------------------------------------------
const FILES: { name: string; dir: string; song: number }[] = [
  { name: "Ivory Current v2.als", dir: "Sketches/2019", song: 0 },
  { name: "Gentle Cascade.als", dir: "Projects/Violet Window", song: 1 },
  { name: "Ivory Current final.als", dir: "Desktop/old", song: 0 },
  { name: "Hazy Carousel 2018.als", dir: "Backup/Drafts", song: 2 },
  { name: "Gentle Cascade v2.1.als", dir: "Projects/Violet Window", song: 1 },
  { name: "Copper Quarry.als", dir: "Collabs/Copper Quarry Project", song: 3 },
  { name: "Ivory Current [old].als", dir: "Backup/Sketches", song: 0 },
  { name: "Hazy Carousel 2.als", dir: "Downloads", song: 2 },
  { name: "Gentle Cascade v2 [P].als", dir: "Projects/Violet Window", song: 1 },
  { name: "Low Mosaic mix.als", dir: "Studio/Low Mosaic", song: 4 },
  { name: "Copper Quarry 07-01.als", dir: "Collabs/Copper Quarry Project", song: 3 },
  { name: "Low Mosaic.als", dir: "Studio/Low Mosaic", song: 4 },
];
const SONGS = [
  { title: "Ivory Current", meta: "Kaito Brenner · 85 bpm · Cm", stage: "Draft", col: "#eda100", art: "#9b5a4f" },
  { title: "Gentle Cascade", meta: "Nova Lindqvist · 80 bpm · Fm", stage: "Mix", col: "#2a78d6", art: "#7a4f8f" },
  { title: "Hazy Carousel", meta: "Nova Lindqvist · 84 bpm · Am", stage: "Draft", col: "#eda100", art: "#5f8a45" },
  { title: "Copper Quarry", meta: "Nova Lindqvist · 77 bpm · C", stage: "Arrangement", col: "#e87ba4", art: "#4f5f94" },
  { title: "Low Mosaic", meta: "Nova Lindqvist · 77 bpm · Am", stage: "Mix", col: "#2a78d6", art: "#4f5590" },
];
export const Catalogue: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const gather = (i: number) => prog(f, b(2) + i * 1.5, b(4), ease);
  const count = SONGS.map((_, s) => FILES.filter((x) => x.song === s).length);
  return (
    <SceneIn>
      <Backdrop mode="light" />
      {/* the files, scattered at first, then each flies into its song's row */}
      {FILES.map((file, i) => {
        const col = i % 3, row = Math.floor(i / 3);
        const sx = 170 + col * 560 + ((i * 97) % 90) - 45, sy = 120 + row * 175 + ((i * 53) % 60) - 30;
        const rot = (((i * 37) % 9) - 4) * 1.2;
        const tx = 1060, ty = 150 + file.song * 142 + 40;
        const g = gather(i);
        const appear = prog(f, i * 2, i * 2 + 14, easeOut);
        const drift = Math.sin((f + i * 20) / 45) * 6 * (1 - g);
        return (
          <div key={i} style={{ position: "absolute", left: sx + (tx - sx) * g, top: sy + (ty - sy) * g + drift, transform: `rotate(${rot * (1 - g)}deg) scale(${1 - 0.55 * g})`,
            transformOrigin: "0 0", opacity: appear * (1 - prog(f, b(4) - 6, b(4) + 4)), padding: "16px 22px", borderRadius: 12, background: c.panel,
            boxShadow: "0 12px 30px -16px rgba(40,30,20,.35), 0 0 0 1px rgba(22,22,22,.08)", fontFamily: MONO, whiteSpace: "nowrap" }}>
            <div style={{ fontSize: 26, color: c.ink, display: "flex", alignItems: "center", gap: 12 }}>
              <svg width="22" height="26" viewBox="0 0 22 26"><path d="M2 1h12l6 6v18H2z" fill="none" stroke={c.muted} strokeWidth="1.6" /><path d="M14 1v6h6" fill="none" stroke={c.muted} strokeWidth="1.6" /></svg>
              {file.name}
            </div>
            <div style={{ fontSize: 17, color: c.faint, marginTop: 4, paddingLeft: 34 }}>{file.dir}</div>
          </div>
        );
      })}
      {/* the songs */}
      {SONGS.map((s, i) => {
        const a = prog(f, b(3) + i * 3, b(4) + i * 3, easeOut);
        return (
          <div key={s.title} style={{ position: "absolute", left: 1060 - (1 - a) * 30, top: 150 + i * 142, width: 690, height: 118, opacity: a, borderRadius: 16, background: c.panel,
            boxShadow: "0 18px 40px -22px rgba(40,30,20,.4), 0 0 0 1px rgba(22,22,22,.08)", display: "flex", alignItems: "center", gap: 22, padding: "0 26px", fontFamily: SANS }}>
            <div style={{ width: 74, height: 74, borderRadius: 12, background: `linear-gradient(135deg, ${s.art}, ${s.art}cc)`, color: "#fff", display: "grid", placeItems: "center", fontSize: 32, fontWeight: 600 }}>{s.title[0]}</div>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 30, fontWeight: 600, color: c.ink }}>{s.title}</div>
              <div style={{ fontSize: 20, color: c.muted, marginTop: 2 }}>{s.meta}</div>
            </div>
            <div style={{ textAlign: "right" }}>
              <div style={{ fontSize: 20, color: c.ink, display: "flex", alignItems: "center", gap: 8, justifyContent: "flex-end" }}><i style={{ width: 10, height: 10, borderRadius: 10, background: s.col, display: "inline-block" }} />{s.stage}</div>
              <div style={{ fontFamily: MONO, fontSize: 17, color: c.faint, marginTop: 4 }}>{count[i]} versions</div>
            </div>
          </div>
        );
      })}
      <div style={{ position: "absolute", left: 160, top: 450, width: 820, opacity: prog(f, b(4.5), b(5.5)), transform: `translateY(${(1 - prog(f, b(4.5), b(5.6), easeOut)) * 14}px)` }}>
        <div style={{ fontFamily: MONO, fontSize: 76, fontWeight: 600, color: c.ink, letterSpacing: "-0.03em", lineHeight: 1 }}>12 files</div>
        <div style={{ fontFamily: SANS, fontSize: 34, color: c.muted, marginTop: 18 }}>versions, backups and copies, read where they are</div>
        <div style={{ fontFamily: MONO, fontSize: 76, fontWeight: 600, color: c.ink, letterSpacing: "-0.03em", lineHeight: 1, marginTop: 44 }}>5 songs</div>
      </div>
      <Captions mode="light" kicker="01 · Catalogue" caps={[{ from: 6, to: 9999, text: "Years of .als files become one catalogue of songs" }]} />
    </SceneIn>
  );
};

// ---- 3. songs list + overview -----------------------------------------------------------------
export const Songs: React.FC = () => (
  <ShotScene mode="light" shot="songs-light" kicker="02 · Songs"
    // push in, from the whole window to the overview
    keys={[{ f: 0, s: 1, u: 0.5, v: 0.5 }, { f: b(3), s: 1.42, u: 0.71, v: 0.56 }, { f: b(6) + 10, s: 1.46, u: 0.72, v: 0.57 }]}
    caps={[{ from: 6, to: 9999, text: "Every song in one list, filtered your way" }]} />
);

// ---- 4. a song sheet --------------------------------------------------------------------------
export const Song: React.FC = () => (
  <ShotScene mode="light" shot="song-light" kicker="03 · A song"
    // starts close on the header, tilts down to the render and the checklist on the bar
    keys={[{ f: 0, s: 1.5, u: 0.62, v: 0.3 }, { f: b(1), s: 1.5, u: 0.62, v: 0.3 }, { f: b(4), s: 1.55, u: 0.7, v: 0.62 }, { f: b(8) + 10, s: 1.57, u: 0.71, v: 0.64 }]}
    caps={[{ from: 6, to: b(4), text: "A sheet per song: stage, key, tempo" }, { from: b(4), to: 9999, text: "Its reference render, and what is left to finish" }]} />
);

// ---- 5. every version of an idea, in one song ----------------------------------------------------
export const Versions: React.FC = () => (
  <ShotScene mode="light" shot="songv-light" kicker="04 · Versions"
    // pull back: from the "3 old versions · 25 backups" line to the whole projects card
    keys={[{ f: 0, s: 2.6, u: 0.57, v: 0.31 }, { f: b(4), s: 1.35, u: 0.55, v: 0.45 }, { f: b(6) + 10, s: 1.33, u: 0.55, v: 0.45 }]}
    caps={[{ from: 6, to: 9999, text: "Every version of an idea, gathered in one song" }]} />
);

// ---- 6. mood board (mockup: the preview has no board files) ----------------------------------
const tile = (bg: string): React.CSSProperties => ({ borderRadius: 12, background: bg, boxShadow: "0 14px 30px -18px rgba(40,30,20,.45)" });
export const Mood: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const pop = (i: number): React.CSSProperties => { const a = b(0.5 + i * 0.5); return { opacity: prog(f, a - 4, a + 8), transform: `translateY(${(1 - prog(f, a - 4, a + 14, easeOut)) * 18}px) rotate(${[-1.5, 1, -0.5, 1.5, -1, 0.5][i % 6] * prog(f, a - 4, a + 14)}deg)` }; };
  const card: React.CSSProperties = { background: c.panel, border: `1px solid ${c.line}`, borderRadius: 12, padding: "20px 24px", fontFamily: SANS, color: c.ink, boxShadow: "0 14px 30px -20px rgba(40,30,20,.4)" };
  const label: React.CSSProperties = { fontFamily: MONO, fontSize: 15, letterSpacing: ".12em", textTransform: "uppercase", color: c.muted };
  const sw = ["#d9a05b", "#7a4f8f", "#2e5c6e", "#efe3cf"];
  const played = prog(f, b(4), b(8), (t) => t);
  return (
    <SceneIn>
      <Backdrop mode="light" />
      <div style={{ position: "absolute", left: 250, top: 60, width: 1420, height: 820, borderRadius: 16, background: c.panel2, border: `1px solid ${c.line}`, padding: "30px 36px",
        boxShadow: "0 40px 90px -30px rgba(40,30,20,.3)", opacity: prog(f, 0, 20) }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 18 }}>
          <div style={{ fontFamily: MONO, fontWeight: 600, fontSize: 30, color: c.ink }}>Mood board</div>
          <div style={{ fontFamily: SANS, fontSize: 22, color: c.muted }}>Gentle Cascade</div>
          <div style={{ flex: 1 }} />
          {["+ Image", "+ Idea", "+ Lyrics"].map((t) => <div key={t} style={{ fontFamily: SANS, fontSize: 20, color: c.ink, marginLeft: 18 }}>{t}</div>)}
        </div>
        <div style={{ position: "relative", marginTop: 28, height: 690 }}>
          <div style={{ position: "absolute", left: 0, top: 0, width: 430, height: 300, ...tile("linear-gradient(160deg, #f3c27a 0%, #e0795a 45%, #6b3b5e 100%)"), ...pop(0) }}>
            <div style={{ position: "absolute", left: 0, right: 0, bottom: 0, height: 110, borderRadius: "0 0 12px 12px", background: "linear-gradient(180deg, transparent, rgba(40,20,40,.55))" }} />
            <div style={{ position: "absolute", left: 18, bottom: 14, fontFamily: MONO, fontSize: 16, color: "#fff" }}>dusk-harbor.jpg</div>
          </div>
          <div style={{ position: "absolute", left: 455, top: 0, width: 300, height: 380, ...tile("linear-gradient(200deg, #9fc4cf 0%, #3f6f86 55%, #1d3445 100%)"), ...pop(1) }}>
            <div style={{ position: "absolute", left: 18, bottom: 14, fontFamily: MONO, fontSize: 16, color: "#fff" }}>cover-idea.png</div>
          </div>
          <div style={{ position: "absolute", left: 780, top: 0, width: 560, ...card, ...pop(2) }}>
            <div style={label}>Idea</div>
            <div style={{ fontSize: 30, marginTop: 10, lineHeight: 1.35 }}>Half-time drop after the second chorus, keep the tape hiss in the intro.</div>
          </div>
          <div style={{ position: "absolute", left: 780, top: 220, width: 560, ...card, ...pop(3) }}>
            <div style={label}>Lyrics · verse 1</div>
            <div style={{ fontSize: 28, marginTop: 10, lineHeight: 1.45, fontStyle: "italic" }}>Slow tide, paper boats in a row<br />we leave the harbor, let it go</div>
          </div>
          <div style={{ position: "absolute", left: 0, top: 330, width: 430, ...card, ...pop(4) }}>
            <div style={label}>Colours</div>
            <div style={{ display: "flex", gap: 16, marginTop: 16 }}>
              {sw.map((x) => (
                <div key={x} style={{ textAlign: "center" }}>
                  <div style={{ width: 70, height: 70, borderRadius: 70, background: x, border: `1px solid ${c.line}` }} />
                  <div style={{ fontFamily: MONO, fontSize: 14, color: c.muted, marginTop: 8 }}>{x}</div>
                </div>
              ))}
            </div>
          </div>
          <div style={{ position: "absolute", left: 455, top: 420, width: 885, height: 230, ...tile("#2b2a28"), ...pop(5) }}>
            <svg width="885" height="230" style={{ position: "absolute", inset: 0 }}>
              {Array.from({ length: 108 }, (_, i) => {
                const hh = 12 + 62 * Math.abs(Math.sin(i * 0.37) * Math.cos(i * 0.11));
                return <rect key={i} x={36 + i * 7.6} y={128 - hh} width={4} height={hh * 2} rx={2} fill={i / 108 < played ? "#e5352b" : "#8c867c"} />;
              })}
            </svg>
            <div style={{ position: "absolute", left: 18, top: 12, fontFamily: MONO, fontSize: 16, color: "#cfc8bc" }}>reference render · plays in the app</div>
          </div>
        </div>
      </div>
      <Captions mode="light" kicker="05 · Mood board" caps={[{ from: 6, to: 9999, text: "A mood board per song: images, ideas, lyrics, colours" }]} />
    </SceneIn>
  );
};

// ---- 7. repair, on a copy (one short scene) --------------------------------------------------
export const Copy: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  const appear = prog(f, b(3), b(4), easeOut);
  const flash = prog(f, b(4), b(4) + 8) * (1 - prog(f, b(5), b(7)));
  const rows = [
    { name: "Ivory Current.als", date: "14/03/2021 22:41", type: "Ableton Live Set", size: "412 KB", kind: "orig" },
    { name: "Ivory Current [R].als", date: "Today 10:02", type: "Ableton Live Set", size: "415 KB", kind: "copy" },
    { name: "Samples", date: "Today 10:02", type: "File folder", size: "", kind: "dir" },
  ];
  const W = 1240, X = (1920 - W) / 2;
  return (
    <SceneIn>
      <Backdrop mode="light" />
      <div style={{ position: "absolute", left: X, top: 150, width: W, borderRadius: 12, overflow: "hidden", background: "#fff", fontFamily: '"Segoe UI", ' + SANS,
        boxShadow: "0 40px 90px -30px rgba(40,30,20,.35), 0 0 0 1px rgba(22,22,22,.10)", opacity: prog(f, 0, 20), transform: `scale(${0.97 + 0.03 * prog(f, 0, 30, easeOut)})` }}>
        <div style={{ height: 52, background: "#f3f3f3", display: "flex", alignItems: "center", padding: "0 20px", gap: 14, fontSize: 21, color: "#333", borderBottom: "1px solid #e5e5e5" }}>
          <svg width="26" height="22" viewBox="0 0 26 22"><path d="M1 3h9l2 3h13v15H1z" fill="#f2c55c" /></svg>
          <span>Music</span><span style={{ color: "#999" }}>›</span><span>Projects</span><span style={{ color: "#999" }}>›</span><span style={{ fontWeight: 600 }}>Ivory Current Project</span>
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 260px 230px 140px", padding: "12px 30px", fontSize: 18, color: "#777", borderBottom: "1px solid #eee" }}>
          <span>Name</span><span>Date modified</span><span>Type</span><span style={{ textAlign: "right" }}>Size</span>
        </div>
        {rows.map((r) => {
          const isCopy = r.kind === "copy";
          return (
            <div key={r.name} style={{ height: isCopy ? 74 * appear : 74, overflow: "hidden", opacity: isCopy ? appear : 1, background: isCopy ? `rgba(229,53,43,${0.1 * flash + 0.04})` : "transparent" }}>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 260px 230px 140px", alignItems: "center", height: 74, padding: "0 30px", fontSize: 24, color: "#1a1a1a",
                transform: isCopy ? `translateX(${(1 - appear) * -24}px)` : undefined }}>
                <span style={{ display: "flex", alignItems: "center", gap: 16 }}>
                  {r.kind === "dir"
                    ? <svg width="30" height="26" viewBox="0 0 26 22"><path d="M1 3h9l2 3h13v15H1z" fill="#f2c55c" /></svg>
                    : <svg width="30" height="30" viewBox="0 0 30 30"><rect x="3" y="2" width="24" height="26" rx="4" fill="#161616" /><text x="15" y="20" fontSize="10" fontFamily="Arial" fontWeight="700" textAnchor="middle" fill="#fff">ALS</text></svg>}
                  {r.name}
                  {isCopy && <span style={{ marginLeft: 8, fontFamily: MONO, fontSize: 16, color: "#fff", background: "#e5352b", padding: "3px 10px", borderRadius: 20 }}>repaired copy</span>}
                  {r.kind === "orig" && <span style={{ marginLeft: 8, fontFamily: MONO, fontSize: 16, color: "#555", border: "1px solid #ccc", padding: "2px 10px", borderRadius: 20, opacity: prog(f, b(5) - 4, b(5) + 8) }}>untouched</span>}
                </span>
                <span style={{ color: "#666", fontSize: 21 }}>{r.date}</span><span style={{ color: "#666", fontSize: 21 }}>{r.type}</span><span style={{ color: "#666", fontSize: 21, textAlign: "right" }}>{r.size}</span>
              </div>
            </div>
          );
        })}
        <div style={{ height: 24 }} />
      </div>
      <div style={{ position: "absolute", left: X, top: 560, width: W, display: "flex", gap: 18, justifyContent: "center" }}>
        {["Plugin renamed after a reinstall", "Moved to VST3", "Samples found in another folder"].map((t, i) => (
          <div key={t} style={{ fontFamily: SANS, fontSize: 26, color: c.ink, background: c.panel, border: `1px solid ${c.line}`, borderRadius: 40, padding: "12px 26px",
            opacity: prog(f, b(0.5 + i * 0.5) - 4, b(0.5 + i * 0.5) + 8), transform: `translateY(${(1 - prog(f, b(0.5 + i * 0.5) - 4, b(0.5 + i * 0.5) + 12, easeOut)) * 12}px)` }}>{t}</div>
        ))}
      </div>
      <Captions mode="light" kicker="06 · Repair" caps={[{ from: 6, to: 9999, text: "Won't open? Spool repairs a copy, next to the original" }]} />
    </SceneIn>
  );
};

// ---- 8. releases ------------------------------------------------------------------------------
export const Release: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.light;
  // a type picker over the release header: Single -> EP -> Album
  const step = f < b(2) ? 0 : f < b(3) ? 1 : 2;
  const picker = (
    <div style={{ position: "absolute", left: 0.322 * WIN.w, top: 0.098 * WIN.h, display: "flex", gap: 6, padding: 5, borderRadius: 10, background: c.panel, border: `1px solid ${c.line}`,
      boxShadow: "0 10px 30px -12px rgba(40,30,20,.35)", fontFamily: SANS, fontSize: 15, opacity: prog(f, b(0.5), b(1)) * (1 - prog(f, b(5), b(5.5))) }}>
      {["Single", "EP", "Album"].map((t, i) => (
        <div key={t} style={{ padding: "5px 14px", borderRadius: 7, background: i === step ? c.ink : "transparent", color: i === step ? c.bg : c.muted, fontWeight: i === step ? 600 : 400 }}>{t}</div>
      ))}
    </div>
  );
  return (
    <ShotScene mode="light" shot="release-light" kicker="07 · Releases" overlay={picker}
      // a lateral truck, left to right, at a fixed medium zoom
      keys={[{ f: 0, s: 1.3, u: 0.4, v: 0.33 }, { f: b(6) + 10, s: 1.3, u: 0.58, v: 0.36 }]}
      caps={[{ from: 6, to: 9999, text: "Plan a single, an EP or an album, track by track" }]} />
  );
};

// ---- 9. statistics, then light -> dark --------------------------------------------------------
export const Theme: React.FC = () => {
  const f = useCurrentFrame();
  const wipe = prog(f, b(4), b(6), Easing.bezier(0.65, 0, 0.35, 1)); // 0..1 across the screen, lands on the bar
  // zoom back out before the wipe, so the whole window changes theme in view
  const keys: Key[] = [{ f: 0, s: 1, u: 0.5, v: 0.5 }, { f: b(10) + 10, s: 1.07, u: 0.5, v: 0.42 }]; // one slow push, the whole window in view for the wipe
  const layer = (mode: Mode) => (
    <AbsoluteFill>
      <Backdrop mode={mode} />
      <AbsoluteFill style={cameraStyle(f, keys)}>
        <WindowFrame mode={mode}><Shot name={`stats-${mode}`} /></WindowFrame>
      </AbsoluteFill>
      <Captions mode={mode} kicker={f < b(5) ? "08 · Statistics" : "09 · Appearance"}
        caps={[{ from: 6, to: b(4), text: "Statistics on the whole catalogue" }, { from: b(5.5), to: 9999, text: "Light or dark, or as Windows is set" }]} />
    </AbsoluteFill>
  );
  const x = wipe * 1920;
  return (
    <SceneIn>
      {layer("light")}
      <AbsoluteFill style={{ clipPath: `inset(0 ${1920 - x}px 0 0)` }}>{layer("dark")}</AbsoluteFill>
      {wipe > 0 && wipe < 1 && <div style={{ position: "absolute", left: x - 2, top: 0, width: 4, height: 1080, background: "#e5352b", boxShadow: "0 0 30px rgba(229,53,43,.6)" }} />}
    </SceneIn>
  );
};

// ---- 10. English and French -------------------------------------------------------------------
export const Lang: React.FC = () => {
  const f = useCurrentFrame();
  const c = T.dark;
  const fr = prog(f, b(4) - 8, b(4) + 4, Easing.inOut(Easing.quad));
  const pill = (
    <div style={{ position: "absolute", right: 120, top: 60, display: "flex", gap: 6, padding: 6, borderRadius: 12, background: c.panel, border: `1px solid ${c.line}`,
      boxShadow: "0 16px 40px -16px rgba(0,0,0,.8)", fontFamily: SANS, fontSize: 19, opacity: prog(f, b(1), b(2)) }}>
      {["English", "Français"].map((t, i) => {
        const on = i === 0 ? 1 - fr : fr;
        return <div key={t} style={{ padding: "8px 18px", borderRadius: 8, background: `rgba(241,237,230,${on})`, color: on > 0.5 ? "#161616" : c.muted, fontWeight: 500 }}>{t}</div>;
      })}
    </div>
  );
  return (
    <ShotScene mode="dark" shot="home-dark" kicker="10 · Languages"
      overlay={<Shot name="frhome-dark" style={{ opacity: fr }} />} stage={pill}
      // close on the page title, panning right across the cards
      keys={[{ f: 0, s: 1.45, u: 0.27, v: 0.22 }, { f: b(8) + 10, s: 1.45, u: 0.6, v: 0.3 }]}
      caps={[{ from: 6, to: 9999, text: "In English and in French" }]} />
  );
};

// ---- 11. outro --------------------------------------------------------------------------------
export const Outro: React.FC = () => {
  const f = useCurrentFrame();
  const { fps } = useVideoConfig();
  const c = T.dark;
  const pop = spring({ frame: f - 2, fps, config: { damping: 20, stiffness: 70 } });
  const line = (a: number) => ({ opacity: prog(f, a - 8, a + 8), transform: `translateY(${(1 - prog(f, a - 8, a + 14, easeOut)) * 14}px)` });
  return (
    <SceneIn>
      <AbsoluteFill style={{ background: `radial-gradient(1100px 650px at 50% 40%, rgba(255,255,255,.05), transparent 70%), #161616` }} />
      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", flexDirection: "column" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 26, transform: `scale(${0.9 + 0.1 * pop})`, opacity: pop }}>
          <SpoolMark size={110} mode="light" />
          <div style={{ fontFamily: MONO, fontWeight: 600, fontSize: 110, letterSpacing: "-0.04em", color: c.ink, lineHeight: 1 }}>spool</div>
        </div>
        <div style={{ marginTop: 46, fontFamily: SANS, fontSize: 40, color: c.ink, ...line(b(1)) }}>Free and open source · Windows</div>
        <div style={{ marginTop: 20, fontFamily: MONO, fontSize: 36, color: "#ff3b2f", letterSpacing: "0.01em", ...line(b(2)) }}>artists.fatstep.com/spool</div>
        <div style={{ marginTop: 80, display: "flex", alignItems: "center", gap: 18, fontFamily: SANS, fontSize: 26, color: c.muted, ...line(b(4)) }}>
          by <FatstepLogo height={40} color={c.ink} markColor={c.mark} />
        </div>
      </AbsoluteFill>
      <AbsoluteFill style={{ background: "#0b0b0b", opacity: prog(f, OUTRO_LEN - 24, OUTRO_LEN) }} />
    </SceneIn>
  );
};

export { WIN };

// ---- poster: the same window, half light, half dark ---------------------------------------------
export const Poster: React.FC = () => {
  const layer = (mode: Mode) => {
    const c = T[mode];
    return (
      <AbsoluteFill>
        <Backdrop mode={mode} />
        <WindowFrame mode={mode}><Shot name={`songs-${mode}`} /></WindowFrame>
        <div style={{ position: "absolute", left: 0, right: 0, top: 918, display: "flex", alignItems: "center", justifyContent: "center", gap: 24 }}>
          <SpoolMark size={64} mode={mode === "light" ? "dark" : "light"} />
          <div style={{ fontFamily: MONO, fontWeight: 600, fontSize: 60, letterSpacing: "-0.04em", color: c.ink }}>spool</div>
          <div style={{ width: 2, height: 46, background: c.line, margin: "0 14px" }} />
          <div style={{ fontFamily: SANS, fontSize: 38, color: c.ink }}>Your Ableton Live catalogue, sorted and repaired.</div>
        </div>
      </AbsoluteFill>
    );
  };
  return (
    <AbsoluteFill>
      {layer("light")}
      <AbsoluteFill style={{ clipPath: "polygon(1060px 0, 1920px 0, 1920px 1080px, 860px 1080px)" }}>{layer("dark")}</AbsoluteFill>
      <svg width="1920" height="1080" style={{ position: "absolute", inset: 0 }}><line x1="1060" y1="0" x2="860" y2="1080" stroke="#e5352b" strokeWidth="4" /></svg>
    </AbsoluteFill>
  );
};

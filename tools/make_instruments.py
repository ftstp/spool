"""Builds Spool's recorded instruments (web/public/instruments, web/src/lib/instruments.json) from VSCO 2 CE WAVs (CC0): each file's real pitch
measured, trimmed, faded, normalised, mono, encoded Opus. Writes the .ogg files and a manifest.
    python -I make_instruments.py <wav dir> <out dir> <manifest.json>
"""
import sys, os, json, re, subprocess, math
import numpy as np
import soundfile as sf

src, out, man = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
FF = r"C:/Portable-libs/ffmpeg-essentials_build/bin/ffmpeg.exe"

# per instrument: which files, how long a note is kept, its release, its level
SETS = {
    "piano": (r"^Player_dyn2", 5.0, 0.25, 0.9),
    "strings": (r"^(VlnEns|susvib)", 3.5, 0.35, 1.3),
    "brass": (r"^(Sum_SHTrumpet|MOHorn)", 2.8, 0.2, 0.85),
}


def pitch(x, sr):
    """The fundamental, by autocorrelation over a steady part (after the attack)."""
    a = int(sr * 0.25)
    seg = x[a: a + int(sr * 0.5)]
    if len(seg) < 2048:
        seg = x[: int(sr * 0.5)]
    seg = seg - seg.mean()
    seg = seg * np.hanning(len(seg))
    n = len(seg)
    spec = np.fft.rfft(seg, 2 * n)
    ac = np.fft.irfft(spec * np.conj(spec))[:n]
    lo, hi = int(sr / 2200), int(sr / 25)
    ac[:lo] = 0
    # the first strong peak (not the octave below)
    k = lo + int(np.argmax(ac[lo:hi]))
    best = ac[k]
    for div in (2, 3):
        k2 = int(round(k / div))
        if k2 > lo:
            w = ac[max(lo, k2 - 3): k2 + 4]
            if w.max() > 0.85 * best:
                k = max(lo, k2 - 3) + int(np.argmax(w))
    f = sr / k
    return 69 + 12 * math.log2(f / 440)


NOTE = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}
PIANO = {int(l.split("=")[0]): int(l.split("=")[1]) for l in open(os.path.join(src, "MappingChart.txt"), encoding="utf-8") if "=" in l and l[0].isdigit()}


def named(f):
    m = re.search(r"_dyn\d_rr\d_(\d{3})", f)
    if m:
        return PIANO[int(m.group(1))]
    m = re.search(r"_([A-G]#?)(\d)_", f)
    return None if not m else (int(m.group(2)) + 1) * 12 + NOTE[m.group(1)]


manifest = {}
for inst, (pat, keep, rel, gain) in SETS.items():
    zones = []
    for f in sorted(os.listdir(src)):
        if not f.lower().endswith(".wav") or not re.search(pat, f):
            continue
        x, sr = sf.read(os.path.join(src, f), always_2d=True)
        x = x.mean(axis=1)
        # the note starts at the first sound
        thr = 0.02 * np.abs(x).max()
        start = int(np.argmax(np.abs(x) > thr))
        start = max(0, start - int(sr * 0.005))
        x = x[start: start + int(sr * keep)]
        fade = int(sr * min(0.4, keep / 4))
        x[-fade:] *= np.linspace(1, 0, fade) ** 2
        x = x / max(1e-6, np.abs(x).max()) * 0.89
        measured = pitch(x, sr)
        by_name = named(f)
        note = round(measured)
        # the name says the pitch class; the measure, the octave and the tuning
        if inst == "piano":
            # the sampler's own chart is exact (the measure took an octave below once)
            note = by_name
        elif by_name is not None:
            cand = [by_name + 12 * k for k in range(-3, 4)]
            note = min(cand, key=lambda c: abs(c - measured))
        cents = round((measured - note) * 100)
        base = f"{inst}-{note}"
        tmp = os.path.join(out, base + ".wav")
        sf.write(tmp, x.astype(np.float32), sr)
        dst = os.path.join(out, base + ".ogg")
        subprocess.run([FF, "-y", "-loglevel", "error", "-i", tmp, "-ac", "1", "-ar", "48000", "-c:a", "libopus", "-b:a", "64k", dst], check=True)
        os.remove(tmp)
        zones.append({"note": note, "file": base + ".ogg", "cents": cents})
        print(f"{inst:8} {f:40} name {by_name} measured {measured:6.2f} -> {note} ({cents:+d} c) {os.path.getsize(dst)//1024} KB")
    zones.sort(key=lambda z: z["note"])
    # one file a note: the first kept
    seen, uniq = set(), []
    for z in zones:
        if z["note"] not in seen:
            seen.add(z["note"])
            uniq.append(z)
    manifest[inst] = {"zones": [{"note": z["note"], "file": z["file"]} for z in uniq], "len": keep, "release": rel, "gain": gain}
json.dump(manifest, open(man, "w", encoding="utf-8"), indent=2)
print("total", sum(os.path.getsize(os.path.join(out, f)) for f in os.listdir(out)) // 1024, "KB")

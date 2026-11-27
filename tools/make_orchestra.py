"""Adds the orchestra to Spool's recorded instruments (web/public/instruments, the manifest
web/src/lib/instruments.json): strings by section, pizzicato, harp, woodwinds, brass apart,
timpani, glockenspiel, from VSCO 2 Community Edition WAVs (CC0).

    python -I make_orchestra.py <vsco dir> <out dir> <manifest.json>

<vsco dir> holds one folder per instrument below (the notes chosen from the library: one
dynamic, one round-robin a note). Each note: its real pitch measured (the name says the pitch
class, the measure the octave and the tuning), trimmed to its first sound, mono, normalised,
Opus at 64 kb/s. A held instrument gets a sustain loop: a stretch of its steady part whose end
is crossfaded into its start, written in the file, so a note held longer than the recording
goes on instead of fading (the player loops between `loop` seconds). Each instrument's level
is set from its notes' loudness, so they sit together. The manifest's other instruments (the
piano, the strings, the brass) are kept as they are.
"""
import sys, os, json, re, subprocess, math
import numpy as np
import soundfile as sf

src, out, man = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
FF = r"C:/Portable-libs/ffmpeg-essentials_build/bin/ffmpeg.exe"

# folder(s) -> (seconds kept, release, held (a sustain loop), the loudness it is set to)
SETS = {
    "viola": (["viola"], 3.2, 0.35, True, 1.0),
    "cello": (["cello"], 3.2, 0.35, True, 1.0),
    "contrabass": (["contrabass"], 3.2, 0.35, True, 1.0),
    "pizz": (["pizz", "cellopizz"], 1.6, 0.15, False, 1.0),
    "harp": (["harp"], 3.0, 0.4, False, 1.0),
    "flute": (["flute"], 3.0, 0.25, True, 0.9),
    "oboe": (["oboe"], 3.0, 0.2, True, 0.85),
    "clarinet": (["clarinet"], 3.0, 0.2, True, 0.9),
    "bassoon": (["bassoon"], 3.0, 0.2, True, 0.95),
    "horn": (["horn"], 3.0, 0.3, True, 0.95),
    "trumpet": (["trumpet"], 3.0, 0.2, True, 0.85),
    "trombone": (["trombone"], 3.0, 0.25, True, 0.95),
    "tuba": (["tuba"], 3.0, 0.25, True, 1.0),
    "timpani": (["timpani"], 3.5, 0.5, False, 1.0),
    "glock": (["glock"], 3.0, 0.6, False, 0.8),
}
NOTE = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}


def pitch(x, sr, top=4200):
    """The fundamental, by autocorrelation over a steady part (after the attack), at most
    `top` Hz."""
    a = int(sr * 0.25)
    seg = x[a: a + int(sr * 0.5)]
    if len(seg) < 2048:
        seg = x[: int(sr * 0.5)]
    seg = seg - seg.mean()
    seg = seg * np.hanning(len(seg))
    n = len(seg)
    spec = np.fft.rfft(seg, 2 * n)
    ac = np.fft.irfft(spec * np.conj(spec))[:n]
    lo, hi = int(sr / top), int(sr / 25)
    ac[:lo] = 0
    k = lo + int(np.argmax(ac[lo:hi]))
    best = ac[k]
    for div in (2, 3):
        k2 = int(round(k / div))
        if k2 > lo:
            w = ac[max(lo, k2 - 3): k2 + 4]
            if w.max() > 0.85 * best:
                k = max(lo, k2 - 3) + int(np.argmax(w))
    return 69 + 12 * math.log2((sr / k) / 440)


def named(f):
    m = re.search(r"_([A-G]#?)(-?\d)(?:_|\.|$)", f)
    return None if not m else (int(m.group(2)) + 1) * 12 + NOTE[m.group(1)]


def sustain_loop(x, sr, n_from, n_to, fade):
    """A loop [a, b) in x's steady part, its last `fade` samples crossfaded (equal power) with
    those before a: played from b back to a, nothing jumps. Returns x (changed) and a, b."""
    a, b = n_from, n_to
    # b on a rising zero crossing, near n_to, so the seam is where the wave crosses anyway
    zc = np.where((x[b - 2000: b] < 0) & (x[b - 1999: b + 1] >= 0))[0]
    if len(zc):
        b = b - 2000 + int(zc[-1])
    t = np.linspace(0, math.pi / 2, fade)
    x = x.copy()
    x[b - fade: b] = x[b - fade: b] * np.cos(t) + x[a - fade: a] * np.sin(t)
    return x, a, b


def decoded(path):
    """The file as a player hears it: Opus decoded (its delay taken off), 48 kHz mono."""
    import io
    raw = subprocess.run([FF, "-loglevel", "error", "-i", path, "-f", "wav", "-"], capture_output=True, check=True).stdout
    y, ysr = sf.read(io.BytesIO(raw))
    return (y if y.ndim == 1 else y.mean(axis=1)), ysr


def refit_loop(path, loop):
    """The loop's end moved (a few hundred samples at most) to where the decoded file joins
    its start best: Opus keeps the sound, not each sample, so the seam made in the WAV may
    land a little off once encoded. Compared over 64 samples before each point."""
    y, ysr = decoded(path)
    a, b = int(round(loop[0] * ysr)), min(len(y), int(round(loop[1] * ysr)))
    k = 64
    ref = y[a - k: a + 1]
    best, best_e = b, None
    for c in range(max(a + ysr // 2, b - 600), b + 1):
        e = float(np.sum((y[c - k - 1: c] - ref) ** 2))
        if best_e is None or e < best_e:
            best, best_e = c, e
    return [round(a / ysr, 5), round(best / ysr, 5)]


manifest = json.load(open(man, encoding="utf-8")) if os.path.isfile(man) else {}
for inst, (dirs, keep, rel, held, level) in SETS.items():
    zones = []
    rms = []
    for d in dirs:
        folder = os.path.join(src, d)
        # how this folder counts octaves: most of VSCO names a note an octave below MIDI's
        # count ("C3" sounds as C4, 60), its harp does not. The median of what its notes
        # measure against their names says it, whatever a few wrong measures say.
        offs = []
        for f in sorted(os.listdir(folder)):
            nm = named(f)
            if nm is None or not f.lower().endswith(".wav"):
                continue
            y, ysr = sf.read(os.path.join(folder, f), always_2d=True)
            offs.append(pitch(y.mean(axis=1)[: int(ysr * 1.5)], ysr) - nm)
        shift = 12 * round(float(np.median(offs)) / 12) if offs else 12
        print(f"{inst:10} {d}: octave shift {shift:+d}")
        for f in sorted(os.listdir(folder)):
            if not f.lower().endswith(".wav"):
                continue
            x, sr = sf.read(os.path.join(folder, f), always_2d=True)
            x = x.mean(axis=1)
            thr = 0.02 * np.abs(x).max()
            start = max(0, int(np.argmax(np.abs(x) > thr)) - int(sr * 0.005))
            x = x[start: start + int(sr * keep)]
            if len(x) < sr * 0.3:
                continue
            # a timpani's note is low: measured below 700 Hz (its overtones are not harmonic)
            measured = pitch(x, sr, 700 if inst == "timpani" else 4200)
            by_name = named(f)
            # no name (the timpani): the measure, counted as the library's others are
            note = round(measured) + (12 if inst == "timpani" else 0)
            if by_name is not None:
                # VSCO names a note an octave below MIDI's count ("C3" sounds as C4, 60): the
                # name, an octave up; the measure only checks it (a glockenspiel's or a deep
                # tuba's overtones fool it)
                note = by_name + shift
                if abs(measured - note) > 0.6:
                    print(f"   check {f}: named {note}, measured {measured:.2f}")
            loop = None
            if held and len(x) >= sr * (keep - 0.05):
                # the steady part: from 0.9 s (the attack and its bloom over) to 0.15 s before
                # the end; crossfade 0.25 s
                x, a, b = sustain_loop(x, sr, int(sr * 0.9), len(x) - int(sr * 0.15), int(sr * 0.25))
                x = x[:b]
                loop = [round(a / sr, 4), round(b / sr, 4)]
            else:
                fade = int(sr * min(0.4, keep / 4))
                x[-fade:] *= np.linspace(1, 0, fade) ** 2
            x = x / max(1e-6, np.abs(x).max()) * 0.89
            rms.append(float(np.sqrt(np.mean(x[: int(sr * 1.5)] ** 2))))
            base = f"{inst}-{note}"
            tmp = os.path.join(out, base + ".wav")
            sf.write(tmp, x.astype(np.float32), sr)
            dst = os.path.join(out, base + ".ogg")
            subprocess.run([FF, "-y", "-loglevel", "error", "-i", tmp, "-ac", "1", "-ar", "48000", "-c:a", "libopus", "-b:a", "64k", dst], check=True)
            os.remove(tmp)
            z = {"note": note, "file": base + ".ogg"}
            if loop:
                z["loop"] = refit_loop(dst, loop)
            zones.append(z)
            print(f"{inst:10} {f:44} name {by_name} measured {measured:6.2f} -> {note} {'loop ' + str(loop) if loop else ''} {os.path.getsize(dst) // 1024} KB")
    zones.sort(key=lambda z: z["note"])
    seen, uniq = set(), []
    for z in zones:
        if z["note"] not in seen:
            seen.add(z["note"])
            uniq.append(z)
    # set together: a quiet recording (a flute) lifted, a loud one (a tuba) brought down
    med = float(np.median(rms)) if rms else 0.2
    gain = round(max(0.4, min(2.2, level * 0.18 / max(1e-3, med))), 2)
    manifest[inst] = {"zones": uniq, "len": keep, "release": rel, "gain": gain, **({"held": True} if held else {})}
json.dump(manifest, open(man, "w", encoding="utf-8", newline="\n"), indent=2)
print("total", sum(os.path.getsize(os.path.join(out, f)) for f in os.listdir(out)) // 1024, "KB")

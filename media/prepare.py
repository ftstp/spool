"""Fills media/public/ (not committed) with what the video reads.

  python prepare.py <shots_dir> <music_file>

<shots_dir> holds the 2880x1800 PNG captures of the browser preview, made with FICTIONAL data only
(<name>-light.png / <name>-dark.png, plus songv-*.png and frhome-*.png). They become public/shots/*.jpg.

The music is "Strung Out" by Lowpoly (SNG-911, 94 bpm). On its beat grid (see src/beat.ts:
first beat 0.2014 s, beat 0.63829 s, bar = 4 beats) the soundtrack is
  bars 29-47 of the track (74.245 s -> 122.756 s: the soft bar that opens the "solo" section,
  then its groove), spliced on the downbeat into bars 52-53, the track's last phrase and its
  final chord, which rings out (132.970 s -> the video's end).
Bars 48-51 are left out: bar 52 is the closest match to bar 48 in the whole section (chroma +
MFCC similarity), so the jump lands where the music would have gone. The splice is a 25 ms
equal-power crossfade centred on the downbeat; the end fades over the last 1.5 s (already
near silence). Normalised to -16 LUFS (two-pass loudnorm) into public/music.wav.
"""
import json, subprocess, sys
from pathlib import Path
import numpy as np
import soundfile as sf
from PIL import Image

HERE = Path(__file__).parent
SHOTS, MUSIC = Path(sys.argv[1]), sys.argv[2]
T0, BEAT, FPS = 0.2014, 0.6382895, 30
BAR = 4 * BEAT
bar = lambda n: T0 + n * BAR
TOTAL_S = round(84 * BEAT * FPS + 2 * FPS) / FPS  # = TOTAL in src/beat.ts

out = HERE / "public" / "shots"
out.mkdir(parents=True, exist_ok=True)
for f in sorted(SHOTS.glob("*.png")):
    Image.open(f).convert("RGB").save(out / (f.stem + ".jpg"), "JPEG", quality=92, subsampling=0)
print("shots", len(list(out.glob("*.jpg"))))

y, sr = sf.read(MUSIC, always_2d=True)
s = lambda t: int(round(t * sr))
x = int(0.0125 * sr)  # half the crossfade
a = y[s(bar(29)): s(bar(48)) + x]
rest = TOTAL_S - (bar(48) - bar(29))
c = y[s(bar(52)) - x: s(bar(52)) + s(rest)]
w = np.linspace(0, np.pi / 2, 2 * x)[:, None]
mid = a[-2 * x:] * np.cos(w) + c[:2 * x] * np.sin(w)
music = np.concatenate([a[:-2 * x], mid, c[2 * x:]])[: s(TOTAL_S)]
fi, fo = int(0.02 * sr), int(1.5 * sr)
music[:fi] *= np.linspace(0, 1, fi)[:, None]
music[-fo:] *= np.linspace(1, 0, fo)[:, None]
tmp = HERE / "public" / "music-raw.wav"
sf.write(tmp, music, sr, subtype="PCM_24")

run = lambda *a: subprocess.run(["ffmpeg", "-hide_banner", "-y", "-i", str(tmp), *a], capture_output=True, text=True)
probe = run("-af", "loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json", "-f", "null", "-").stderr
m = json.loads(probe[probe.rindex("{"):probe.rindex("}") + 1])
ln = (f"loudnorm=I=-16:TP=-1.5:LRA=11:measured_I={m['input_i']}:measured_TP={m['input_tp']}"
      f":measured_LRA={m['input_lra']}:measured_thresh={m['input_thresh']}:offset={m['target_offset']}:linear=true")
r = run("-af", ln, "-ar", "48000", "-ac", "2", str(HERE / "public" / "music.wav"))
assert r.returncode == 0, r.stderr[-500:]
tmp.unlink()
print("music", round(len(music) / sr, 3), "s,", m["input_i"], "LUFS -> -16")

# ---- the vertical shorts: 8 bars from three different parts of the track, each joined on the
# downbeat to bar 53 (the final chord, which rings out), louder for phones (-14 LUFS).
SHORT_S = (round(36 * BEAT * FPS) + 30) / FPS  # = SHORT_TOTAL in src/shorts.tsx
SHORTS = {1: (10, 18), 2: (21, 29), 3: (45, 53)}  # [first bar, bar after the last]: intro build, the "inst" section, the outro
for n, (b0, b1) in SHORTS.items():
    a = y[s(bar(b0)): s(bar(b1)) + x]
    c = y[s(bar(53)) - x: s(bar(53)) + s(SHORT_S - (bar(b1) - bar(b0)))]
    mid = a[-2 * x:] * np.cos(w) + c[:2 * x] * np.sin(w)
    clip = np.concatenate([a[:-2 * x], mid, c[2 * x:]])[: s(SHORT_S)]
    clip[:fi] *= np.linspace(0, 1, fi)[:, None]
    fo2 = int(0.6 * sr)
    clip[-fo2:] *= np.linspace(1, 0, fo2)[:, None]
    sf.write(tmp, clip, sr, subtype="PCM_24")
    probe = run("-af", "loudnorm=I=-14:TP=-1.0:LRA=11:print_format=json", "-f", "null", "-").stderr
    m = json.loads(probe[probe.rindex("{"):probe.rindex("}") + 1])
    ln = (f"loudnorm=I=-14:TP=-1.0:LRA=11:measured_I={m['input_i']}:measured_TP={m['input_tp']}"
          f":measured_LRA={m['input_lra']}:measured_thresh={m['input_thresh']}:offset={m['target_offset']}:linear=true")
    r = run("-af", ln, "-ar", "48000", "-ac", "2", str(HERE / "public" / f"music-short-{n}.wav"))
    assert r.returncode == 0, r.stderr[-500:]
    tmp.unlink()
    print(f"short {n}: track {bar(b0):.3f}-{bar(b1):.3f} s + {bar(53):.3f} s to the end, {len(clip) / sr:.3f} s")

"""From Remotion's masters in out/ to the files that ship:
  out/spool-tour.mp4         H.264 yuv420p, two-pass to stay under ~21 MB, AAC, faststart
  out/spool-tour-poster.webp 1600 px, from out/poster.png (npm run poster)
  out/spool-tour-loop.webp   silent animated WebP, 960 px, for the README (a repo video does not play there)
Run after `npm run render` and `npm run poster`."""
import subprocess
from pathlib import Path

OUT = Path(__file__).parent / "out"
RAW = OUT / "spool-tour-raw.mkv"
run = lambda *a: subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *a], check=True, cwd=OUT)

v = ["-vf", "scale=in_range=full:out_range=tv,format=yuv420p", "-c:v", "libx264", "-preset", "slower", "-b:v", "1900k",
     "-maxrate", "4000k", "-bufsize", "8000k", "-profile:v", "high", "-level", "4.1", "-color_range", "tv"]
run("-i", RAW.name, *v, "-pass", "1", "-passlogfile", "x264pass", "-an", "-f", "mp4", "NUL")
run("-i", RAW.name, *v, "-pass", "2", "-passlogfile", "x264pass", "-movflags", "+faststart",
    "-c:a", "aac", "-b:a", "160k", "-ar", "48000", "spool-tour.mp4")
for p in OUT.glob("x264pass*"): p.unlink()

run("-i", "poster.png", "-vf", "scale=1600:-1:flags=lanczos", "-c:v", "libwebp", "-quality", "86", "spool-tour-poster.webp")

# the loop: files -> songs, the mood board, statistics + the light/dark wipe (frame ranges of the
# master: each scene from its beat + FADE to the next scene's beat, see src/beat.ts)
segs = [(163, 306), (699, 843), (1121, 1302)]
fc, fade = [], 0.6
for i, (a, b) in enumerate(segs):
    fc.append(f"[0:v]trim=start_frame={a}:end_frame={b},setpts=PTS-STARTPTS[s{i}]")
for i in range(1, len(segs)):
    src = "s0" if i == 1 else f"x{i - 1}"
    off_i = sum((b - a) / 30 for a, b in segs[:i]) - fade * i
    fc.append(f"[{src}][s{i}]xfade=transition=fade:duration={fade}:offset={off_i:.3f}[x{i}]")
fc.append(f"[x{len(segs) - 1}]fps=15,scale=960:-1:flags=lanczos[v]")
run("-i", RAW.name, "-filter_complex", ";".join(fc), "-map", "[v]", "-an", "-c:v", "libwebp_anim", "-lossless", "0",
    "-quality", "68", "-compression_level", "6", "-loop", "0", "spool-tour-loop.webp")
for f in ("spool-tour.mp4", "spool-tour-poster.webp", "spool-tour-loop.webp"):
    print(f, round((OUT / f).stat().st_size / 1e6, 2), "MB")

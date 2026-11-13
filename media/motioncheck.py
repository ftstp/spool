"""Motion check for slow zooms and pans. For each pair of consecutive frames, fits the motion of a
region with OpenCV's ECC (a similarity transform: scale + translation, sub-pixel, using the whole
region rather than one patch) and prints the per-frame scale ratio and the displacement of the
region's centre. A smooth camera gives values of one sign that change gradually; whole-pixel
stepping or jitter shows up as 0/1 px alternation or sign flips.
  python motioncheck.py "out/jit/SpoolLoop-{f}.png" 1460 10 x,y,w,h"""
import sys
import cv2
import numpy as np

pat, start, n = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
x, y, w, h = (int(v) for v in sys.argv[4].split(","))
fr = [cv2.imread(pat.format(f=start + i), cv2.IMREAD_GRAYSCALE)[y:y + h, x:x + w].astype(np.float32) / 255 for i in range(n)]
cx, cy = w / 2, h / 2
scales, dxs, dys = [], [], []
for a, b in zip(fr, fr[1:]):
    warp = np.eye(2, 3, dtype=np.float32)
    crit = (cv2.TERM_CRITERIA_EPS | cv2.TERM_CRITERIA_COUNT, 500, 1e-7)
    _, warp = cv2.findTransformECC(a, b, warp, cv2.MOTION_AFFINE, crit, None, 5)
    # warp maps b's coords to a's: x_a = W [x_b, y_b, 1]; invert to get the motion a -> b
    W = np.vstack([warp, [0, 0, 1]]); Wi = np.linalg.inv(W)
    s = np.sqrt(abs(np.linalg.det(Wi[:2, :2])))
    c = Wi @ np.array([cx, cy, 1])
    scales.append(s); dxs.append(c[0] - cx); dys.append(c[1] - cy)
fmt = lambda v: " ".join(f"{x:+.3f}" for x in v)
print(f"region {x},{y} {w}x{h}, frames {start}-{start + n - 1}")
print("  scale per frame (x1e3 - 1000):", " ".join(f"{(s - 1) * 1e3:+.3f}" for s in scales))
print("  centre dx px:", fmt(dxs))
print("  centre dy px:", fmt(dys))
mono = lambda v: bool(np.all(np.array(v) >= -0.03) or np.all(np.array(v) <= 0.03))
steps = lambda v: float(np.max(np.abs(np.diff(v)))) if len(v) > 1 else 0.0
print(f"  monotonic: scale={mono([s - 1 for s in scales])} dx={mono(dxs)} dy={mono(dys)};"
      f" largest frame-to-frame change: dx {steps(dxs):.3f} px, dy {steps(dys):.3f} px")

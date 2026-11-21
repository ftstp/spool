#!/usr/bin/env python3
"""Export a LAION CLAP checkpoint to ONNX for Spool's `crates/audio`, and
build the zero-shot style embeddings the crate ships with.

Models (both Apache-2.0, same HTSAT-large + RoBERTa architecture and the same
feature extractor), pinned to the revisions in MODELS:
  music   https://huggingface.co/laion/larger_clap_music
  general https://huggingface.co/laion/larger_clap_general
Pick one with `--model` (before the sub-command). See crates/audio/README.md for
why `general` is the one shipped: the published `larger_clap_music` weights
produce collapsed embeddings.

Requires: torch (CPU), transformers, onnx, onnxruntime, numpy, soxr, and
ffmpeg/ffprobe on PATH for `embed`.

    python export.py --model general export --out models/general   # audio fp32 + int8, text fp32
    python export.py --model general styles --out ../../crates/audio/styles/styles.json
    python export.py embed --onnx models/general/audio_model.onnx a.wav b.m4a --json ref.json
    python export.py embed --torch --pcm a.f32 ... --json ref.json  # raw 48 kHz mono f32le input
    python export.py golden                    # mel golden values for the Rust unit test

The `embed` reference follows the crate exactly: up to 3 evenly spaced 10 s
windows (skipping the first and last 5 %), log-mel from transformers'
ClapFeatureExtractor (rand_trunc path: Slaney mel), embeddings averaged and
re-normalised.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

MODELS = {
    "music": ("laion/larger_clap_music", "a0b4534a14f58e20944452dff00a22a06ce629d1"),
    "general": ("laion/larger_clap_general", "ada0c23a36c4e8582805bb38fec3905903f18b41"),
}
MODEL_ID, REVISION = MODELS["general"]  # replaced by --model
SR = 48_000
WINDOW = 480_000
FRAMES = 1001
N_MELS = 64
MAX_WINDOWS = 3

# Styles for zero-shot tagging: display name -> prompt subjects. Each subject is
# expanded with every template and the text embeddings are averaged.
TEMPLATES = ["a {} track", "{} music"]
STYLES: dict[str, list[str]] = {
    "drum and bass": ["drum and bass"],
    "liquid drum and bass": ["liquid drum and bass"],
    "jungle": ["jungle breakbeat"],
    "dubstep": ["dubstep"],
    "riddim": ["riddim dubstep"],
    "future bass": ["future bass"],
    "trap": ["trap"],
    "hip hop": ["hip hop"],
    "boom bap": ["boom bap hip hop"],
    "lo-fi hip hop": ["lo-fi hip hop"],
    "r&b": ["r&b"],
    "house": ["house"],
    "deep house": ["deep house"],
    "tech house": ["tech house"],
    "progressive house": ["progressive house"],
    "electro house": ["electro house"],
    "bass house": ["bass house"],
    "techno": ["techno"],
    "hard techno": ["hard techno"],
    "trance": ["trance"],
    "psytrance": ["psytrance"],
    "hardstyle": ["hardstyle"],
    "garage": ["garage"],
    "uk garage": ["uk garage", "2-step garage"],
    "breakbeat": ["breakbeat"],
    "electro": ["electro"],
    "ambient": ["ambient"],
    "downtempo": ["downtempo"],
    "chillwave": ["chillwave"],
    "synthwave": ["synthwave"],
    "synth-pop": ["synth-pop"],
    "disco": ["disco"],
    "nu disco": ["nu disco"],
    "funk": ["funk"],
    "soul": ["soul"],
    "jazz": ["jazz"],
    "reggae": ["reggae"],
    "dub": ["dub reggae"],
    "dancehall": ["dancehall"],
    "afrobeat": ["afrobeat", "afrobeats"],
    "amapiano": ["amapiano"],
    "reggaeton": ["reggaeton"],
    "pop": ["pop"],
    "rock": ["rock"],
    "metal": ["heavy metal"],
    "cinematic": ["cinematic"],
    "orchestral": ["orchestral"],
    "experimental": ["experimental electronic"],
    "glitch hop": ["glitch hop"],
    "industrial": ["industrial"],
}


def load_model():
    from transformers import ClapModel

    model = ClapModel.from_pretrained(MODEL_ID, revision=REVISION)
    model.eval()
    return model


def feature_extractor():
    from transformers import ClapFeatureExtractor

    return ClapFeatureExtractor.from_pretrained(MODEL_ID, revision=REVISION)


# ---------------------------------------------------------------- export


def cmd_export(args):
    import torch

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    model = load_model()
    assert not model.config.audio_config.enable_fusion, "fusion models need is_longer + 4-channel input"

    class Audio(torch.nn.Module):
        def __init__(self, m):
            super().__init__()
            self.m = m

        def forward(self, input_features):
            return self.m.get_audio_features(input_features=input_features).pooler_output

    class Text(torch.nn.Module):
        def __init__(self, m):
            super().__init__()
            self.m = m

        def forward(self, input_ids, attention_mask):
            return self.m.get_text_features(input_ids=input_ids, attention_mask=attention_mask).pooler_output

    audio_path = out / "audio_model.onnx"
    dummy = torch.randn(2, 1, FRAMES, N_MELS)
    with torch.no_grad():
        torch.onnx.export(
            Audio(model), (dummy,), str(audio_path), input_names=["input_features"], output_names=["audio_embeds"],
            dynamic_axes={"input_features": {0: "batch"}, "audio_embeds": {0: "batch"}}, opset_version=17,
            dynamo=False,
        )
    text_path = out / "text_model.onnx"
    ids = torch.ones(2, 16, dtype=torch.long)
    with torch.no_grad():
        torch.onnx.export(
            Text(model), (ids, torch.ones_like(ids)), str(text_path), input_names=["input_ids", "attention_mask"],
            output_names=["text_embeds"],
            dynamic_axes={"input_ids": {0: "batch", 1: "seq"}, "attention_mask": {0: "batch", 1: "seq"},
                          "text_embeds": {0: "batch"}},
            opset_version=17, dynamo=False,
        )

    from onnxruntime.quantization import QuantType, quantize_dynamic

    int8_path = out / "audio_model_int8.onnx"
    quantize_dynamic(str(audio_path), str(int8_path), weight_type=QuantType.QInt8)

    # Sanity check: ONNX fp32 and int8 vs torch on random input (cosine per row).
    import onnxruntime as ort

    model.eval()

    x = np.random.default_rng(0).standard_normal((2, 1, FRAMES, N_MELS)).astype(np.float32) * 20 - 40
    with torch.no_grad():
        ref = Audio(model)(torch.from_numpy(x)).numpy()
    for p in (audio_path, int8_path):
        got = ort.InferenceSession(str(p), providers=["CPUExecutionProvider"]).run(None, {"input_features": x})[0]
        cos = (ref * got).sum(-1) / np.linalg.norm(ref, axis=-1) / np.linalg.norm(got, axis=-1)
        print(f"{p.name} vs torch, cosine:", cos.round(5).tolist())
    manifest = {
        "model": MODEL_ID,
        "revision": REVISION,
        "license": "Apache-2.0",
        "audio_input": "input_features", "audio_output": "audio_embeds",
        "files": {p.name: p.stat().st_size for p in (audio_path, int8_path, text_path)},
        "logit_scale_audio": float(model.logit_scale_a.detach().exp()),
        "logit_scale_text": float(model.logit_scale_t.detach().exp()),
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest, indent=2))


# ---------------------------------------------------------------- styles


def cmd_styles(args):
    import torch
    from transformers import AutoTokenizer

    model = load_model()
    tok = AutoTokenizer.from_pretrained(MODEL_ID, revision=REVISION)
    entries = []
    for name, subjects in STYLES.items():
        prompts = [t.format(s) for s in subjects for t in TEMPLATES]
        inputs = tok(prompts, padding=True, return_tensors="pt")
        with torch.no_grad():
            e = model.get_text_features(**inputs).pooler_output.numpy()
        mean = e.mean(axis=0)
        mean /= np.linalg.norm(mean)
        entries.append({"name": name, "prompts": prompts, "embedding": [round(float(v), 4) for v in mean]})
    scale = float(model.logit_scale_a.detach().exp())
    doc = {
        "model": MODEL_ID,
        "revision": REVISION,
        "templates": TEMPLATES,
        # Softmax temperature for the style probabilities: the checkpoint's learned
        # audio-to-text scale (38.7 for larger_clap_general). larger_clap_music's is ~1.03,
        # which would make the softmax uniform, so it falls back to CLIP's 100.
        "logit_scale": round(scale, 4) if scale > 5 else 100.0,
        "checkpoint_logit_scale": round(scale, 4),
        "styles": entries,
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(doc, separators=(",", ":")))
    print(f"{len(entries)} styles -> {args.out} ({Path(args.out).stat().st_size} bytes)")


# ---------------------------------------------------------------- reference embedding


def window_starts(n: int) -> list[int]:
    if n <= WINDOW:
        return []
    edge = n // 20
    a, b = edge, n - edge
    usable = b - a
    if usable < WINDOW:
        return [(n - WINDOW) // 2]
    count = min(max(usable // WINDOW, 1), MAX_WINDOWS)
    starts = []
    for i in range(count):
        centre = a + (i + 0.5) * usable / count
        start = max(int(round_half_away(centre - WINDOW / 2)), a)
        starts.append(min(start, b - WINDOW))
    return starts


def round_half_away(x: float) -> float:
    # Rust's f64::round (half away from zero), not Python's banker's rounding.
    return math.floor(x + 0.5) if x >= 0 else -math.floor(-x + 0.5)


def windows(x: np.ndarray) -> list[np.ndarray]:
    if len(x) <= WINDOW:
        return [x]  # the feature extractor applies repeatpad itself
    return [x[s : s + WINDOW] for s in window_starts(len(x))]


def decode_48k_mono(path: str) -> np.ndarray:
    """ffmpeg decodes at the file's own rate; channels are averaged and soxr resamples (as librosa.load)."""
    import soxr

    probe = json.loads(subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=sample_rate,channels",
         "-of", "json", path], capture_output=True, check=True).stdout)
    st = probe["streams"][0]
    sr, ch = int(st["sample_rate"]), int(st["channels"])
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-f", "f32le", "-acodec", "pcm_f32le", "-"],
                         capture_output=True, check=True).stdout
    x = np.frombuffer(raw, dtype=np.float32).reshape(-1, ch).mean(axis=1)
    if sr != SR:
        x = soxr.resample(x, sr, SR, quality="VHQ").astype(np.float32)
    return x


def features(fe, ws: list[np.ndarray]) -> np.ndarray:
    return np.concatenate(
        [np.asarray(fe(w, sampling_rate=SR, truncation="rand_trunc", padding="repeatpad", return_tensors="np")
                    ["input_features"], dtype=np.float32) for w in ws], axis=0)


def cmd_embed(args):
    fe = feature_extractor()
    if args.torch:
        import torch

        model = load_model()

        def run(feats):
            with torch.no_grad():
                return model.get_audio_features(input_features=torch.from_numpy(feats)).pooler_output.numpy()
    else:
        import onnxruntime as ort

        sess = ort.InferenceSession(args.onnx, providers=["CPUExecutionProvider"])

        def run(feats):
            return sess.run(None, {"input_features": feats})[0]

    results = {}
    for path in args.files:
        t0 = time.time()
        if args.pcm:
            x = np.fromfile(path, dtype=np.float32)
        else:
            x = decode_48k_mono(path)
        ws = windows(x)
        e = run(features(fe, ws))
        mean = e.mean(axis=0)
        mean /= np.linalg.norm(mean)
        results[os.path.basename(path)] = {
            "embedding": mean.tolist(), "windows": window_starts(len(x)), "samples": len(x),
            "per_window": e.tolist(), "seconds": time.time() - t0,
        }
        print(f"{os.path.basename(path)}: {len(ws)} windows, {time.time() - t0:.2f} s", file=sys.stderr)
    Path(args.json).write_text(json.dumps(results))


# ---------------------------------------------------------------- golden mel values


def golden_signal() -> np.ndarray:
    t = np.arange(WINDOW, dtype=np.float64) / SR
    s = 0.3 * np.sin(2 * np.pi * 220 * t) + 0.2 * np.sin(2 * np.pi * 3000 * t) \
        + 0.1 * np.sin(2 * np.pi * (100 * t + 600 * t * t))
    s = np.where(t > 8.0, s * 0.01, s)
    return s.astype(np.float32)


def cmd_golden(args):
    fe = feature_extractor()
    m = features(fe, [golden_signal()])[0, 0]
    assert m.shape == (FRAMES, N_MELS), m.shape
    picks = [(0, 0), (0, 10), (1, 5), (17, 3), (100, 20), (250, 31), (400, 40), (500, 63), (600, 7),
             (799, 12), (801, 12), (900, 50), (1000, 0), (1000, 33)]
    rng = np.random.default_rng(1)
    picks += [(int(rng.integers(FRAMES)), int(rng.integers(N_MELS))) for _ in range(16)]
    rows = ",\n".join(f"    ({t}, {b}, {m[t, b]:.5f})" for t, b in picks)
    print(f"// Generated by tools/clap/export.py golden.\nconst GOLDEN: &[(usize, usize, f32)] = &[\n{rows},\n];")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--model", choices=sorted(MODELS), default="general")
    sub = p.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("export")
    e.add_argument("--out", required=True)
    s = sub.add_parser("styles")
    s.add_argument("--out", required=True)
    m = sub.add_parser("embed")
    m.add_argument("files", nargs="+")
    m.add_argument("--onnx", help="audio ONNX model (or --torch)")
    m.add_argument("--torch", action="store_true", help="run the PyTorch model instead of ONNX")
    m.add_argument("--pcm", action="store_true", help="inputs are raw 48 kHz mono float32 little-endian")
    m.add_argument("--json", required=True)
    sub.add_parser("golden")
    args = p.parse_args()
    global MODEL_ID, REVISION
    MODEL_ID, REVISION = MODELS[args.model]
    {"export": cmd_export, "styles": cmd_styles, "embed": cmd_embed, "golden": cmd_golden}[args.cmd](args)


if __name__ == "__main__":
    main()

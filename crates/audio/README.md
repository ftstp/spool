# audio

Audio analysis for Spool. From a song's reference render (wav, aif, mp3, m4a/aac,
flac) it produces:

- a **CLAP audio embedding** (512 values, L2-normalised) for "sounds like" search,
- **zero-shot style scores** against ~50 style prompts whose text embeddings are
  precomputed and shipped in `styles/styles.json` (only the audio model is needed),
- **tempo** (bpm, folded into 80-180) and **key** (major/minor), computed in plain Rust.

Audio files are only ever opened for reading.

```rust
// onnxruntime.dll and the model in one folder; load once, reuse
let analyzer = audio::Analyzer::new("path/to/models")?;
let a = analyzer.analyze("Song.wav")?;
// a.embedding, a.bpm, a.key ("F# minor"), a.duration_s, a.styles (highest first)
let similarity = audio::cosine(&a.embedding, &other.embedding);
```

`cargo run --release -p audio --example analyze -- <model dir or .onnx> <files...>` prints
the result with per-stage timings.

## Model

[LAION CLAP `larger_clap_general`](https://huggingface.co/laion/larger_clap_general),
HTSAT-large audio encoder + RoBERTa text encoder, by LAION
([LAION-AI/CLAP](https://github.com/LAION-AI/CLAP), Wu, Chen et al., "Large-scale
Contrastive Language-Audio Pretraining with Feature Fusion and Keyword-to-Caption
Augmentation", ICASSP 2023). Licence: **Apache-2.0**. Pinned revision
`ada0c23a36c4e8582805bb38fec3905903f18b41`.

**Why not `larger_clap_music`.** It was the first choice (same licence, same
architecture, music-only training), but the published weights (revision
`a0b4534a14f58e20944452dff00a22a06ce629d1`) are collapsed: every text prompt embeds to
the same vector (pairwise cosine 0.999 for "drum and bass", "orchestral music", "a dog
barking"), every track to nearly the same vector (0.98-1.0), and its learned logit scale
is 1.03 instead of ~15-40. This is the checkpoint itself (identical with transformers
4.46 and 5.23, all 555 weights load), not the export. `larger_clap_general` and
`clap-htsat-unfused` behave normally. `export.py --model music` still exports it; the
crate code is the same for both, and `Analyzer` only reports styles when the
`manifest.json` beside the model names the model the styles were computed with.

The model files are not in git (fp32 audio 280 MB, int8 audio 75 MB, text 501 MB).
Export them with [`tools/clap/export.py`](../../tools/clap/export.py):

```
python -m venv venv && venv\Scripts\pip install torch --index-url https://download.pytorch.org/whl/cpu
venv\Scripts\pip install transformers onnx onnxruntime numpy soxr
venv\Scripts\python tools/clap/export.py --model general export --out models/general
venv\Scripts\python tools/clap/export.py --model general styles --out crates/audio/styles/styles.json
```

`export` writes `audio_model.onnx` (fp32), `audio_model_int8.onnx` (dynamic int8
weights), `text_model.onnx` and `manifest.json`. `Analyzer::new(dir)` prefers the int8
model: its embeddings stay within cosine 0.997 of fp32 on real tracks (mean 0.998).

## Preprocessing (must match the model)

Identical to `transformers.ClapFeatureExtractor` on its non-fusion path
(`truncation="rand_trunc"`, `enable_fusion` is false for this model): 48 kHz mono,
10 s windows, `repeatpad` for shorter audio, periodic Hann 1024, hop 480, centred frames
with reflect padding, power spectrum, 64 **Slaney** mel bands (Slaney scale and area
norm, not the HTK bank used on the fusion path) from 50 Hz to 14 kHz,
`10*log10(max(x, 1e-10))`, no `top_db`. A unit test checks values against
transformers' output.

`rand_trunc` takes one random 10 s crop. For a deterministic result the crate embeds up
to three evenly spaced windows (centres of the thirds of the track, first and last 5 %
skipped), averages the embeddings and re-normalises. `export.py embed` does the same in
Python.

Decoding is [symphonia](https://github.com/pdeljanov/Symphonia) (MPL-2.0), resampling
[rubato](https://github.com/HEnquist/rubato) (MIT), FFTs realfft/rustfft (MIT/Apache-2.0).

## ONNX Runtime: loaded at run time, downloaded on opt-in

Inference uses [`ort`](https://ort.pyke.io) 2.0.0-rc.15 (MIT/Apache-2.0) with the
`load-dynamic` feature: **nothing of ONNX Runtime is linked into Spool.exe** and no binary
is downloaded at build time. `Analyzer::load(runtime, model, threads)` loads the library
from the path given, only when called, and a library that is missing or does not load
is an error there, not a crash (Spool.exe's imports are checked: no onnxruntime, no
DirectML). Spool starts and works with the analysis off and nothing downloaded.

In the app (`crates/core/src/sound.rs`), turning the analysis on in Settings > Spool
downloads, once, into `<data>/models/clap-general-ada0c23/`:

| File | From | SHA-256 |
|---|---|---|
| `audio_model_int8.onnx` (75.3 MB) | Spool's GitHub release `clap-general-ada0c23` | `3933ab99...89db396ce` |
| `manifest.json`, `NOTICE.txt` | same release | in `sound.rs` |
| `onnxruntime.dll` 1.31.1 (17.3 MB) | Microsoft's release `onnxruntime-win-x64-1.31.1.zip`, only this entry, by HTTP ranges (6.2 MB) | `cb2fa518...0492db0c` |

Every file is checked against the SHA-256 written in `sound.rs` (a file that differs is
deleted and refused), half-downloaded files resume, and "Remove the model" deletes the
folder. `SPOOL_MODEL_DIR=<folder>` takes the same files from a local folder instead
(development), checked the same way. Microsoft's `onnxruntime.dll` needs the Visual C++
2015-2022 runtime (`MSVCP140.dll`); without it, loading fails and Settings says so.

The analysis runs on one low-priority thread, one inference thread, never during a scan
or a repair, and skips a render whose path, size, date and model have not changed
(table `audio_analysis`). About 1.5 s per render on a Ryzen 9 3950X.

## Decoding: encoder priming

symphonia 0.5 decodes every sample of an AAC stream, including the encoder's priming
(1024 samples for ffmpeg's AAC encoder, 2112 for Apple's), which players and ffmpeg skip
using the MP4 edit list. `mp4.rs` reads that edit list and skips the same samples (as
ffmpeg, the padded tail is kept); MP3 uses symphonia's gapless mode (LAME tag). With it,
Rust and the Python reference decode the same number of samples and their embeddings
match at cosine 1.0000 on the 8 test m4a files (0.994-0.998 before).

## Sounds like, and the style vote

A song's sheet lists the 5 songs whose reference render is closest (cosine), leaving out
songs of the same folder and songs sharing the render. On a real catalogue (177 renders),
two unrelated renders score about 0.55, 1 pair in 100 reaches 0.87, and a song's closest
render is about 0.87: the sheet says "very close" from 0.9, "close" from 0.8.

The style suggestion (`catalog::style`, the existing suggestion with its reasons) gets one
more clue: a vote of the 5 closest renders among the songs that already have a style, each
weighing its similarity, counted when one style holds 80 % of the weight. Leave-one-out on
that catalogue (110 analysed songs with a style, 19 styles, the commonest 22 %):

| k | share | coverage | precision |
|---|---|---|---|
| 1 | - | 100 % | 53 % |
| 5 | 50 % | 63 % | 67 % |
| **5** | **80 %** | **33 %** | **83 %** |
| 5 | 90 % | 19 % | 100 % |
| 10 | 80 % | 20 % | 100 % |

In the full suggestion (with the folder and file names, plugins, tempo...) the vote took
part in 23 of the 151 songs with a style, all right; for the 271 songs without one, it
backs the first choice of 21 and is the only clue for 3.

## Style scores

Not shown in the app (too weak, see below): `Analysis::styles` is a softmax (temperature = the checkpoint's logit scale, 38.7) over the
cosines between the embedding and each style's averaged prompt embeddings ("a {style}
track", "{style} music"). Checked against the catalogue's genre tags (themselves Essentia
predictions, mapped by hand to the 50 styles, 135 tracks): top-1 11 %, top-3 31 %, below
always guessing "pop" (24 %) because a few styles ("nu disco", "trap", "synth-pop") win
everywhere. `StyleSet::score_centered` subtracts the mean embedding of the user's library
first; on held-out tracks that gave top-1 21 %, top-3 49 %. Treat styles as hints. The
embedding itself is the stronger signal: a track's nearest neighbour shares a genre tag
43 % of the time, against 12 % for a random pair.

## Tempo and key

Both run on the track decimated to 12 kHz.

- **Tempo**: spectral-flux onset envelope (30 log bands, 200 frames/s, detrended),
  autocorrelation, comb score over 60-200 bpm on the period and its first 8 multiples,
  parabolic refinement on a far multiple, octave folding into 80-180 bpm.
- **Key**: STFT (1.5 Hz bins) folded into semitones with per-semitone averaging
  (constant-Q like) between 80 Hz and 2 kHz, tuning correction, per-frame normalisation,
  Krumhansl-Schmuckler matching with Temperley's (1999) profiles.

Measured against the Essentia values in the Fatstep catalogue (68 held-out tracks, the
settings were chosen on another 68): tempo within 2 % for 56 %, plus 35 % at half or
double (Essentia reports dubstep at 70-75 bpm; we fold into 80-180), so 91 % right up to
the octave. Key: 32 % exact, 12 % relative, 15 % a fifth away. Key is the weak part: it is
stored but not shown in the app. A constant-Q chroma (librosa's CQT chroma reached
35-50 % exact on the same tracks) is the next step.

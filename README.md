# Spool

**Your Ableton Live projects, organised, playable and repaired.** A free, open-source
desktop app for producers, by [Fatstep](https://fatstep.com/tools/spool).

Spool reads your project folders and turns years of `.als` files into a catalogue of
songs: one song per idea, however many versions. Listen to any project without opening
Live, follow its chords and progressions as it plays, compare your mixes with your
references, keep track of what each song still needs, and find your samples and MIDI again.
When a project no longer opens, Spool shows why and repairs it on a copy.

Free, open source, in English and French. Windows for now.

[![A tour of Spool: the catalogue, a song sheet and its arrangement, statistics, light and dark](docs/media/spool-tour-loop.webp)](https://fatstep.com/tools/spool)

*A short look. The full tour, with sound, is on [the download page](https://fatstep.com/tools/spool).*

**What's new:** [CHANGELOG.md](CHANGELOG.md) lists every version, and each one has its
notes in [Releases](https://github.com/ftstp/spool/releases).

## Features

### Catalogue and organisation

- Every project grouped into songs: versions, working copies, older saves and renders,
  however your folders are laid out.
- Style, key and tempo filled in by themselves, from the project first and the audio to
  back it; hover a field to see where it comes from and pick another suggestion. A song
  shows two styles when the second is nearly as sure as the first ("disco, electro").
- Stage, rating, release date and notes per song; the stage Spool guesses is shown beside
  the one you set.
- A Projects view that lists every project file (`.als`) on its own, newest or oldest
  first, to find old ideas again and open one in Live in a click.
- List, board, calendar and folders views. Drag a song onto a day of the calendar to set its
  release date, or onto a folder to move it (asked first, undoable).
- Filters by theme (music, progress, technical), several values at once for artist, style,
  key, type and plugins, saved as presets you switch between, one of them applied at start.
- Global search by title, tempo (`174`, `170-175`), key (`Fm`, `F#m`, `Gbm`), artist,
  style, folder, project file, release or instrument, every word in any order.
- How Spool reads your folders, shown with your own examples: where a title comes from,
  which folders are a single song, each rule previewed before and after it applies.
- Split a song in two, attach one to another, gather a folder into one song, merge two
  artists: every change is undone with Ctrl+Z, and no file moves.
- Optional arranging of your folders on disk (off by default), each move checked and
  undoable.
- Song tabs you can reorder, resizable panes, Undo and Redo everywhere.

### Listen without opening Live

- Play a project straight from its `.als`: MIDI tracks through a built-in synth whose sound
  is guessed from each track's instrument, preset, name and effects; audio clips, Simpler
  and Drum Rack from their own samples, in time with the arrangement.
- Mixed by role, not by the project's faders: kick, bass and voice in front, leads and keys
  a little behind, pads, hats and effects further back, each track measured and brought to
  its place (a sample at its track's level, whatever it was exported at). Volume automation
  and mutes kept, the cuts of EQ Eight and Auto Filter with their sweeps. The whole always at
  the same listening level, two tracks or forty, a quieter part never more than 6 dB under.
- An orchestra recorded for the MIDI tracks (strings by section, pizzicato, harp, woodwinds,
  brass, timpani, glockenspiel), chosen from a track's sound menu or from its name.
- Velocity, pitch bend and glide kept; a limiter keeps the sound clean.
- Mute and solo per track and per group (Ctrl+click adds to the solo), remembered per
  project; reference tracks and tracks muted in Live start muted.
- Playback starts at the first locator or the first bar that sounds, never on silence.
- A render exported from a marker or the loop is aligned with the arrangement (found by
  listening, or set by hand), so the playhead and the chords stay in time with it.
- Switch between the project and its render at the same point, from the player bar, on any
  page.
- Key control: transpose what plays a semitone at a time, without changing the tempo; the
  player bar shows the key heard and the tempo in BPM, with a level meter under the volume
  and, for a render or a sample, the loudness in LUFS under it, so a loud master still reads.
- The arrangement drawn from the project: tracks coloured by role, clips in place, MIDI
  clips with their notes, sections from your locators. Click the ruler to play from there.
  It follows the playhead (Follow playback turns it off; scrolling by hand pauses it), and
  a small meter by each track shows what plays.
- Zoom and travel as in Live: Ctrl + wheel, a drag on the timeline, the + and - keys, a
  scrollbar; Whole song fits the song itself, clips left far after it aside.
- Add, rename, move or remove Live's markers in the arrangement; Open in Live then asks
  which changes to carry into a copy written next to the project.
- Play a release from start to end with previous and next.

### Chords and progressions

- A chord row under the arrangement that names the chord at the playhead, and a chord
  chart by section, four bars a line, repeats counted.
- Chord names in American (`Am7`), Latin (`La m7`) or degrees (`vi`), with slash basses
  (`F#m/A`) only when the bass holds them.
- A staff mode: the tune taken from the melody clips, written with real note values and
  rests under the time signature, with 8va and 8vb.
- The player bar shows the chord you hear and the next one, for any render.
- Click a bar to play from there; play every chord of the song in a row.
- Your song's loop, variations of it, and the well-known progressions of its mode in its
  key, to hear and drag into Live.
- What could come after a chord, by level and by style, each explained in a line, with the
  key's chords, the circle of fifths or a keyboard as an aid.
- Each MIDI clip shows its chords; hover it for its notes, range and velocities.
- Play a MIDI keyboard through Spool, with the sound you choose (piano, organ, synth,
  bass... or one of the recorded orchestra's instruments, from violins to timpani).

### References and comparison

- Mark any audio file as a reference (a released track) for a song, an artist or a release.
- A/B between your render and another version or a reference, at the same moment and the
  same loudness.
- Loudness, peak, average level, dynamics, format and the balance of frequencies on one
  chart, with the gap range by range from sub to highs, at equal loudness.
- How much Spotify, Deezer or Apple Music would turn your track down.
- Timed comments on a render ("the drop is empty at 1:32"): added as you listen, ticked
  when dealt with, a click plays from there.
- What changed between two saves of a project: tempo, length, tracks added, removed or
  renamed, and each track's plugins, notes, clips and samples.

### The song sheet

- The render stays at the top with its waveform and comments while you scroll.
- Sections you open with a click or the keys 1 to 0: arrangement, chords, render,
  projects, resources, checklist, mood board (with the notes) and more, laid out so the
  columns stay even whatever is open.
- Finishing checks: your own, saved as presets for every song or some; the cover, title,
  artist and lyrics tick themselves.
- Notes as separate items. Right-click the waveform to pin one to a moment: it shows as it
  plays, a click plays from there, and the render and the project keep their own.
- Resources: the samples the project plays (in the project, in a library, or missing), the
  unused ones in its folders, and its MIDI files, each playable and draggable into Live.
- Every version of the project, older ones included, opens in Live from the sheet.
- "Unstick this song": your own MIDI clips and loops that fit its key and tempo, heard
  alone or over the render from its next bar, then dragged into Live.

### Samples

- Your sample folders sorted by kind (kick, snare, bass, pad, vocals...), effects by type
  (riser, impact, sweep...), ambiences (field recordings, crowds, foley) and drum loops by part (top, percussion, fill, break...), loop or
  one-shot, key and tempo; a loop or a one-shot whose name gives no key listened to for the
  notes it plays.
- A small waveform on every row: a hit, a pad, a riser or a drum loop told at a glance.
- Samples with an unhelpful name sorted by ear, as questions: a folder with the samples
  elsewhere that sound almost the same, answered at once, each keeping its own category; what
  sounds like your answer proposed next. Undo and Redo, a gauge of your library sorted.
- The whole folder under the question, what your songs use, and samples deleted on request
  (to the Windows Recycle Bin, after a warning).
- Played faster or slower without changing the key, or locked to your song's tempo; loop
  button, previous and next.
- "Sounds alike" for every sample, and samples found from a song in its key (or relative)
  and tempo (or half or double).
- The songs that use each sample, the ones never used, and the duplicates with the space
  they take.
- Drag into Live, always as a copy.

### MIDI

- Your MIDI files and the MIDI clips written in all your projects (chords, basslines,
  melodies, beats), each once, with the song it comes from, by kind, key and tempo.
- Played by real recorded instruments (piano, strings, brass) and the built-in synth (pad,
  sub, bass, a drum kit), the sound changed from the player bar as it plays, transposed
  or retimed, then dragged into Live as you heard them, as a copy.
- A MIDI file or a whole project plays in the player bar as a piano roll: each track in its
  own colour, the drums in a lane of their own, a project's audio clips as bars, the track
  and note named on hover.
- Chords and melodies told apart by what the notes do, not only by the name.
- Save any MIDI clip of a project as a file.

### Mood boards and media

- A mood board per song, release and artist: pictures, notes, lyrics, palette, fonts,
  presets, sounds and MIDI files, linked from anywhere on your disk without copying them.
- Drop files anywhere on a song's sheet to add them; send several media to a board at once.
- A Media page with every song's pictures, videos, fonts and presets, and the media outside
  your projects by folder.
- An Unused audio list: the sounds no song uses, ready to link as a render, a reference or
  a mood board item, with songs of a close name suggested first.
- Right-click any picture to make it the main picture of a song, a release or an artist.

### Artists and releases

- A sheet for every artist: a profile (contact, links, collecting society and IPI, notes),
  their songs, featurings, releases, styles, keys and tempos, a mood board (files dropped
  from Explorer) and a photo.
- Singles, EPs and albums: tracklist with each track's style, key, tempo and last save,
  what is ready, notes, cover and mood board (files dropped from Explorer); suggested from
  your folders, and opened in Explorer in one click.
- Collaborators per track, with the names you already use suggested; a release sets them
  on all its tracks at once.
- Right-click a release or a track to rename, reorder, copy the tracklist or delete
  (undoable).

### Repair and upkeep

- When a project will not open (samples moved, a plugin renamed after a reinstall or a move
  to VST3, a project from another computer), Spool shows why in plain steps: repair at
  once, what to install first, the samples still lost.
- The projects recovered over the scans charted, and the plugins uninstalled since Spool
  started following them, each with the day it was installed, the day it went and the
  projects it is missing from.
- A repaired copy also gathers the samples found elsewhere into the project, like Live's
  Collect All and Save.
- Optional treatments, on a copy: track colours, names and order by role, cleanup of empty
  tracks and unused returns, section markers, a template (tempo, scale, master rack).
- Projects copied as they are into other folders flagged, with a way to archive the copy.
- Settings > Storage shows everything Spool keeps; the catalogue and its safety copy are
  compressed.

### Home, statistics and starter packs

- A Home page you arrange yourself: the figures at the top, the blocks below (every
  statistics chart included) and their order.
- Favourites: a star on songs, projects, artists, releases, samples, MIDI, mood board
  pictures and plugins, all found again on Home.
- A line at the top that turns through what is useful today: the next release, the song
  nearly done, an idea left aside, your week.
- Statistics on your whole catalogue.
- Starter pack: a starting project for a style, built from the devices and samples you
  actually use.

### Audio analysis

- On by default, can be turned off: from each song's render, the songs that sound alike,
  the measured tempo, and the styles it sounds like, which count as one more clue for
  each song's style (your own style names are kept).
- Styles read in folder and file names: some 400 styles, the broad genres and a few recent
  ones (bass house, phonk, amapiano...), however they are written ("Hip-Hop", "D&B").
- Spool learns your styles from your folders, from your corrections, and from a few
  questions where it is unsure ("Spool thinks this is dnb. Right?"), each answer used for
  every song that sounds alike.
- No style question at the start. A "Your styles" page asks, once each: whether a word of
  your folders that comes back over several songs is a style of yours ("jazz fanfare"), and,
  where what Spool hears disagrees with your folders, whether it is your name for it
  ("Downtempo is my lofi"). You can also add your own style words.
- It runs on your computer with LAION's CLAP model
  ([`larger_clap_general`](https://huggingface.co/laion/larger_clap_general), Apache-2.0),
  downloaded once in the background after the first setup. It slows down while Live is open
  so Live does not crackle. See [crates/audio](crates/audio/README.md).

### Everywhere

- Light or dark theme, English or French.
- Your projects never leave your computer. Error reports and usage statistics are sent
  only if you turn on sharing (asked once, changed any time in Settings).

## Screenshots

| Light | Dark |
|---|---|
| ![Home, light](docs/screenshots/home-light.webp) | ![Home, dark](docs/screenshots/home-dark.webp) |
| ![The songs list, light](docs/screenshots/songs-light.webp) | ![The songs list, dark](docs/screenshots/songs-dark.webp) |
| ![A song sheet, light](docs/screenshots/song-light.webp) | ![A song sheet, dark](docs/screenshots/song-dark.webp) |
| ![Timed comments and A/B comparison, light](docs/screenshots/compare-light.webp) | ![Timed comments and A/B comparison, dark](docs/screenshots/compare-dark.webp) |
| ![Samples, light](docs/screenshots/samples-light.webp) | ![Samples, dark](docs/screenshots/samples-dark.webp) |
| ![MIDI files, light](docs/screenshots/midi-light.webp) | ![MIDI files, dark](docs/screenshots/midi-dark.webp) |
| ![A release, light](docs/screenshots/release-light.webp) | ![A release, dark](docs/screenshots/release-dark.webp) |
| ![Statistics, light](docs/screenshots/stats-light.webp) | ![Statistics, dark](docs/screenshots/stats-dark.webp) |
| ![Repair, light](docs/screenshots/repair-light.webp) | ![Settings, dark](docs/screenshots/settings-dark.webp) |

## What Spool writes

Spool reads your projects to build the catalogue. It writes only when you ask: a repair or a
treatment writes a new copy next to the original (`Song [R].als`), and arranging your folders
on disk (off until you turn it on in Settings) moves files, each move checked and undoable
with Ctrl+Z. Stages, releases, mood boards and the audio analysis live in the app's own
database.

## Download

Windows: **[fatstep.com/tools/spool](https://fatstep.com/tools/spool)**. The installer
needs no administrator rights. Spool checks for a new version shortly after it starts and
every 6 hours, and only offers it: nothing installs without your click. Updates are signed;
a download that does not carry Spool's signature is refused.

### "Unknown publisher" on the first install

Windows SmartScreen warns about the installer because it is not signed with a Windows
code-signing certificate. That is a deliberate choice for now: a certificate costs a few
hundred euros a year, requires an identity check of the publisher, and the SmartScreen
reputation it unlocks only builds up over time and downloads. For a free tool at its start,
we would rather wait. Click "More info", then "Run anyway".

What protects you meanwhile:

- the source code is here, and each release on the download page shows the installer's
  SHA-256 checksum, so you can check the file you got is the one published;
- every update is signed with Fatstep's own key (minisign, through Tauri's updater), and
  Spool refuses an update that does not carry that signature.

Spool sends nothing about your projects anywhere. Its network requests are that
update check (`artists.fatstep.com/spool/download/latest.json`), the usage statistics if you
turn on sharing (see below), plus one download for the
audio analysis, in the background after the first setup unless you turn it off in Settings >
Spool: about 80 MB, once, the CLAP
audio model from this repository's releases on GitHub and Microsoft's ONNX Runtime from
Microsoft's releases on GitHub, each file checked against a SHA-256 written in the source.
Nothing is uploaded: the analysis runs on your computer, and "Remove the model" deletes the
download.

Nothing is sent unless you turn on sharing. Spool asks once (at the end of the setup, or at
the first start after an update) and you can change it any time in Settings > Data. If you
share, Spool sends errors and anonymous usage statistics to PostHog (EU), under a random
identifier made on your computer, with no account: the app's version and system, which pages
and tools are used (once per launch each), a catalogue's size as a range (for example
"100-1k"), and error messages with every path, file and song name taken out. Never a project,
song title, file name or path, plugin list, e-mail or computer name. Turning it off stops
everything and deletes the identifier. What may be sent is listed in
[`crates/core/src/telemetry.rs`](crates/core/src/telemetry.rs).

## Build from source

Requirements: Rust (stable, 1.85+), Node 20+ with pnpm, and on Windows the WebView2 runtime.

```
pnpm --dir web install
cargo test --workspace
cd apps/desktop
../../web/node_modules/.bin/tauri dev      # the app, with live reload
../../web/node_modules/.bin/tauri build    # an installer, in target/release/bundle
```

How it is put together: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Licence

GPL-3.0-or-later (see [LICENSE](LICENSE)). You may use, study, change and share Spool; a
modified version you distribute must stay under the same licence, with its source.

"Spool" and "Fatstep" are names of Fatstep; a fork must use another name.

The optional audio analysis downloads a model that is not part of Spool's source: LAION's
CLAP `larger_clap_general` (Apache-2.0, by LAION, <https://github.com/LAION-AI/CLAP>),
converted to ONNX by [`tools/clap/export.py`](tools/clap/export.py), and Microsoft's ONNX
Runtime (MIT). Their notices ship with the download.

The recorded piano, strings and brass come from VSCO 2 Community Edition (public domain).

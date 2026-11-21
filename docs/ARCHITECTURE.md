# Architecture

A local-first desktop app that catalogues, repairs and organises Ableton Live projects.
Rust core, Tauri 2 desktop shell, Svelte 5 + TypeScript interface. The core knows nothing
of the shell, so other shells (another DAW's tooling, a macOS build) can reuse it.

Licence: GPL-3.0-or-later for everything in this repository (see "Licensing").

## Principles

1. **The core knows nothing about the UI or the transport.** Every feature is a Rust
   function in a library crate, callable from a test, from the desktop shell or from a
   web server. No Tauri, no HTTP in `crates/core` and below.
2. **One definition per type.** Data exchanged with the interface is defined once in
   Rust (`serde` + `specta`); the TypeScript types are generated from it. No hand-written
   duplicate of a struct on the other side.
3. **The user's projects are never modified.** Repairs and treatments write copies next
   to the original; file moves are journaled and undoable; nothing is ever overwritten.
4. **Fast by construction.** Streaming XML parsing, parallel reads (rayon), incremental
   scans (only changed files are re-read), SQLite in WAL mode, a file watcher instead of
   polling, virtualised lists in the interface, push events instead of refresh loops.
5. **Small crates, one job each.** A contributor can work on key detection without
   reading the repair writer.
6. **The previous Python app is the reference** until the port reaches parity: outputs are
   compared on a real corpus of projects (see "Parity").

## Workspace

```
plugin-fixer/
  Cargo.toml                 workspace
  crates/
    als/        read and write .als files (gzip XML): tempo, key, tracks, devices, plugin
                references and states, samples, names; key and mode estimation; writer
                that preserves the original formatting
    plugins/    what is installed (Live's plugin database), the previous system's plugins,
                matching and classification of each reference, plugin state converters
                (AU -> VST2)
    catalog/    songs: identity rules (the song folder), versions, variants, merges,
                stages, checklists, style deduction, key choice, suggestions
    repair/     repair plans and copies, treatments (colours, names, order,
                cleaning, markers, template), copy naming
    fs/         file operations: move, create folders, mood boards, media folders,
                reorganisation plans, the undo journal for file operations
    store/      SQLite: schema migrations, repositories, the user's choices mirror
                (choices.json), undo snapshots
    index/      the scanner: walking, incremental parallel reads, file watcher, sample
                index, background passes, progress events
    core/       the application facade: one `App` with typed commands and queries
                (the API), events, settings. The only crate the shells talk to.
  apps/
    desktop/    Tauri 2 shell: exposes core commands, forwards core events, native
                dialogs, opening files in Live / Explorer / Finder
    server/     (later) axum HTTP server exposing the same core API for a web version
  web/          Svelte 5 + TypeScript + Vite
    src/lib/api/      generated types + a transport interface (Tauri invoke today,
                      fetch/WebSocket for the web version), the only place that knows
    src/lib/stores/   app state (songs, filters, player, undo)
    src/lib/components/  SongCard, Player, FilterBar, SearchBox, Sheet, Kanban...
    src/routes/       pages: home, songs (list, folders, board, calendar, grid),
                      repair (with its treatments tab), settings
  platform/     (inside crates where needed) Windows / macOS specifics behind traits:
                plugin folders, Live database location, AU availability, reveal in
                Explorer / Finder, old-system drive
  tests/fixtures/   small .als files and expected outputs
  tools/parity/     scripts that dump the Python app's results as JSON for comparison
  docs/
```

Dependency direction (an arrow means "uses"):

```
apps/desktop, apps/server -> core -> index -> {catalog, repair, fs} -> {als, plugins, store}
```

Lower crates never depend on higher ones. `als` and `plugins` depend on nothing of ours.

## Data

- SQLite (rusqlite, bundled), one file in the app data folder (`%APPDATA%` /
  `~/Library/Application Support`), WAL mode, numbered migrations in `crates/store`.
- `choices.json` next to it: the user's decisions mirrored on every change, restored into
  an empty base (same role as in the Python app).
- Import of the Python app's `apf.sqlite` and `choices.json` on first launch, so nothing
  the user decided is lost in the move.
- Caches (converted audio, waveform peaks, old-drive sample index) in the cache folder,
  safe to delete.

## Interface

- Svelte 5 runes, TypeScript strict, Vite. No UI framework dependency beyond that; a small
  in-house component set with design tokens (colours, spacing, radius) in one CSS file.
- Lists of hundreds of songs are virtualised. Images and audio are served by Tauri's
  asset protocol (no HTTP server in the desktop app).
- State changes arrive as events from the core (`songs_changed`, `scan_progress`,
  `undo_available`...): no polling.
- All user-facing text in per-locale JSON (French first, English next). Never the
  em-dash character in user-facing text.

## Security

- Desktop: no local HTTP server, so no cross-site or DNS-rebinding exposure; Tauri
  capabilities restrict what the interface may call; file access only under the user's
  projects root, library folders and app folders.
- Server (later): authentication, per-user storage, the same path checks.

## Performance targets

- Cold start to usable home: under 1 s with an existing base.
- Full read of ~1 400 projects: under 20 s on 8 cores (Python app: about 3 min).
- An idle pass (nothing changed): under 200 ms.
- Any filter or view change in the interface: under 16 ms (one frame) for 1 000 songs.

## Parity

While porting, each crate is checked against the Python app on the real corpus:
`tools/parity` dumps, per `.als`, what the Python reader returns (JSON); the Rust reader's
output must match field by field (tempo, key estimate and confidence, devices, plugin
references, presets, samples...). Differences are either fixed or written down as
deliberate. The writer (repair copies) is checked by re-reading its output and by
opening copies in Live.

## Licensing

- Everything here is GPL-3.0-or-later, and Spool stays free: no paid edition is planned.
  Anyone may use, study, modify and share it; a modified version that is distributed must
  stay GPL, with its source.
- Contributions are signed off (Developer Certificate of Origin, see CONTRIBUTING.md); each
  contributor keeps the copyright of their code.
- Services Spool may connect to later (a Fatstep for Artists account, sending a demo) run on
  their own servers with their own code; the app's side of them is in this repository.
